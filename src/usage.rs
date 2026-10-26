use std::collections::{HashMap, HashSet};

use crate::binding_id::BindingId;
use crate::facts::{
    ExceptionKind, ExternalFactLookupIndex, FactSet, NullabilityState, TrivialAccessorKind,
};
use crate::java_ir::{
    BinaryOp, Block, ConstructorInvocationKind, Expr, JavaType, LambdaBody, LambdaParam, Literal,
    Member, Modifier, Param, Program, ResolvedCall, ResolvedConstructor, Stmt, SwitchLabel,
    TypeDecl, UnaryOp,
};
use crate::java_signature::{JavaCallableKey, JavaOwnerKey, source_method_rust_name};
use crate::nullability_plan::{TernaryNullRefinementPlan, plan_ternary_null_refinement};
use crate::stdlib::is_content_read_method;
use crate::types::TypeEnv;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UsageFacts {
    methods: HashMap<UsageMethodKey, MethodUsageFacts>,
    methods_by_rust_name: HashMap<UsageMethodKey, MethodUsageFacts>,
    methods_by_java_key: HashMap<JavaCallableKey, MethodUsageFacts>,
    constructors: HashMap<UsageConstructorKey, MethodUsageFacts>,
}

/// Immutable semantic facts and their shared external-member index for one
/// usage-analysis pass. Large external packs make rebuilding the index at each
/// expression an accidental quadratic operation.
struct UsageAnalysisContext<'a> {
    facts: &'a FactSet,
    external_lookup: ExternalFactLookupIndex<'a>,
    source_symbols: Option<crate::resolve::SourceSymbols>,
    source_return_tables: HashMap<String, HashMap<(String, usize), NullabilityState>>,
}

impl<'a> UsageAnalysisContext<'a> {
    fn new(facts: &'a FactSet) -> Self {
        Self {
            facts,
            external_lookup: facts.external_lookup_index(),
            source_symbols: None,
            source_return_tables: HashMap::new(),
        }
    }

    fn source_return_table(
        &self,
        owner: &str,
    ) -> Option<&HashMap<(String, usize), NullabilityState>> {
        let owner = self
            .source_symbols
            .as_ref()
            .map_or(owner, |symbols| symbols.source_type_name(owner));
        self.source_return_tables.get(owner)
    }

    fn collect_source_return_tables(&mut self, ty: &TypeDecl) {
        let table = source_return_nullability_by_call_name(ty, self);
        let owner = self
            .source_symbols
            .as_ref()
            .map_or(ty.name.as_str(), |symbols| {
                symbols.source_type_name(&ty.name)
            })
            .to_string();
        self.source_return_tables.insert(owner, table);
        for member in &ty.members {
            if let Member::Type(nested) = member {
                self.collect_source_return_tables(nested);
            }
        }
    }
}

impl std::ops::Deref for UsageAnalysisContext<'_> {
    type Target = FactSet;

    fn deref(&self) -> &Self::Target {
        self.facts
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct CallGraph {
    nodes: HashSet<CallableKey>,
    edges: Vec<CallGraphEdge>,
    anonymous_methods: HashMap<CallableKey, crate::java_ir::MethodDecl>,
    callback_bodies: HashMap<CallableKey, (Vec<Param>, Block)>,
}

impl CallGraph {
    pub fn nodes(&self) -> &HashSet<CallableKey> {
        &self.nodes
    }

    pub fn edges(&self) -> &[CallGraphEdge] {
        &self.edges
    }

    pub(crate) fn anonymous_methods(&self) -> &HashMap<CallableKey, crate::java_ir::MethodDecl> {
        &self.anonymous_methods
    }

    /// Bodies outside named declarations still need the same intrinsic-effect
    /// analysis as source methods (throw, bounds checks, division, casts).
    pub(crate) fn synthetic_callable_bodies(
        &self,
    ) -> impl Iterator<Item = (&CallableKey, &[Param], &Block)> {
        self.anonymous_methods
            .iter()
            .filter_map(|(key, method)| {
                method
                    .body
                    .as_ref()
                    .map(|body| (key, method.params.as_slice(), body))
            })
            .chain(
                self.callback_bodies
                    .iter()
                    .map(|(key, (params, body))| (key, params.as_slice(), body)),
            )
    }

    #[cfg(test)]
    pub(crate) fn from_edges(edges: Vec<CallGraphEdge>) -> Self {
        Self {
            nodes: edges
                .iter()
                .flat_map(|edge| [edge.caller.clone(), edge.callee.clone()])
                .collect(),
            edges,
            anonymous_methods: HashMap::new(),
            callback_bodies: HashMap::new(),
        }
    }

    pub fn from_nodes(nodes: impl IntoIterator<Item = CallableKey>) -> Self {
        Self {
            nodes: nodes.into_iter().collect(),
            edges: Vec::new(),
            anonymous_methods: HashMap::new(),
            callback_bodies: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum CallableKey {
    #[non_exhaustive]
    Method {
        type_name: JavaOwnerKey,
        method_name: String,
    },
    #[non_exhaustive]
    Constructor {
        type_name: JavaOwnerKey,
        rust_name: String,
    },
    /// Lexical owner of callbacks declared in a field initializer. It is
    /// distinct from every constructor body and does not invoke the callback.
    FieldInitializer {
        type_name: JavaOwnerKey,
        field_name: String,
    },
    #[non_exhaustive]
    Lambda {
        owner: Box<CallableKey>,
        ordinal: usize,
    },
    /// A declaration-scoped SAM contract, not a lexical lambda ordinal.
    CallbackParameter {
        owner: Box<CallableKey>,
        index: usize,
    },
    #[non_exhaustive]
    AnonymousMethod {
        owner: Box<CallableKey>,
        ordinal: usize,
        method_name: String,
    },
    #[non_exhaustive]
    ExternalMethod {
        receiver_type: Option<String>,
        method_name: String,
        arg_count: usize,
    },
    #[non_exhaustive]
    ExternalConstructor { type_name: String, arg_count: usize },
}

impl CallableKey {
    /// The key for a declared source method.
    ///
    /// Always use this rather than building `CallableKey::Method` by hand. The
    /// `method_name` field holds the **Rust** name, and one place built it from
    /// `method.name` — the Java name — instead: `override_groups_for_program`,
    /// whose groups therefore matched the fallibility plan's keys only when the
    /// two spellings happened to coincide. For anything camelCased or
    /// overloaded they do not, so the override equalization silently did
    /// nothing and a second store had to compensate
    /// ([0010](../claude_audits/0010-fallibility-is-decided-three-times-and-disagrees.md)).
    ///
    /// Identity spelled two ways is
    /// [0009](../claude_audits/done/0009-overload-identity-collapses-onto-rust-name.md)'s
    /// defect; this constructor makes that particular instance of it
    /// unrepresentable.
    pub(crate) fn source_method_for_type(
        ty: &crate::java_ir::TypeDecl,
        method: &crate::java_ir::MethodDecl,
    ) -> Self {
        CallableKey::Method {
            type_name: JavaOwnerKey::source_type(ty),
            method_name: crate::java_signature::source_method_rust_name(method),
        }
    }

    pub(crate) fn source_method(
        type_name: impl Into<String>,
        method: &crate::java_ir::MethodDecl,
    ) -> Self {
        CallableKey::Method {
            type_name: JavaOwnerKey::resolved_source(type_name),
            method_name: crate::java_signature::source_method_rust_name(method),
        }
    }

    pub(crate) fn source_constructor_for_type(
        ty: &crate::java_ir::TypeDecl,
        constructor: &crate::java_ir::ConstructorDecl,
    ) -> Self {
        CallableKey::Constructor {
            type_name: JavaOwnerKey::source_type(ty),
            rust_name: crate::java_signature::source_constructor_rust_name(ty, constructor),
        }
    }

    pub(crate) fn resolved_method(resolved: &ResolvedCall) -> Self {
        Self::method_identity(resolved.declaring_type.clone(), resolved.rust_name.clone())
    }

    pub(crate) fn method_identity(
        type_name: impl Into<String>,
        method_name: impl Into<String>,
    ) -> Self {
        CallableKey::Method {
            type_name: JavaOwnerKey::resolved_source(type_name),
            method_name: method_name.into(),
        }
    }

    pub(crate) fn resolved_constructor(resolved: &ResolvedConstructor) -> Self {
        Self::constructor_identity(resolved.declaring_type.clone(), resolved.rust_name.clone())
    }

    pub(crate) fn constructor_identity(
        type_name: impl Into<String>,
        rust_name: impl Into<String>,
    ) -> Self {
        CallableKey::Constructor {
            type_name: JavaOwnerKey::resolved_source(type_name),
            rust_name: rust_name.into(),
        }
    }

    pub fn external_method(
        receiver_type: Option<impl Into<String>>,
        method_name: impl Into<String>,
        arg_count: usize,
    ) -> Self {
        CallableKey::ExternalMethod {
            receiver_type: receiver_type.map(Into::into),
            method_name: method_name.into(),
            arg_count,
        }
    }

    pub(crate) fn external_constructor(type_name: impl Into<String>, arg_count: usize) -> Self {
        CallableKey::ExternalConstructor {
            type_name: type_name.into(),
            arg_count,
        }
    }

    pub fn method_identity_parts(&self) -> Option<(&str, &str)> {
        match self {
            CallableKey::Method {
                type_name,
                method_name,
            } => Some((type_name.as_str(), method_name)),
            _ => None,
        }
    }

    pub fn constructor_identity_parts(&self) -> Option<(&str, &str)> {
        match self {
            CallableKey::Constructor {
                type_name,
                rust_name,
            } => Some((type_name.as_str(), rust_name)),
            _ => None,
        }
    }

    pub fn external_method_parts(&self) -> Option<(Option<&str>, &str, usize)> {
        match self {
            CallableKey::ExternalMethod {
                receiver_type,
                method_name,
                arg_count,
            } => Some((receiver_type.as_deref(), method_name, *arg_count)),
            _ => None,
        }
    }

    pub fn external_constructor_parts(&self) -> Option<(&str, usize)> {
        match self {
            CallableKey::ExternalConstructor {
                type_name,
                arg_count,
            } => Some((type_name, *arg_count)),
            _ => None,
        }
    }

    pub fn is_lambda(&self) -> bool {
        matches!(self, CallableKey::Lambda { .. })
    }

    pub fn anonymous_method_parts(&self) -> Option<(usize, &str)> {
        match self {
            CallableKey::AnonymousMethod {
                ordinal,
                method_name,
                ..
            } => Some((*ordinal, method_name)),
            _ => None,
        }
    }

    pub(crate) fn diagnostic_label(&self) -> String {
        match self {
            CallableKey::Method {
                type_name,
                method_name,
            } => format!("{}::{method_name}", type_name.as_str()),
            CallableKey::Constructor {
                type_name,
                rust_name,
            } => format!("{}::{rust_name}", type_name.as_str()),
            CallableKey::FieldInitializer {
                type_name,
                field_name,
            } => format!("{}::{field_name}#initializer", type_name.as_str()),
            CallableKey::Lambda { owner, ordinal } => {
                format!("{}#lambda{ordinal}", owner.diagnostic_label())
            }
            CallableKey::CallbackParameter { owner, index } => {
                format!("{}#callback{index}", owner.diagnostic_label())
            }
            CallableKey::AnonymousMethod {
                owner,
                ordinal,
                method_name,
            } => format!("{}#anon{ordinal}::{method_name}", owner.diagnostic_label()),
            CallableKey::ExternalMethod {
                receiver_type,
                method_name,
                ..
            } => match receiver_type {
                Some(receiver_type) => format!("{receiver_type}::{method_name}"),
                None => method_name.clone(),
            },
            CallableKey::ExternalConstructor {
                type_name,
                arg_count,
            } => format!("{type_name}::new/{arg_count}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallGraphEdge {
    caller: CallableKey,
    callee: CallableKey,
    kind: CallGraphEdgeKind,
}

impl CallGraphEdge {
    #[cfg(test)]
    pub(crate) fn new(caller: CallableKey, callee: CallableKey, kind: CallGraphEdgeKind) -> Self {
        Self {
            caller,
            callee,
            kind,
        }
    }

    pub fn caller(&self) -> &CallableKey {
        &self.caller
    }

    pub fn callee(&self) -> &CallableKey {
        &self.callee
    }

    pub fn kind(&self) -> CallGraphEdgeKind {
        self.kind
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CallGraphEdgeKind {
    DirectMethodCall,
    ConstructorCall,
    ConstructorDelegationThis,
    ConstructorDelegationSuper,
    AnonymousClassConstructor,
    AnonymousClassMethod,
    AnonymousOverrideContract,
    CallbackContractEquality,
    LambdaBody,
    HandledExceptionBoundary,
    UnresolvedExternalMethodCall,
    UnresolvedExternalConstructorCall,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct UsageMethodKey {
    owner: JavaOwnerKey,
    method_name: String,
}

impl UsageMethodKey {
    pub(crate) fn source_method(ty: &TypeDecl, method: &crate::java_ir::MethodDecl) -> Self {
        Self {
            owner: JavaOwnerKey::source_type(ty),
            method_name: source_method_rust_name(method),
        }
    }

    pub(crate) fn legacy_java_name_source_method(
        ty: &TypeDecl,
        method: &crate::java_ir::MethodDecl,
    ) -> Self {
        Self {
            owner: JavaOwnerKey::source_type(ty),
            method_name: method.name.clone(),
        }
    }

    pub(crate) fn current_rust_method_name(
        type_name: impl Into<String>,
        method_name: impl Into<String>,
    ) -> Self {
        Self {
            owner: JavaOwnerKey::resolved_source(type_name),
            method_name: method_name.into(),
        }
    }

    pub(crate) fn type_name(&self) -> &str {
        self.owner.as_str()
    }

    pub(crate) fn method_name(&self) -> &str {
        &self.method_name
    }

    pub(crate) fn owner(&self) -> &JavaOwnerKey {
        &self.owner
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct UsageConstructorKey {
    owner: JavaOwnerKey,
    rust_name: String,
}

impl UsageConstructorKey {
    pub(crate) fn source_constructor(
        ty: &TypeDecl,
        ctor: &crate::java_ir::ConstructorDecl,
    ) -> Self {
        Self {
            owner: JavaOwnerKey::source_type(ty),
            rust_name: crate::java_signature::source_constructor_rust_name(ty, ctor),
        }
    }

    pub(crate) fn current_rust_constructor_name(
        type_name: impl Into<String>,
        rust_name: impl Into<String>,
    ) -> Self {
        Self {
            owner: JavaOwnerKey::resolved_source(type_name),
            rust_name: rust_name.into(),
        }
    }

    pub(crate) fn type_name(&self) -> &str {
        self.owner.as_str()
    }

    pub(crate) fn rust_name(&self) -> &str {
        &self.rust_name
    }

    pub(crate) fn owner(&self) -> &JavaOwnerKey {
        &self.owner
    }
}

fn rust_method_family_key_matches(
    key: &UsageMethodKey,
    type_name: &str,
    method_name: &str,
) -> bool {
    if key.type_name() != type_name {
        return false;
    }
    let rust_base = crate::parser::to_snake_case(method_name);
    let overload_prefix = format!("{rust_base}_");
    key.method_name() == rust_base || key.method_name().starts_with(&overload_prefix)
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MethodUsageFacts {
    // Transient plans keyed by identity in this immutable AST traversal. They
    // are consumed on entering the callback and cleared before publishing facts.
    callback_value_domains: HashMap<(u8, usize), NativeCallbackValueDomains>,
    active_callback_result: Option<NativeCollectionStorageSlot>,
    source_local_slots: Option<std::sync::Arc<crate::null_diagnostic::SourceLocalSlots>>,
    bindings: HashMap<String, BindingUseFacts>,
    bindings_by_id: HashMap<BindingId, BindingUseFacts>,
    type_params_in_scope: Vec<String>,
    same_type_method_arg_targets: HashMap<(String, usize), Option<Vec<JavaType>>>,
    call_sites: Vec<CallSiteUse>,
    constructor_call_sites: Vec<ConstructorCallSiteUse>,
    returned_call_results: Vec<ReturnedCallResultUse>,
    assigned_call_results: Vec<AssignedCallResultUse>,
    native_collection_storage_flows:
        Vec<(NativeCollectionStorageSlot, NativeCollectionStorageSlot)>,
    native_collection_view_backings: Vec<(
        NativeCollectionStorageSlot,
        NativeCollectionStorageSlot,
        usize,
    )>,
    // Physical compatibility across a source reference transfer. Unlike a
    // stored child, this does not itself demand a retained allocation.
    native_collection_reference_transfers:
        Vec<(NativeCollectionStorageSlot, NativeCollectionStorageSlot)>,
    // Retained callbacks need compatible local/synchronized carriers without
    // aliasing identities or equating unrelated generic value domains.
    native_callback_capture_domains:
        Vec<(NativeCollectionStorageSlot, NativeCollectionStorageSlot)>,
    native_callback_environment_domains:
        Vec<(NativeCollectionStorageSlot, NativeCollectionStorageSlot)>,
    nullable_collection_storage_flows:
        Vec<(NativeCollectionStorageSlot, NativeCollectionStorageSlot)>,
    native_collection_retention_requirements: Vec<NativeCollectionStorageSlot>,
    native_list_projection_slots: Vec<NativeCollectionStorageSlot>,
    native_map_entry_backing_storage: HashMap<BindingId, NativeCollectionStorageSlot>,
    null_assigned_source_fields: Vec<crate::facts::FieldKey>,
    field_uses: Vec<FieldUse>,
    // Shared once per owner across callable facts. Static storage still
    // contributes field constraints, but never mutates an instance receiver.
    static_field_names: std::sync::Arc<HashSet<String>>,
    self_argument_uses: Vec<SelfArgumentUse>,
    mutates_self: bool,
    uses_receiver: bool,
    active_non_null_bindings: HashSet<BindingId>,
    active_non_null_idless_names: HashSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SelfArgumentUse {
    name: String,
    arg_count: usize,
    boundary: CallArgumentBoundary,
}

impl UsageFacts {
    fn method_usage_by_key(&self, key: &UsageMethodKey) -> Option<&MethodUsageFacts> {
        self.methods.get(key)
    }

    fn constructor_usage_by_key(&self, key: &UsageConstructorKey) -> Option<&MethodUsageFacts> {
        self.constructors.get(key)
    }

    pub(crate) fn current_rust_method_usage(
        &self,
        type_name: &str,
        method_name: &str,
    ) -> Option<&MethodUsageFacts> {
        let key = UsageMethodKey::current_rust_method_name(type_name, method_name);
        self.methods_by_rust_name
            .get(&key)
            .or_else(|| self.method_usage_by_key(&key))
    }

    pub(crate) fn current_rust_constructor_usage(
        &self,
        type_name: &str,
        rust_name: &str,
    ) -> Option<&MethodUsageFacts> {
        self.constructor_usage_by_key(&UsageConstructorKey::current_rust_constructor_name(
            type_name, rust_name,
        ))
    }

    #[cfg(test)]
    pub(crate) fn insert_method_usage(&mut self, key: UsageMethodKey, usage: MethodUsageFacts) {
        self.methods_by_rust_name.insert(key.clone(), usage.clone());
        self.methods.insert(key, usage);
    }

    pub(crate) fn method_entries(
        &self,
    ) -> impl Iterator<Item = (&UsageMethodKey, &MethodUsageFacts)> {
        // Enumerate declarations, not compatibility aliases. A later overload's
        // Java-name alias (e.g. build(int[])) can replace the `build` key for
        // build(), even though their emitted Rust names remain distinct.
        self.methods_by_rust_name.iter()
    }

    pub(crate) fn constructor_entries(
        &self,
    ) -> impl Iterator<Item = (&UsageConstructorKey, &MethodUsageFacts)> {
        self.constructors.iter()
    }

    pub(crate) fn method_usages(&self) -> impl Iterator<Item = &MethodUsageFacts> {
        self.methods_by_rust_name.values()
    }

    pub(crate) fn constructor_usages(&self) -> impl Iterator<Item = &MethodUsageFacts> {
        self.constructors.values()
    }

    pub(crate) fn binding_facts_for_rust_method_family(
        &self,
        type_name: &str,
        method_name: &str,
        binding_id: &BindingId,
    ) -> Option<&BindingUseFacts> {
        let mut matches = self
            .methods_by_rust_name
            .iter()
            .filter_map(|(key, method)| {
                rust_method_family_key_matches(key, type_name, method_name)
                    .then(|| method.binding_facts_for_id(binding_id))
                    .flatten()
            });
        let first = matches.next()?;
        matches.next().is_none().then_some(first)
    }

    fn source_method_usage_by_java_key(&self, key: &JavaCallableKey) -> Option<&MethodUsageFacts> {
        self.methods_by_java_key.get(key)
    }

    pub fn source_method_usage(
        &self,
        ty: &TypeDecl,
        method: &crate::java_ir::MethodDecl,
    ) -> Option<&MethodUsageFacts> {
        self.source_method_usage_for_owner(JavaOwnerKey::source_type(ty), method)
    }

    /// Whether this exact Java callable slot mutates its receiver in any
    /// equivalent package-summary copy of the declaring source type.
    pub(crate) fn source_method_or_alias_mutates(
        &self,
        ty: &TypeDecl,
        method: &crate::java_ir::MethodDecl,
    ) -> bool {
        let query = JavaCallableKey::source_method_for_type(ty, method);
        if self
            .methods_by_java_key
            .get(&query)
            .is_some_and(MethodUsageFacts::mutates_self)
        {
            return true;
        }
        let candidates = self
            .methods_by_java_key
            .iter()
            .filter(|(candidate, usage)| usage.mutates_self() && candidate.slot() == query.slot());
        let owner_query_ambiguous = legacy_usage_owner_query_is_ambiguous(
            candidates.clone().map(|(candidate, _)| candidate.owner()),
            query.owner(),
        );
        candidates.into_iter().any(|(candidate, _)| {
            legacy_usage_owner_fact_matches(candidate.owner(), query.owner(), owner_query_ambiguous)
        })
    }

    /// Whether any source implementation of this Java method family mutates
    /// its receiver. Generic and wildcard receivers can erase the concrete
    /// declaring owner, so signature and expression lowering share this
    /// conservative capability query.
    pub(crate) fn source_method_name_anywhere_mutates(&self, method_name: &str) -> bool {
        let canonical_method = crate::parser::to_snake_case(method_name);
        self.method_entries().any(|(key, method_usage)| {
            (key.method_name() == method_name || key.method_name() == canonical_method)
                && method_usage.mutates_self()
        })
    }

    fn source_method_usage_for_owner(
        &self,
        owner: JavaOwnerKey,
        method: &crate::java_ir::MethodDecl,
    ) -> Option<&MethodUsageFacts> {
        self.source_method_usage_by_java_key(&JavaCallableKey::source_method_for_owner(
            owner, method,
        ))
    }

    pub(crate) fn resolved_source_method_usage_for_owner(
        &self,
        owner: &str,
        method: &crate::java_ir::MethodDecl,
    ) -> Option<&MethodUsageFacts> {
        self.source_method_usage_for_owner(JavaOwnerKey::resolved_source(owner), method)
    }

    pub(crate) fn resolved_source_method_usage(
        &self,
        resolved: &ResolvedCall,
        source_name: &str,
    ) -> Option<&MethodUsageFacts> {
        self.source_method_usage_by_java_key(&resolved_source_method_key(resolved, source_name))
    }

    pub fn source_constructor_usage(
        &self,
        ty: &TypeDecl,
        ctor: &crate::java_ir::ConstructorDecl,
    ) -> Option<&MethodUsageFacts> {
        self.constructor_usage_by_key(&UsageConstructorKey::source_constructor(ty, ctor))
    }
}

#[cfg(test)]
impl UsageFacts {
    pub(crate) fn insert_source_method_usage(
        &mut self,
        ty: &TypeDecl,
        method: &crate::java_ir::MethodDecl,
        usage: MethodUsageFacts,
    ) {
        self.methods_by_java_key
            .insert(JavaCallableKey::source_method_for_type(ty, method), usage);
    }

    pub(crate) fn source_method_usage_has_collapsed_owner_alias(
        &self,
        aliases: &[&str],
        java_name: &str,
    ) -> bool {
        self.methods_by_java_key
            .keys()
            .any(|key| aliases.contains(&key.owner()) && key.java_name() == java_name)
    }

    pub(crate) fn insert_rust_name_method_usage(
        &mut self,
        key: UsageMethodKey,
        usage: MethodUsageFacts,
    ) {
        self.methods_by_rust_name.insert(key, usage);
    }
}

fn resolved_source_method_key(resolved: &ResolvedCall, source_name: &str) -> JavaCallableKey {
    JavaCallableKey::resolved_source_method(
        JavaOwnerKey::resolved_source(resolved.declaring_type.clone()),
        source_name,
        &resolved.arg_target_types,
        resolved.kind,
    )
}

fn contains_resolved_source_method_key(
    methods: &HashSet<JavaCallableKey>,
    resolved: &ResolvedCall,
    source_name: &str,
) -> bool {
    methods.contains(&resolved_source_method_key(resolved, source_name))
}

fn contains_aliased_resolved_source_method_key(
    methods: &HashSet<JavaCallableKey>,
    resolved: &ResolvedCall,
    source_name: &str,
) -> bool {
    let query = resolved_source_method_key(resolved, source_name);
    let owner_query_ambiguous = legacy_usage_owner_query_is_ambiguous(
        methods
            .iter()
            .filter(|candidate| candidate.slot() == query.slot())
            .map(JavaCallableKey::owner),
        query.owner(),
    );
    methods.iter().any(|candidate| {
        candidate.slot() == query.slot()
            && legacy_usage_owner_fact_matches(
                candidate.owner(),
                query.owner(),
                owner_query_ambiguous,
            )
    })
}

impl MethodUsageFacts {
    pub(crate) fn source_local_null_slot(
        &self,
        binding: &BindingId,
    ) -> Option<&crate::null_diagnostic::SourceLocalSlot> {
        self.source_local_slots.as_ref()?.get(binding)
    }

    pub(crate) fn source_array_loop_receiver_slot(&self, binding: &BindingId) -> Option<usize> {
        self.source_local_slots
            .as_ref()?
            .array_loop_receiver(binding)
    }
    pub(crate) fn null_assigned_source_fields(&self) -> &[crate::facts::FieldKey] {
        &self.null_assigned_source_fields
    }

    pub(crate) fn native_collection_retention_requirements(
        &self,
    ) -> &[NativeCollectionStorageSlot] {
        &self.native_collection_retention_requirements
    }

    /// Whether evaluating the body reads the enclosing instance, including
    /// field reads which do not participate in a specialized FieldUse flow.
    pub(crate) fn uses_receiver(&self) -> bool {
        self.uses_receiver
    }

    pub fn call_sites(&self) -> &[CallSiteUse] {
        &self.call_sites
    }

    pub fn constructor_call_sites(&self) -> &[ConstructorCallSiteUse] {
        &self.constructor_call_sites
    }

    pub fn returned_call_results(&self) -> &[ReturnedCallResultUse] {
        &self.returned_call_results
    }

    pub fn field_uses(&self) -> &[FieldUse] {
        &self.field_uses
    }

    fn same_type_call_arg_target_type(
        &self,
        name: &str,
        arg_count: usize,
        arg_index: usize,
    ) -> Option<&JavaType> {
        self.same_type_method_arg_targets
            .get(&(name.to_string(), arg_count))
            .and_then(Option::as_ref)
            .and_then(|targets| targets.get(arg_index))
    }

    pub fn mutates_self(&self) -> bool {
        self.mutates_self
    }

    fn field_is_static(&self, name: &str) -> bool {
        self.static_field_names.contains(name)
            || self
                .static_field_names
                .contains(&crate::parser::to_snake_case(name))
    }

    pub(crate) fn self_passes_unknown_boundary(&self) -> bool {
        self.self_argument_uses
            .iter()
            .any(|use_site| use_site.boundary == CallArgumentBoundary::Unknown)
    }

    pub fn type_params_in_scope(&self) -> &[String] {
        &self.type_params_in_scope
    }

    #[cfg(test)]
    pub(crate) fn mark_mutates_self_for_test(&mut self) {
        self.mutates_self = true;
    }

    #[cfg(test)]
    pub(crate) fn from_exact_binding_facts(
        bindings: impl IntoIterator<Item = BindingUseFacts>,
    ) -> Self {
        let mut facts = Self::default();
        for binding in bindings {
            if let Some(binding_id) = binding.binding.clone() {
                facts.bindings_by_id.insert(binding_id, binding);
            }
        }
        facts
    }

    #[cfg(test)]
    pub(crate) fn from_idless_compatibility_facts(
        bindings: impl IntoIterator<Item = BindingUseFacts>,
    ) -> Self {
        let mut facts = Self::default();
        for binding in bindings {
            if binding.binding.is_none() {
                facts.bindings.insert(binding.name.clone(), binding);
            }
        }
        facts
    }

    #[cfg(test)]
    fn from_visible_binding_fixture_facts(
        bindings: impl IntoIterator<Item = BindingUseFacts>,
    ) -> Self {
        let mut facts = Self::default();
        for binding in bindings {
            facts.insert_visible_binding_facts_for_scope(binding);
        }
        facts
    }

    #[cfg(test)]
    pub(crate) fn insert_binding_id_test_fact(
        &mut self,
        binding: BindingId,
        facts: BindingUseFacts,
    ) {
        self.bindings_by_id.insert(binding, facts);
    }

    #[cfg(test)]
    pub(crate) fn insert_idless_compatibility_test_fact(
        &mut self,
        name: impl Into<String>,
        facts: BindingUseFacts,
    ) {
        self.bindings.insert(name.into(), facts);
    }

    pub(crate) fn binding_facts_for_declaration_id(
        &self,
        binding: &BindingId,
    ) -> Option<&BindingUseFacts> {
        self.bindings_by_id
            .get(binding)
            .filter(|facts| binding_id_facts_are_current(binding, facts))
    }

    fn idless_compatibility_binding_facts_for_source_name(
        &self,
        name: &str,
    ) -> Option<&BindingUseFacts> {
        self.current_visible_binding_facts_for_name(name)
    }

    fn current_visible_binding_facts_for_name(&self, name: &str) -> Option<&BindingUseFacts> {
        let name_binding = self.bindings.get(name)?;
        if !self.binding_use_facts_are_current(name, name_binding) {
            return None;
        }
        if let Some(id) = &name_binding.binding {
            return self.binding_facts_for_declaration_id(id);
        }
        Some(name_binding)
    }

    pub(crate) fn binding_facts_for_param(&self, param: &Param) -> Option<&BindingUseFacts> {
        if let Some(binding) = &param.binding {
            self.binding_facts_for_declaration_id(binding)
        } else {
            self.idless_compatibility_binding_facts_for_source_name(&param.source_name)
        }
    }

    fn visible_binding_id_for_name(&self, name: &str) -> Option<BindingId> {
        self.current_visible_binding_facts_for_name(name)
            .and_then(|binding| binding.binding.clone())
    }

    fn visible_binding_java_type_for_name(&self, name: &str) -> Option<&JavaType> {
        let binding = self.current_visible_binding_facts_for_name(name)?;
        binding.binding.as_ref()?;
        Some(&binding.java_ty)
    }

    pub(crate) fn receiver_handle_java_type(&self, handle: &str) -> Option<&JavaType> {
        self.visible_binding_java_type_for_name(handle)
    }

    pub(crate) fn binding_facts_for_id(&self, binding_id: &BindingId) -> Option<&BindingUseFacts> {
        self.bindings_by_id
            .get(binding_id)
            .filter(|facts| binding_id_facts_are_current(binding_id, facts))
    }

    fn name_binding_is_current(&self, name: &str) -> bool {
        self.current_visible_binding_facts_for_name(name).is_some()
    }

    fn snapshot_visible_binding_for_scope(&self, name: &str) -> Option<BindingUseFacts> {
        self.current_visible_binding_facts_for_name(name).cloned()
    }

    fn current_visible_name_binding_mut_for_scope(
        &mut self,
        name: &str,
    ) -> Option<&mut BindingUseFacts> {
        if self.name_binding_is_current(name) {
            self.bindings.get_mut(name)
        } else {
            None
        }
    }

    fn hide_visible_binding_for_scope(&mut self, name: &str) -> Option<BindingUseFacts> {
        let saved = self.snapshot_visible_binding_for_scope(name);
        self.bindings.remove(name);
        saved
    }

    fn restore_visible_binding_for_scope(&mut self, name: String, saved: Option<BindingUseFacts>) {
        self.bindings.remove(&name);
        if let Some(saved) = saved {
            let restored = if let Some(binding) = saved.binding.as_ref() {
                let Some(current) = self.bindings_by_id.get(binding).cloned() else {
                    return;
                };
                current
            } else {
                saved
            };
            if restored.name != name {
                return;
            }
            if let Some(binding) = &restored.binding {
                self.bindings.insert(name, restored.clone());
                self.bindings_by_id.insert(binding.clone(), restored);
            } else {
                self.bindings.insert(name, restored);
            }
        }
    }

    fn insert_visible_binding_for_scope(
        &mut self,
        name: String,
        binding: Option<BindingId>,
        java_ty: JavaType,
    ) -> Option<BindingUseFacts> {
        let binding_facts = BindingUseFacts {
            name: name.clone(),
            binding: binding.clone(),
            java_ty,
            uses: Vec::new(),
        };
        self.insert_visible_binding_facts_for_scope(binding_facts)
    }

    fn insert_visible_binding_facts_for_scope(
        &mut self,
        binding_facts: BindingUseFacts,
    ) -> Option<BindingUseFacts> {
        let name = binding_facts.name.clone();
        if let Some(binding) = binding_facts.binding.clone() {
            self.bindings_by_id.insert(binding, binding_facts.clone());
        }
        self.bindings.insert(name, binding_facts)
    }

    fn record_visible_binding_use(&mut self, name: &str, usage: BindingUse) {
        if let Some(binding_id) = self.visible_binding_id_for_name(name) {
            if let Some(binding) = self.bindings_by_id.get_mut(&binding_id) {
                binding.uses.push(usage);
            }
        } else if let Some(binding) = self.current_visible_name_binding_mut_for_scope(name) {
            binding.uses.push(usage);
        }
    }

    fn record_declared_binding_use(
        &mut self,
        name: &str,
        binding_id: Option<&BindingId>,
        usage: BindingUse,
    ) {
        let mut recorded_by_id = false;
        if let Some(binding_id) = binding_id {
            let binding_is_current = self
                .bindings_by_id
                .get(binding_id)
                .is_some_and(|facts| binding_id_facts_are_current(binding_id, facts));
            if binding_is_current && let Some(binding) = self.bindings_by_id.get_mut(binding_id) {
                binding.uses.push(usage.clone());
                recorded_by_id = true;
            }
        }
        if let Some(binding) = self.current_visible_name_binding_mut_for_scope(name) {
            binding.uses.push(usage);
        } else if !recorded_by_id {
            self.record_visible_binding_use(name, usage);
        }
    }

    pub fn all_binding_facts(&self) -> Vec<&BindingUseFacts> {
        let mut facts = Vec::new();
        let mut seen_bindings = HashSet::new();
        for (binding_id, binding) in &self.bindings_by_id {
            if !binding_id_facts_are_current(binding_id, binding) {
                continue;
            }
            seen_bindings.insert(binding_id.clone());
            facts.push(binding);
        }
        for (name, binding) in &self.bindings {
            if !self.binding_use_facts_are_current(name, binding) {
                continue;
            }
            if let Some(binding_id) = &binding.binding {
                if seen_bindings.contains(binding_id) {
                    continue;
                }
            }
            facts.push(binding);
        }
        facts
    }

    fn id_less_binding_name_is_unambiguous(&self, name: &str) -> bool {
        !self.bindings_by_id.keys().any(|id| id.name() == name)
    }

    fn binding_use_facts_are_current(&self, name: &str, binding: &BindingUseFacts) -> bool {
        if binding.name != name {
            return false;
        }
        binding
            .binding
            .as_ref()
            .map(|binding_id| {
                binding_id.name() == name
                    && self
                        .bindings_by_id
                        .get(binding_id)
                        .is_some_and(|facts| binding_id_facts_are_current(binding_id, facts))
            })
            .unwrap_or_else(|| self.id_less_binding_name_is_unambiguous(name))
    }
}

fn binding_id_facts_are_current(binding_id: &BindingId, facts: &BindingUseFacts) -> bool {
    facts.name == binding_id.name()
        && facts
            .binding
            .as_ref()
            .is_none_or(|stored| stored == binding_id)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BindingUseFacts {
    name: String,
    binding: Option<BindingId>,
    java_ty: JavaType,
    uses: Vec<BindingUse>,
}

impl BindingUseFacts {
    #[cfg(test)]
    pub(crate) fn new(
        name: impl Into<String>,
        binding: Option<BindingId>,
        java_ty: JavaType,
        uses: Vec<BindingUse>,
    ) -> Self {
        Self {
            name: name.into(),
            binding,
            java_ty,
            uses,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn binding(&self) -> Option<&BindingId> {
        self.binding.as_ref()
    }

    pub fn java_ty(&self) -> &JavaType {
        &self.java_ty
    }

    pub fn uses(&self) -> &[BindingUse] {
        &self.uses
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BindingUse {
    ContentRead,
    /// An array element reached through this many index operations is
    /// compared with Java null. The signature solver uses this evidence to
    /// retain `Option` at the corresponding nested sequence layer.
    IndexedElementComparedToNull {
        index_depth: usize,
    },
    AssignedSubtypeToLocal {
        local_name: String,
        local_binding: Option<BindingId>,
        actual_type: JavaType,
    },
    AssignedSubtypeToCollectionElement {
        collection_name: String,
        collection_binding: Option<BindingId>,
        element_type: JavaType,
        actual_type: JavaType,
    },
    AssignedSubtypeToCallArgument {
        target_type: JavaType,
        actual_type: JavaType,
    },
    AssignedToLocal {
        local_name: String,
        local_binding: Option<BindingId>,
    },
    AssignedFromCall {
        name: String,
        receiver_type: Option<JavaType>,
        boundary: CallArgumentBoundary,
    },
    PassedToCall {
        name: String,
        receiver_type: Option<JavaType>,
        arg_count: usize,
        arg_index: usize,
        boundary: CallArgumentBoundary,
    },
    ReceiverForCall {
        name: String,
        arg_count: usize,
        boundary: CallArgumentBoundary,
        has_literal_null_arg: bool,
        literal_null_arg_indices: Vec<usize>,
        argument_bindings: Vec<(usize, BindingId)>,
        non_null_arg_indices: Vec<usize>,
        nullable_source_call_arg_indices: Vec<usize>,
    },
    StoredInField {
        field_name: String,
    },
    /// A selected call retains the argument in its result or other storage.
    RetainedByCall,
    Returned,
    ReturnedNullableSequenceElements,
    CastTarget {
        target_ty: JavaType,
    },
    InstanceOfTarget {
        target_ty: JavaType,
    },
    Mutated,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FieldUse {
    AssignedToLocal {
        field_name: String,
        local_name: String,
        local_binding: Option<BindingId>,
    },
    AssignedSubtypeToField {
        field_name: String,
        field_type: JavaType,
        actual_type: JavaType,
    },
    AssignedSubtypeToCallArgument {
        target_type: JavaType,
        actual_type: JavaType,
    },
    AssignedSubtypeToReturn {
        target_type: JavaType,
        actual_type: JavaType,
    },
    AssignedSubtypeToCollectionElement {
        field_name: String,
        element_type: JavaType,
        actual_type: JavaType,
    },
    AssignedFromCall {
        field_name: String,
        name: String,
        receiver_type: Option<JavaType>,
        boundary: CallArgumentBoundary,
    },
    PassedToCall {
        field_name: String,
        name: String,
        receiver_type: Option<JavaType>,
        arg_count: usize,
        arg_index: usize,
        boundary: CallArgumentBoundary,
    },
    ReceiverForCall {
        field_name: String,
        receiver_type: Option<JavaType>,
        name: String,
        arg_count: usize,
        literal_null_arg_indices: Vec<usize>,
        argument_bindings: Vec<(usize, BindingId)>,
        non_null_arg_indices: Vec<usize>,
        nullable_source_call_arg_indices: Vec<usize>,
    },
    Returned {
        field_name: String,
    },
    ReturnedNullableSequenceElements {
        field_name: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallArgumentBoundary {
    InternalSameType,
    KnownContentRead,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CollectionArgumentOrigin {
    MethodReturn(ResolvedCall),
    Field { owner: String, name: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum NativeCollectionStorageRoot {
    Binding(BindingId),
    Return,
    Origin(CollectionArgumentOrigin),
    MethodArgument {
        call: ResolvedCall,
        index: usize,
    },
    ConstructorArgument {
        call: ResolvedConstructor,
        index: usize,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NativeCollectionStorageSlot {
    pub root: NativeCollectionStorageRoot,
    pub path: Vec<usize>,
    pub ty: JavaType,
    // Preserve the declaration behind a projected path. Legacy sequence-depth
    // constraints address map values at child one, not map keys at child zero.
    pub root_ty: JavaType,
}

impl NativeCollectionStorageSlot {
    fn child(&self, path: &[usize]) -> Option<Self> {
        let ty = crate::slot_shape::RustSlotShape::native_child_type_at_path(&self.ty, path)?;
        let mut child = self.clone();
        child.path.extend_from_slice(path);
        child.ty = ty;
        Some(child)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NativeCallbackValueDomains {
    result: Option<NativeCollectionStorageSlot>,
}

/// Invocation transfers a value, not ownership of its Java allocation. Use
/// the ordinary value edge for nullability and the retained-reference edge
/// for mutable native children, just as direct source calls do.
fn record_functional_value_domain_transfer(
    source: NativeCollectionStorageSlot,
    target: NativeCollectionStorageSlot,
    facts: &mut MethodUsageFacts,
    types: &TypeEnv,
    context: &UsageAnalysisContext<'_>,
) {
    facts
        .nullable_collection_storage_flows
        .push((source.clone(), target.clone()));
    if native_storage_value_type(&source.ty, types, context)
        && native_storage_value_type(&target.ty, types, context)
    {
        facts.native_collection_storage_flows.push((source, target));
    } else {
        let effective = crate::effective_facts::EffectiveFacts::new(context.facts);
        if effective.native_functional_carrier(&source.ty).is_some()
            && effective.native_functional_carrier(&target.ty).is_some()
        {
            facts
                .native_collection_reference_transfers
                .push((source, target));
        }
    }
}

/// Parsed source callable signatures own invocation nullability. Their generic
/// argument domains carry allocation placement; putting the same root null in
/// a type argument as well would instantiate Option<A> with A = Option<Payload>.
fn record_functional_projected_transfer(
    source: NativeCollectionStorageSlot,
    target: NativeCollectionStorageSlot,
    interface: &JavaType,
    facts: &mut MethodUsageFacts,
    types: &TypeEnv,
    context: &UsageAnalysisContext<'_>,
) {
    if source_functional_call(interface, context).is_some() {
        if native_storage_value_type(&source.ty, types, context)
            && native_storage_value_type(&target.ty, types, context)
        {
            facts.native_collection_storage_flows.push((source, target));
        }
    } else {
        record_functional_value_domain_transfer(source, target, facts, types, context);
    }
}

fn source_functional_call(
    ty: &JavaType,
    context: &UsageAnalysisContext<'_>,
) -> Option<ResolvedCall> {
    let ty = crate::types::optional_element_type(ty).unwrap_or_else(|| ty.clone());
    context
        .source_symbols
        .as_ref()?
        .source_functional_interface_call(&ty)
}

impl MethodUsageFacts {
    pub(crate) fn native_list_projection_slots(&self) -> &[NativeCollectionStorageSlot] {
        &self.native_list_projection_slots
    }

    pub(crate) fn nullable_collection_storage_flows(
        &self,
    ) -> &[(NativeCollectionStorageSlot, NativeCollectionStorageSlot)] {
        &self.nullable_collection_storage_flows
    }

    pub(crate) fn native_collection_storage_flows(
        &self,
    ) -> &[(NativeCollectionStorageSlot, NativeCollectionStorageSlot)] {
        &self.native_collection_storage_flows
    }

    pub(crate) fn native_collection_view_backings(
        &self,
    ) -> &[(
        NativeCollectionStorageSlot,
        NativeCollectionStorageSlot,
        usize,
    )] {
        &self.native_collection_view_backings
    }

    pub(crate) fn native_collection_reference_transfers(
        &self,
    ) -> &[(NativeCollectionStorageSlot, NativeCollectionStorageSlot)] {
        &self.native_collection_reference_transfers
    }

    pub(crate) fn native_callback_capture_domains(
        &self,
    ) -> &[(NativeCollectionStorageSlot, NativeCollectionStorageSlot)] {
        &self.native_callback_capture_domains
    }

    pub(crate) fn native_callback_environment_domains(
        &self,
    ) -> &[(NativeCollectionStorageSlot, NativeCollectionStorageSlot)] {
        &self.native_callback_environment_domains
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallSiteUse {
    name: String,
    arg_count: usize,
    result_returned: bool,
    receiver_is_self: bool,
    receiver_targets_self_field: bool,
    receiver_type: Option<JavaType>,
    null_arg_indices: Vec<usize>,
    nullable_source_call_arg_indices: Vec<usize>,
    argument_bindings: Vec<(usize, BindingId)>,
    argument_self_fields: Vec<(usize, String)>,
    collection_argument_origins: Vec<(usize, CollectionArgumentOrigin)>,
    argument_result_calls: Vec<(usize, ResolvedCall)>,
    native_return_argument_calls: Vec<(usize, CallResultBoundaryPolicy)>,
    boundary: CallArgumentBoundary,
    resolved: Option<ResolvedCall>,
}

impl CallSiteUse {
    pub(crate) fn native_return_argument_calls(&self) -> &[(usize, CallResultBoundaryPolicy)] {
        &self.native_return_argument_calls
    }

    pub(crate) fn argument_result_calls(&self) -> &[(usize, ResolvedCall)] {
        &self.argument_result_calls
    }
    pub(crate) fn collection_argument_origins(&self) -> &[(usize, CollectionArgumentOrigin)] {
        &self.collection_argument_origins
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn arg_count(&self) -> usize {
        self.arg_count
    }

    pub fn result_returned(&self) -> bool {
        self.result_returned
    }

    pub fn receiver_is_self(&self) -> bool {
        self.receiver_is_self
    }

    pub fn receiver_targets_self_field(&self) -> bool {
        self.receiver_targets_self_field
    }

    pub fn receiver_type(&self) -> Option<&JavaType> {
        self.receiver_type.as_ref()
    }

    pub fn null_arg_indices(&self) -> &[usize] {
        &self.null_arg_indices
    }

    pub fn nullable_source_call_arg_indices(&self) -> &[usize] {
        &self.nullable_source_call_arg_indices
    }

    pub fn argument_bindings(&self) -> &[(usize, BindingId)] {
        &self.argument_bindings
    }

    pub fn argument_self_fields(&self) -> &[(usize, String)] {
        &self.argument_self_fields
    }

    pub fn boundary(&self) -> CallArgumentBoundary {
        self.boundary
    }

    pub fn resolved(&self) -> Option<&ResolvedCall> {
        self.resolved.as_ref()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConstructorCallSiteUse {
    name: String,
    arg_count: usize,
    null_arg_indices: Vec<usize>,
    nullable_source_call_arg_indices: Vec<usize>,
    argument_bindings: Vec<(usize, BindingId)>,
    argument_self_fields: Vec<(usize, String)>,
    collection_argument_origins: Vec<(usize, CollectionArgumentOrigin)>,
    argument_result_calls: Vec<(usize, ResolvedCall)>,
    native_return_argument_calls: Vec<(usize, CallResultBoundaryPolicy)>,
    boundary: CallArgumentBoundary,
    resolved: Option<ResolvedConstructor>,
}

impl ConstructorCallSiteUse {
    pub(crate) fn native_return_argument_calls(&self) -> &[(usize, CallResultBoundaryPolicy)] {
        &self.native_return_argument_calls
    }

    pub(crate) fn argument_result_calls(&self) -> &[(usize, ResolvedCall)] {
        &self.argument_result_calls
    }
    pub(crate) fn collection_argument_origins(&self) -> &[(usize, CollectionArgumentOrigin)] {
        &self.collection_argument_origins
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn arg_count(&self) -> usize {
        self.arg_count
    }

    pub fn null_arg_indices(&self) -> &[usize] {
        &self.null_arg_indices
    }

    pub fn nullable_source_call_arg_indices(&self) -> &[usize] {
        &self.nullable_source_call_arg_indices
    }

    pub fn argument_bindings(&self) -> &[(usize, BindingId)] {
        &self.argument_bindings
    }

    pub fn argument_self_fields(&self) -> &[(usize, String)] {
        &self.argument_self_fields
    }

    pub fn boundary(&self) -> CallArgumentBoundary {
        self.boundary
    }

    pub fn resolved(&self) -> Option<&ResolvedConstructor> {
        self.resolved.as_ref()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReturnedCallResultUse {
    name: String,
    arg_count: usize,
    receiver_type: Option<JavaType>,
    boundary: CallArgumentBoundary,
    resolved: Option<ResolvedCall>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AssignedCallResultUse {
    pub binding: BindingId,
    pub call: CallResultBoundaryPolicy,
}

impl MethodUsageFacts {
    pub(crate) fn assigned_call_results(&self) -> &[AssignedCallResultUse] {
        &self.assigned_call_results
    }
}

impl ReturnedCallResultUse {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn arg_count(&self) -> usize {
        self.arg_count
    }

    pub fn receiver_type(&self) -> Option<&JavaType> {
        self.receiver_type.as_ref()
    }

    pub fn boundary(&self) -> CallArgumentBoundary {
        self.boundary
    }

    pub fn resolved(&self) -> Option<&ResolvedCall> {
        self.resolved.as_ref()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallResultBoundaryPolicy {
    name: String,
    arg_count: usize,
    receiver_type: Option<JavaType>,
    boundary: CallArgumentBoundary,
    resolved: Option<ResolvedCall>,
}

impl CallResultBoundaryPolicy {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn arg_count(&self) -> usize {
        self.arg_count
    }

    pub fn receiver_type(&self) -> Option<&JavaType> {
        self.receiver_type.as_ref()
    }

    pub fn boundary(&self) -> CallArgumentBoundary {
        self.boundary
    }

    pub fn resolved(&self) -> Option<&ResolvedCall> {
        self.resolved.as_ref()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConstructorInvocationBoundaryPolicy {
    kind: ConstructorInvocationKind,
    boundary: CallArgumentBoundary,
}

impl ConstructorInvocationBoundaryPolicy {
    pub fn kind(&self) -> ConstructorInvocationKind {
        self.kind
    }

    pub fn boundary(&self) -> CallArgumentBoundary {
        self.boundary
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallArgumentBoundaryPolicy {
    name: String,
    boundary: CallArgumentBoundary,
}

impl CallArgumentBoundaryPolicy {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn boundary(&self) -> CallArgumentBoundary {
        self.boundary
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MethodApiBoundaryPolicy {
    public_api: bool,
}

impl MethodApiBoundaryPolicy {
    pub fn public_api(&self) -> bool {
        self.public_api
    }
}

pub fn analyze_usage(program: &Program) -> UsageFacts {
    analyze_usage_with_facts(program, &FactSet::default())
}

pub fn analyze_usage_with_facts(program: &Program, semantic_facts: &FactSet) -> UsageFacts {
    let mut semantic_facts = UsageAnalysisContext::new(semantic_facts);
    semantic_facts.source_symbols = Some(crate::resolve::source_symbols_from_program(program));
    for ty in &program.types {
        semantic_facts.collect_source_return_tables(ty);
    }
    let mut facts = UsageFacts::default();
    for ty in &program.types {
        analyze_type_usage(program, ty, &mut facts, &semantic_facts);
    }
    refine_source_null_assertion_call_boundaries(program, &mut facts);
    refine_source_declared_argument_call_boundaries(program, &mut facts);
    propagate_self_mutability_through_self_calls(&mut facts);
    facts
}

/// A source helper that only rejects `null` and returns its `Object` argument
/// cannot retain that argument.  Treat it like the other known content-read
/// boundaries, so the storage plan does not turn an otherwise local value into
/// `Rc<RefCell<_>>` merely because a Java null assertion was factored into a
/// sibling class.
fn refine_source_null_assertion_call_boundaries(program: &Program, facts: &mut UsageFacts) {
    let mut targets = HashSet::new();
    for ty in &program.types {
        collect_strict_null_assertion_targets(ty, &mut targets);
    }
    if targets.is_empty() {
        return;
    }
    for usage in facts
        .methods
        .values_mut()
        .chain(facts.methods_by_rust_name.values_mut())
        .chain(facts.methods_by_java_key.values_mut())
        .chain(facts.constructors.values_mut())
    {
        let mut call_groups = HashMap::<(String, usize), bool>::new();
        for call in &usage.call_sites {
            let Some(resolved) = &call.resolved else {
                continue;
            };
            let key = (call.name.clone(), call.arg_count);
            let is_assertion = targets.contains(&(
                resolved.declaring_type.clone(),
                resolved.rust_name.clone(),
                call.arg_count,
            ));
            call_groups
                .entry(key)
                .and_modify(|all_assertions| *all_assertions &= is_assertion)
                .or_insert(is_assertion);
        }
        refine_usage_call_boundaries(usage, &call_groups);
    }
}

/// A resolved source call whose parameter has the same declared source type as
/// its argument is not an FFI hand-off.  In particular, context-source
/// utilities routinely inspect such values without retaining them.  Preserve
/// that source boundary so a local does not get erased solely because its
/// helper lives in a separate input source.
fn refine_source_declared_argument_call_boundaries(program: &Program, facts: &mut UsageFacts) {
    let mut source_types = HashSet::new();
    for ty in &program.types {
        collect_source_type_names(ty, &mut source_types);
    }
    if source_types.is_empty() {
        return;
    }
    let mut targets = HashMap::<(String, usize, usize), Option<JavaType>>::new();
    for usage in facts
        .methods
        .values()
        .chain(facts.methods_by_rust_name.values())
        .chain(facts.methods_by_java_key.values())
        .chain(facts.constructors.values())
    {
        for call in &usage.call_sites {
            let Some(resolved) = &call.resolved else {
                continue;
            };
            for (index, param_ty) in resolved.arg_target_types.iter().enumerate() {
                if !source_declared_call_param_type(param_ty, &source_types) {
                    continue;
                }
                let key = (call.name.clone(), call.arg_count, index);
                targets
                    .entry(key)
                    .and_modify(|existing| {
                        if existing.as_ref() != Some(param_ty) {
                            *existing = None;
                        }
                    })
                    .or_insert_with(|| Some(param_ty.clone()));
            }
        }
    }
    if targets.is_empty() {
        return;
    }
    for usage in facts
        .methods
        .values_mut()
        .chain(facts.methods_by_rust_name.values_mut())
        .chain(facts.methods_by_java_key.values_mut())
        .chain(facts.constructors.values_mut())
    {
        for binding in usage.bindings_by_id.values_mut() {
            let binding_ty = binding.java_ty().clone();
            for use_site in &mut binding.uses {
                let BindingUse::PassedToCall {
                    name,
                    arg_count,
                    arg_index,
                    boundary,
                    ..
                } = use_site
                else {
                    continue;
                };
                if *boundary == CallArgumentBoundary::Unknown
                    && targets
                        .get(&(name.clone(), *arg_count, *arg_index))
                        .and_then(Option::as_ref)
                        .is_some_and(|target| *target == binding_ty)
                {
                    *boundary = CallArgumentBoundary::KnownContentRead;
                }
            }
        }
    }
}

fn collect_source_type_names(ty: &TypeDecl, names: &mut HashSet<String>) {
    names.insert(ty.name.clone());
    for member in &ty.members {
        if let Member::Type(nested) = member {
            collect_source_type_names(nested, names);
        }
    }
}

fn source_declared_call_param_type(java_ty: &JavaType, source_types: &HashSet<String>) -> bool {
    match java_ty {
        JavaType::Named { name, .. } | JavaType::Unknown(name) => source_types.contains(name),
        _ => false,
    }
}

fn refine_usage_call_boundaries(
    usage: &mut MethodUsageFacts,
    call_groups: &HashMap<(String, usize), bool>,
) {
    let is_strict_assertion = |name: &str, arg_count: usize| {
        call_groups
            .get(&(name.to_string(), arg_count))
            .copied()
            .unwrap_or(false)
    };
    for binding in usage.bindings_by_id.values_mut() {
        for use_site in &mut binding.uses {
            if let BindingUse::PassedToCall {
                name,
                arg_count,
                boundary,
                ..
            } = use_site
                && *boundary == CallArgumentBoundary::Unknown
                && is_strict_assertion(name, *arg_count)
            {
                *boundary = CallArgumentBoundary::KnownContentRead;
            }
        }
    }
    for field_use in &mut usage.field_uses {
        if let FieldUse::PassedToCall {
            name,
            arg_count,
            boundary,
            ..
        } = field_use
            && *boundary == CallArgumentBoundary::Unknown
            && is_strict_assertion(name, *arg_count)
        {
            *boundary = CallArgumentBoundary::KnownContentRead;
        }
    }
    for use_site in &mut usage.self_argument_uses {
        if use_site.boundary == CallArgumentBoundary::Unknown
            && is_strict_assertion(&use_site.name, use_site.arg_count)
        {
            use_site.boundary = CallArgumentBoundary::KnownContentRead;
        }
    }
}

fn collect_strict_null_assertion_targets(
    ty: &TypeDecl,
    targets: &mut HashSet<(String, String, usize)>,
) {
    for member in &ty.members {
        match member {
            Member::Method(method) if method_is_strict_object_null_assertion(method) => {
                targets.insert((
                    ty.name.clone(),
                    source_method_rust_name(method),
                    method.params.len(),
                ));
            }
            Member::Type(nested) => collect_strict_null_assertion_targets(nested, targets),
            _ => {}
        }
    }
}

fn method_is_strict_object_null_assertion(method: &crate::java_ir::MethodDecl) -> bool {
    let [param] = method.params.as_slice() else {
        return false;
    };
    if !matches!(
        &param.ty,
        JavaType::Named { name, .. } | JavaType::Unknown(name)
            if crate::stdlib::canonical_external_receiver_type_key(name)
                .rsplit('.').next().is_some_and(|simple| simple == "Object")
    ) {
        return false;
    }
    let Some(body) = &method.body else {
        return false;
    };
    let [
        Stmt::If {
            condition,
            then_block,
            else_block,
        },
        rest @ ..,
    ] = body.statements.as_slice()
    else {
        return false;
    };
    else_block.is_none()
        && matches!(condition, Expr::Binary { op: BinaryOp::Eq, left, right }
            if (matches!(left.as_ref(), Expr::Var(name) if name == &param.name)
                && matches!(right.as_ref(), Expr::Literal(Literal::Null)))
             || (matches!(right.as_ref(), Expr::Var(name) if name == &param.name)
                && matches!(left.as_ref(), Expr::Literal(Literal::Null))))
        && !then_block.statements.is_empty()
        && then_block
            .statements
            .iter()
            .all(|stmt| matches!(stmt, Stmt::Throw(_)))
        && rest.iter().all(|stmt| match stmt {
            Stmt::Return(None) => true,
            Stmt::Return(Some(Expr::Var(name))) => name == &param.name,
            _ => false,
        })
}

fn propagate_self_mutability_through_self_calls(facts: &mut UsageFacts) {
    loop {
        let source_methods = facts
            .methods_by_java_key
            .keys()
            .cloned()
            .collect::<HashSet<_>>();
        let mutating_source_methods = facts
            .methods_by_java_key
            .iter()
            .filter_map(|(key, usage)| usage.mutates_self.then(|| key.clone()))
            .collect::<HashSet<_>>();
        let mut mutating_compat_methods = facts
            .methods
            .iter()
            .chain(facts.methods_by_rust_name.iter())
            .filter_map(|(key, usage)| {
                usage
                    .mutates_self
                    .then(|| (key.type_name().to_string(), key.method_name.clone()))
            })
            .collect::<HashSet<_>>();
        mutating_compat_methods.extend(facts.methods_by_java_key.iter().filter_map(
            |(key, usage)| {
                usage
                    .mutates_self
                    .then(|| (key.owner().to_string(), key.java_name().to_string()))
            },
        ));
        let mut changed = false;
        // The owner type is carried by each map's key, so an *unqualified* self
        // call can be identified without a resolved callee. That matters:
        // `resolve_expr_calls` does not descend into lambda bodies, so a call
        // made inside `xs.forEach(x -> take(v))` reaches here with
        // `resolved: None`, and the method that only mutates through a lambda
        // was inferred `&self`
        // ([0033](../claude_audits/done/0033-receiver-mutability-not-propagated-through-lambdas.md)).
        let mut mark = |owner: &str, usage: &mut MethodUsageFacts| {
            if usage.mutates_self {
                return;
            }
            if usage.call_sites.iter().any(|call_site| {
                if !(call_site.receiver_is_self || call_site.receiver_targets_self_field) {
                    return false;
                }
                if let Some(resolved) = &call_site.resolved {
                    if contains_resolved_source_method_key(
                        &mutating_source_methods,
                        resolved,
                        &call_site.name,
                    ) {
                        return true;
                    }
                    // Focused-package analysis can retain both an emitted
                    // declaration owner and its package-qualified summary
                    // owner. Resolution may point at the bodyless/read-only
                    // copy while the equivalent callable slot on the emitted
                    // owner contains the mutating body. Match that exact Java
                    // slot across owner aliases before concluding that a
                    // resolved source target is read-only; retaining the slot
                    // comparison preserves overload separation.
                    if contains_aliased_resolved_source_method_key(
                        &mutating_source_methods,
                        resolved,
                        &call_site.name,
                    ) {
                        return true;
                    }
                    if source_methods
                        .contains(&resolved_source_method_key(resolved, &call_site.name))
                    {
                        return false;
                    }
                    let rust_name = crate::parser::to_snake_case(&call_site.name);
                    if legacy_mutating_method_contains(
                        &mutating_compat_methods,
                        &resolved.declaring_type,
                        &call_site.name,
                    ) || legacy_mutating_method_contains(
                        &mutating_compat_methods,
                        &resolved.declaring_type,
                        &resolved.rust_name,
                    ) || legacy_mutating_method_contains(
                        &mutating_compat_methods,
                        &resolved.declaring_type,
                        &rust_name,
                    ) {
                        return true;
                    }
                }
                if call_site
                    .receiver_type
                    .as_ref()
                    .is_some_and(|receiver_type| {
                        java_type_name_for_usage(receiver_type).is_some_and(|receiver_name| {
                            legacy_mutating_method_contains(
                                &mutating_compat_methods,
                                receiver_name,
                                &call_site.name,
                            )
                        })
                    })
                {
                    return true;
                }
                // Unqualified call naming a mutating method of the enclosing
                // type. This is what an unresolved `take(v)` is.
                call_site.receiver_is_self
                    && mutating_compat_methods
                        .contains(&(owner.to_string(), call_site.name.clone()))
            }) {
                usage.mutates_self = true;
                changed = true;
            }
        };
        for (key, usage) in facts.methods.iter_mut() {
            mark(key.type_name(), usage);
        }
        for (key, usage) in facts.methods_by_rust_name.iter_mut() {
            mark(key.type_name(), usage);
        }
        for (key, usage) in facts.methods_by_java_key.iter_mut() {
            mark(key.owner(), usage);
        }
        if !changed {
            break;
        }
    }
}

fn legacy_mutating_method_contains(
    methods: &HashSet<(String, String)>,
    owner: &str,
    method: &str,
) -> bool {
    let owner_query_ambiguous = legacy_usage_owner_query_is_ambiguous(
        methods
            .iter()
            .filter_map(|(candidate_owner, candidate_method)| {
                (candidate_method == method).then_some(candidate_owner.as_str())
            }),
        owner,
    );
    methods.iter().any(|(candidate_owner, candidate_method)| {
        candidate_method == method
            && legacy_usage_owner_fact_matches(candidate_owner, owner, owner_query_ambiguous)
    })
}

fn legacy_usage_owner_query_is_ambiguous<'a>(
    candidates: impl Iterator<Item = &'a str>,
    query_owner: &str,
) -> bool {
    let query_simple = usage_type_name_simple_segment(query_owner);
    let matches = candidates
        .filter(|candidate| {
            *candidate != query_owner
                && !usage_distinct_qualified_type_owners(candidate, query_owner)
                && usage_type_name_simple_segment(candidate) == query_simple
        })
        .collect::<HashSet<_>>();
    matches.len() > 1
}

fn legacy_usage_owner_fact_matches(
    fact_owner: &str,
    query_owner: &str,
    query_ambiguous: bool,
) -> bool {
    fact_owner == query_owner
        || (!query_ambiguous
            && !usage_distinct_qualified_type_owners(fact_owner, query_owner)
            && usage_type_name_simple_segment(fact_owner)
                == usage_type_name_simple_segment(query_owner))
}

fn usage_type_name_simple_segment(type_name: &str) -> &str {
    type_name
        .rsplit(['.', '$', '_'])
        .next()
        .filter(|segment| !segment.is_empty())
        .unwrap_or(type_name)
}

fn usage_distinct_qualified_type_owners(left: &str, right: &str) -> bool {
    let package_summary_alias = |qualified: &str, emitted: &str| {
        let qualified = qualified.replace(['.', '$'], "_");
        let emitted = emitted.replace(['.', '$'], "_");
        qualified.len() > emitted.len()
            && qualified.ends_with(&emitted)
            && qualified.as_bytes()[qualified.len() - emitted.len() - 1] == b'_'
    };
    if package_summary_alias(left, right) || package_summary_alias(right, left) {
        return false;
    }
    (left.contains('.') || left.contains('$'))
        && (right.contains('.') || right.contains('$'))
        && left != right
}

pub fn build_call_graph(program: &Program) -> CallGraph {
    build_call_graph_with_facts(program, &FactSet::default())
}

pub fn build_call_graph_with_facts(program: &Program, facts: &FactSet) -> CallGraph {
    build_call_graph_with_optional_source_callbacks(program, facts, None)
}

#[cfg(test)]
pub(crate) fn build_call_graph_with_source_callbacks(
    program: &Program,
    facts: &FactSet,
    callbacks: &crate::source_callbacks::SourceCallbackPlan,
) -> CallGraph {
    build_call_graph_with_optional_source_callbacks(program, facts, Some(callbacks))
}

fn build_call_graph_with_optional_source_callbacks(
    program: &Program,
    facts: &FactSet,
    callbacks: Option<&crate::source_callbacks::SourceCallbackPlan>,
) -> CallGraph {
    let source_symbols = crate::resolve::source_symbols_from_program(program);
    build_call_graph_with_source_callback_symbols(program, facts, callbacks, &source_symbols)
}

pub(crate) fn build_call_graph_with_source_callback_symbols(
    program: &Program,
    facts: &FactSet,
    callbacks: Option<&crate::source_callbacks::SourceCallbackPlan>,
    source_symbols: &crate::resolve::SourceSymbols,
) -> CallGraph {
    let mut graph = CallGraph::default();
    for ty in &program.types {
        build_type_call_graph(ty, program, source_symbols, &mut graph, facts, callbacks);
    }
    graph
}

fn analyze_type_usage(
    program: &Program,
    ty: &TypeDecl,
    facts: &mut UsageFacts,
    semantic_facts: &UsageAnalysisContext<'_>,
) {
    let internal_methods = internal_method_names(ty);
    let same_type_method_arg_targets = same_type_method_arg_targets(ty);
    let fallback_return_nullability;
    let source_return_nullability = match semantic_facts.source_return_table(&ty.name) {
        Some(table) => table,
        None => {
            fallback_return_nullability =
                source_return_nullability_by_call_name(ty, semantic_facts);
            &fallback_return_nullability
        }
    };
    let mut field_names = field_names(ty, program);
    let mut field_types = field_types(ty, program);
    for field in
        crate::inherited_members::external_inherited_instance_fields_for_type(ty, semantic_facts)
    {
        for name in [field.field_name(), field.rust_name()] {
            field_names.insert(name.to_string());
            field_types
                .entry(name.to_string())
                .or_insert_with(|| field.field_type().clone());
        }
    }
    let current_super_type = ty.extends.as_deref();
    let static_field_names = std::sync::Arc::new(
        semantic_facts
            .source_symbols
            .as_ref()
            .map(|symbols| symbols.source_static_field_names(&ty.name))
            .unwrap_or_default(),
    );
    for member in &ty.members {
        match member {
            Member::Method(method) => {
                let mut method_facts = MethodUsageFacts::default();
                method_facts.source_local_slots =
                    crate::null_diagnostic::SourceLocalSlots::for_method(method)
                        .map(std::sync::Arc::new);
                method_facts.static_field_names = std::sync::Arc::clone(&static_field_names);
                method_facts.same_type_method_arg_targets = same_type_method_arg_targets.clone();
                method_facts.type_params_in_scope =
                    usage_type_params_in_scope(&ty.type_params, &method.type_params);
                let mut type_env =
                    usage_type_env(ty, &method.type_params, &method.type_param_bounds);
                for param in &method.params {
                    type_env.insert_param(param.source_name.clone(), param.ty.clone());
                    insert_binding_use_facts(
                        &mut method_facts,
                        param.source_name.clone(),
                        param.binding.clone(),
                        param.ty.clone(),
                    );
                }
                if let Some(body) = &method.body {
                    for stmt in &body.statements {
                        analyze_stmt(
                            stmt,
                            &mut method_facts,
                            &ty.name,
                            current_super_type,
                            Some(&method.return_type),
                            &internal_methods,
                            &field_names,
                            &field_types,
                            semantic_facts,
                            &type_env,
                            &source_return_nullability,
                        );
                    }
                }
                method_facts.callback_value_domains.clear();
                let rust_key = UsageMethodKey::source_method(ty, method);
                let java_key = UsageMethodKey::legacy_java_name_source_method(ty, method);
                facts
                    .methods_by_rust_name
                    .insert(rust_key, method_facts.clone());
                let rust_key = UsageMethodKey::source_method(ty, method);
                facts.methods.insert(rust_key, method_facts.clone());
                let canonical_java_key =
                    JavaCallableKey::source_method_in_program(program, ty, method);
                let declaration_java_key = JavaCallableKey::source_method_for_type(ty, method);
                facts
                    .methods_by_java_key
                    .insert(canonical_java_key.clone(), method_facts.clone());
                // Most declaration consumers hold `&TypeDecl`, whose name is
                // simple for an ordinary packaged compilation unit because the
                // package lives on `Program`. Keep that declaration-local key
                // alongside the package-qualified identity. Batch programs
                // qualify colliding peers in `TypeDecl::name`, so this does not
                // collapse `a.Item` and `b.Item` back onto `Item`.
                if declaration_java_key != canonical_java_key {
                    facts
                        .methods_by_java_key
                        .insert(declaration_java_key, method_facts.clone());
                }
                facts.methods.insert(java_key, method_facts);
            }
            Member::Constructor(ctor) => {
                let mut ctor_facts = MethodUsageFacts::default();
                ctor_facts.static_field_names = std::sync::Arc::clone(&static_field_names);
                ctor_facts.same_type_method_arg_targets = same_type_method_arg_targets.clone();
                ctor_facts.type_params_in_scope =
                    usage_type_params_in_scope(&ty.type_params, &ctor.type_params);
                let mut type_env = usage_type_env(ty, &ctor.type_params, &ctor.type_param_bounds);
                for param in &ctor.params {
                    type_env.insert_param(param.source_name.clone(), param.ty.clone());
                    insert_binding_use_facts(
                        &mut ctor_facts,
                        param.source_name.clone(),
                        param.binding.clone(),
                        param.ty.clone(),
                    );
                }
                if let Some(body) = &ctor.body {
                    for stmt in &body.statements {
                        analyze_stmt(
                            stmt,
                            &mut ctor_facts,
                            &ty.name,
                            current_super_type,
                            None,
                            &internal_methods,
                            &field_names,
                            &field_types,
                            semantic_facts,
                            &type_env,
                            &source_return_nullability,
                        );
                    }
                }
                ctor_facts.callback_value_domains.clear();
                facts.constructors.insert(
                    UsageConstructorKey::source_constructor(ty, ctor),
                    ctor_facts,
                );
            }
            Member::Type(nested) => analyze_type_usage(program, nested, facts, semantic_facts),
            _ => {}
        }
    }
}

fn same_type_method_arg_targets(ty: &TypeDecl) -> HashMap<(String, usize), Option<Vec<JavaType>>> {
    let mut targets = HashMap::new();
    for member in &ty.members {
        let Member::Method(method) = member else {
            continue;
        };
        let key = (method.name.clone(), method.params.len());
        let value = method
            .params
            .iter()
            .map(|param| param.ty.clone())
            .collect::<Vec<_>>();
        targets
            .entry(key)
            .and_modify(|existing| *existing = None)
            .or_insert(Some(value));
    }
    targets
}

fn usage_type_env(
    ty: &TypeDecl,
    member_type_params: &[String],
    member_type_param_bounds: &[Option<JavaType>],
) -> TypeEnv {
    let mut env = TypeEnv::new();
    insert_usage_type_param_bounds(&mut env, &ty.type_params, &ty.type_param_bounds);
    insert_usage_type_param_bounds(&mut env, member_type_params, member_type_param_bounds);
    env
}

fn usage_type_params_in_scope(
    type_params: &[String],
    member_type_params: &[String],
) -> Vec<String> {
    let mut scope = type_params.to_vec();
    scope.extend(member_type_params.iter().cloned());
    scope
}

fn insert_usage_type_param_bounds(
    env: &mut TypeEnv,
    type_params: &[String],
    type_param_bounds: &[Option<JavaType>],
) {
    for (name, bound) in type_params.iter().zip(type_param_bounds.iter()) {
        if let Some(bound) = bound {
            env.insert_type_param_bound(name.clone(), bound.clone());
        }
    }
}

fn build_type_call_graph(
    ty: &TypeDecl,
    program: &Program,
    source_symbols: &crate::resolve::SourceSymbols,
    graph: &mut CallGraph,
    facts: &FactSet,
    callbacks: Option<&crate::source_callbacks::SourceCallbackPlan>,
) {
    let internal_methods = all_method_names(ty);
    let callable_env = |params: &[Param], type_params: &[String], bounds: &[Option<JavaType>]| {
        let mut env = usage_type_env(ty, type_params, bounds);
        for (name, java_ty) in field_types(ty, program) {
            env.insert_field(name, java_ty);
        }
        for param in params {
            env.insert_local(param.name.clone(), param.ty.clone());
            env.insert_local(param.source_name.clone(), param.ty.clone());
        }
        for import in &program.imports {
            if !import.is_static
                && !import.is_wildcard
                && let Some(leaf) = import.path.rsplit('.').next()
            {
                env.insert_static_type_alias(leaf, import.path.clone());
            }
        }
        env
    };
    for member in &ty.members {
        match member {
            Member::Field(field) => {
                if let Some(initializer) = &field.initializer
                    && callback_value_needs_contract(initializer)
                {
                    let owner = CallableKey::FieldInitializer {
                        type_name: JavaOwnerKey::source_type(ty),
                        field_name: field.name.clone(),
                    };
                    let mut context = CallGraphBuildContext::new(
                        ty.name.clone(),
                        internal_methods.clone(),
                        owner,
                        program,
                        source_symbols,
                        callable_env(&[], &[], &[]),
                    );
                    context.source_callbacks = callbacks;
                    // Register only the deferred invocation contract. Field
                    // initialization must not execute a lambda/factory body.
                    collect_source_functional_value_contract(
                        &field.ty,
                        initializer,
                        graph,
                        &mut context,
                        facts,
                    );
                }
            }
            Member::Method(method) => {
                let caller = CallableKey::source_method_for_type(ty, method);
                graph.nodes.insert(caller.clone());
                if let Some(body) = &method.body {
                    let mut context = CallGraphBuildContext::new(
                        ty.name.clone(),
                        internal_methods.clone(),
                        caller,
                        program,
                        source_symbols,
                        callable_env(
                            &method.params,
                            &method.type_params,
                            &method.type_param_bounds,
                        ),
                    );
                    context.source_callbacks = callbacks;
                    context.return_type = Some(method.return_type.clone());
                    for (index, param) in method.params.iter().enumerate() {
                        if callbacks.is_some_and(|plan| plan.is_inline(ty, method, index)) {
                            let key = CallableKey::CallbackParameter {
                                owner: Box::new(context.current_callable.clone()),
                                index,
                            };
                            graph.nodes.insert(key.clone());
                            for name in [&param.name, &param.source_name] {
                                context
                                    .callback_parameters
                                    .insert(name.clone(), (key.clone(), param.ty.clone()));
                            }
                        }
                    }
                    collect_call_graph_edges_from_block(body, graph, &mut context, facts);
                }
            }
            Member::Constructor(ctor) => {
                let caller = CallableKey::source_constructor_for_type(ty, ctor);
                graph.nodes.insert(caller.clone());
                if let Some(body) = &ctor.body {
                    let mut context = CallGraphBuildContext::new(
                        ty.name.clone(),
                        internal_methods.clone(),
                        caller,
                        program,
                        source_symbols,
                        callable_env(&ctor.params, &ctor.type_params, &ctor.type_param_bounds),
                    );
                    context.source_callbacks = callbacks;
                    collect_call_graph_edges_from_block(body, graph, &mut context, facts);
                }
            }
            Member::Type(nested) => {
                build_type_call_graph(nested, program, source_symbols, graph, facts, callbacks)
            }
            _ => {}
        }
    }
}

#[derive(Debug, Clone)]
struct CallGraphBuildContext<'a> {
    current_type: String,
    internal_methods: HashSet<String>,
    current_callable: CallableKey,
    program: &'a Program,
    source_symbols: &'a crate::resolve::SourceSymbols,
    source_callbacks: Option<&'a crate::source_callbacks::SourceCallbackPlan>,
    callback_parameters: HashMap<String, (CallableKey, JavaType)>,
    return_type: Option<JavaType>,
    type_env: TypeEnv,
    consumed_callback_bodies: HashSet<SourceLambdaBodyKey>,
    anonymous_super_type: Option<&'a TypeDecl>,
    anonymous_method_param_types: HashMap<String, JavaType>,
    anonymous_member_methods: std::sync::Arc<Vec<(&'a crate::java_ir::MethodDecl, CallableKey)>>,
    next_lambda_ordinal: usize,
    next_anonymous_ordinal: usize,
    handled_catch_types: Vec<JavaType>,
}

impl<'a> CallGraphBuildContext<'a> {
    fn new(
        current_type: String,
        internal_methods: HashSet<String>,
        current_callable: CallableKey,
        program: &'a Program,
        source_symbols: &'a crate::resolve::SourceSymbols,
        type_env: TypeEnv,
    ) -> Self {
        Self {
            current_type,
            internal_methods,
            current_callable,
            program,
            source_symbols,
            source_callbacks: None,
            callback_parameters: HashMap::new(),
            return_type: None,
            type_env,
            consumed_callback_bodies: HashSet::new(),
            anonymous_super_type: None,
            anonymous_method_param_types: HashMap::new(),
            anonymous_member_methods: Default::default(),
            next_lambda_ordinal: 0,
            next_anonymous_ordinal: 0,
            handled_catch_types: Vec::new(),
        }
    }

    fn child_for(&self, current_callable: CallableKey) -> Self {
        Self {
            current_type: self.current_type.clone(),
            internal_methods: self.internal_methods.clone(),
            current_callable,
            program: self.program,
            source_symbols: self.source_symbols,
            source_callbacks: self.source_callbacks,
            callback_parameters: self.callback_parameters.clone(),
            return_type: None,
            type_env: self.type_env.clone(),
            consumed_callback_bodies: self.consumed_callback_bodies.clone(),
            anonymous_super_type: self.anonymous_super_type,
            anonymous_method_param_types: self.anonymous_method_param_types.clone(),
            anonymous_member_methods: self.anonymous_member_methods.clone(),
            next_lambda_ordinal: 0,
            next_anonymous_ordinal: 0,
            handled_catch_types: Vec::new(),
        }
    }

    fn with_handled_exception_boundary(&self, catches: &[crate::java_ir::CatchClause]) -> Self {
        let mut context = self.clone();
        for catch in catches {
            for catch_ty in &catch.types {
                if !context.handled_catch_types.contains(catch_ty) {
                    context.handled_catch_types.push(catch_ty.clone());
                }
            }
        }
        context
    }

    fn in_handled_exception_boundary(&self) -> bool {
        !self.handled_catch_types.is_empty()
    }

    fn next_lambda(&mut self) -> CallableKey {
        let ordinal = self.next_lambda_ordinal;
        self.next_lambda_ordinal += 1;
        CallableKey::Lambda {
            owner: Box::new(self.current_callable.clone()),
            ordinal,
        }
    }

    fn next_anonymous_method(&mut self, method_name: String) -> CallableKey {
        let ordinal = self.next_anonymous_ordinal;
        self.next_anonymous_ordinal += 1;
        CallableKey::AnonymousMethod {
            owner: Box::new(self.current_callable.clone()),
            ordinal,
            method_name,
        }
    }
}

/// Identity in the immutable source AST, not structural equality: two lambdas
/// with identical text still have distinct lexical captures and callables.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum SourceLambdaBodyKey {
    Expr(*const Expr),
    Block(*const Block),
}

impl SourceLambdaBodyKey {
    fn domain_key(self) -> (u8, usize) {
        match self {
            Self::Expr(expr) => (0, expr as usize),
            Self::Block(block) => (1, block as usize),
        }
    }
    fn for_lambda(body: &LambdaBody) -> Self {
        match body {
            LambdaBody::Expr(expr) => Self::Expr(expr.as_ref()),
            LambdaBody::Block(block) => Self::Block(block),
        }
    }
}

fn all_method_names(ty: &TypeDecl) -> HashSet<String> {
    ty.members
        .iter()
        .filter_map(|member| match member {
            Member::Method(method) => Some(method.name.clone()),
            _ => None,
        })
        .collect()
}

fn collect_call_graph_edges_from_block(
    block: &Block,
    graph: &mut CallGraph,
    context: &mut CallGraphBuildContext<'_>,
    facts: &FactSet,
) {
    let outer_types = context.type_env.clone();
    for stmt in &block.statements {
        collect_call_graph_edges_from_stmt(stmt, graph, context, facts);
        context.type_env.learn_stmt(stmt);
    }
    context.type_env = outer_types;
}

fn collect_call_graph_edges_from_stmt(
    stmt: &Stmt,
    graph: &mut CallGraph,
    context: &mut CallGraphBuildContext<'_>,
    facts: &FactSet,
) {
    match stmt {
        Stmt::VarDecl {
            ty, initializer, ..
        } => {
            if let Some(initializer) = initializer {
                collect_source_functional_value_contract(ty, initializer, graph, context, facts);
                collect_call_graph_edges_from_expr(initializer, graph, context, facts);
            }
        }
        Stmt::Assign { target, value } => {
            if callback_value_needs_contract(value) {
                let ty = crate::types::infer_expr_type(target, &context.type_env);
                collect_source_functional_value_contract(&ty, value, graph, context, facts);
            }
            collect_call_graph_edges_from_expr(target, graph, context, facts);
            collect_call_graph_edges_from_expr(value, graph, context, facts);
        }
        Stmt::Return(Some(expr)) => {
            if let Some(ty) = context.return_type.clone() {
                collect_source_functional_value_contract(&ty, expr, graph, context, facts);
            }
            collect_call_graph_edges_from_expr(expr, graph, context, facts);
        }
        Stmt::Expr(expr) | Stmt::Throw(expr) => {
            collect_call_graph_edges_from_expr(expr, graph, context, facts);
        }
        Stmt::Return(None) | Stmt::Break(_) | Stmt::Continue(_) | Stmt::Unsupported(_) => {}
        Stmt::If {
            condition,
            then_block,
            else_block,
        } => {
            collect_call_graph_edges_from_expr(condition, graph, context, facts);
            collect_call_graph_edges_from_block(then_block, graph, context, facts);
            if let Some(else_block) = else_block {
                collect_call_graph_edges_from_block(else_block, graph, context, facts);
            }
        }
        Stmt::While {
            condition, body, ..
        }
        | Stmt::DoWhile {
            condition, body, ..
        } => {
            collect_call_graph_edges_from_expr(condition, graph, context, facts);
            collect_call_graph_edges_from_block(body, graph, context, facts);
        }
        Stmt::For {
            init,
            condition,
            update,
            body,
            ..
        } => {
            for stmt in init {
                collect_call_graph_edges_from_stmt(stmt, graph, context, facts);
            }
            if let Some(condition) = condition {
                collect_call_graph_edges_from_expr(condition, graph, context, facts);
            }
            for expr in update {
                collect_call_graph_edges_from_expr(expr, graph, context, facts);
            }
            collect_call_graph_edges_from_block(body, graph, context, facts);
        }
        Stmt::EnhancedFor { iterable, body, .. } => {
            collect_call_graph_edges_from_expr(iterable, graph, context, facts);
            if let Some(plan) = crate::call_plan::plan_enhanced_for_iterator(
                iterable,
                &context.type_env,
                &context.current_type,
                context.source_symbols,
                facts,
            ) {
                if let Expr::Call {
                    receiver,
                    name,
                    args,
                    resolved,
                    ..
                } = &plan.call
                {
                    let (callee, kind) = call_graph_method_target(
                        receiver.as_deref(),
                        name,
                        args,
                        resolved,
                        context,
                        facts,
                    );
                    add_call_graph_edge(graph, context, callee, kind, facts);
                }
                for operation in crate::stdlib::IteratorProtocolOperation::ALL {
                    add_call_graph_edge(
                        graph,
                        context,
                        CallableKey::external_method(
                            Some("Iterator"),
                            operation.checked_method(),
                            0,
                        ),
                        CallGraphEdgeKind::UnresolvedExternalMethodCall,
                        facts,
                    );
                }
            }
            collect_call_graph_edges_from_block(body, graph, context, facts);
        }
        Stmt::Switch { selector, cases } => {
            collect_call_graph_edges_from_expr(selector, graph, context, facts);
            for case in cases {
                for stmt in &case.statements {
                    collect_call_graph_edges_from_stmt(stmt, graph, context, facts);
                }
            }
        }
        Stmt::Try {
            resources,
            body,
            catches,
            finally_block,
        } => {
            for resource in resources {
                collect_call_graph_edges_from_stmt(resource, graph, context, facts);
            }
            if catch_clauses_do_not_rethrow_checked_exceptions(catches, facts) {
                let mut try_context = context.with_handled_exception_boundary(catches);
                collect_call_graph_edges_from_block(body, graph, &mut try_context, facts);
            } else {
                collect_call_graph_edges_from_block(body, graph, context, facts);
            }
            for catch in catches {
                collect_call_graph_edges_from_block(&catch.body, graph, context, facts);
            }
            if let Some(finally_block) = finally_block {
                collect_call_graph_edges_from_block(finally_block, graph, context, facts);
            }
        }
        Stmt::ConstructorInvocation {
            kind,
            args,
            resolved,
        } => {
            let (callee, edge_kind) = match kind {
                ConstructorInvocationKind::This => (
                    resolved
                        .as_ref()
                        .map(|resolved| resolved_constructor_call_graph_key(resolved, context))
                        .unwrap_or_else(|| {
                            CallableKey::constructor_identity(context.current_type.clone(), "new")
                        }),
                    CallGraphEdgeKind::ConstructorDelegationThis,
                ),
                ConstructorInvocationKind::Super => (
                    resolved
                        .as_ref()
                        .map(|resolved| resolved_constructor_call_graph_key(resolved, context))
                        .unwrap_or_else(|| CallableKey::external_constructor("super", args.len())),
                    CallGraphEdgeKind::ConstructorDelegationSuper,
                ),
            };
            add_call_graph_edge(graph, context, callee, edge_kind, facts);
            if let Some(resolved) = resolved {
                collect_source_constructor_callback_arguments(
                    resolved, args, graph, context, facts,
                );
            }
            for arg in args {
                collect_call_graph_edges_from_expr(arg, graph, context, facts);
            }
        }
        Stmt::LocalType(ty) => build_type_call_graph(
            ty,
            context.program,
            context.source_symbols,
            graph,
            facts,
            context.source_callbacks,
        ),
        Stmt::Block(block) | Stmt::LabeledBlock { body: block, .. } => {
            collect_call_graph_edges_from_block(block, graph, context, facts)
        }
    }
}

/// Connect actual callback bodies to the declaring formal contract. Merely
/// constructing a callback does not execute it: the invocation edge is added
/// separately where the formal's SAM is called.
fn collect_source_callback_arguments(
    call: &ResolvedCall,
    args: &[Expr],
    graph: &mut CallGraph,
    context: &mut CallGraphBuildContext<'_>,
    facts: &FactSet,
) {
    let Some((owner, method)) = crate::java_signature::resolved_source_method_decl(
        context.program,
        &context.current_type,
        call,
    ) else {
        return;
    };
    for (index, (argument, param)) in args.iter().zip(&method.params).enumerate() {
        let ty = call.arg_target_types.get(index).unwrap_or(&param.ty);
        let source_sam = context.source_symbols.source_functional_interface_call(ty);
        if source_sam.is_none()
            && !context
                .source_callbacks
                .is_some_and(|callbacks| callbacks.is_inline(owner, method, index))
        {
            continue;
        }
        let JavaType::Named {
            name,
            args: type_args,
        } = ty
        else {
            continue;
        };
        let canonical = crate::stdlib::canonical_external_receiver_type_key(name);
        let shape = crate::stdlib::functional_interface_shape(&canonical);
        let (formal, inputs, body_edge) = if let Some(source_sam) = &source_sam {
            (
                resolved_method_call_graph_key(source_sam, context),
                source_sam.arg_target_types.clone(),
                CallGraphEdgeKind::CallbackContractEquality,
            )
        } else {
            let Some(inputs) = shape
                .as_ref()
                .and_then(|shape| shape.input_types(type_args))
            else {
                continue;
            };
            (
                CallableKey::CallbackParameter {
                    owner: Box::new(CallableKey::source_method_for_type(owner, method)),
                    index,
                },
                inputs,
                CallGraphEdgeKind::LambdaBody,
            )
        };
        let contract = context.child_for(formal.clone());
        match argument {
            Expr::Var(name) if context.callback_parameters.contains_key(name) => {
                if let Some((source, _)) = context.callback_parameters.get(name) {
                    add_call_graph_edge(
                        graph,
                        &contract,
                        source.clone(),
                        CallGraphEdgeKind::CallbackContractEquality,
                        facts,
                    );
                }
            }
            Expr::Lambda { .. } | Expr::MethodReference { .. } => {
                collect_typed_callback_body(
                    argument,
                    formal,
                    inputs,
                    source_sam.as_ref().map(|call| call.return_type.clone()),
                    body_edge,
                    graph,
                    context,
                    facts,
                );
            }
            Expr::Ternary { .. } if source_sam.is_some() => {
                collect_typed_callback_body(
                    argument,
                    formal,
                    inputs,
                    source_sam.as_ref().map(|call| call.return_type.clone()),
                    body_edge,
                    graph,
                    context,
                    facts,
                );
            }
            _ => {
                // An existing native functional value has the checked SAM
                // ABI regardless of which lambda/source object produced it.
                // Link its invocation effect to the formal, not to argument
                // evaluation: unused and lazy callbacks must stay lazy.
                if source_sam.is_none()
                    && let Some(shape) = shape.as_ref()
                    && crate::effective_facts::EffectiveFacts::new(facts)
                        .native_functional_carrier(ty)
                        .is_some()
                {
                    add_call_graph_edge(
                        graph,
                        &contract,
                        CallableKey::external_method(
                            Some(name),
                            shape.result_method_name(),
                            inputs.len(),
                        ),
                        CallGraphEdgeKind::UnresolvedExternalMethodCall,
                        facts,
                    );
                }
            }
        }
    }
}

/// A returned/stored source SAM has the same invocation contract as a callback
/// argument. Register its typed body before generic expression traversal so
/// creation is not mistaken for execution and all adapters share one Result ABI.
fn collect_source_functional_value_contract(
    ty: &JavaType,
    expr: &Expr,
    graph: &mut CallGraph,
    context: &mut CallGraphBuildContext<'_>,
    facts: &FactSet,
) {
    if !callback_value_needs_contract(expr) {
        return;
    }
    let (ty, expr) = match expr {
        Expr::Cast { ty, expr }
            if context
                .source_symbols
                .source_functional_interface_call(ty)
                .is_some() =>
        {
            (ty, expr.as_ref())
        }
        _ => (ty, expr),
    };
    let Some(sam) = context.source_symbols.source_functional_interface_call(ty) else {
        return;
    };
    collect_typed_callback_body(
        expr,
        resolved_method_call_graph_key(&sam, context),
        sam.arg_target_types.clone(),
        Some(sam.return_type.clone()),
        CallGraphEdgeKind::CallbackContractEquality,
        graph,
        context,
        facts,
    );
}

/// Constructor formals retain source SAMs just like fields and ordinary
/// method arguments. Creation does not invoke the callback: only its SAM
/// descriptor is linked to the typed body here.
fn collect_source_constructor_callback_arguments(
    constructor: &crate::java_ir::ResolvedConstructor,
    args: &[Expr],
    graph: &mut CallGraph,
    context: &mut CallGraphBuildContext<'_>,
    facts: &FactSet,
) {
    for (argument, param) in args.iter().zip(&constructor.params) {
        collect_source_functional_value_contract(&param.ty, argument, graph, context, facts);
    }
}

fn callback_value_needs_contract(expr: &Expr) -> bool {
    match expr {
        Expr::Lambda { .. } | Expr::MethodReference { .. } => true,
        Expr::Cast { ty, expr } if !matches!(ty, JavaType::Primitive(_) | JavaType::Void) => {
            callback_value_needs_contract(expr)
        }
        Expr::Ternary {
            then_expr,
            else_expr,
            ..
        } => callback_value_needs_contract(then_expr) || callback_value_needs_contract(else_expr),
        _ => false,
    }
}

fn collect_typed_callback_body(
    expr: &Expr,
    formal: CallableKey,
    inputs: Vec<JavaType>,
    result: Option<JavaType>,
    body_edge: CallGraphEdgeKind,
    graph: &mut CallGraph,
    context: &mut CallGraphBuildContext<'_>,
    facts: &FactSet,
) {
    if !callback_value_needs_contract(expr) {
        return;
    }
    let contract = context.child_for(formal.clone());
    match expr {
        Expr::Lambda { params, body } if params.len() == inputs.len() => {
            if !context
                .consumed_callback_bodies
                .insert(SourceLambdaBodyKey::for_lambda(body))
            {
                return;
            }
            collect_typed_callback_lambda(
                params, body, formal, inputs, result, body_edge, graph, context, facts,
            );
        }
        Expr::MethodReference {
            target,
            method,
            receiver,
        } => {
            if method == "new"
                && let Some(call) = context
                    .source_symbols
                    .constructor_reference_with_input_types(
                        target,
                        &inputs,
                        &context.current_type,
                        &context.type_env,
                        facts,
                    )
            {
                add_call_graph_edge(
                    graph,
                    &contract,
                    resolved_constructor_call_graph_key(&call, context),
                    CallGraphEdgeKind::ConstructorCall,
                    facts,
                );
                context
                    .consumed_callback_bodies
                    .insert(SourceLambdaBodyKey::Expr(expr));
                return;
            }
            if let Some(call) = crate::resolve::resolve_source_method_reference_with_receiver(
                target,
                receiver.as_deref(),
                method,
                &inputs,
                &context.current_type,
                context.source_symbols,
                &context.type_env,
                facts,
            ) {
                // Only the adapter shares the SAM contract. An infallible
                // referenced method need not acquire a fallible signature.
                add_call_graph_edge(
                    graph,
                    &contract,
                    resolved_method_call_graph_key(&call, context),
                    CallGraphEdgeKind::DirectMethodCall,
                    facts,
                );
                context
                    .consumed_callback_bodies
                    .insert(SourceLambdaBodyKey::Expr(expr));
            } else if let Some(Expr::Lambda { params, body }) =
                crate::resolve::native_method_reference_lambda_with_receiver(
                    target,
                    receiver.as_deref(),
                    method,
                    &inputs,
                    context.source_symbols,
                    &context.type_env,
                    facts,
                    &facts.external_lookup_index(),
                )
            {
                context
                    .consumed_callback_bodies
                    .insert(SourceLambdaBodyKey::Expr(expr));
                // Synthetic adapters do not have immutable-source pointer
                // identities. Register the original reference only; sharing
                // a freed temporary lambda address would merge callbacks.
                collect_typed_callback_lambda(
                    &params, &body, formal, inputs, result, body_edge, graph, context, facts,
                );
            }
        }
        Expr::Ternary {
            then_expr,
            else_expr,
            ..
        } => {
            collect_typed_callback_body(
                then_expr,
                formal.clone(),
                inputs.clone(),
                result.clone(),
                body_edge,
                graph,
                context,
                facts,
            );
            collect_typed_callback_body(
                else_expr, formal, inputs, result, body_edge, graph, context, facts,
            );
        }
        _ => {}
    }
}

fn collect_typed_callback_lambda(
    params: &[crate::java_ir::LambdaParam],
    body: &LambdaBody,
    formal: CallableKey,
    inputs: Vec<JavaType>,
    result: Option<JavaType>,
    body_edge: CallGraphEdgeKind,
    graph: &mut CallGraph,
    context: &mut CallGraphBuildContext<'_>,
    facts: &FactSet,
) {
    let contract = context.child_for(formal);
    let lambda = context.next_lambda();
    add_call_graph_edge(graph, &contract, lambda.clone(), body_edge, facts);
    let mut child = context.child_for(lambda.clone());
    child.return_type = result;
    let mut typed_params = Vec::new();
    for (param, input) in params.iter().zip(inputs) {
        for name in [&param.name, &param.source_name] {
            child.type_env.insert_local(name.clone(), input.clone());
            child.callback_parameters.remove(name);
        }
        typed_params.push(Param {
            name: param.name.clone(),
            source_name: param.source_name.clone(),
            ty: input,
            is_varargs: false,
            binding: None,
        });
    }
    let body = match body {
        LambdaBody::Expr(expr) => {
            if let Some(ty) = child.return_type.clone() {
                collect_source_functional_value_contract(&ty, expr, graph, &mut child, facts);
            }
            collect_call_graph_edges_from_expr(expr, graph, &mut child, facts);
            Block {
                statements: vec![Stmt::Return(Some((**expr).clone()))],
            }
        }
        LambdaBody::Block(block) => {
            collect_call_graph_edges_from_block(block, graph, &mut child, facts);
            block.clone()
        }
    };
    graph.callback_bodies.insert(lambda, (typed_params, body));
}

fn collect_call_graph_edges_from_expr(
    expr: &Expr,
    graph: &mut CallGraph,
    context: &mut CallGraphBuildContext<'_>,
    facts: &FactSet,
) {
    if let Expr::Cast { ty, expr: inner } = expr {
        collect_source_functional_value_contract(ty, inner, graph, context, facts);
    }
    if let Expr::MethodReference {
        receiver: Some(receiver),
        ..
    } = expr
    {
        collect_call_graph_edges_from_expr(receiver, graph, context, facts);
    }
    if matches!(expr, Expr::MethodReference { .. })
        && context
            .consumed_callback_bodies
            .contains(&SourceLambdaBodyKey::Expr(expr))
    {
        return;
    }
    match expr {
        Expr::Call {
            receiver,
            name,
            args,
            resolved,
            ..
        } => {
            let (callee, edge_kind) =
                call_graph_method_target(receiver.as_deref(), name, args, resolved, context, facts);
            add_call_graph_edge(graph, context, callee, edge_kind, facts);
            if let Some(call) = resolved {
                collect_source_callback_arguments(call, args, graph, context, facts);
            }
            if let Some((_, _, comparator)) =
                crate::call_plan::native_ordering_consumer(expr, facts, &context.type_env)
            {
                if let Expr::Var(callback) = comparator
                    && let Some((key, _)) = context.callback_parameters.get(callback)
                {
                    add_call_graph_edge(
                        graph,
                        context,
                        key.clone(),
                        CallGraphEdgeKind::DirectMethodCall,
                        facts,
                    );
                } else {
                    // Lowering adapts each consumed value to the native
                    // checked Comparator contract. That contract also applies
                    // to source implementors and target-typed factory/lambda
                    // expressions, not just expressions already typed as the
                    // facade. Attribute invocation here, never at creation.
                    let owner = "Comparator";
                    let ty = JavaType::Named {
                        name: owner.to_string(),
                        args: Vec::new(),
                    };
                    if let Some(sam) = crate::effective_facts::EffectiveFacts::new(facts)
                        .native_functional_carrier(&ty)
                    {
                        add_call_graph_edge(
                            graph,
                            context,
                            CallableKey::external_method(Some(owner), sam.result_method_name(), 2),
                            CallGraphEdgeKind::UnresolvedExternalMethodCall,
                            facts,
                        );
                    }
                }
            }
            if let Some(stream) = crate::stream::build_stream_plan(expr, &context.type_env, facts) {
                stream.visit_consumed_callbacks(&mut |lambda, input| {
                    let body_key = match lambda.body() {
                        crate::stream::StreamLambdaBody::Expr(expr) => {
                            Some(SourceLambdaBodyKey::Expr(expr))
                        }
                        crate::stream::StreamLambdaBody::Block(block) => {
                            Some(SourceLambdaBodyKey::Block(block))
                        }
                        _ => None,
                    };
                    if let Some(key) = body_key {
                        if context.consumed_callback_bodies.insert(key) {
                            let callback = context.next_lambda();
                            add_call_graph_edge(
                                graph,
                                context,
                                callback.clone(),
                                CallGraphEdgeKind::LambdaBody,
                                facts,
                            );
                            let mut child = context.child_for(callback);
                            child.type_env.insert_local(
                                lambda.param_name().to_string(),
                                input.java_ty.clone(),
                            );
                            child
                                .type_env
                                .insert_local(lambda.rust_param_name(), input.java_ty.clone());
                            match lambda.body() {
                                crate::stream::StreamLambdaBody::Expr(expr) => {
                                    collect_call_graph_edges_from_expr(
                                        expr, graph, &mut child, facts,
                                    )
                                }
                                crate::stream::StreamLambdaBody::Block(block) => {
                                    collect_call_graph_edges_from_block(
                                        block, graph, &mut child, facts,
                                    )
                                }
                                _ => unreachable!(),
                            }
                        }
                        return;
                    }
                    let crate::stream::StreamLambdaBody::MethodReference {
                        target,
                        method,
                        receiver,
                    } = lambda.body()
                    else {
                        return;
                    };
                    // Preserve the established current-receiver callback
                    // handling below; add only previously absent invocation
                    // edges for bound and type-qualified source references.
                    if matches!(target, "this" | "self") {
                        return;
                    }
                    if let Some(call) =
                        crate::resolve::resolve_source_method_reference_with_receiver(
                            target,
                            receiver,
                            method,
                            std::slice::from_ref(&input.java_ty),
                            &context.current_type,
                            context.source_symbols,
                            &context.type_env,
                            facts,
                        )
                    {
                        let callback = context.next_lambda();
                        add_call_graph_edge(
                            graph,
                            context,
                            callback.clone(),
                            CallGraphEdgeKind::LambdaBody,
                            facts,
                        );
                        let child = context.child_for(callback);
                        add_call_graph_edge(
                            graph,
                            &child,
                            resolved_method_call_graph_key(&call, context),
                            CallGraphEdgeKind::DirectMethodCall,
                            facts,
                        );
                    }
                });
            }
            if let Some(receiver) = receiver {
                collect_call_graph_edges_from_expr(receiver, graph, context, facts);
            }
            for arg in args {
                collect_call_graph_edges_from_expr(arg, graph, context, facts);
            }
        }
        Expr::New {
            ty,
            qualifier,
            args,
            body,
            resolved,
            ..
        } => {
            if let Some(qualifier) = qualifier {
                collect_call_graph_edges_from_expr(qualifier, graph, context, facts);
            }
            let callee = resolved
                .as_ref()
                .map(|resolved| resolved_constructor_call_graph_key(resolved, context))
                .unwrap_or_else(|| {
                    CallableKey::external_constructor(java_type_call_graph_name(ty), args.len())
                });
            let edge_kind = if body.is_some() {
                CallGraphEdgeKind::AnonymousClassConstructor
            } else if resolved.is_some() {
                CallGraphEdgeKind::ConstructorCall
            } else {
                CallGraphEdgeKind::UnresolvedExternalConstructorCall
            };
            add_call_graph_edge(graph, context, callee, edge_kind, facts);
            if let Some(resolved) = resolved {
                collect_source_constructor_callback_arguments(
                    resolved, args, graph, context, facts,
                );
            }
            for arg in args {
                collect_call_graph_edges_from_expr(arg, graph, context, facts);
            }
            if let Some(body) = body {
                let anonymous_base = match ty {
                    JavaType::Named { name, .. } => {
                        crate::inherited_members::find_type_decl_for_owner_context(
                            context.program,
                            &context.current_type,
                            name,
                        )
                    }
                    _ => None,
                };
                // Allocate every member identity before visiting any body, so
                // forward/sibling calls bind to this anonymous object rather
                // than to a same-named method of its enclosing class.
                let members = std::sync::Arc::new(
                    body.iter()
                        .filter_map(|member| {
                            let Member::Method(method) = member else {
                                return None;
                            };
                            Some((method, context.next_anonymous_method(method.name.clone())))
                        })
                        .collect::<Vec<_>>(),
                );
                for (method, anonymous) in members.iter() {
                    graph
                        .anonymous_methods
                        .insert(anonymous.clone(), (**method).clone());
                    add_call_graph_edge(
                        graph,
                        context,
                        anonymous.clone(),
                        CallGraphEdgeKind::AnonymousClassMethod,
                        facts,
                    );
                    if let Some(base) = anonymous_base {
                        for contract in
                            anonymous_override_contract_keys(base, method, context.program)
                        {
                            let edge = CallGraphEdge {
                                caller: contract.clone(),
                                callee: anonymous.clone(),
                                kind: CallGraphEdgeKind::AnonymousOverrideContract,
                            };
                            graph.nodes.insert(contract);
                            if !graph.edges.contains(&edge) {
                                graph.edges.push(edge);
                            }
                        }
                    }
                    if let Some(method_body) = &method.body {
                        let mut child = context.child_for(anonymous.clone());
                        child.anonymous_member_methods = members.clone();
                        child
                            .type_env
                            .enter_anonymous_field_scope(body.iter().filter_map(|member| {
                                match member {
                                    Member::Field(field) => {
                                        Some((field.name.clone(), field.ty.clone()))
                                    }
                                    _ => None,
                                }
                            }));
                        for param in &method.params {
                            child
                                .type_env
                                .insert_local(param.name.clone(), param.ty.clone());
                            child
                                .type_env
                                .insert_local(param.source_name.clone(), param.ty.clone());
                        }
                        child.anonymous_super_type = anonymous_base;
                        child.return_type = Some(method.return_type.clone());
                        child.anonymous_method_param_types = method
                            .params
                            .iter()
                            .map(|param| (param.name.clone(), param.ty.clone()))
                            .collect();
                        collect_call_graph_edges_from_block(method_body, graph, &mut child, facts);
                    }
                }
            }
        }
        Expr::Lambda { body, .. } => {
            if context
                .consumed_callback_bodies
                .contains(&SourceLambdaBodyKey::for_lambda(body))
            {
                return;
            }
            let lambda = context.next_lambda();
            add_call_graph_edge(
                graph,
                context,
                lambda.clone(),
                CallGraphEdgeKind::LambdaBody,
                facts,
            );
            let mut child = context.child_for(lambda);
            match body {
                LambdaBody::Expr(expr) => {
                    collect_call_graph_edges_from_expr(expr, graph, &mut child, facts)
                }
                LambdaBody::Block(block) => {
                    collect_call_graph_edges_from_block(block, graph, &mut child, facts)
                }
            }
        }
        Expr::MethodReference { target, method, .. }
            if matches!(target.as_str(), "this" | "self") =>
        {
            // A method reference is an implicit callback body. Model it like
            // an explicit lambda so fallibility reaches the method which
            // creates or passes the callback. The functional target type is
            // not retained on this IR node, so keep every matching overload;
            // typed overload selection happens later during lowering.
            let callback = context.next_lambda();
            add_call_graph_edge(
                graph,
                context,
                callback.clone(),
                CallGraphEdgeKind::LambdaBody,
                facts,
            );
            let child = context.child_for(callback);
            for callee in current_receiver_method_reference_targets(method, context) {
                add_call_graph_edge(
                    graph,
                    &child,
                    callee,
                    CallGraphEdgeKind::DirectMethodCall,
                    facts,
                );
            }
        }
        Expr::FieldAccess(receiver, _) | Expr::Unary { expr: receiver, .. } => {
            collect_call_graph_edges_from_expr(receiver, graph, context, facts);
        }
        Expr::Binary { left, right, .. } => {
            collect_call_graph_edges_from_expr(left, graph, context, facts);
            collect_call_graph_edges_from_expr(right, graph, context, facts);
        }
        Expr::Cast { expr, .. } | Expr::InstanceOf { expr, .. } => {
            collect_call_graph_edges_from_expr(expr, graph, context, facts);
        }
        Expr::ArrayAccess(array, index) => {
            collect_call_graph_edges_from_expr(array, graph, context, facts);
            collect_call_graph_edges_from_expr(index, graph, context, facts);
        }
        Expr::ArrayCreation {
            dimensions,
            initializer,
            ..
        } => {
            for expr in dimensions {
                collect_call_graph_edges_from_expr(expr, graph, context, facts);
            }
            if let Some(initializer) = initializer {
                for expr in initializer {
                    collect_call_graph_edges_from_expr(expr, graph, context, facts);
                }
            }
        }
        Expr::ArrayInitializer(dimensions) => {
            for expr in dimensions {
                collect_call_graph_edges_from_expr(expr, graph, context, facts);
            }
        }
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => {
            collect_call_graph_edges_from_expr(condition, graph, context, facts);
            collect_call_graph_edges_from_expr(then_expr, graph, context, facts);
            collect_call_graph_edges_from_expr(else_expr, graph, context, facts);
        }
        Expr::Switch { selector, cases } => {
            collect_call_graph_edges_from_expr(selector, graph, context, facts);
            for case in cases {
                for label in &case.labels {
                    if let SwitchLabel::Expr(expr) = label {
                        collect_call_graph_edges_from_expr(expr, graph, context, facts);
                    }
                }
                collect_call_graph_edges_from_expr(&case.value, graph, context, facts);
            }
        }
        Expr::Literal(_)
        | Expr::Var(_)
        | Expr::This
        | Expr::Super
        | Expr::MethodReference { .. }
        | Expr::ClassLiteral(_)
        | Expr::Raw(_)
        | Expr::Unsupported { .. } => {}
    }
}

fn record_indexed_null_comparison_use(
    facts: &mut MethodUsageFacts,
    maybe_indexed: &Expr,
    maybe_null: &Expr,
) {
    if !matches!(maybe_null, Expr::Literal(Literal::Null)) {
        return;
    }
    record_indexed_expr_compared_to_null(facts, maybe_indexed);
}

fn record_indexed_expr_compared_to_null(facts: &mut MethodUsageFacts, maybe_indexed: &Expr) {
    match maybe_indexed {
        Expr::Ternary {
            then_expr,
            else_expr,
            ..
        } => {
            record_indexed_expr_compared_to_null(facts, then_expr);
            record_indexed_expr_compared_to_null(facts, else_expr);
            return;
        }
        Expr::Switch { cases, .. } => {
            for case in cases {
                record_indexed_expr_compared_to_null(facts, &case.value);
            }
            return;
        }
        _ => {}
    }
    let mut current = maybe_indexed;
    let mut index_depth = 0;
    while let Expr::ArrayAccess(array, _) = current {
        index_depth += 1;
        current = array;
    }
    if index_depth > 0
        && let Expr::Var(name) = current
    {
        record_use(
            facts,
            name,
            BindingUse::IndexedElementComparedToNull { index_depth },
        );
    }
}

fn current_receiver_method_reference_targets(
    method_name: &str,
    context: &CallGraphBuildContext<'_>,
) -> Vec<CallableKey> {
    let Some(owner) = crate::inherited_members::find_type_decl_for_owner_context(
        context.program,
        &context.current_type,
        &context.current_type,
    ) else {
        return Vec::new();
    };
    let mut targets = owner
        .members
        .iter()
        .filter_map(|member| match member {
            Member::Method(method)
                if method.name == method_name && !method.modifiers.contains(&Modifier::Static) =>
            {
                Some(CallableKey::source_method_for_type(owner, method))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    targets.extend(
        crate::inherited_members::inherited_instance_method_refs_for_type(owner, context.program)
            .into_iter()
            .filter(|inherited| inherited.method().name == method_name)
            .map(|inherited| {
                CallableKey::source_method_for_type(inherited.declaring_ty(), inherited.method())
            }),
    );
    targets.sort_by_key(CallableKey::diagnostic_label);
    targets.dedup();
    targets
}

fn call_graph_method_target(
    receiver: Option<&Expr>,
    name: &str,
    args: &[Expr],
    resolved: &Option<ResolvedCall>,
    context: &CallGraphBuildContext<'_>,
    facts: &FactSet,
) -> (CallableKey, CallGraphEdgeKind) {
    if name == "accept"
        && args.len() == 1
        && let Some(receiver) = receiver
        && let Some(protocol) = crate::stdlib::native_primitive_consumer_receiver_bound(
            &crate::types::infer_expr_type(receiver, &context.type_env),
            &context.type_env,
            facts,
        )
    {
        return (
            CallableKey::external_method(Some(&protocol.owner), "accept_result", args.len()),
            CallGraphEdgeKind::DirectMethodCall,
        );
    }
    if receiver.is_none_or(|receiver| matches!(receiver, Expr::This))
        && !context.anonymous_member_methods.is_empty()
    {
        let methods = context
            .anonymous_member_methods
            .iter()
            .map(|(method, _)| *method)
            .collect::<Vec<_>>();
        let arg_types = args
            .iter()
            .map(|arg| crate::types::infer_expr_type(arg, &context.type_env))
            .collect::<Vec<_>>();
        if let Some(selected) = crate::resolve::select_scoped_instance_method(
            &methods,
            name,
            &arg_types,
            &context.current_type,
            context.source_symbols,
            &context.type_env,
            facts,
        ) && let Some((_, key)) = context
            .anonymous_member_methods
            .iter()
            .find(|(method, _)| std::ptr::eq(*method, selected))
        {
            return (key.clone(), CallGraphEdgeKind::DirectMethodCall);
        }
    }
    if let Some(Expr::Var(receiver)) = receiver
        && let Some((key, JavaType::Named { name: owner, .. })) =
            context.callback_parameters.get(receiver)
        && let Some(shape) = crate::stdlib::functional_interface_shape(
            &crate::stdlib::canonical_external_receiver_type_key(owner),
        )
        && shape.method() == name
        && shape.inputs().len() == args.len()
    {
        return (key.clone(), CallGraphEdgeKind::DirectMethodCall);
    }
    if matches!(receiver, Some(Expr::Super)) {
        if let Some(base) = context.anonymous_super_type
            && let Some(target) =
                anonymous_super_method_key(base, name, args, resolved.as_ref(), context)
        {
            return (target, CallGraphEdgeKind::DirectMethodCall);
        }
        if let Some(resolved) = resolved {
            return (
                resolved_method_call_graph_key(resolved, context),
                CallGraphEdgeKind::DirectMethodCall,
            );
        }
        return (
            CallableKey::external_method(Some("super"), name, args.len()),
            CallGraphEdgeKind::UnresolvedExternalMethodCall,
        );
    }
    if let Some(resolved) = resolved {
        if let Some(sam) = crate::stdlib::functional_interface_shape(
            &crate::stdlib::canonical_external_receiver_type_key(&resolved.declaring_type),
        ) && sam.method() == name
            && sam.inputs().len() == args.len()
            && !crate::call_plan::java_type_owner_is_parsed_source(
                &resolved.declaring_type,
                &context.type_env,
                facts,
            )
        {
            return (
                CallableKey::external_method(
                    Some(&resolved.declaring_type),
                    sam.result_method_name(),
                    args.len(),
                ),
                CallGraphEdgeKind::DirectMethodCall,
            );
        }
        return (
            resolved_method_call_graph_key(resolved, context),
            CallGraphEdgeKind::DirectMethodCall,
        );
    }
    let arg_types = args
        .iter()
        .map(|arg| crate::types::infer_expr_type(arg, &context.type_env))
        .collect::<Vec<_>>();
    if let Some(resolved) = crate::resolve::resolve_source_call_with_types(
        receiver,
        name,
        &arg_types,
        &context.current_type,
        context.source_symbols,
        &context.type_env,
        facts,
    ) {
        return (
            resolved_method_call_graph_key(&resolved, context),
            CallGraphEdgeKind::DirectMethodCall,
        );
    }
    if context.internal_methods.contains(name)
        && receiver.is_none_or(|receiver| matches!(receiver, Expr::This))
    {
        return (
            CallableKey::method_identity(context.current_type.clone(), name),
            CallGraphEdgeKind::DirectMethodCall,
        );
    }
    if let Some(receiver) = receiver {
        let ty = crate::resolve::infer_source_expr_type_with_facts(
            receiver,
            &context.current_type,
            context.source_symbols,
            &context.type_env,
            facts,
        );
        let ty = crate::types::optional_element_type(&ty).unwrap_or(ty);
        if let JavaType::Named { name: owner, .. } = &ty
            && let Some(sam) = crate::stdlib::functional_interface_shape(
                &crate::stdlib::canonical_external_receiver_type_key(&owner),
            )
            && sam.method() == name
            && sam.inputs().len() == args.len()
            && !crate::call_plan::java_type_owner_is_parsed_source(&owner, &context.type_env, facts)
        {
            return (
                CallableKey::external_method(Some(owner), sam.result_method_name(), args.len()),
                CallGraphEdgeKind::UnresolvedExternalMethodCall,
            );
        }
        // A constructed/chained receiver need not have a source declaration,
        // but its modeled nominal type is still the callee's identity. Dropping
        // it here disconnected the external throw/result contract from the
        // same mapped call that lowering emits.
        if let Some(owner) = java_type_name_for_usage(&ty)
            && facts
                .external_api_effect_for_receiver_with_arity_or_rust_name(
                    Some(owner),
                    name,
                    Some(args.len()),
                )
                .is_some()
        {
            return (
                CallableKey::external_method(Some(owner), name, args.len()),
                CallGraphEdgeKind::UnresolvedExternalMethodCall,
            );
        }
    }
    (
        CallableKey::external_method(
            receiver.and_then(expr_call_graph_type_name),
            name,
            args.len(),
        ),
        CallGraphEdgeKind::UnresolvedExternalMethodCall,
    )
}

/// Translate a resolver identity back to the declaration identity used by
/// call-graph nodes. A packaged compilation unit stores its package on
/// `Program`, not on each `TypeDecl`, while a resolved call necessarily uses
/// the qualified Java owner. Keeping those two spellings as separate keys
/// disconnects cross-file fallibility propagation.
fn resolved_method_call_graph_key(
    resolved: &ResolvedCall,
    context: &CallGraphBuildContext<'_>,
) -> CallableKey {
    let Some((owner, method)) = crate::java_signature::resolved_source_method_decl(
        context.program,
        &context.current_type,
        resolved,
    ) else {
        return CallableKey::resolved_method(resolved);
    };
    CallableKey::source_method_for_type(owner, method)
}

fn resolved_constructor_call_graph_key(
    resolved: &ResolvedConstructor,
    context: &CallGraphBuildContext<'_>,
) -> CallableKey {
    let Some(owner) = crate::inherited_members::find_type_decl_for_owner_context(
        context.program,
        &context.current_type,
        &resolved.declaring_type,
    ) else {
        return CallableKey::resolved_constructor(resolved);
    };
    let mut candidates = owner.members.iter().filter_map(|member| match member {
        Member::Constructor(constructor)
            if crate::java_signature::source_constructor_rust_name(owner, constructor)
                == resolved.rust_name =>
        {
            Some(constructor)
        }
        _ => None,
    });
    let Some(constructor) = candidates.next() else {
        return CallableKey::resolved_constructor(resolved);
    };
    if candidates.next().is_some() {
        return CallableKey::resolved_constructor(resolved);
    }
    CallableKey::source_constructor_for_type(owner, constructor)
}

fn anonymous_override_contract_keys(
    base: &TypeDecl,
    override_method: &crate::java_ir::MethodDecl,
    program: &Program,
) -> Vec<CallableKey> {
    let wanted_name = &override_method.name;
    let wanted_params = crate::java_signature::erased_param_types(&override_method.params);
    let mut contracts = Vec::new();
    let mut pending = vec![base];
    let mut visited = HashSet::new();
    while let Some(owner) = pending.pop() {
        if !visited.insert(owner.name.clone()) {
            continue;
        }
        for member in &owner.members {
            let Member::Method(method) = member else {
                continue;
            };
            if method.modifiers.contains(&Modifier::Static)
                || method.modifiers.contains(&Modifier::Private)
                || &method.name != wanted_name
            {
                continue;
            }
            let effective_params = if owner.kind == crate::java_ir::TypeKind::Interface {
                crate::representation_lowering::interface_method_types_in_target(
                    program, base, owner, method,
                )
                .1
            } else {
                method.params.iter().map(|param| param.ty.clone()).collect()
            };
            let erased_params = method
                .params
                .iter()
                .zip(&effective_params)
                .map(|(param, ty)| crate::java_signature::erased_type_name(ty, param.is_varargs))
                .collect::<Vec<_>>();
            if erased_params != wanted_params {
                continue;
            }
            let key = CallableKey::source_method_for_type(owner, method);
            if !contracts.contains(&key) {
                contracts.push(key);
            }
        }
        for parent in owner.extends.iter().chain(owner.implements.iter()) {
            if let Some(parent) = crate::inherited_members::find_type_decl_for_owner_context(
                program,
                &owner.name,
                parent,
            ) {
                pending.push(parent);
            }
        }
    }
    contracts
}

fn anonymous_super_method_key(
    base: &TypeDecl,
    name: &str,
    args: &[Expr],
    resolved: Option<&ResolvedCall>,
    context: &CallGraphBuildContext<'_>,
) -> Option<CallableKey> {
    let mut candidates = base
        .members
        .iter()
        .filter_map(|member| match member {
            Member::Method(method)
                if !method.modifiers.contains(&Modifier::Static)
                    && method.name == name
                    && method.params.len() == args.len() =>
            {
                Some((base, method))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    candidates.extend(
        crate::inherited_members::inherited_instance_method_refs_for_type(base, context.program)
            .into_iter()
            .filter(|inherited| {
                inherited.method().name == name && inherited.method().params.len() == args.len()
            })
            .map(|inherited| (inherited.declaring_ty(), inherited.method())),
    );
    if let Some(resolved) = resolved {
        let selected = candidates
            .iter()
            .copied()
            .filter(|(_, method)| source_method_rust_name(method) == resolved.rust_name)
            .collect::<Vec<_>>();
        if let [(owner, method)] = selected.as_slice() {
            return Some(CallableKey::source_method_for_type(owner, method));
        }
    }
    let arg_types = args
        .iter()
        .map(|arg| anonymous_call_graph_arg_type(arg, context))
        .collect::<Option<Vec<_>>>();
    if let Some(arg_types) = arg_types {
        let selected = candidates
            .iter()
            .copied()
            .filter(|(_, method)| {
                method
                    .params
                    .iter()
                    .zip(&arg_types)
                    .all(|(param, actual)| param.ty == *actual)
            })
            .collect::<Vec<_>>();
        if let [(owner, method)] = selected.as_slice() {
            return Some(CallableKey::source_method_for_type(owner, method));
        }
    }
    match candidates.as_slice() {
        [(owner, method)] => Some(CallableKey::source_method_for_type(owner, method)),
        _ => None,
    }
}

fn anonymous_call_graph_arg_type(
    expr: &Expr,
    context: &CallGraphBuildContext<'_>,
) -> Option<JavaType> {
    match expr {
        Expr::Var(name) => context.anonymous_method_param_types.get(name).cloned(),
        Expr::Literal(literal) => Some(crate::types::literal_type(literal)),
        Expr::Cast { ty, .. } => Some(ty.clone()),
        _ => None,
    }
}

fn add_call_graph_edge(
    graph: &mut CallGraph,
    context: &CallGraphBuildContext<'_>,
    callee: CallableKey,
    kind: CallGraphEdgeKind,
    facts: &FactSet,
) {
    let caller = &context.current_callable;
    let kind = if context.in_handled_exception_boundary()
        && matches!(
            kind,
            CallGraphEdgeKind::DirectMethodCall
                | CallGraphEdgeKind::ConstructorCall
                | CallGraphEdgeKind::UnresolvedExternalMethodCall
                | CallGraphEdgeKind::UnresolvedExternalConstructorCall
        )
        && callable_checked_exceptions_are_caught(&callee, &context.handled_catch_types, facts)
    {
        CallGraphEdgeKind::HandledExceptionBoundary
    } else {
        kind
    };
    graph.nodes.insert(caller.clone());
    graph.nodes.insert(callee.clone());
    let edge = CallGraphEdge {
        caller: caller.clone(),
        callee,
        kind,
    };
    if !graph.edges.contains(&edge) {
        graph.edges.push(edge);
    }
}

fn catch_clauses_do_not_rethrow_checked_exceptions(
    catches: &[crate::java_ir::CatchClause],
    facts: &FactSet,
) -> bool {
    !catches.is_empty()
        && catches
            .iter()
            .all(|catch| block_does_not_rethrow_checked_exception(&catch.body, facts))
}

fn callable_checked_exceptions_are_caught(
    callee: &CallableKey,
    catch_types: &[JavaType],
    facts: &FactSet,
) -> bool {
    let checked = callable_checked_exception_types(callee, facts);
    !checked.is_empty()
        && checked
            .iter()
            .all(|thrown| exception_caught_by_any(thrown, catch_types, facts))
}

fn callable_checked_exception_types(callee: &CallableKey, facts: &FactSet) -> Vec<JavaType> {
    let thrown = match callee {
        CallableKey::Method {
            type_name,
            method_name,
        }
        | CallableKey::Constructor {
            type_name,
            rust_name: method_name,
        } => facts
            .method_effect_for(type_name.as_str(), method_name)
            .map(|effect| effect.thrown.clone())
            .unwrap_or_default(),
        CallableKey::ExternalMethod {
            receiver_type,
            method_name,
            arg_count,
        } => facts
            .external_api_effect_for_receiver_with_arity_or_rust_name(
                receiver_type.as_deref(),
                method_name,
                Some(*arg_count),
            )
            .map(|fact| fact.thrown.clone())
            .unwrap_or_default(),
        CallableKey::ExternalConstructor { .. }
        | CallableKey::FieldInitializer { .. }
        | CallableKey::Lambda { .. }
        | CallableKey::CallbackParameter { .. }
        | CallableKey::AnonymousMethod { .. } => Vec::new(),
    };
    thrown
        .into_iter()
        .filter(|ty| {
            java_type_name_for_usage(ty).is_some_and(|name| {
                facts.exception_kind_for(name) != Some(ExceptionKind::Unchecked)
            })
        })
        .collect()
}

fn exception_caught_by_any(thrown: &JavaType, catch_types: &[JavaType], facts: &FactSet) -> bool {
    catch_types
        .iter()
        .any(|catch_ty| exception_assignable_to_catch(thrown, catch_ty, facts))
}

fn exception_assignable_to_catch(thrown: &JavaType, catch_ty: &JavaType, facts: &FactSet) -> bool {
    let (Some(thrown_name), Some(catch_name)) = (
        java_type_name_for_usage(thrown),
        java_type_name_for_usage(catch_ty),
    ) else {
        return false;
    };
    if crate::facts::external_type_name_matches(thrown_name, catch_name)
        || matches!(
            catch_name,
            "Exception" | "java.lang.Exception" | "Throwable" | "java.lang.Throwable"
        )
    {
        return true;
    }
    facts
        .hierarchy_lookup_index()
        .hierarchy_subtype_policy(thrown, catch_ty)
        .reaches()
}

fn block_does_not_rethrow_checked_exception(block: &Block, facts: &FactSet) -> bool {
    !block
        .statements
        .iter()
        .any(|stmt| stmt_rethrows_checked_exception(stmt, facts))
}

fn stmt_rethrows_checked_exception(stmt: &Stmt, facts: &FactSet) -> bool {
    match stmt {
        Stmt::Throw(expr) => thrown_expr_is_checked_exception(expr, facts),
        Stmt::Block(block) | Stmt::LabeledBlock { body: block, .. } => {
            !block_does_not_rethrow_checked_exception(block, facts)
        }
        Stmt::If {
            then_block,
            else_block,
            ..
        } => {
            !block_does_not_rethrow_checked_exception(then_block, facts)
                || else_block
                    .as_ref()
                    .is_some_and(|block| !block_does_not_rethrow_checked_exception(block, facts))
        }
        Stmt::Try {
            body,
            catches,
            finally_block,
            ..
        } => {
            !block_does_not_rethrow_checked_exception(body, facts)
                || catches
                    .iter()
                    .any(|catch| !block_does_not_rethrow_checked_exception(&catch.body, facts))
                || finally_block
                    .as_ref()
                    .is_some_and(|block| !block_does_not_rethrow_checked_exception(block, facts))
        }
        _ => false,
    }
}

fn thrown_expr_is_checked_exception(expr: &Expr, facts: &FactSet) -> bool {
    let Some(type_name) = thrown_expr_type_name(expr) else {
        return true;
    };
    facts.exception_kind_for(&type_name) != Some(ExceptionKind::Unchecked)
}

fn thrown_expr_type_name(expr: &Expr) -> Option<String> {
    match expr {
        Expr::New { ty, .. } | Expr::Cast { ty, .. } => Some(java_type_call_graph_name(ty)),
        Expr::Var(name) => Some(name.clone()),
        _ => None,
    }
}

fn java_type_name_for_usage(java_ty: &JavaType) -> Option<&str> {
    match java_ty {
        JavaType::Named { name, .. } | JavaType::Unknown(name) => Some(name),
        JavaType::Intersection(bounds) => bounds.iter().find_map(java_type_name_for_usage),
        JavaType::String => Some("String"),
        JavaType::Primitive(_) | JavaType::Void | JavaType::Array(_) | JavaType::Wildcard(_) => {
            None
        }
    }
}

fn expr_call_graph_type_name(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Cast { ty, .. } => Some(java_type_call_graph_name(ty)),
        Expr::ClassLiteral(ty) => Some(java_type_call_graph_name(ty)),
        Expr::Super => Some("super".to_string()),
        Expr::This => None,
        _ => None,
    }
}

fn java_type_call_graph_name(ty: &JavaType) -> String {
    match ty {
        JavaType::Void => "void".to_string(),
        JavaType::Primitive(primitive) => format!("{primitive:?}"),
        JavaType::String => "String".to_string(),
        JavaType::Array(element) => format!("{}[]", java_type_call_graph_name(element)),
        JavaType::Named { name, .. } | JavaType::Unknown(name) => name.clone(),
        JavaType::Intersection(bounds) => bounds
            .iter()
            .map(java_type_call_graph_name)
            .collect::<Vec<_>>()
            .join(" & "),
        JavaType::Wildcard(_) => "?".to_string(),
    }
}

fn field_names(ty: &TypeDecl, program: &Program) -> HashSet<String> {
    let mut names = ty
        .members
        .iter()
        .filter_map(|member| match member {
            Member::Field(field) => Some(field.name.clone()),
            _ => None,
        })
        .collect::<HashSet<_>>();
    names.extend(
        crate::inherited_members::inherited_instance_field_refs_for_type(ty, program)
            .into_iter()
            .map(|inherited| inherited.field().name.clone()),
    );
    names
}

fn field_types(ty: &TypeDecl, program: &Program) -> HashMap<String, JavaType> {
    let mut types = ty
        .members
        .iter()
        .filter_map(|member| match member {
            Member::Field(field) => Some((field.name.clone(), field.ty.clone())),
            _ => None,
        })
        .collect::<HashMap<_, _>>();
    for inherited in crate::inherited_members::inherited_instance_field_refs_for_type(ty, program) {
        // A field declared on the subclass hides a superclass field of the
        // same Java name. Preserve that lookup rule while making all other
        // inherited storage visible to mutation analysis.
        types
            .entry(inherited.field().name.clone())
            .or_insert_with(|| inherited.field().ty.clone());
    }
    types
}

fn internal_method_names(ty: &TypeDecl) -> HashSet<String> {
    ty.members
        .iter()
        .filter_map(|member| match member {
            Member::Method(method)
                if !method_api_boundary_policy(&method.modifiers).public_api() =>
            {
                Some(method.name.clone())
            }
            _ => None,
        })
        .collect()
}

pub fn method_api_boundary_policy(modifiers: &[Modifier]) -> MethodApiBoundaryPolicy {
    MethodApiBoundaryPolicy {
        public_api: modifiers
            .iter()
            .any(|modifier| matches!(modifier, Modifier::Public | Modifier::Protected)),
    }
}

fn analyze_stmt(
    stmt: &Stmt,
    facts: &mut MethodUsageFacts,
    current_type: &str,
    current_super_type: Option<&str>,
    declared_return_type: Option<&JavaType>,
    internal_methods: &HashSet<String>,
    field_names: &HashSet<String>,
    field_types: &HashMap<String, JavaType>,
    semantic_facts: &UsageAnalysisContext<'_>,
    type_env: &TypeEnv,
    source_return_nullability: &HashMap<(String, usize), NullabilityState>,
) {
    match stmt {
        Stmt::VarDecl {
            ty,
            name,
            source_name,
            initializer,
            binding,
            ..
        } => {
            let initialized_from_field = initializer.as_ref().and_then(|initializer| {
                self_field_name(initializer, facts, field_names).map(str::to_string)
            });
            insert_binding_use_facts(facts, source_name.clone(), binding.clone(), ty.clone());
            if let Some(field_name) = initialized_from_field {
                facts.field_uses.push(FieldUse::AssignedToLocal {
                    field_name,
                    local_name: name.clone(),
                    local_binding: binding.clone(),
                });
            }
            if let Some(initializer) = initializer {
                if let Some(binding) = binding
                    && has_projectable_native_value_domain(initializer)
                {
                    record_native_value_domain_storage(
                        initializer,
                        NativeCollectionStorageSlot {
                            root: NativeCollectionStorageRoot::Binding(binding.clone()),
                            path: vec![],
                            ty: ty.clone(),
                            root_ty: ty.clone(),
                        },
                        facts,
                        current_type,
                        type_env,
                        semantic_facts,
                    );
                }
                if crate::effective_facts::EffectiveFacts::new(semantic_facts.facts)
                    .native_iterable_element_type(ty)
                    .is_some()
                {
                    let storage_types = native_storage_type_env(
                        &[initializer],
                        facts,
                        current_type,
                        type_env,
                        semantic_facts,
                    );
                    let source_ty = native_storage_expr_type(
                        initializer,
                        facts,
                        current_type,
                        &storage_types,
                        semantic_facts,
                    );
                    if native_storage_value_type(&source_ty, &storage_types, semantic_facts)
                        && let Some(mut source) = native_collection_storage_slot(
                            initializer,
                            facts,
                            current_type,
                            &storage_types,
                            semantic_facts,
                        )
                    {
                        // A retained view needs the source allocation even
                        // when its origin is a field or a method result rather
                        // than a lexical binding. Reuse canonical origin IDs.
                        source.ty = source_ty;
                        facts.native_collection_retention_requirements.push(source);
                    }
                }
                if native_storage_value_type(ty, type_env, semantic_facts)
                    || crate::effective_facts::EffectiveFacts::new(semantic_facts.facts)
                        .native_iterable_element_type(ty)
                        .is_some()
                {
                    let storage_types = native_storage_type_env(
                        &[initializer],
                        facts,
                        current_type,
                        type_env,
                        semantic_facts,
                    );
                    if let Some(binding) = binding
                        && let Some(source) = native_collection_storage_slot(
                            initializer,
                            facts,
                            current_type,
                            &storage_types,
                            semantic_facts,
                        )
                        && !source.path.is_empty()
                    {
                        facts.native_collection_storage_flows.push((
                            source,
                            NativeCollectionStorageSlot {
                                root: NativeCollectionStorageRoot::Binding(binding.clone()),
                                path: vec![],
                                ty: ty.clone(),
                                root_ty: ty.clone(),
                            },
                        ));
                    }
                }
                if let Some(actual_type) = assigned_subtype_local_actual_type(
                    initializer,
                    ty,
                    type_env,
                    semantic_facts,
                    facts,
                ) {
                    facts.record_declared_binding_use(
                        source_name,
                        binding.as_ref(),
                        BindingUse::AssignedSubtypeToLocal {
                            local_name: name.clone(),
                            local_binding: binding.clone(),
                            actual_type,
                        },
                    );
                }
                if let Expr::Var(source_name) = initializer {
                    record_use(
                        facts,
                        source_name,
                        BindingUse::AssignedToLocal {
                            local_name: name.clone(),
                            local_binding: binding.clone(),
                        },
                    );
                }
                for policy in call_result_boundary_policies_with_context(
                    initializer,
                    internal_methods,
                    facts,
                    current_type,
                    current_super_type,
                    field_names,
                    field_types,
                    semantic_facts,
                    type_env,
                ) {
                    if let Some(binding) = binding.as_ref() {
                        facts.assigned_call_results.push(AssignedCallResultUse {
                            binding: binding.clone(),
                            call: policy.clone(),
                        });
                    }
                    facts.record_declared_binding_use(
                        source_name,
                        binding.as_ref(),
                        BindingUse::AssignedFromCall {
                            name: policy.name,
                            receiver_type: policy.receiver_type,
                            boundary: policy.boundary,
                        },
                    );
                }
                analyze_expr(
                    initializer,
                    facts,
                    current_type,
                    current_super_type,
                    internal_methods,
                    field_names,
                    field_types,
                    semantic_facts,
                    type_env,
                    source_return_nullability,
                );
            }
        }
        Stmt::Assign { target, value } => {
            record_null_source_field_assignment(
                target,
                value,
                facts,
                current_type,
                type_env,
                semantic_facts,
            );
            record_native_assignment_value_domains(
                target,
                value,
                facts,
                current_type,
                type_env,
                semantic_facts,
            );
            if native_storage_value_type(
                &native_storage_expr_type(value, facts, current_type, type_env, semantic_facts),
                type_env,
                semantic_facts,
            ) {
                let storage_types = native_storage_type_env(
                    &[target, value],
                    facts,
                    current_type,
                    type_env,
                    semantic_facts,
                );
                if native_storage_value_type(
                    &crate::types::infer_expr_type(value, &storage_types),
                    &storage_types,
                    semantic_facts,
                ) && let Some(source) = native_collection_storage_slot(
                    value,
                    facts,
                    current_type,
                    &storage_types,
                    semantic_facts,
                ) && let Some(target_slot) = native_collection_storage_slot(
                    target,
                    facts,
                    current_type,
                    &storage_types,
                    semantic_facts,
                ) && (!source.path.is_empty() || !target_slot.path.is_empty())
                {
                    facts
                        .native_collection_storage_flows
                        .push((source, target_slot));
                }
            }
            if let Expr::Var(name) = target {
                if is_unshadowed_field_name(facts, field_names, name) {
                    if !facts.field_is_static(name) {
                        facts.mutates_self = true;
                    }
                    for policy in call_result_boundary_policies_with_context(
                        value,
                        internal_methods,
                        facts,
                        current_type,
                        current_super_type,
                        field_names,
                        field_types,
                        semantic_facts,
                        type_env,
                    ) {
                        facts.field_uses.push(FieldUse::AssignedFromCall {
                            field_name: name.clone(),
                            name: policy.name,
                            receiver_type: policy.receiver_type,
                            boundary: policy.boundary,
                        });
                    }
                    if let Some(field_ty) = declared_field_type(field_types, name)
                        && let Some(actual_type) = assigned_subtype_local_actual_type(
                            value,
                            field_ty,
                            type_env,
                            semantic_facts,
                            facts,
                        )
                    {
                        facts.field_uses.push(FieldUse::AssignedSubtypeToField {
                            field_name: name.clone(),
                            field_type: field_ty.clone(),
                            actual_type,
                        });
                    }
                    analyze_stored_expr(value, facts, name);
                } else {
                    record_use(facts, name, BindingUse::Mutated);
                    let local_binding = facts.visible_binding_id_for_name(name);
                    if let Some(field_name) = self_field_name(value, facts, field_names) {
                        facts.field_uses.push(FieldUse::AssignedToLocal {
                            field_name: field_name.to_string(),
                            local_name: name.clone(),
                            local_binding: local_binding.clone(),
                        });
                    }
                    if let Some(target_binding) = facts.current_visible_binding_facts_for_name(name)
                        && let Some(actual_type) = assigned_subtype_local_actual_type(
                            value,
                            target_binding.java_ty(),
                            type_env,
                            semantic_facts,
                            facts,
                        )
                    {
                        record_use(
                            facts,
                            name,
                            BindingUse::AssignedSubtypeToLocal {
                                local_name: name.clone(),
                                local_binding: local_binding.clone(),
                                actual_type,
                            },
                        );
                    }
                    if let Expr::Var(source_name) = value {
                        record_use(
                            facts,
                            source_name,
                            BindingUse::AssignedToLocal {
                                local_name: name.clone(),
                                local_binding,
                            },
                        );
                    }
                    for policy in call_result_boundary_policies_with_context(
                        value,
                        internal_methods,
                        facts,
                        current_type,
                        current_super_type,
                        field_names,
                        field_types,
                        semantic_facts,
                        type_env,
                    ) {
                        if let Some(binding) = facts.visible_binding_id_for_name(name) {
                            facts.assigned_call_results.push(AssignedCallResultUse {
                                binding,
                                call: policy.clone(),
                            });
                        }
                        record_use(
                            facts,
                            name,
                            BindingUse::AssignedFromCall {
                                name: policy.name,
                                receiver_type: policy.receiver_type,
                                boundary: policy.boundary,
                            },
                        );
                    }
                }
            }
            if let Expr::FieldAccess(_, field_name) = target {
                if target_is_this_field(target) && !facts.field_is_static(field_name) {
                    facts.mutates_self = true;
                }
                for policy in call_result_boundary_policies_with_context(
                    value,
                    internal_methods,
                    facts,
                    current_type,
                    current_super_type,
                    field_names,
                    field_types,
                    semantic_facts,
                    type_env,
                ) {
                    facts.field_uses.push(FieldUse::AssignedFromCall {
                        field_name: field_name.clone(),
                        name: policy.name,
                        receiver_type: policy.receiver_type,
                        boundary: policy.boundary,
                    });
                }
                if let Some(field_ty) = declared_field_type(field_types, field_name)
                    && let Some(actual_type) = assigned_subtype_local_actual_type(
                        value,
                        field_ty,
                        type_env,
                        semantic_facts,
                        facts,
                    )
                {
                    facts.field_uses.push(FieldUse::AssignedSubtypeToField {
                        field_name: field_name.clone(),
                        field_type: field_ty.clone(),
                        actual_type,
                    });
                }
                analyze_stored_expr(value, facts, field_name);
            }
            if array_assignment_targets_self_field(target, facts, field_names) {
                facts.mutates_self = true;
            }
            analyze_expr(
                target,
                facts,
                current_type,
                current_super_type,
                internal_methods,
                field_names,
                field_types,
                semantic_facts,
                type_env,
                source_return_nullability,
            );
            analyze_expr(
                value,
                facts,
                current_type,
                current_super_type,
                internal_methods,
                field_names,
                field_types,
                semantic_facts,
                type_env,
                source_return_nullability,
            );
        }
        Stmt::Return(Some(expr)) => {
            if let Some(destination) = facts.active_callback_result.clone() {
                record_native_value_domain_storage(
                    expr,
                    destination,
                    facts,
                    current_type,
                    type_env,
                    semantic_facts,
                );
                analyze_expr(
                    expr,
                    facts,
                    current_type,
                    current_super_type,
                    internal_methods,
                    field_names,
                    field_types,
                    semantic_facts,
                    type_env,
                    source_return_nullability,
                );
                return;
            }
            if let Some(target) = declared_return_type {
                record_native_value_domain_storage(
                    expr,
                    NativeCollectionStorageSlot {
                        root: NativeCollectionStorageRoot::Return,
                        path: vec![],
                        ty: target.clone(),
                        root_ty: target.clone(),
                    },
                    facts,
                    current_type,
                    type_env,
                    semantic_facts,
                );
            }
            if let Some(ty) = declared_return_type
                && (native_storage_value_type(ty, type_env, semantic_facts)
                    || crate::effective_facts::EffectiveFacts::new(semantic_facts.facts)
                        .native_iterable_element_type(ty)
                        .is_some())
            {
                let storage_types =
                    native_storage_type_env(&[expr], facts, current_type, type_env, semantic_facts);
                if let Some(source) = native_collection_storage_slot(
                    expr,
                    facts,
                    current_type,
                    &storage_types,
                    semantic_facts,
                ) && !source.path.is_empty()
                {
                    facts.native_collection_storage_flows.push((
                        source,
                        NativeCollectionStorageSlot {
                            root: NativeCollectionStorageRoot::Return,
                            path: vec![],
                            ty: ty.clone(),
                            root_ty: ty.clone(),
                        },
                    ));
                }
            }
            let forwarded_array =
                stream_of_forwarded_array_source(expr, facts, field_names, field_types, type_env);
            let returned_storage = forwarded_array.unwrap_or(expr);
            let forwards_nullable_sequence_elements = forwarded_array.is_some_and(|source| {
                returned_expr_type(source, facts, field_names, field_types, type_env).is_some_and(
                    |ty| {
                    matches!(
                        ty,
                        JavaType::Array(element)
                            if !matches!(element.as_ref(), JavaType::Primitive(_) | JavaType::Void)
                    )
                    },
                )
            });
            if let Expr::Var(name) = returned_storage {
                if is_unshadowed_field_name(facts, field_names, name) {
                    if forwards_nullable_sequence_elements {
                        facts
                            .field_uses
                            .push(FieldUse::ReturnedNullableSequenceElements {
                                field_name: name.clone(),
                            });
                    } else {
                        facts.field_uses.push(FieldUse::Returned {
                            field_name: name.clone(),
                        });
                    }
                } else {
                    if forwards_nullable_sequence_elements {
                        record_use(facts, name, BindingUse::ReturnedNullableSequenceElements);
                    } else {
                        record_use(facts, name, BindingUse::Returned);
                    }
                }
            }
            record_returned_subtype_use(
                expr,
                declared_return_type,
                facts,
                type_env,
                semantic_facts,
            );
            record_returned_call_result_use(
                expr,
                facts,
                internal_methods,
                current_type,
                current_super_type,
                field_names,
                field_types,
                semantic_facts,
                type_env,
            );
            if let Some(source) = sequence_element_preserving_return_source(expr) {
                record_returned_call_result_use(
                    source,
                    facts,
                    internal_methods,
                    current_type,
                    current_super_type,
                    field_names,
                    field_types,
                    semantic_facts,
                    type_env,
                );
            }
            if let Expr::FieldAccess(receiver, field_name) = returned_storage
                && matches!(receiver.as_ref(), Expr::This)
            {
                if forwards_nullable_sequence_elements {
                    facts
                        .field_uses
                        .push(FieldUse::ReturnedNullableSequenceElements {
                            field_name: field_name.clone(),
                        });
                } else {
                    facts.field_uses.push(FieldUse::Returned {
                        field_name: field_name.clone(),
                    });
                }
            }
            analyze_return_expr(
                expr,
                facts,
                current_type,
                current_super_type,
                declared_return_type,
                internal_methods,
                field_names,
                field_types,
                semantic_facts,
                type_env,
                source_return_nullability,
            );
        }
        Stmt::Return(None) => {}
        Stmt::Expr(expr) => {
            if expr_mutates_self_field(
                expr,
                facts,
                current_type,
                current_super_type,
                field_names,
                field_types,
                semantic_facts,
            ) {
                facts.mutates_self = true;
            }
            analyze_expr(
                expr,
                facts,
                current_type,
                current_super_type,
                internal_methods,
                field_names,
                field_types,
                semantic_facts,
                type_env,
                source_return_nullability,
            );
        }
        Stmt::If {
            condition,
            then_block,
            else_block,
        } => {
            analyze_expr(
                condition,
                facts,
                current_type,
                current_super_type,
                internal_methods,
                field_names,
                field_types,
                semantic_facts,
                type_env,
                source_return_nullability,
            );
            let then_pattern_bindings = save_true_pattern_bindings(facts, condition);
            let then_non_null = activate_branch_non_null_binding(facts, condition, true);
            analyze_scoped_statements(
                &then_block.statements,
                facts,
                current_type,
                current_super_type,
                declared_return_type,
                internal_methods,
                field_names,
                field_types,
                semantic_facts,
                type_env,
                source_return_nullability,
            );
            restore_branch_non_null_binding(facts, then_non_null);
            restore_scoped_binding_uses(facts, then_pattern_bindings);
            if let Some(else_block) = else_block {
                let else_pattern_bindings = save_false_pattern_bindings(facts, condition);
                let else_non_null = activate_branch_non_null_binding(facts, condition, false);
                analyze_scoped_statements(
                    &else_block.statements,
                    facts,
                    current_type,
                    current_super_type,
                    declared_return_type,
                    internal_methods,
                    field_names,
                    field_types,
                    semantic_facts,
                    type_env,
                    source_return_nullability,
                );
                restore_branch_non_null_binding(facts, else_non_null);
                restore_scoped_binding_uses(facts, else_pattern_bindings);
            }
        }
        Stmt::While {
            condition, body, ..
        } => {
            analyze_expr(
                condition,
                facts,
                current_type,
                current_super_type,
                internal_methods,
                field_names,
                field_types,
                semantic_facts,
                type_env,
                source_return_nullability,
            );
            let body_pattern_bindings = save_true_pattern_bindings(facts, condition);
            let body_non_null = activate_branch_non_null_binding(facts, condition, true);
            analyze_scoped_statements(
                &body.statements,
                facts,
                current_type,
                current_super_type,
                declared_return_type,
                internal_methods,
                field_names,
                field_types,
                semantic_facts,
                type_env,
                source_return_nullability,
            );
            restore_branch_non_null_binding(facts, body_non_null);
            restore_scoped_binding_uses(facts, body_pattern_bindings);
        }
        Stmt::DoWhile {
            condition, body, ..
        } => {
            analyze_scoped_statements(
                &body.statements,
                facts,
                current_type,
                current_super_type,
                declared_return_type,
                internal_methods,
                field_names,
                field_types,
                semantic_facts,
                type_env,
                source_return_nullability,
            );
            analyze_expr(
                condition,
                facts,
                current_type,
                current_super_type,
                internal_methods,
                field_names,
                field_types,
                semantic_facts,
                type_env,
                source_return_nullability,
            );
        }
        Stmt::For {
            init,
            condition,
            update,
            body,
            ..
        } => {
            let saved_init_bindings = save_direct_var_decl_bindings(facts, init);
            for stmt in init {
                analyze_stmt(
                    stmt,
                    facts,
                    current_type,
                    current_super_type,
                    declared_return_type,
                    internal_methods,
                    field_names,
                    field_types,
                    semantic_facts,
                    type_env,
                    source_return_nullability,
                );
            }
            if let Some(condition) = condition {
                analyze_expr(
                    condition,
                    facts,
                    current_type,
                    current_super_type,
                    internal_methods,
                    field_names,
                    field_types,
                    semantic_facts,
                    type_env,
                    source_return_nullability,
                );
            }
            for expr in update {
                analyze_expr(
                    expr,
                    facts,
                    current_type,
                    current_super_type,
                    internal_methods,
                    field_names,
                    field_types,
                    semantic_facts,
                    type_env,
                    source_return_nullability,
                );
            }
            let body_pattern_bindings = condition
                .as_ref()
                .map(|condition| save_true_pattern_bindings(facts, condition))
                .unwrap_or_default();
            let body_non_null = condition
                .as_ref()
                .and_then(|condition| activate_branch_non_null_binding(facts, condition, true));
            analyze_scoped_statements(
                &body.statements,
                facts,
                current_type,
                current_super_type,
                declared_return_type,
                internal_methods,
                field_names,
                field_types,
                semantic_facts,
                type_env,
                source_return_nullability,
            );
            restore_branch_non_null_binding(facts, body_non_null);
            restore_scoped_binding_uses(facts, body_pattern_bindings);
            restore_scoped_binding_uses(facts, saved_init_bindings);
        }
        Stmt::EnhancedFor {
            var_type,
            var_name: _,
            source_var_name,
            iterable,
            body,
            binding,
            ..
        } => {
            let storage_types =
                native_storage_type_env(&[iterable], facts, current_type, type_env, semantic_facts);
            let source_ty = native_storage_expr_type(
                iterable,
                facts,
                current_type,
                &storage_types,
                semantic_facts,
            );
            if native_storage_value_type(&source_ty, &storage_types, semantic_facts)
                && crate::effective_facts::EffectiveFacts::new(semantic_facts.facts)
                    .retained_native_collection_kind(&source_ty)
                    .is_some()
                && let Some(source) = native_collection_storage_slot(
                    iterable,
                    facts,
                    current_type,
                    &storage_types,
                    semantic_facts,
                )
                && matches!(
                    source.root,
                    NativeCollectionStorageRoot::Origin(CollectionArgumentOrigin::Field { .. })
                )
                && !facts
                    .native_collection_retention_requirements
                    .contains(&source)
            {
                // Java's cursor owns a reference to stored collection state;
                // a callback can reenter the owner and change that state.
                // Retain the allocation, never a field loan or snapshot copy.
                facts.native_collection_retention_requirements.push(source);
            }
            if let Some(binding) = binding
                && let Expr::Call {
                    receiver: Some(map),
                    name,
                    args,
                    ..
                } = iterable
                && args.is_empty()
                && matches!(name.as_str(), "entrySet" | "entry_set")
            {
                let types =
                    native_storage_type_env(&[map], facts, current_type, type_env, semantic_facts);
                let ty = native_storage_expr_type(map, facts, current_type, &types, semantic_facts);
                if native_storage_value_type(&ty, &types, semantic_facts)
                    && crate::stdlib::java_collection_kind_for_type(&ty)
                        .is_some_and(|kind| kind.is_map_like())
                    && let Some(slot) = native_collection_storage_slot(
                        map,
                        facts,
                        current_type,
                        &types,
                        semantic_facts,
                    )
                    && !crate::liveness::LivenessAnalysis::body_assigns_outer_binding(
                        body,
                        source_var_name,
                    )
                {
                    facts
                        .native_map_entry_backing_storage
                        .insert(binding.clone(), slot);
                }
            }
            let native_callback = crate::effective_facts::EffectiveFacts::new(semantic_facts.facts)
                .native_functional_carrier(var_type)
                .is_some();
            if (native_storage_value_type(var_type, type_env, semantic_facts) || native_callback)
                && let Some(binding) = binding
            {
                // Resolve the iterable in the enclosing scope: the loop
                // declaration may shadow the very binding being iterated.
                let storage_types = native_storage_type_env(
                    &[iterable],
                    facts,
                    current_type,
                    type_env,
                    semantic_facts,
                );
                let source_ty = native_storage_expr_type(
                    iterable,
                    facts,
                    current_type,
                    &storage_types,
                    semantic_facts,
                );
                if (native_storage_value_type(&source_ty, &storage_types, semantic_facts)
                    || crate::effective_facts::EffectiveFacts::new(semantic_facts.facts)
                        .native_iterable_element_type(&source_ty)
                        .is_some())
                    && let Some(mut source) = native_collection_storage_slot(
                        iterable,
                        facts,
                        current_type,
                        &storage_types,
                        semantic_facts,
                    )
                {
                    source.path.push(0);
                    source.ty = var_type.clone();
                    let target = NativeCollectionStorageSlot {
                        root: NativeCollectionStorageRoot::Binding(binding.clone()),
                        path: vec![],
                        ty: var_type.clone(),
                        root_ty: var_type.clone(),
                    };
                    if native_callback {
                        // Reading an array/Iterable element copies its Java
                        // callback reference. Carry the exact child ABI into
                        // the loop binding without wrapping the facade in a
                        // new native container allocation.
                        facts
                            .nullable_collection_storage_flows
                            .push((source.clone(), target.clone()));
                        facts
                            .native_collection_reference_transfers
                            .push((source, target));
                    } else {
                        facts.native_collection_storage_flows.push((source, target));
                    }
                }
            }
            analyze_expr(
                iterable,
                facts,
                current_type,
                current_super_type,
                internal_methods,
                field_names,
                field_types,
                semantic_facts,
                type_env,
                source_return_nullability,
            );
            let saved_binding = facts.hide_visible_binding_for_scope(source_var_name);
            insert_binding_use_facts(
                facts,
                source_var_name.clone(),
                binding.clone(),
                var_type.clone(),
            );
            analyze_scoped_statements(
                &body.statements,
                facts,
                current_type,
                current_super_type,
                declared_return_type,
                internal_methods,
                field_names,
                field_types,
                semantic_facts,
                type_env,
                source_return_nullability,
            );
            restore_binding_use_facts(facts, source_var_name.clone(), saved_binding);
        }
        Stmt::Switch { selector, cases } => {
            analyze_expr(
                selector,
                facts,
                current_type,
                current_super_type,
                internal_methods,
                field_names,
                field_types,
                semantic_facts,
                type_env,
                source_return_nullability,
            );
            for case in cases {
                let case_pattern_bindings = save_switch_case_pattern_bindings(facts, case);
                analyze_scoped_statements(
                    &case.statements,
                    facts,
                    current_type,
                    current_super_type,
                    declared_return_type,
                    internal_methods,
                    field_names,
                    field_types,
                    semantic_facts,
                    type_env,
                    source_return_nullability,
                );
                restore_scoped_binding_uses(facts, case_pattern_bindings);
            }
        }
        Stmt::Block(block) | Stmt::LabeledBlock { body: block, .. } => {
            analyze_scoped_statements(
                &block.statements,
                facts,
                current_type,
                current_super_type,
                declared_return_type,
                internal_methods,
                field_names,
                field_types,
                semantic_facts,
                type_env,
                source_return_nullability,
            );
        }
        Stmt::Try {
            resources,
            body,
            catches,
            finally_block,
        } => {
            let saved_resource_bindings = save_direct_var_decl_bindings(facts, resources);
            for resource in resources {
                analyze_stmt(
                    resource,
                    facts,
                    current_type,
                    current_super_type,
                    declared_return_type,
                    internal_methods,
                    field_names,
                    field_types,
                    semantic_facts,
                    type_env,
                    source_return_nullability,
                );
            }
            analyze_scoped_statements(
                &body.statements,
                facts,
                current_type,
                current_super_type,
                declared_return_type,
                internal_methods,
                field_names,
                field_types,
                semantic_facts,
                type_env,
                source_return_nullability,
            );
            restore_scoped_binding_uses(facts, saved_resource_bindings);
            for catch in catches {
                let mut catch_env = type_env.clone();
                let catch_ty = catch
                    .types
                    .first()
                    .cloned()
                    .unwrap_or_else(|| JavaType::Unknown("Throwable".to_string()));
                catch_env.insert_local(catch.source_name.clone(), catch_ty);
                let saved_catch_binding = facts.hide_visible_binding_for_scope(&catch.source_name);
                insert_binding_use_facts(
                    facts,
                    catch.source_name.clone(),
                    catch.binding.clone(),
                    catch
                        .types
                        .first()
                        .cloned()
                        .unwrap_or_else(|| JavaType::Unknown("Throwable".to_string())),
                );
                analyze_scoped_statements(
                    &catch.body.statements,
                    facts,
                    current_type,
                    current_super_type,
                    declared_return_type,
                    internal_methods,
                    field_names,
                    field_types,
                    semantic_facts,
                    &mut catch_env,
                    source_return_nullability,
                );
                restore_binding_use_facts(facts, catch.source_name.clone(), saved_catch_binding);
            }
            if let Some(finally_block) = finally_block {
                let mut finally_env = type_env.clone();
                analyze_scoped_statements(
                    &finally_block.statements,
                    facts,
                    current_type,
                    current_super_type,
                    declared_return_type,
                    internal_methods,
                    field_names,
                    field_types,
                    semantic_facts,
                    &mut finally_env,
                    source_return_nullability,
                );
            }
        }
        Stmt::Throw(expr) => analyze_expr(
            expr,
            facts,
            current_type,
            current_super_type,
            internal_methods,
            field_names,
            field_types,
            semantic_facts,
            type_env,
            source_return_nullability,
        ),
        Stmt::ConstructorInvocation {
            kind,
            args,
            resolved,
        } => {
            if let Some(call) = resolved {
                record_constructor_argument_value_domains(
                    args,
                    call,
                    facts,
                    current_type,
                    type_env,
                    semantic_facts,
                );
            }
            let policy = constructor_invocation_boundary_policy(*kind);
            facts.constructor_call_sites.push(ConstructorCallSiteUse {
                name: "constructor".to_string(),
                native_return_argument_calls: native_return_argument_calls(
                    args,
                    internal_methods,
                    facts,
                    current_type,
                    current_super_type,
                    field_names,
                    field_types,
                    semantic_facts,
                    type_env,
                ),
                argument_result_calls: argument_result_calls(args),
                arg_count: args.len(),
                null_arg_indices: null_argument_indices(args),
                nullable_source_call_arg_indices: nullable_source_call_arg_indices(
                    args,
                    source_return_nullability,
                    semantic_facts,
                    facts,
                    current_type,
                    type_env,
                ),
                argument_bindings: call_argument_bindings(args, facts),
                argument_self_fields: call_argument_self_fields(args, facts, field_names),
                boundary: policy.boundary,
                collection_argument_origins: collection_argument_origins(
                    args,
                    type_env,
                    semantic_facts,
                ),
                resolved: resolved.clone(),
            });
            for (arg_index, arg) in args.iter().enumerate() {
                record_call_arg_use(
                    facts,
                    current_type,
                    field_names,
                    field_types,
                    semantic_facts,
                    arg,
                    "constructor",
                    None,
                    args.len(),
                    arg_index,
                    policy.boundary,
                );
                analyze_nested_argument_expr(
                    arg,
                    facts,
                    current_type,
                    current_super_type,
                    internal_methods,
                    field_names,
                    field_types,
                    semantic_facts,
                    type_env,
                    source_return_nullability,
                );
            }
        }
        Stmt::LocalType(_) | Stmt::Break(_) | Stmt::Continue(_) | Stmt::Unsupported(_) => {}
    }
}

pub fn constructor_invocation_boundary_policy(
    kind: ConstructorInvocationKind,
) -> ConstructorInvocationBoundaryPolicy {
    let boundary = match kind {
        ConstructorInvocationKind::This => CallArgumentBoundary::InternalSameType,
        ConstructorInvocationKind::Super => CallArgumentBoundary::Unknown,
    };
    ConstructorInvocationBoundaryPolicy { kind, boundary }
}

fn is_unshadowed_field_name(
    facts: &MethodUsageFacts,
    field_names: &HashSet<String>,
    name: &str,
) -> bool {
    field_names.contains(name) && !facts.name_binding_is_current(name)
}

fn call_result_boundary_policy_with_context(
    expr: &Expr,
    internal_methods: &HashSet<String>,
    facts: &MethodUsageFacts,
    current_type: &str,
    current_super_type: Option<&str>,
    field_names: &HashSet<String>,
    field_types: &HashMap<String, JavaType>,
    semantic_facts: &UsageAnalysisContext<'_>,
    type_env: &TypeEnv,
) -> Option<CallResultBoundaryPolicy> {
    match expr {
        Expr::Call {
            receiver,
            name,
            args,
            resolved,
            ..
        } => {
            let receiver_type = receiver.as_deref().and_then(|receiver| {
                call_result_receiver_type_with_context(
                    receiver,
                    facts,
                    current_type,
                    current_super_type,
                    field_names,
                    field_types,
                    semantic_facts,
                )
                .or_else(|| {
                    // Native views may have a fluent external call receiver,
                    // with no source ResolvedCall. Reuse indexed external
                    // return planning and only enrich the inspected locals.
                    if !semantic_facts
                        .facts
                        .native_return_allocations
                        .keys()
                        .any(|key| key.member_name() == name && key.arity() == args.len())
                    {
                        return None;
                    }
                    let types = native_storage_type_env(
                        &[receiver],
                        facts,
                        current_type,
                        type_env,
                        semantic_facts,
                    );
                    if let Expr::Call {
                        receiver,
                        name,
                        args,
                        ..
                    } = receiver
                    {
                        crate::call_plan::external_call_lowered_type_with_external_index(
                            receiver.as_deref(),
                            name,
                            args.len(),
                            semantic_facts.facts,
                            &semantic_facts.external_lookup,
                            &types,
                            Some(current_type),
                        )
                        .and_then(|lowered| lowered.java_ty().cloned())
                    } else {
                        Some(crate::types::infer_expr_type(receiver, &types))
                    }
                })
            });
            let boundary =
                call_argument_boundary_policy(receiver.as_deref(), name, internal_methods).boundary;
            let boundary =
                resolved_receiver_call_boundary(boundary, receiver.as_deref(), resolved.as_ref());
            let boundary = modeled_receiver_call_boundary(
                boundary,
                receiver_type.as_ref(),
                name,
                args.len(),
                semantic_facts,
                type_env,
            );
            let boundary =
                source_receiver_call_boundary(boundary, receiver_type.as_ref(), semantic_facts);
            Some(CallResultBoundaryPolicy {
                name: name.clone(),
                arg_count: args.len(),
                boundary,
                receiver_type,
                resolved: resolved.clone(),
            })
        }
        Expr::New { resolved, .. } => Some(CallResultBoundaryPolicy {
            name: "new".to_string(),
            arg_count: 0,
            receiver_type: None,
            boundary: constructor_result_boundary_policy(resolved.as_ref()),
            resolved: None,
        }),
        _ => None,
    }
}

fn call_result_boundary_policies_with_context(
    expr: &Expr,
    internal_methods: &HashSet<String>,
    facts: &MethodUsageFacts,
    current_type: &str,
    current_super_type: Option<&str>,
    field_names: &HashSet<String>,
    field_types: &HashMap<String, JavaType>,
    semantic_facts: &UsageAnalysisContext<'_>,
    type_env: &TypeEnv,
) -> Vec<CallResultBoundaryPolicy> {
    match expr {
        Expr::Cast { expr, .. } => call_result_boundary_policies_with_context(
            expr,
            internal_methods,
            facts,
            current_type,
            current_super_type,
            field_names,
            field_types,
            semantic_facts,
            type_env,
        ),
        Expr::Ternary {
            then_expr,
            else_expr,
            ..
        } => {
            let mut policies = call_result_boundary_policies_with_context(
                then_expr,
                internal_methods,
                facts,
                current_type,
                current_super_type,
                field_names,
                field_types,
                semantic_facts,
                type_env,
            );
            policies.extend(call_result_boundary_policies_with_context(
                else_expr,
                internal_methods,
                facts,
                current_type,
                current_super_type,
                field_names,
                field_types,
                semantic_facts,
                type_env,
            ));
            policies
        }
        _ => call_result_boundary_policy_with_context(
            expr,
            internal_methods,
            facts,
            current_type,
            current_super_type,
            field_names,
            field_types,
            semantic_facts,
            type_env,
        )
        .into_iter()
        .collect(),
    }
}

fn call_result_receiver_type_with_context(
    receiver: &Expr,
    facts: &MethodUsageFacts,
    current_type: &str,
    current_super_type: Option<&str>,
    field_names: &HashSet<String>,
    field_types: &HashMap<String, JavaType>,
    semantic_facts: &UsageAnalysisContext<'_>,
) -> Option<JavaType> {
    match receiver {
        Expr::Var(name) if is_unshadowed_field_name(facts, field_names, name) => {
            source_field_type(current_type, name, field_types, semantic_facts)
        }
        Expr::FieldAccess(inner, field_name) if matches!(inner.as_ref(), Expr::This) => {
            source_field_type(current_type, field_name, field_types, semantic_facts)
        }
        Expr::Super => current_super_type.map(|name| JavaType::Unknown(name.to_string())),
        _ => call_result_receiver_type(receiver, facts),
    }
}

fn call_result_receiver_type(receiver: &Expr, facts: &MethodUsageFacts) -> Option<JavaType> {
    match receiver {
        Expr::Var(name) => agreed_binding_java_type(facts, name),
        Expr::This | Expr::Super => None,
        Expr::Cast { ty, .. } => Some(ty.clone()),
        _ => None,
    }
}

fn assigned_subtype_local_actual_type(
    value: &Expr,
    target_ty: &JavaType,
    type_env: &TypeEnv,
    semantic_facts: &UsageAnalysisContext<'_>,
    facts: &MethodUsageFacts,
) -> Option<JavaType> {
    // The usage TypeEnv contains declaration binders, not the evolving local
    // binding table. Reuse its lexical binding facts for local upcasts just
    // as source argument and return boundaries already do.
    let actual_ty = match value {
        Expr::Var(name) => agreed_binding_java_type(facts, name)
            .unwrap_or_else(|| crate::types::infer_expr_type(value, type_env)),
        _ => crate::types::infer_expr_type(value, type_env),
    };
    assigned_subtype_actual_type(&actual_ty, target_ty, type_env, semantic_facts)
}

fn assigned_subtype_actual_type(
    actual_ty: &JavaType,
    target_ty: &JavaType,
    type_env: &TypeEnv,
    semantic_facts: &UsageAnalysisContext<'_>,
) -> Option<JavaType> {
    if actual_ty != target_ty
        && semantic_facts
            .hierarchy_subtype_policy(actual_ty, target_ty)
            .reaches()
    {
        return Some(actual_ty.clone());
    }
    if let Some(bound_ty) = assigned_subtype_actual_type_from_type_param_bound(
        actual_ty,
        target_ty,
        type_env,
        semantic_facts,
    ) {
        return Some(bound_ty);
    }
    assigned_subtype_actual_type_from_source_hierarchy_alias(actual_ty, target_ty, semantic_facts)
}

fn assigned_subtype_actual_type_from_type_param_bound(
    actual_ty: &JavaType,
    target_ty: &JavaType,
    type_env: &TypeEnv,
    semantic_facts: &UsageAnalysisContext<'_>,
) -> Option<JavaType> {
    let actual_name = usage_java_type_name(actual_ty)?;
    let bound_ty = type_env.type_param_bound(actual_name)?;
    if bound_ty == target_ty {
        return None;
    }
    semantic_facts
        .hierarchy_subtype_policy(bound_ty, target_ty)
        .reaches()
        .then(|| bound_ty.clone())
}

fn assigned_subtype_actual_type_from_source_hierarchy_alias(
    actual_ty: &JavaType,
    target_ty: &JavaType,
    semantic_facts: &UsageAnalysisContext<'_>,
) -> Option<JavaType> {
    let actual_name = usage_java_type_name(actual_ty)?;
    let target_name = usage_java_type_name(target_ty)?;
    let actual_simple = simple_usage_type_name(actual_name);
    let target_simple = simple_usage_type_name(target_name);
    if actual_simple == target_simple {
        return None;
    }
    let candidates = semantic_facts
        .type_capability_overrides
        .iter()
        .filter(|fact| {
            fact.provenance == crate::facts::FactProvenance::ParsedSource
                && simple_usage_type_name(&fact.type_name) == actual_simple
        })
        .collect::<Vec<_>>();
    let [candidate] = candidates.as_slice() else {
        return None;
    };
    let qualified_actual = JavaType::Named {
        name: candidate.type_name.clone(),
        args: Vec::new(),
    };
    let reaches_target = semantic_facts
        .reachable_hierarchy_edges_for_type(&qualified_actual)
        .iter()
        .any(|edge| simple_usage_type_name(&edge.super_name) == target_simple);
    reaches_target.then_some(qualified_actual)
}

fn simple_usage_type_name(name: &str) -> &str {
    name.rsplit('.').next().unwrap_or(name)
}

fn analyze_expr(
    expr: &Expr,
    facts: &mut MethodUsageFacts,
    current_type: &str,
    current_super_type: Option<&str>,
    internal_methods: &HashSet<String>,
    field_names: &HashSet<String>,
    field_types: &HashMap<String, JavaType>,
    semantic_facts: &UsageAnalysisContext<'_>,
    type_env: &TypeEnv,
    source_return_nullability: &HashMap<(String, usize), NullabilityState>,
) {
    analyze_expr_with_context(
        expr,
        facts,
        current_type,
        current_super_type,
        internal_methods,
        field_names,
        field_types,
        semantic_facts,
        type_env,
        source_return_nullability,
        false,
    );
}

fn analyze_return_expr(
    expr: &Expr,
    facts: &mut MethodUsageFacts,
    current_type: &str,
    current_super_type: Option<&str>,
    _declared_return_type: Option<&JavaType>,
    internal_methods: &HashSet<String>,
    field_names: &HashSet<String>,
    field_types: &HashMap<String, JavaType>,
    semantic_facts: &UsageAnalysisContext<'_>,
    type_env: &TypeEnv,
    source_return_nullability: &HashMap<(String, usize), NullabilityState>,
) {
    analyze_expr_with_context(
        expr,
        facts,
        current_type,
        current_super_type,
        internal_methods,
        field_names,
        field_types,
        semantic_facts,
        type_env,
        source_return_nullability,
        true,
    );
}

fn analyze_nested_argument_expr(
    expr: &Expr,
    facts: &mut MethodUsageFacts,
    current_type: &str,
    current_super_type: Option<&str>,
    internal_methods: &HashSet<String>,
    field_names: &HashSet<String>,
    field_types: &HashMap<String, JavaType>,
    semantic_facts: &UsageAnalysisContext<'_>,
    type_env: &TypeEnv,
    source_return_nullability: &HashMap<(String, usize), NullabilityState>,
) {
    if matches!(expr, Expr::Var(_)) {
        return;
    }
    analyze_expr(
        expr,
        facts,
        current_type,
        current_super_type,
        internal_methods,
        field_names,
        field_types,
        semantic_facts,
        type_env,
        source_return_nullability,
    );
}

fn call_receiver_is_self(receiver: Option<&Expr>) -> bool {
    matches!(receiver, None | Some(Expr::This) | Some(Expr::Super))
}

fn null_argument_indices(args: &[Expr]) -> Vec<usize> {
    args.iter()
        .enumerate()
        .filter_map(|(index, arg)| expr_is_null_literal(arg).then_some(index))
        .collect()
}

fn call_argument_bindings(args: &[Expr], facts: &MethodUsageFacts) -> Vec<(usize, BindingId)> {
    args.iter()
        .enumerate()
        .filter_map(|(index, arg)| {
            let Expr::Var(name) = arg else {
                return None;
            };
            facts
                .visible_binding_id_for_name(name)
                .map(|binding| (index, binding))
        })
        .collect()
}

fn non_null_argument_indices(args: &[Expr], facts: &MethodUsageFacts) -> Vec<usize> {
    args.iter()
        .enumerate()
        .filter_map(|(index, arg)| {
            let Expr::Var(name) = arg else {
                return None;
            };
            let refined = facts
                .visible_binding_id_for_name(name)
                .is_some_and(|binding| facts.active_non_null_bindings.contains(&binding))
                || (facts.visible_binding_id_for_name(name).is_none()
                    && facts.active_non_null_idless_names.contains(name));
            refined.then_some(index)
        })
        .collect()
}

fn call_argument_self_fields(
    args: &[Expr],
    facts: &MethodUsageFacts,
    field_names: &HashSet<String>,
) -> Vec<(usize, String)> {
    args.iter()
        .enumerate()
        .filter_map(|(index, arg)| {
            self_field_name(arg, facts, field_names).map(|field| (index, field.to_string()))
        })
        .collect()
}

/// Enrich only the expressions under inspection, using lexical binding facts
/// rather than copying every callable local or guessing names across scopes.
fn native_storage_expr_type(
    expr: &Expr,
    facts: &MethodUsageFacts,
    owner: &str,
    types: &TypeEnv,
    context: &UsageAnalysisContext<'_>,
) -> JavaType {
    match expr {
        Expr::Var(name) => agreed_binding_java_type(facts, name)
            .or_else(|| {
                context
                    .source_symbols
                    .as_ref()?
                    .source_field_identity(owner, name)
                    .map(|(_, _, ty)| ty)
            })
            .unwrap_or_else(|| crate::types::infer_expr_type(expr, types)),
        Expr::This => JavaType::Named {
            name: owner.into(),
            args: vec![],
        },
        Expr::FieldAccess(receiver, field) => {
            let ty = native_storage_expr_type(receiver, facts, owner, types, context);
            context
                .source_symbols
                .as_ref()
                .and_then(|symbols| symbols.source_field_identity_for_receiver(&ty, field))
                .map(|(_, _, ty)| ty)
                .unwrap_or_else(|| crate::types::infer_expr_type(expr, types))
        }
        _ => crate::types::infer_expr_type(expr, types),
    }
}

fn native_storage_type_env(
    expressions: &[&Expr],
    facts: &MethodUsageFacts,
    owner: &str,
    types: &TypeEnv,
    context: &UsageAnalysisContext<'_>,
) -> TypeEnv {
    let mut types = types.clone();
    for expr in expressions {
        crate::semantic_analysis::walk_expr_outside_lambdas(expr, &mut |expr| {
            if let Expr::Var(name) = expr {
                if let Some(ty) = agreed_binding_java_type(facts, name) {
                    types.insert_local(name, ty);
                } else if let Some((_, _, ty)) = context
                    .source_symbols
                    .as_ref()
                    .and_then(|symbols| symbols.source_field_identity(owner, name))
                {
                    types.insert_field(name, ty);
                }
            }
        });
    }
    if context.source_symbols.is_some() {
        for expr in expressions {
            crate::semantic_analysis::walk_expr_outside_lambdas(expr, &mut |expr| {
                if let Expr::FieldAccess(receiver, field) = expr {
                    let receiver_ty =
                        native_storage_expr_type(receiver, facts, owner, &types, context);
                    let Some((_, _, ty)) = context.source_symbols.as_ref().and_then(|symbols| {
                        symbols.source_field_identity_for_receiver(&receiver_ty, field)
                    }) else {
                        // An unresolved qualified owner is not evidence of a
                        // value field. Inventing one here hides SDK type paths
                        // from the shared static-owner planner.
                        return;
                    };
                    if matches!(receiver.as_ref(), Expr::This) {
                        types.insert_field(field, ty);
                    } else {
                        let receiver_ty =
                            native_storage_expr_type(receiver, facts, owner, &types, context);
                        let receiver_ty = crate::types::optional_element_type(&receiver_ty)
                            .unwrap_or(receiver_ty);
                        if let Some(owner) = usage_java_type_name(&receiver_ty) {
                            types.insert_type_field(owner, field, ty);
                        }
                    }
                }
            });
        }
    }
    types
}

fn native_storage_value_type(
    ty: &JavaType,
    types: &TypeEnv,
    context: &UsageAnalysisContext<'_>,
) -> bool {
    let ty = crate::types::optional_element_type(ty).unwrap_or_else(|| ty.clone());
    matches!(ty, JavaType::Array(_))
        || (crate::stdlib::java_collection_kind_for_type(&ty).is_some()
            && usage_java_type_name(&ty).is_some_and(|name| {
                crate::stdlib::is_known_java_platform_subtype(name, "Collection")
                    || crate::stdlib::is_known_java_platform_subtype(name, "Map")
            })
            && !usage_java_type_name(&ty).is_some_and(|name| {
                crate::call_plan::java_type_owner_is_parsed_source(name, types, context.facts)
            }))
}

fn record_native_call_input_domains(
    expr: &Expr,
    facts: &mut MethodUsageFacts,
    owner: &str,
    types: &TypeEnv,
    context: &UsageAnalysisContext<'_>,
) {
    let Expr::Call {
        receiver: Some(receiver),
        name,
        args,
        resolved,
        ..
    } = expr
    else {
        return;
    };
    let foreach_types = (name == "forEach" && args.len() == 1)
        .then(|| native_storage_type_env(&[receiver], facts, owner, types, context));
    if let Some((values, action)) = foreach_types
        .as_ref()
        .and_then(|types| crate::call_plan::native_iterable_for_each(expr, types, context.facts))
    {
        let storage_types =
            native_storage_type_env(&[values, action], facts, owner, types, context);
        if let (Some(source), Some(action)) = (
            native_collection_storage_slot(values, facts, owner, &storage_types, context)
                .and_then(|slot| slot.child(&[0])),
            native_collection_storage_slot(action, facts, owner, &storage_types, context),
        ) && let Some(target) = native_functional_value_slot(&action, Some(0), context)
        {
            record_functional_value_domain_transfer(source, target, facts, &storage_types, context);
        }
        return;
    }
    let receiver_ty = native_storage_expr_type(receiver, facts, owner, types, context);
    // Stored scalar references have nullable domains too. A nullable field
    // or call result inserted into a native container must widen that child,
    // just as an explicit null does; query-only operands must never do so.
    if native_storage_value_type(&receiver_ty, types, context)
        && let Some(method) =
            native_storage_method_for_type(receiver, name, args, &receiver_ty, types, context)
        && !method.stored_argument_children().is_empty()
    {
        let storage_types = native_storage_type_env(&[expr], facts, owner, types, context);
        if let Some(container) =
            native_collection_storage_slot(receiver, facts, owner, &storage_types, context)
        {
            let non_null_arguments = non_null_argument_indices(args, facts);
            for (argument, child) in method.stored_argument_children() {
                let Some(value) = args.get(*argument) else {
                    continue;
                };
                // Reference casts already have canonical destination flows.
                // Branch-refined values must not widen
                // storage back to the nullable domain of their source binding.
                if non_null_arguments.contains(argument) || matches!(value, Expr::Cast { .. }) {
                    continue;
                }
                let Some(target) = container.child(&[*child]) else {
                    continue;
                };
                if matches!(target.ty, JavaType::Primitive(_) | JavaType::Void) {
                    continue;
                }
                if expr_is_null_literal(value) {
                    if crate::stdlib::java_collection_kind_for_type(&receiver_ty).is_some_and(
                        |kind| {
                            crate::call_plan::nullable_collection_element_argument_index(
                                kind,
                                name,
                                args.len(),
                            ) == Some(*argument)
                        },
                    ) {
                        // A literal null has no source binding, but it still
                        // seeds the same directional value-domain graph. This
                        // includes collections reached through another object.
                        record_native_value_domain_storage(
                            value,
                            target,
                            facts,
                            owner,
                            &storage_types,
                            context,
                        );
                    }
                    continue;
                }
                if let Some(source) =
                    native_collection_storage_slot(value, facts, owner, &storage_types, context)
                {
                    facts
                        .nullable_collection_storage_flows
                        .push((source, target));
                }
            }
        }
    }
    if let Some(receiver_name) = usage_java_type_name(&receiver_ty)
        && crate::effective_facts::EffectiveFacts::new(context.facts)
            .native_functional_carrier(&receiver_ty)
            .is_some()
        && crate::stdlib::NativeFunctionalComposition::fixed_for_call(
            receiver_name,
            name,
            args.len(),
        )
        .is_some()
    {
        // A composition used directly in another expression still retains
        // both operands. Its carrier ABI cannot depend on having a named
        // destination local (for example staticNoop.andThen(local) == local).
        let storage_types = native_storage_type_env(&[expr], facts, owner, types, context);
        if let (Some(receiver), Some(argument)) = (
            native_collection_storage_slot(receiver, facts, owner, &storage_types, context),
            native_collection_storage_slot(&args[0], facts, owner, &storage_types, context),
        ) {
            facts
                .native_callback_capture_domains
                .push((argument, receiver));
        }
        return;
    }
    if !functional_value_invocation_matches(
        &receiver_ty,
        name,
        resolved.as_ref(),
        args.len(),
        context,
    ) {
        return;
    }
    let storage_types = native_storage_type_env(&[expr], facts, owner, types, context);
    let Some(receiver) =
        native_collection_storage_slot(receiver, facts, owner, &storage_types, context)
    else {
        return;
    };
    for (index, argument) in args.iter().enumerate() {
        for (mut target, path) in functional_value_domain_slots(&receiver, Some(index), context) {
            if path.is_empty() && expr_is_null_literal(argument) {
                if source_functional_call(&receiver.ty, context).is_some() {
                    continue;
                }
                target.ty = crate::types::optional_java_type(target.ty);
                facts
                    .nullable_collection_storage_flows
                    .push((target.clone(), target));
                continue;
            }
            if let Some(source) =
                native_collection_storage_slot(argument, facts, owner, &storage_types, context)
                    .and_then(|source| source.child(&path))
            {
                record_functional_projected_transfer(
                    source,
                    target,
                    &receiver.ty,
                    facts,
                    &storage_types,
                    context,
                );
            } else if native_storage_value_type(&target.ty, &storage_types, context) {
                // A fresh argument may be retained by the invoked callback.
                facts.native_collection_retention_requirements.push(target);
            }
        }
    }
}

fn record_native_ordering_input_domains(
    expr: &Expr,
    facts: &mut MethodUsageFacts,
    owner: &str,
    types: &TypeEnv,
    context: &UsageAnalysisContext<'_>,
) {
    let Some((_, values, comparator)) =
        crate::call_plan::native_ordering_consumer(expr, context.facts, types)
    else {
        return;
    };
    let storage_types =
        native_storage_type_env(&[values, comparator], facts, owner, types, context);
    let Some(mut target) =
        native_collection_storage_slot(comparator, facts, owner, &storage_types, context)
    else {
        return;
    };
    let Some(input) = crate::slot_shape::RustSlotShape::native_child_type_at_path(&target.ty, &[0])
    else {
        return;
    };
    target.path.push(0);
    target.ty = input;
    let array_ty = native_storage_expr_type(values, facts, owner, &storage_types, context);
    if matches!(
        crate::types::optional_element_type(&array_ty).unwrap_or_else(|| array_ty.clone()),
        JavaType::Array(_)
    ) && let Some(physical) = crate::shape_projection::indexed_assignment_shape_with_facts(
        &crate::slot_shape::RustSlotShape::for_storage(array_ty),
        context.facts,
    ) {
        // The Comparator's T must admit the array's physical null elements.
        // Feed this into the existing qualified generic-domain planner so the
        // stored facade and its lambda are instantiated consistently.
        target.ty = physical.java_ty;
        facts
            .nullable_collection_storage_flows
            .push((target.clone(), target.clone()));
    }
    if let Some(mut source) =
        native_collection_storage_slot(values, facts, owner, &storage_types, context)
        && let Some(child) =
            crate::slot_shape::RustSlotShape::native_child_type_at_path(&source.ty, &[0])
    {
        source.path.push(0);
        source.ty = child.clone();
        facts
            .nullable_collection_storage_flows
            .push((source.clone(), target.clone()));
        if native_storage_value_type(&child, &storage_types, context) {
            facts.native_collection_storage_flows.push((source, target));
        }
    }
}

fn record_native_comparison_retention(
    left: &Expr,
    right: &Expr,
    facts: &mut MethodUsageFacts,
    owner: &str,
    types: &TypeEnv,
    context: &UsageAnalysisContext<'_>,
) {
    if matches!(left, Expr::Literal(crate::java_ir::Literal::Null))
        || matches!(right, Expr::Literal(crate::java_ir::Literal::Null))
    {
        return; // A null test observes presence, not allocation identity.
    }
    for operand in [left, right] {
        let ty = native_storage_expr_type(operand, facts, owner, types, context);
        if !matches!(
            crate::types::optional_element_type(&ty).unwrap_or_else(|| ty.clone()),
            JavaType::Array(_)
        ) && crate::effective_facts::EffectiveFacts::new(context.facts)
            .retained_native_container_kind(&ty)
            .is_none()
        {
            continue;
        }
        let storage_types = native_storage_type_env(&[operand], facts, owner, types, context);
        if let Some(slot) =
            native_collection_storage_slot(operand, facts, owner, &storage_types, context)
        {
            // Comparison requires observable allocations, but proves neither
            // aliasing nor equal element domains between the two operands.
            facts.native_collection_retention_requirements.push(slot);
        }
    }
}

fn native_storage_method(
    receiver: &Expr,
    name: &str,
    args: &[Expr],
    types: &TypeEnv,
    context: &UsageAnalysisContext<'_>,
) -> Option<crate::call_plan::NativeCollectionMethod> {
    let ty = crate::types::infer_expr_type(receiver, types);
    native_storage_method_for_type(receiver, name, args, &ty, types, context)
}

fn native_storage_method_for_type(
    receiver: &Expr,
    name: &str,
    args: &[Expr],
    ty: &JavaType,
    types: &TypeEnv,
    context: &UsageAnalysisContext<'_>,
) -> Option<crate::call_plan::NativeCollectionMethod> {
    if !native_storage_value_type(ty, types, context) {
        return None;
    }
    match crate::call_plan::plan_native_collection_receiver_call_for_type(
        receiver,
        name,
        Some(args),
        ty,
        types,
    )? {
        crate::call_plan::CallTargetPlan::NativeCollectionMethod { method, .. } => Some(method),
        _ => None,
    }
}

fn record_null_source_field_assignment(
    target: &Expr,
    value: &Expr,
    facts: &mut MethodUsageFacts,
    owner: &str,
    types: &TypeEnv,
    context: &UsageAnalysisContext<'_>,
) {
    if !matches!(value, Expr::Literal(Literal::Null)) {
        return;
    }
    let storage_types = native_storage_type_env(&[target], facts, owner, types, context);
    if let Some(slot) =
        native_collection_storage_slot(target, facts, owner, &storage_types, context)
        && slot.path.is_empty()
        && let NativeCollectionStorageRoot::Origin(CollectionArgumentOrigin::Field { owner, name }) =
            slot.root
    {
        let field = crate::facts::FieldKey::named(owner, name);
        if !facts.null_assigned_source_fields.contains(&field) {
            facts.null_assigned_source_fields.push(field);
        }
    }
}

fn has_projectable_native_value_domain(expr: &Expr) -> bool {
    if matches!(
        expr,
        Expr::Binary {
            op: BinaryOp::Assign,
            ..
        }
    ) {
        return true;
    }
    matches!(
        expr,
        Expr::ArrayCreation { .. }
            | Expr::ArrayInitializer(_)
            | Expr::Var(_)
            | Expr::Literal(Literal::Null)
            | Expr::FieldAccess(..)
            | Expr::Lambda { .. }
            | Expr::MethodReference { .. }
            | Expr::Call { .. }
            | Expr::New { .. }
            | Expr::ArrayAccess(..)
            | Expr::Cast { .. }
            | Expr::Ternary { .. }
    )
}

/// Statements and nested Java assignment expressions write the same physical
/// slot. Both must connect the RHS value domain before later reads are planned.
fn record_native_assignment_value_domains(
    target: &Expr,
    value: &Expr,
    facts: &mut MethodUsageFacts,
    owner: &str,
    types: &TypeEnv,
    context: &UsageAnalysisContext<'_>,
) {
    if !has_projectable_native_value_domain(value) {
        return;
    }
    let storage_types = native_storage_type_env(&[target], facts, owner, types, context);
    if let Some(destination) =
        native_collection_storage_slot(target, facts, owner, &storage_types, context)
    {
        record_native_value_domain_storage(
            value,
            destination,
            facts,
            owner,
            &storage_types,
            context,
        );
    }
}

/// A successful reference cast preserves Java null, but it does not equate
/// ownership, dispatch, or native allocation carriers. Feed its value domain
/// into the existing directional nullable-storage graph, including casts in
/// conditional branches. Primitive casts/unboxing are not reference copies.
fn record_reference_cast_value_domain(
    expr: &Expr,
    destination: &NativeCollectionStorageSlot,
    facts: &mut MethodUsageFacts,
    owner: &str,
    types: &TypeEnv,
    context: &UsageAnalysisContext<'_>,
) {
    match expr {
        Expr::Ternary {
            then_expr,
            else_expr,
            ..
        } => {
            for branch in [then_expr, else_expr] {
                record_reference_cast_value_domain(
                    branch,
                    destination,
                    facts,
                    owner,
                    types,
                    context,
                );
            }
        }
        Expr::Cast { ty, expr: source }
            if !matches!(ty, JavaType::Primitive(_) | JavaType::Void) =>
        {
            let mut source = source.as_ref();
            while let Expr::Cast { ty, expr } = source {
                if matches!(ty, JavaType::Primitive(_) | JavaType::Void) {
                    return;
                }
                source = expr;
            }
            let types = native_storage_type_env(&[source], facts, owner, types, context);
            if !matches!(
                native_storage_expr_type(source, facts, owner, &types, context),
                JavaType::Primitive(_) | JavaType::Void
            ) && let Some(source) =
                native_collection_storage_slot(source, facts, owner, &types, context)
            {
                facts
                    .nullable_collection_storage_flows
                    .push((source, destination.clone()));
            }
        }
        _ => {}
    }
}

/// Allocation/value contracts are the same for `new`, `this(...)` and
/// `super(...)`. Resolve their destination slots once from constructor identity.
fn record_constructor_argument_value_domains(
    args: &[Expr],
    call: &ResolvedConstructor,
    facts: &mut MethodUsageFacts,
    owner: &str,
    types: &TypeEnv,
    context: &UsageAnalysisContext<'_>,
) {
    for (index, (arg, param)) in args.iter().zip(&call.params).enumerate() {
        if has_projectable_native_value_domain(arg) {
            record_native_value_domain_storage(
                arg,
                NativeCollectionStorageSlot {
                    root: NativeCollectionStorageRoot::ConstructorArgument {
                        call: call.clone(),
                        index,
                    },
                    path: vec![],
                    ty: param.ty.clone(),
                    root_ty: param.ty.clone(),
                },
                facts,
                owner,
                types,
                context,
            );
        }
    }
}

fn record_native_lambda_environment(
    params: &[LambdaParam],
    body: &LambdaBody,
    destination: &NativeCollectionStorageSlot,
    facts: &mut MethodUsageFacts,
    types: &TypeEnv,
    context: &UsageAnalysisContext<'_>,
) {
    let effective = crate::effective_facts::EffectiveFacts::new(context.facts);
    let captures = crate::resolve::captures_for_lambda(
        params,
        body,
        facts.bindings.iter().filter_map(|(name, binding)| {
            binding
                .binding
                .clone()
                .map(|id| (name.as_str(), &binding.java_ty, id))
        }),
    );
    for capture in captures {
        let ty = capture.ty;
        if (native_storage_value_type(&ty, types, context)
            || effective.native_functional_carrier(&ty).is_some())
            && let Some(binding) = capture.binding
        {
            let captured = NativeCollectionStorageSlot {
                root: NativeCollectionStorageRoot::Binding(binding),
                path: vec![],
                ty: ty.clone(),
                root_ty: ty,
            };
            // Publishing a Java array/container through a stored callback
            // retains its reference even when no other lexical alias survives.
            // Mutability belongs in that allocation, allowing the callback's
            // shared invocation path to reenter without an exclusive closure
            // borrow or a copied payload.
            if native_storage_value_type(&captured.ty, types, context)
                && !facts
                    .native_collection_retention_requirements
                    .contains(&captured)
            {
                facts
                    .native_collection_retention_requirements
                    .push(captured.clone());
            }
            facts
                .native_callback_environment_domains
                .push((captured, destination.clone()));
        }
    }
}

/// Project by SAM invocation position. UnaryOperator's two invocation endpoints
/// share child zero, unlike Function's distinct input/result children.
fn native_functional_value_slot(
    root: &NativeCollectionStorageSlot,
    input: Option<usize>,
    context: &UsageAnalysisContext<'_>,
) -> Option<NativeCollectionStorageSlot> {
    let effective = crate::effective_facts::EffectiveFacts::new(context.facts);
    let sam = effective.native_functional_carrier(&root.ty)?;
    let value = match input {
        Some(index) => *sam.inputs().get(index)?,
        None => sam.returns()?,
    };
    let (index, path) = value.type_argument_projection()?;
    // These generic composition families expose whole type arguments. An
    // applied endpoint would need a value root as well as its child domain.
    if !path.is_empty() {
        return None;
    }
    root.child(&[index])
}

/// Both native and parsed source SAMs transfer invocation values through the
/// same storage graph. Project symbolic binders, never equal actual Java types.
fn functional_value_domain_slots(
    root: &NativeCollectionStorageSlot,
    input: Option<usize>,
    context: &UsageAnalysisContext<'_>,
) -> Vec<(NativeCollectionStorageSlot, Vec<usize>)> {
    let ty = crate::types::optional_element_type(&root.ty).unwrap_or_else(|| root.ty.clone());
    if let Some(symbols) = &context.source_symbols
        && let Some(projections) = symbols.source_functional_value_projections(&ty, input)
    {
        if !projections.is_empty() {
            return projections
                .into_iter()
                .filter_map(|(index, path)| Some((root.child(&[index])?, path)))
                .collect();
        }
        // A fixed signature, e.g. int[] apply(int[]), has no generic child.
        // Its ordinary source callable parameter/result is the value domain.
        if let Some(call) = symbols.source_functional_interface_call(&ty) {
            let (domain, value) = match input {
                Some(index) => {
                    let Some(value) = call.arg_target_types.get(index).cloned() else {
                        return Vec::new();
                    };
                    (
                        NativeCollectionStorageRoot::MethodArgument { call, index },
                        value,
                    )
                }
                None => {
                    let value = call.return_type.clone();
                    (
                        NativeCollectionStorageRoot::Origin(
                            CollectionArgumentOrigin::MethodReturn(call),
                        ),
                        value,
                    )
                }
            };
            return vec![(
                NativeCollectionStorageSlot {
                    root: domain,
                    path: vec![],
                    ty: value.clone(),
                    root_ty: value,
                },
                vec![],
            )];
        }
        return Vec::new();
    }
    let effective = crate::effective_facts::EffectiveFacts::new(context.facts);
    let Some(sam) = effective.native_functional_carrier(&ty) else {
        return Vec::new();
    };
    let value = match input {
        Some(index) => sam.inputs().get(index).copied(),
        None => sam.returns(),
    };
    value
        .and_then(|value| value.type_argument_projection())
        .and_then(|(index, path)| Some((root.child(&[index])?, path.to_vec())))
        .into_iter()
        .collect()
}

fn functional_value_invocation_matches(
    ty: &JavaType,
    name: &str,
    resolved: Option<&ResolvedCall>,
    arity: usize,
    context: &UsageAnalysisContext<'_>,
) -> bool {
    let ty = crate::types::optional_element_type(ty).unwrap_or_else(|| ty.clone());
    if let Some(call) = context
        .source_symbols
        .as_ref()
        .and_then(|symbols| symbols.source_functional_interface_call(&ty))
    {
        let selected_name = resolved
            .map(|call| call.rust_name.clone())
            .unwrap_or_else(|| crate::parser::to_snake_case(name));
        return call.arg_target_types.len() == arity && call.rust_name == selected_name;
    }
    crate::effective_facts::EffectiveFacts::new(context.facts)
        .native_functional_carrier(&ty)
        .is_some_and(|sam| sam.method() == name && sam.inputs().len() == arity)
}

fn functional_value_input_types(
    ty: &JavaType,
    context: &UsageAnalysisContext<'_>,
) -> Option<Vec<JavaType>> {
    let ty = crate::types::optional_element_type(ty).unwrap_or_else(|| ty.clone());
    if let Some(call) = context
        .source_symbols
        .as_ref()?
        .source_functional_interface_call(&ty)
    {
        return Some(call.arg_target_types);
    }
    let JavaType::Named { args, .. } = &ty else {
        return None;
    };
    crate::effective_facts::EffectiveFacts::new(context.facts)
        .native_functional_carrier(&ty)?
        .input_types(args)
}

fn record_native_generic_composition_storage(
    expr: &Expr,
    destination: &NativeCollectionStorageSlot,
    facts: &mut MethodUsageFacts,
    owner: &str,
    types: &TypeEnv,
    context: &UsageAnalysisContext<'_>,
) -> bool {
    use crate::stdlib::NativeCompositionValueEndpoint::*;
    let Expr::Call {
        receiver: Some(receiver),
        name,
        args,
        ..
    } = expr
    else {
        return false;
    };
    if crate::stdlib::NativeFunctionalComposition::for_call(name, args.len()).is_none() {
        return false;
    }
    let storage_types = native_storage_type_env(&[expr], facts, owner, types, context);
    let Some(receiver) =
        native_collection_storage_slot(receiver, facts, owner, &storage_types, context)
    else {
        return false;
    };
    let effective = crate::effective_facts::EffectiveFacts::new(context.facts);
    if effective.native_functional_carrier(&receiver.ty).is_none()
        || effective
            .native_functional_carrier(&destination.ty)
            .is_none()
    {
        return false;
    }
    let receiver_ty =
        crate::types::optional_element_type(&receiver.ty).unwrap_or_else(|| receiver.ty.clone());
    let Some(contract) =
        crate::stdlib::NativeGenericCompositionContract::for_call(&receiver_ty, name, args.len())
    else {
        return false;
    };
    let result_ty = crate::types::optional_element_type(&destination.ty)
        .unwrap_or_else(|| destination.ty.clone());
    if contract.argument_type_for_result(&result_ty).is_none() {
        return false;
    }
    let lambda = match &args[0] {
        Expr::Lambda { params, body }
            if params.len() == 1 && params[0].binding.is_some() && params[0].ty.is_some() =>
        {
            Some((params, body))
        }
        _ => None,
    };
    let argument = native_collection_storage_slot(&args[0], facts, owner, &storage_types, context);
    if lambda.is_none()
        && !argument.as_ref().is_some_and(|argument| {
            effective.native_functional_carrier(&argument.ty).is_some()
                && contract
                    .result_type_for_argument(
                        &crate::types::optional_element_type(&argument.ty)
                            .unwrap_or_else(|| argument.ty.clone()),
                    )
                    .is_some()
        })
    {
        return false;
    }
    let endpoint = |value| match value {
        ReceiverInput(index) => native_functional_value_slot(&receiver, Some(index), context),
        ReceiverResult => native_functional_value_slot(&receiver, None, context),
        ResultInput(index) => native_functional_value_slot(destination, Some(index), context),
        ResultResult => native_functional_value_slot(destination, None, context),
        ArgumentInput(index) => {
            if let Some((params, _)) = lambda {
                let param = params.get(index)?;
                let ty = param.ty.clone()?;
                Some(NativeCollectionStorageSlot {
                    root: NativeCollectionStorageRoot::Binding(param.binding.clone()?),
                    path: vec![],
                    ty: ty.clone(),
                    root_ty: ty,
                })
            } else {
                native_functional_value_slot(argument.as_ref()?, Some(index), context)
            }
        }
        ArgumentResult => native_functional_value_slot(argument.as_ref()?, None, context),
    };
    for (from, to) in contract.value_transfers() {
        if from == ArgumentResult
            && let Some((_, body)) = lambda
        {
            facts.callback_value_domains.insert(
                SourceLambdaBodyKey::for_lambda(body).domain_key(),
                NativeCallbackValueDomains {
                    result: endpoint(to),
                },
            );
        } else if let (Some(source), Some(target)) = (endpoint(from), endpoint(to)) {
            record_functional_value_domain_transfer(source, target, facts, &storage_types, context);
        }
    }
    // These are independent callback objects retained by a fresh factory
    // result, not aliases of that result or of each other's generic children.
    for captured in std::iter::once(receiver).chain(argument) {
        facts
            .native_callback_environment_domains
            .push((captured, destination.clone()));
    }
    if let Some((params, body)) = lambda {
        record_native_lambda_environment(params, body, destination, facts, &storage_types, context);
    }
    true
}

/// Source SAM declarations own their invocation ABI, including nullable
/// results. Connect typed bodies to those ordinary callable roots rather than
/// duplicating the null bit inside an instantiated generic type argument.
fn record_source_callback_value_storage(
    expr: &Expr,
    ty: &JavaType,
    destination: &NativeCollectionStorageSlot,
    facts: &mut MethodUsageFacts,
    owner: &str,
    types: &TypeEnv,
    context: &UsageAnalysisContext<'_>,
) -> bool {
    let Some(sam) = source_functional_call(ty, context) else {
        return false;
    };
    let argument = |index: usize| {
        sam.arg_target_types
            .get(index)
            .map(|ty| NativeCollectionStorageSlot {
                root: NativeCollectionStorageRoot::MethodArgument {
                    call: sam.clone(),
                    index,
                },
                path: vec![],
                ty: ty.clone(),
                root_ty: ty.clone(),
            })
    };
    let result = NativeCollectionStorageSlot {
        root: NativeCollectionStorageRoot::Origin(CollectionArgumentOrigin::MethodReturn(
            sam.clone(),
        )),
        path: vec![],
        ty: sam.return_type.clone(),
        root_ty: sam.return_type.clone(),
    };
    match expr {
        Expr::Lambda { params, body } if params.len() == sam.arg_target_types.len() => {
            record_native_lambda_environment(params, body, destination, facts, types, context);
            for (index, param) in params.iter().enumerate() {
                let (Some(binding), Some(source)) = (&param.binding, argument(index)) else {
                    continue;
                };
                let local = NativeCollectionStorageSlot {
                    root: NativeCollectionStorageRoot::Binding(binding.clone()),
                    path: vec![],
                    ty: source.ty.clone(),
                    root_ty: source.ty.clone(),
                };
                record_functional_value_domain_transfer(
                    source.clone(),
                    local.clone(),
                    facts,
                    types,
                    context,
                );
                record_functional_value_domain_transfer(local, source, facts, types, context);
            }
            facts.callback_value_domains.insert(
                SourceLambdaBodyKey::for_lambda(body).domain_key(),
                NativeCallbackValueDomains {
                    result: Some(result),
                },
            );
            true
        }
        Expr::MethodReference {
            target,
            method,
            receiver,
        } if method != "new" => {
            let storage_types = native_storage_type_env(&[expr], facts, owner, types, context);
            let Some(symbols) = context.source_symbols.as_ref() else {
                return false;
            };
            let Some(call) = crate::resolve::resolve_source_method_reference_with_receiver(
                target,
                receiver.as_deref(),
                method,
                &sam.arg_target_types,
                owner,
                symbols,
                &storage_types,
                context.facts,
            ) else {
                return false;
            };
            let offset = sam
                .arg_target_types
                .len()
                .saturating_sub(call.arg_target_types.len());
            for (index, ty) in call.arg_target_types.iter().enumerate() {
                let Some(source) = argument(index + offset) else {
                    continue;
                };
                let target = NativeCollectionStorageSlot {
                    root: NativeCollectionStorageRoot::MethodArgument {
                        call: call.clone(),
                        index,
                    },
                    path: vec![],
                    ty: ty.clone(),
                    root_ty: ty.clone(),
                };
                record_functional_value_domain_transfer(
                    source,
                    target,
                    facts,
                    &storage_types,
                    context,
                );
            }
            let source = NativeCollectionStorageSlot {
                root: NativeCollectionStorageRoot::Origin(CollectionArgumentOrigin::MethodReturn(
                    call.clone(),
                )),
                path: vec![],
                ty: call.return_type.clone(),
                root_ty: call.return_type,
            };
            record_functional_value_domain_transfer(source, result, facts, &storage_types, context);
            true
        }
        _ => false,
    }
}

fn record_native_value_domain_storage(
    expr: &Expr,
    destination: NativeCollectionStorageSlot,
    facts: &mut MethodUsageFacts,
    owner: &str,
    types: &TypeEnv,
    context: &UsageAnalysisContext<'_>,
) {
    if expr_is_null_literal(expr)
        && !matches!(destination.ty, JavaType::Primitive(_) | JavaType::Void)
    {
        let mut nullable = destination;
        nullable.ty = crate::types::optional_java_type(nullable.ty);
        facts
            .nullable_collection_storage_flows
            .push((nullable.clone(), nullable));
        return;
    }
    if let Expr::Cast { ty, expr: inner } = expr
        && record_source_callback_value_storage(
            inner,
            ty,
            &destination,
            facts,
            owner,
            types,
            context,
        )
    {
        return;
    }
    if matches!(expr, Expr::Lambda { .. })
        && record_source_callback_value_storage(
            expr,
            &destination.ty,
            &destination,
            facts,
            owner,
            types,
            context,
        )
    {
        return;
    }
    if let Expr::Binary {
        op: BinaryOp::Assign,
        right,
        ..
    } = expr
    {
        // Assignment yields the converted RHS reference, also stored by the
        // ordinary assignment rule. It does not manufacture a new allocation.
        record_native_value_domain_storage(right, destination, facts, owner, types, context);
        return;
    }
    if let Expr::Ternary {
        then_expr,
        else_expr,
        ..
    } = expr
    {
        // Each arm can forward a local allocation or construct a fresh value.
        // Apply the same value-transfer rules as a direct return/assignment;
        // merely recording calls misses the named-local arm of the join.
        record_native_value_domain_storage(
            then_expr,
            destination.clone(),
            facts,
            owner,
            types,
            context,
        );
        record_native_value_domain_storage(else_expr, destination, facts, owner, types, context);
        return;
    }
    if let Expr::ArrayCreation {
        initializer: Some(values),
        ..
    }
    | Expr::ArrayInitializer(values) = expr
    {
        // A fresh array has distinct outer identity but retains each element's
        // reference. Project those elements through the existing value graph.
        if let Some(child) = destination.child(&[0]) {
            for value in values {
                record_native_value_domain_storage(
                    value,
                    child.clone(),
                    facts,
                    owner,
                    types,
                    context,
                );
            }
        }
        return;
    }
    let effective = crate::effective_facts::EffectiveFacts::new(context.facts);
    if record_native_generic_composition_storage(expr, &destination, facts, owner, types, context) {
        return;
    }
    if let Expr::MethodReference {
        target,
        method,
        receiver,
    } = expr
        && let Some(inputs) = functional_value_input_types(&destination.ty, context)
        && let Some(symbols) = context.source_symbols.as_ref()
    {
        let storage_types = native_storage_type_env(&[expr], facts, owner, types, context);
        let source_sam = source_functional_call(&destination.ty, context);
        if method == "new"
            && let Some(call) = symbols.constructor_reference_with_input_types(
                target,
                &inputs,
                owner,
                &storage_types,
                context.facts,
            )
        {
            for (index, param) in call.params.iter().enumerate() {
                for (source, path) in
                    functional_value_domain_slots(&destination, Some(index), context)
                {
                    let target = NativeCollectionStorageSlot {
                        root: NativeCollectionStorageRoot::ConstructorArgument {
                            call: call.clone(),
                            index,
                        },
                        path: vec![],
                        ty: param.ty.clone(),
                        root_ty: param.ty.clone(),
                    };
                    if let Some(sam) = &source_sam {
                        let sam_index = index;
                        if let Some(ty) = sam.arg_target_types.get(sam_index) {
                            facts.nullable_collection_storage_flows.push((
                                NativeCollectionStorageSlot {
                                    root: NativeCollectionStorageRoot::MethodArgument {
                                        call: sam.clone(),
                                        index: sam_index,
                                    },
                                    path: vec![],
                                    ty: ty.clone(),
                                    root_ty: ty.clone(),
                                },
                                target.clone(),
                            ));
                        }
                    }
                    let Some(target) = target.child(&path) else {
                        continue;
                    };
                    record_functional_projected_transfer(
                        source,
                        target,
                        &destination.ty,
                        facts,
                        &storage_types,
                        context,
                    );
                }
            }
            return;
        }
        if let Some(call) = crate::resolve::resolve_source_method_reference_with_receiver(
            target,
            receiver.as_deref(),
            method,
            &inputs,
            owner,
            symbols,
            &storage_types,
            context.facts,
        ) {
            // Source references and source-to-native SAM adapters share the
            // same invocation domains. Unbound instance references consume
            // their first SAM input as a receiver rather than a parameter.
            let offset = inputs.len().saturating_sub(call.arg_target_types.len());
            for (index, ty) in call.arg_target_types.iter().enumerate() {
                for (source, path) in
                    functional_value_domain_slots(&destination, Some(index + offset), context)
                {
                    let target = NativeCollectionStorageSlot {
                        root: NativeCollectionStorageRoot::MethodArgument {
                            call: call.clone(),
                            index,
                        },
                        path: vec![],
                        ty: ty.clone(),
                        root_ty: ty.clone(),
                    };
                    if let Some(sam) = &source_sam {
                        let sam_index = index + offset;
                        if let Some(ty) = sam.arg_target_types.get(sam_index) {
                            facts.nullable_collection_storage_flows.push((
                                NativeCollectionStorageSlot {
                                    root: NativeCollectionStorageRoot::MethodArgument {
                                        call: sam.clone(),
                                        index: sam_index,
                                    },
                                    path: vec![],
                                    ty: ty.clone(),
                                    root_ty: ty.clone(),
                                },
                                target.clone(),
                            ));
                        }
                    }
                    let Some(target) = target.child(&path) else {
                        continue;
                    };
                    record_functional_projected_transfer(
                        source,
                        target,
                        &destination.ty,
                        facts,
                        &storage_types,
                        context,
                    );
                }
            }
            for (target, path) in functional_value_domain_slots(&destination, None, context) {
                let ty = call.return_type.clone();
                let source = NativeCollectionStorageSlot {
                    root: NativeCollectionStorageRoot::Origin(
                        CollectionArgumentOrigin::MethodReturn(call.clone()),
                    ),
                    path: vec![],
                    ty: ty.clone(),
                    root_ty: ty,
                };
                if let Some(sam) = &source_sam {
                    facts.nullable_collection_storage_flows.push((
                        source.clone(),
                        NativeCollectionStorageSlot {
                            root: NativeCollectionStorageRoot::Origin(
                                CollectionArgumentOrigin::MethodReturn(sam.clone()),
                            ),
                            path: vec![],
                            ty: sam.return_type.clone(),
                            root_ty: sam.return_type.clone(),
                        },
                    ));
                }
                let Some(source) = source.child(&path) else {
                    continue;
                };
                record_functional_projected_transfer(
                    source,
                    target,
                    &destination.ty,
                    facts,
                    &storage_types,
                    context,
                );
            }
            return;
        }
    }
    if let Expr::Call {
        receiver: Some(receiver),
        name,
        args,
        ..
    } = expr
        && let JavaType::Named { name: target, .. } =
            crate::types::optional_element_type(&destination.ty)
                .unwrap_or_else(|| destination.ty.clone())
        && effective
            .native_functional_carrier(&destination.ty)
            .is_some()
        && crate::stdlib::NativeFunctionalComposition::fixed_for_call(&target, name, args.len())
            .is_some()
    {
        let storage_types = native_storage_type_env(&[expr], facts, owner, types, context);
        for operand in [receiver.as_ref(), &args[0]] {
            if let Some(source) =
                native_collection_storage_slot(operand, facts, owner, &storage_types, context)
            {
                facts
                    .native_callback_capture_domains
                    .push((source, destination.clone()));
            }
        }
        return;
    }
    if let Expr::New {
        ty,
        args,
        body: None,
        ..
    } = expr
        && args.len() == 2
        && effective.native_map_entry_constructor(ty)
        && effective
            .native_map_entry_type_args(&destination.ty)
            .is_some()
    {
        // Entry construction retains each Java component reference. Feed
        // these transfers into the same storage graph as native factories.
        for (index, arg) in args.iter().enumerate() {
            let Some(child) = crate::slot_shape::RustSlotShape::native_child_type_at_path(
                &destination.ty,
                &[index],
            ) else {
                continue;
            };
            let mut target = destination.clone();
            target.path.push(index);
            target.ty = child;
            record_native_value_domain_storage(arg, target, facts, owner, types, context);
        }
        return;
    }
    if let Expr::New { ty, body: None, .. } = expr
        && crate::effective_facts::EffectiveFacts::new(context.facts)
            .native_collection_requires_runtime_policy(ty)
        && native_storage_value_type(&destination.ty, types, context)
        && !facts
            .native_collection_retention_requirements
            .contains(&destination)
    {
        // The factory's policy survives even a Collection-typed destination.
        // Seed the existing unary storage graph, not a new variable-name rule.
        facts
            .native_collection_retention_requirements
            .push(destination.clone());
    }
    if let Some(value) =
        crate::call_plan::checked_identity_source_argument(expr, types, context.facts)
        && native_storage_value_type(&destination.ty, types, context)
    {
        let storage_types = native_storage_type_env(&[value], facts, owner, types, context);
        if let Some(source) =
            native_collection_storage_slot(value, facts, owner, &storage_types, context)
        {
            // A stored/returned checked identity is an alias, never a copy.
            // Do not connect outer nullable domains: success proves non-null.
            facts
                .native_collection_retention_requirements
                .push(source.clone());
            facts
                .native_collection_retention_requirements
                .push(destination.clone());
            facts
                .native_collection_storage_flows
                .push((source.clone(), destination.clone()));
            for index in 0..2 {
                if let (Some(source_ty), Some(target_ty)) = (
                    crate::slot_shape::RustSlotShape::native_child_type_at_path(
                        &source.ty,
                        &[index],
                    ),
                    crate::slot_shape::RustSlotShape::native_child_type_at_path(
                        &destination.ty,
                        &[index],
                    ),
                ) {
                    let mut child = source.clone();
                    child.path.push(index);
                    child.ty = source_ty;
                    let mut target = destination.clone();
                    target.path.push(index);
                    target.ty = target_ty;
                    facts
                        .nullable_collection_storage_flows
                        .push((child.clone(), target.clone()));
                    facts
                        .nullable_collection_storage_flows
                        .push((target, child));
                }
            }
        }
        return;
    }
    if let Expr::New {
        ty,
        args,
        body: None,
        ..
    } = expr
        && let [source] = args.as_slice()
        && crate::effective_facts::EffectiveFacts::new(context.facts)
            .retained_native_collection_kind(ty)
            .is_some()
    {
        let storage_types = native_storage_type_env(&[source], facts, owner, types, context);
        if matches!(
            crate::call_plan::plan_native_collection_constructor_for_args(
                ty,
                args,
                &storage_types,
                context.facts,
            ),
            Some(crate::call_plan::NativeCollectionNewTarget::FromSource { .. })
        ) && let Some(mut source) =
            native_collection_storage_slot(source, facts, owner, &storage_types, context)
            && let Some(child) =
                crate::slot_shape::RustSlotShape::native_child_type_at_path(&source.ty, &[0])
            && crate::slot_shape::RustSlotShape::native_child_type_at_path(&destination.ty, &[0])
                .is_some()
        {
            let mut target = destination;
            source.path.push(0);
            source.ty = child.clone();
            target.path.push(0);
            target.ty = child.clone();
            // A copy preserves child values/references, never outer identity
            // or writes from the new collection back into its input.
            facts
                .nullable_collection_storage_flows
                .push((source.clone(), target.clone()));
            if native_storage_value_type(&child, &storage_types, context) {
                facts.native_collection_storage_flows.push((source, target));
            }
            return;
        }
    }
    if let Expr::Call {
        receiver: Some(receiver),
        name,
        args,
        ..
    } = expr
        && matches!(&destination.ty, JavaType::Named { name, args }
            if args.len() == 1 && matches!(crate::stdlib::canonical_external_receiver_type_key(name).as_str(), "ListIterator" | "java.util.ListIterator"))
    {
        let storage_types = native_storage_type_env(&[receiver], facts, owner, types, context);
        if native_storage_method(receiver, name, args, &storage_types, context)
            == Some(crate::call_plan::NativeCollectionMethod::ListIterator)
            && let Some(mut source) =
                native_collection_storage_slot(receiver, facts, owner, &storage_types, context)
            && let Some(child) =
                crate::slot_shape::RustSlotShape::native_child_type_at_path(&source.ty, &[0])
        {
            facts
                .native_collection_retention_requirements
                .push(source.clone());
            source.path.push(0);
            source.ty = child.clone();
            let mut cursor = destination;
            cursor.path.push(0);
            cursor.ty = child.clone();
            // next/previous read this storage; set/add write the same Java
            // references back. Only child domains join, never cursor roots.
            facts
                .nullable_collection_storage_flows
                .push((source.clone(), cursor.clone()));
            facts
                .nullable_collection_storage_flows
                .push((cursor.clone(), source.clone()));
            if native_storage_value_type(&child, &storage_types, context) {
                facts.native_collection_storage_flows.push((source, cursor));
            }
            return;
        }
    }
    if let Expr::Call {
        receiver: Some(receiver),
        name,
        args,
        resolved,
        ..
    } = expr
    {
        let storage_types = native_storage_type_env(&[receiver], facts, owner, types, context);
        let receiver_ty = native_storage_expr_type(receiver, facts, owner, &storage_types, context);
        if functional_value_invocation_matches(
            &receiver_ty,
            name,
            resolved.as_ref(),
            args.len(),
            context,
        ) && let Some(receiver_slot) =
            native_collection_storage_slot(receiver, facts, owner, &storage_types, context)
        {
            let domains = functional_value_domain_slots(&receiver_slot, None, context);
            if !domains.is_empty() {
                for (source, path) in domains {
                    if let Some(target) = destination.child(&path) {
                        record_functional_projected_transfer(
                            source,
                            target,
                            &receiver_ty,
                            facts,
                            &storage_types,
                            context,
                        );
                    }
                }
                return;
            }
        }
        let effective = crate::effective_facts::EffectiveFacts::new(context.facts);
        let applied_cursor = effective
            .native_functional_carrier(&receiver_ty)
            .filter(|sam| sam.method() == name && sam.inputs().len() == args.len())
            .and_then(|sam| sam.returns())
            .and_then(|result| match result {
                crate::stdlib::FunctionalTypeSource::AppliedUnary { argument, .. } => {
                    Some(argument)
                }
                _ => None,
            });
        let cursor_child = applied_cursor.or_else(|| {
            (native_storage_method(receiver, name, args, &storage_types, context)
                == Some(crate::call_plan::NativeCollectionMethod::Iterator))
            .then_some(0)
        });
        if effective
            .native_cursor_element_type(&destination.ty)
            .is_some()
            && let Some(child_index) = cursor_child
            && let Some(mut source) =
                native_collection_storage_slot(receiver, facts, owner, &storage_types, context)
            && let Some(element) = crate::slot_shape::RustSlotShape::native_child_type_at_path(
                &source.ty,
                &[child_index],
            )
            && let Some(target_element) =
                crate::slot_shape::RustSlotShape::native_child_type_at_path(&destination.ty, &[0])
        {
            // Read the producer's qualified elements without equating the
            // cursor facade's root with the backing collection allocation.
            source.path.push(child_index);
            source.ty = element;
            let mut target = destination;
            target.path.push(0);
            target.ty = target_element;
            facts
                .nullable_collection_storage_flows
                .push((source, target));
            return;
        }
        if args.is_empty()
            && effective.retained_native_map_kind(&receiver_ty).is_some()
            && effective
                .retained_native_collection_kind(&destination.ty)
                .is_some()
            && native_storage_method_for_type(
                receiver,
                name,
                args,
                &receiver_ty,
                &storage_types,
                context,
            )
            .is_some_and(|method| {
                matches!(
                    method,
                    crate::call_plan::NativeCollectionMethod::MapKeys
                        | crate::call_plan::NativeCollectionMethod::MapValues
                )
            })
            && let Some(source) =
                native_collection_storage_slot(receiver, facts, owner, &storage_types, context)
        {
            let child = if matches!(name.as_str(), "keySet" | "key_set") {
                0
            } else {
                1
            };
            // The view owns a backing reference, but is a distinct object.
            // Root placement propagates separately from the key-only child
            // mapping; map values must never become view element constraints.
            facts.native_collection_view_backings.push((
                source.clone(),
                destination.clone(),
                child,
            ));
            if let (Some(key), Some(element)) = (
                crate::slot_shape::RustSlotShape::native_child_type_at_path(&source.ty, &[child]),
                crate::slot_shape::RustSlotShape::native_child_type_at_path(&destination.ty, &[0]),
            ) {
                let mut key_slot = source;
                key_slot.path.push(child);
                key_slot.ty = key.clone();
                let mut view_slot = destination;
                view_slot.path.push(0);
                view_slot.ty = element;
                // Map keys have a qualified nullability domain, unlike the
                // legacy sequence-element domain (which denotes map values).
                // The view-backing relation transfers that domain explicitly.
                if native_storage_value_type(&key, &storage_types, context) {
                    facts
                        .native_collection_storage_flows
                        .push((key_slot, view_slot));
                }
            }
            return;
        }
    }
    if matches!(expr, Expr::Call { .. })
        && (native_storage_value_type(&destination.ty, types, context)
            || crate::effective_facts::EffectiveFacts::new(context.facts)
                .native_iterable_element_type(&destination.ty)
                .is_some())
    {
        // Every value-transfer boundary, including inline method/constructor
        // arguments, must establish the same producer's storage contract.
        // Keep declaration/store/return handling here too, avoiding duplicate
        // constraints and a separate factory rule for each consumer kind.
        let storage_types = native_storage_type_env(&[expr], facts, owner, types, context);
        record_native_factory_storage(
            expr,
            destination.clone(),
            facts,
            owner,
            &storage_types,
            context,
        );
    }
    if let Expr::Call {
        receiver: Some(receiver),
        name,
        args,
        ..
    } = expr
        && name == "clone"
        && args.is_empty()
    {
        let storage_types = native_storage_type_env(&[receiver], facts, owner, types, context);
        let receiver_ty = native_storage_expr_type(receiver, facts, owner, &storage_types, context);
        let receiver_ty = crate::types::optional_element_type(&receiver_ty).unwrap_or(receiver_ty);
        if matches!(receiver_ty, JavaType::Array(_))
            && let Some(mut source) =
                native_collection_storage_slot(receiver, facts, owner, &storage_types, context)
            && let Some(element) =
                crate::slot_shape::RustSlotShape::native_child_type_at_path(&source.ty, &[0])
            && let Some(target_element) =
                crate::slot_shape::RustSlotShape::native_child_type_at_path(&destination.ty, &[0])
        {
            let mut target = destination;
            source.path.push(0);
            source.ty = element.clone();
            target.path.push(0);
            target.ty = target_element;
            // Java clone creates a distinct outer array but retains every
            // child reference. Never join the two outer allocation roots.
            facts
                .nullable_collection_storage_flows
                .push((source.clone(), target.clone()));
            if native_storage_value_type(&element, &storage_types, context) {
                facts.native_collection_storage_flows.push((source, target));
            }
            return;
        }
    }
    if matches!(expr, Expr::Call { name, args, .. } if name == "collect" && args.len() == 1)
        && native_storage_value_type(&destination.ty, types, context)
    {
        let storage_types = native_storage_type_env(&[expr], facts, owner, types, context);
        if let Some(stream) = crate::stream::build_stream_plan(expr, &storage_types, context.facts)
            && matches!(stream.terminal(), crate::stream::StreamTerminal::Collect(_))
            && stream.forwards_source_reference_domain()
            && let Some(mut source) = native_collection_storage_slot(
                stream.source().expr(),
                facts,
                owner,
                &storage_types,
                context,
            )
            && let Some(element) =
                crate::slot_shape::RustSlotShape::native_child_type_at_path(&source.ty, &[0])
            && let Some(target_element) =
                crate::slot_shape::RustSlotShape::native_child_type_at_path(&destination.ty, &[0])
        {
            let mut target = destination;
            source.path.push(0);
            source.ty = element.clone();
            target.path.push(0);
            target.ty = target_element;
            // Collect creates a different outer allocation. Only values flow
            // forward; nullable writes to the copy must not widen the input.
            facts
                .nullable_collection_storage_flows
                .push((source.clone(), target.clone()));
            if native_storage_value_type(&element, &storage_types, context) {
                // Copied child references still denote their original Java
                // allocations; never make an outer allocation alias here.
                facts
                    .native_collection_storage_flows
                    .push((source.clone(), target.clone()));
                facts.native_collection_storage_flows.push((target, source));
            }
            return;
        }
    }
    if let Expr::Cast { ty, expr: source } = expr {
        let storage_types = native_storage_type_env(&[source], facts, owner, types, context);
        let actual = native_storage_expr_type(source, facts, owner, &storage_types, context);
        let effective = crate::effective_facts::EffectiveFacts::new(context.facts);
        if let Some(element) = effective.native_iterable_list_projection_element(&actual, ty)
            && effective
                .native_iterable_list_projection_element(&actual, &destination.ty)
                .is_some()
            && let Some(mut source) =
                native_collection_storage_slot(source, facts, owner, &storage_types, context)
        {
            facts.native_list_projection_slots.push(destination.clone());
            // A reference cast preserves Java null without equating the
            // Iterable and List view's ownership/dispatch/storage contracts.
            facts
                .nullable_collection_storage_flows
                .push((source.clone(), destination.clone()));
            let mut target = destination;
            source.path.push(0);
            source.ty = element.clone();
            target.path.push(0);
            target.ty = element.clone();
            // The view and its receiver expose the same element references,
            // but not the same physical outer Rust carrier. Do not lift the
            // Iterable itself to a native allocation handle.
            facts
                .nullable_collection_storage_flows
                .push((source.clone(), target.clone()));
            facts
                .nullable_collection_storage_flows
                .push((target.clone(), source.clone()));
            if native_storage_value_type(&element, &storage_types, context) {
                facts
                    .native_collection_storage_flows
                    .push((source.clone(), target.clone()));
                facts.native_collection_storage_flows.push((target, source));
            }
            return;
        }
    }
    // Specialized projections above already record the same root value flow
    // alongside their child/view relationships. Record ordinary reference
    // casts only after those handlers to keep one canonical edge per flow.
    record_reference_cast_value_domain(expr, &destination, facts, owner, types, context);
    if !matches!(expr, Expr::Lambda { .. } | Expr::MethodReference { .. })
        && let Some(sam) = crate::effective_facts::EffectiveFacts::new(context.facts)
            .native_functional_carrier(&destination.ty)
        && let Some(symbols) = &context.source_symbols
    {
        let storage_types = native_storage_type_env(&[expr], facts, owner, types, context);
        let actual = native_storage_expr_type(expr, facts, owner, &storage_types, context);
        if usage_java_type_name(&actual).is_some_and(|name| {
            crate::call_plan::java_type_owner_is_parsed_source(name, &storage_types, context.facts)
        }) {
            if let Expr::Var(name) = expr {
                // The native facade retains this source receiver after the
                // conversion returns. Reuse canonical escaping-storage facts
                // so a parameter carries its Java handle, not &payload.
                record_use(facts, name, BindingUse::RetainedByCall);
            }
            // The selected source method and the native facade share the
            // invocation's input/result domains. Nominal implements arguments
            // alone do not preserve retained arrays or nullable results.
            let mut receiver_types = storage_types.clone();
            receiver_types.insert_local("__java_usage_sam_receiver", actual);
            let target_ty = crate::types::optional_element_type(&destination.ty)
                .unwrap_or_else(|| destination.ty.clone());
            if let Some(target_name) = usage_java_type_name(&target_ty) {
                for name in ["andThen", "compose"] {
                    if crate::stdlib::NativeFunctionalComposition::fixed_for_call(
                        target_name,
                        name,
                        1,
                    )
                    .is_none()
                    {
                        continue;
                    }
                    let Some(call) =
                        crate::resolve::resolve_source_method_reference_with_input_types(
                            "__java_usage_sam_receiver",
                            name,
                            &[target_ty.clone()],
                            owner,
                            symbols,
                            &receiver_types,
                            context.facts,
                        )
                    else {
                        continue;
                    };
                    if !crate::call_plan::java_type_owner_is_parsed_source(
                        &call.declaring_type,
                        &receiver_types,
                        context.facts,
                    ) {
                        continue;
                    }
                    let ty = call.arg_target_types[0].clone();
                    let parameter = NativeCollectionStorageSlot {
                        root: NativeCollectionStorageRoot::MethodArgument {
                            call: call.clone(),
                            index: 0,
                        },
                        path: vec![],
                        ty: ty.clone(),
                        root_ty: ty,
                    };
                    let mut nullable = parameter.clone();
                    nullable.ty = crate::types::optional_java_type(nullable.ty);
                    facts
                        .nullable_collection_storage_flows
                        .push((nullable.clone(), nullable));
                    facts
                        .native_callback_capture_domains
                        .push((parameter, destination.clone()));
                    let result = NativeCollectionStorageSlot {
                        root: NativeCollectionStorageRoot::Origin(
                            CollectionArgumentOrigin::MethodReturn(call.clone()),
                        ),
                        path: vec![],
                        ty: call.return_type.clone(),
                        root_ty: call.return_type,
                    };
                    facts
                        .native_callback_capture_domains
                        .push((result, destination.clone()));
                }
            }
            if let JavaType::Named { args, .. } = target_ty
                && let Some(inputs) = sam.input_types(&args)
                && let Some(call) = crate::resolve::resolve_source_method_reference_with_input_types(
                    "__java_usage_sam_receiver",
                    sam.method(),
                    &inputs,
                    owner,
                    symbols,
                    &receiver_types,
                    context.facts,
                )
            {
                for (parameter, input) in sam.inputs().iter().enumerate() {
                    let Some((index, path)) = input.type_argument_projection() else {
                        continue;
                    };
                    let Some(ty) = call.arg_target_types.get(parameter) else {
                        continue;
                    };
                    let parameter = NativeCollectionStorageSlot {
                        root: NativeCollectionStorageRoot::MethodArgument {
                            call: call.clone(),
                            index: parameter,
                        },
                        path: vec![],
                        ty: ty.clone(),
                        root_ty: ty.clone(),
                    };
                    if let Some(source) = destination.child(&[index])
                        && let Some(target) = parameter.child(path)
                    {
                        record_functional_value_domain_transfer(
                            source,
                            target,
                            facts,
                            &storage_types,
                            context,
                        );
                    }
                }
                if let Some((index, path)) = sam
                    .returns()
                    .and_then(|result| result.type_argument_projection())
                {
                    let result = NativeCollectionStorageSlot {
                        root: NativeCollectionStorageRoot::Origin(
                            CollectionArgumentOrigin::MethodReturn(call.clone()),
                        ),
                        path: vec![],
                        ty: call.return_type.clone(),
                        root_ty: call.return_type.clone(),
                    };
                    if let Some(source) = result.child(path)
                        && let Some(target) = destination.child(&[index])
                    {
                        record_functional_value_domain_transfer(
                            source,
                            target,
                            facts,
                            &storage_types,
                            context,
                        );
                    }
                }
                return;
            }
        }
    }
    let Expr::Lambda { body, params } = expr else {
        if !has_projectable_native_value_domain(expr) {
            return;
        }
        let effective = crate::effective_facts::EffectiveFacts::new(context.facts);
        // Ordinary reference presence also flows through source classes and
        // generic SAM results. Native allocation handling below still checks
        // eligibility separately; this gate only excludes scalar values.
        if matches!(destination.ty, JavaType::Primitive(_) | JavaType::Void) {
            return;
        }
        let storage_types = native_storage_type_env(&[expr], facts, owner, types, context);
        if let Some(mut source) =
            native_collection_storage_slot(expr, facts, owner, &storage_types, context)
        {
            if effective.native_cursor_element_type(&source.ty).is_some()
                && effective
                    .native_cursor_element_type(&destination.ty)
                    .is_some()
                && let Some(element) =
                    crate::slot_shape::RustSlotShape::native_child_type_at_path(&source.ty, &[0])
                && let Some(target_element) =
                    crate::slot_shape::RustSlotShape::native_child_type_at_path(
                        &destination.ty,
                        &[0],
                    )
            {
                source.path.push(0);
                source.ty = element;
                let mut target = destination;
                target.path.push(0);
                target.ty = target_element;
                facts
                    .nullable_collection_storage_flows
                    .push((source, target));
                return;
            }
            if effective.native_functional_carrier(&source.ty).is_some()
                && effective
                    .native_functional_carrier(&destination.ty)
                    .is_some()
            {
                // Native SAM facades already clone a canonical callback
                // reference. Propagate its local/synchronized ABI without
                // adding a container-style RefCell/Mutex allocation shell.
                facts
                    .nullable_collection_storage_flows
                    .push((source.clone(), destination.clone()));
                facts
                    .native_collection_reference_transfers
                    .push((source.clone(), destination.clone()));
            }
            if let Some(source_functional) = effective.native_functional_carrier(&source.ty)
                && let Some(target_functional) =
                    effective.native_functional_carrier(&destination.ty)
                && source_functional.inputs().len() == target_functional.inputs().len()
            {
                // Parent views can reorder generic arguments (UnaryOperator<T>
                // becomes Function<T,T>). Connect invocation domains by SAM
                // position, never by coincident generic child indices. Inputs
                // flow into the original callback, results flow out of it.
                let inputs = target_functional
                    .inputs()
                    .iter()
                    .copied()
                    .zip(source_functional.inputs().iter().copied())
                    .map(|(input, output)| (&destination, &source, input, output));
                let result = source_functional
                    .returns()
                    .zip(target_functional.returns())
                    .map(|(input, output)| (&source, &destination, input, output));
                for (from, to, input, output) in inputs.chain(result) {
                    let (Some((input_index, _)), Some((output_index, _))) = (
                        input.type_argument_projection(),
                        output.type_argument_projection(),
                    ) else {
                        continue;
                    };
                    if let Some(from) = from.child(&[input_index])
                        && let Some(to) = to.child(&[output_index])
                    {
                        record_functional_value_domain_transfer(
                            from,
                            to,
                            facts,
                            &storage_types,
                            context,
                        );
                    }
                }
                return;
            }
            let child = |ty: &JavaType| {
                crate::types::sequence_element_type(ty)
                    .or_else(|| effective.native_iterable_element_type(ty))
            };
            if source.path.is_empty()
                && (effective.native_iterable_element_type(&source.ty).is_some()
                    || effective
                        .native_iterable_element_type(&destination.ty)
                        .is_some())
                && let (Some(source_element), Some(target_element)) =
                    (child(&source.ty), child(&destination.ty))
                && source_element == target_element
            {
                // The view preserves presence of its Java receiver as well
                // as the shared child storage domain. Nullable producers must
                // not become required merely by being exposed as Iterable.
                facts
                    .nullable_collection_storage_flows
                    .push((source.clone(), destination.clone()));
                let mut target = destination;
                source.path.push(0);
                source.ty = source_element;
                target.path.push(0);
                target.ty = target_element;
                facts
                    .nullable_collection_storage_flows
                    .push((source.clone(), target.clone()));
                facts
                    .nullable_collection_storage_flows
                    .push((target, source));
                return;
            }
            if source.path.is_empty() {
                // Nullability belongs to every Java reference value, including
                // source-class results of a typed callback. Allocation tier
                // propagation remains restricted to native carriers below.
                if !matches!(source.ty, JavaType::Primitive(_) | JavaType::Void)
                    && !matches!(destination.ty, JavaType::Primitive(_) | JavaType::Void)
                {
                    facts
                        .nullable_collection_storage_flows
                        .push((source.clone(), destination.clone()));
                }
                // Native container references cross source field and callable boundaries
                // without materialization. Keep the whole producer/consumer
                // edge, including a returned array forwarded into a call.
                // Unrelated local-only value transfers still do not imply
                // outer allocation retention. Fresh-copy constructors are
                // handled separately and never alias these outer roots.
                if native_storage_value_type(&source.ty, &storage_types, context)
                    && native_storage_value_type(&destination.ty, &storage_types, context)
                    && (matches!(
                        crate::types::optional_element_type(&source.ty)
                            .unwrap_or_else(|| source.ty.clone()),
                        JavaType::Array(_)
                    ) || [&source.root, &destination.root].iter().any(|root| {
                        matches!(
                            root,
                            NativeCollectionStorageRoot::Origin(
                                CollectionArgumentOrigin::Field { .. }
                                    | CollectionArgumentOrigin::MethodReturn(_)
                            ) | NativeCollectionStorageRoot::MethodArgument { .. }
                                | NativeCollectionStorageRoot::ConstructorArgument { .. }
                                | NativeCollectionStorageRoot::Return
                        ) || matches!(root, NativeCollectionStorageRoot::Binding(binding)
                            if binding.kind() == crate::binding_id::BindingKind::Param)
                    }))
                {
                    if matches!(
                        crate::types::optional_element_type(&source.ty)
                            .unwrap_or_else(|| source.ty.clone()),
                        JavaType::Array(_)
                    ) {
                        facts
                            .native_collection_storage_flows
                            .push((source, destination));
                    } else {
                        let stored_alias = matches!(
                            source.root,
                            NativeCollectionStorageRoot::Origin(
                                CollectionArgumentOrigin::Field { .. }
                            )
                        ) || matches!(
                            destination.root,
                            NativeCollectionStorageRoot::Origin(
                                CollectionArgumentOrigin::Field { .. }
                            )
                        );
                        let borrowed_return = matches!(
                            destination.root,
                            NativeCollectionStorageRoot::Return
                        ) && matches!(&source.root, NativeCollectionStorageRoot::Binding(binding)
                                if binding.kind() == crate::binding_id::BindingKind::Param);
                        if (stored_alias || borrowed_return)
                            && !facts
                                .native_collection_retention_requirements
                                .contains(&source)
                        {
                            // Stored/caller-owned references can outlive this
                            // transfer. A fresh unaliased local may still move
                            // its value carrier when returned; do not retain
                            // every unique result merely because it is named.
                            facts
                                .native_collection_retention_requirements
                                .push(source.clone());
                        }
                        facts
                            .native_collection_reference_transfers
                            .push((source, destination));
                    }
                }
                return;
            }
            // Reading a structural child transfers its value domain to the
            // destination. This is directional, not allocation aliasing.
            if native_storage_value_type(&source.ty, &storage_types, context)
                && native_storage_value_type(&destination.ty, &storage_types, context)
            {
                // A retrieved native container is itself a reference, even
                // when the outer is a value carrier. Retain the child, not
                // the enclosing collection/array allocation.
                facts
                    .native_collection_storage_flows
                    .push((source.clone(), destination.clone()));
            }
            facts
                .nullable_collection_storage_flows
                .push((source, destination));
        }
        return;
    };
    let effective = crate::effective_facts::EffectiveFacts::new(context.facts);
    let Some(functional) = effective.native_functional_carrier(&destination.ty) else {
        return;
    };
    record_native_lambda_environment(params, body, &destination, facts, types, context);
    if params.iter().all(|param| param.binding.is_some()) {
        let target_ty = crate::types::optional_element_type(&destination.ty)
            .unwrap_or_else(|| destination.ty.clone());
        let JavaType::Named { args, .. } = target_ty else {
            return;
        };
        let Some(inputs) = functional.input_types(&args) else {
            return;
        };
        if inputs.len() != params.len() {
            return;
        }
        for ((param, ty), input) in params.iter().zip(inputs).zip(functional.inputs()) {
            let Some((index, path)) = input.type_argument_projection() else {
                continue;
            };
            let parameter = NativeCollectionStorageSlot {
                root: NativeCollectionStorageRoot::Binding(param.binding.clone().unwrap()),
                path: vec![],
                ty: ty.clone(),
                root_ty: ty,
            };
            if let Some(source) = destination.child(&[index])
                && let Some(target) = parameter.child(path)
            {
                // The formal and its mutable local binding share one emitted
                // callback ABI, unlike a source call's independent argument slot.
                facts
                    .nullable_collection_storage_flows
                    .push((target.clone(), source.clone()));
                record_functional_value_domain_transfer(source, target, facts, types, context);
            }
        }
        let result = match functional.returns() {
            Some(crate::stdlib::FunctionalTypeSource::TypeArgument(index)) => {
                destination.child(&[index])
            }
            _ => None,
        };
        facts.callback_value_domains.insert(
            SourceLambdaBodyKey::for_lambda(body).domain_key(),
            NativeCallbackValueDomains { result },
        );
        return;
    }
    let Some(crate::stdlib::FunctionalTypeSource::TypeArgument(index)) = functional.returns()
    else {
        return;
    };
    let target = crate::types::optional_element_type(&destination.ty)
        .unwrap_or_else(|| destination.ty.clone());
    let JavaType::Named { args, .. } = target else {
        return;
    };
    let Some(result) = functional.return_type(&args) else {
        return;
    };
    let Some(inputs) = functional.input_types(&args) else {
        return;
    };
    if inputs.len() != params.len() {
        return;
    }
    let mut lambda_types = types.clone();
    for (param, ty) in params.iter().zip(inputs) {
        lambda_types.insert_local(param.name.clone(), ty.clone());
        lambda_types.insert_local(param.source_name.clone(), ty);
    }
    let mut target = destination;
    target.path.push(index);
    target.ty = result;
    let mut callback_bindings = params
        .iter()
        .flat_map(|param| [param.name.clone(), param.source_name.clone()])
        .collect::<HashSet<_>>();
    if let LambdaBody::Block(block) = body {
        crate::semantic_analysis::walk_block_stmts(block, &mut |stmt| {
            if let Stmt::VarDecl {
                name, source_name, ..
            } = stmt
            {
                callback_bindings.insert(name.clone());
                callback_bindings.insert(source_name.clone());
            }
        });
    }
    let mut record = |source: &Expr| {
        // Record a directional value-domain edge, not an allocation alias.
        // Canonical binding identities keep captures separate from fields or
        // same-named declarations in other scopes.
        // This edge represents a capture of an enclosing canonical slot.
        // Callback-local bindings need their own result-flow identities; do
        // not accidentally resolve one to a same-named enclosing capture.
        let mut callback_local = false;
        crate::semantic_analysis::walk_expr_outside_lambdas(source, &mut |expr| {
            callback_local |= matches!(expr, Expr::Var(name) if callback_bindings.contains(name));
        });
        if callback_local {
            return;
        }
        let storage_types =
            native_storage_type_env(&[source], facts, owner, &lambda_types, context);
        if let Some(source) =
            native_collection_storage_slot(source, facts, owner, &storage_types, context)
        {
            facts
                .nullable_collection_storage_flows
                .push((source.clone(), target.clone()));
            // A native Iterable facade exposes the returned collection's
            // actual element storage, not a copy. Both carriers must agree
            // on descendant nullability even when another caller widens the
            // generic callback parameter. Root presence remains directional.
            if effective.native_iterable_element_type(&target.ty).is_some()
                && (native_storage_value_type(&source.ty, &storage_types, context)
                    || effective.native_iterable_element_type(&source.ty).is_some())
                && let Some(element) =
                    crate::slot_shape::RustSlotShape::native_child_type_at_path(&source.ty, &[0])
                && let Some(target_element) =
                    crate::slot_shape::RustSlotShape::native_child_type_at_path(&target.ty, &[0])
            {
                let mut child = source.clone();
                child.path.push(0);
                child.ty = element;
                let mut target_child = target.clone();
                target_child.path.push(0);
                target_child.ty = target_element;
                facts
                    .nullable_collection_storage_flows
                    .push((child.clone(), target_child.clone()));
                facts
                    .nullable_collection_storage_flows
                    .push((target_child, child));
            }
        }
    };
    match body {
        LambdaBody::Expr(source) => record(source),
        LambdaBody::Block(block) => {
            crate::semantic_analysis::walk_block_stmts(block, &mut |stmt| {
                if let Stmt::Return(Some(source)) = stmt {
                    record(source);
                }
            })
        }
    }
}

fn record_native_factory_storage(
    expr: &Expr,
    destination: NativeCollectionStorageSlot,
    facts: &mut MethodUsageFacts,
    owner: &str,
    types: &TypeEnv,
    context: &UsageAnalysisContext<'_>,
) {
    let Expr::Call {
        receiver,
        name,
        args,
        resolved,
        ..
    } = expr
    else {
        return;
    };
    if let Some(receiver) = receiver.as_deref()
        && matches!(args.as_slice(), [Expr::ArrayCreation { initializer, dimensions, .. }]
            if (initializer.is_none() && dimensions.len() == 1)
                || (initializer.as_ref().is_some_and(Vec::is_empty) && dimensions.is_empty()))
        && matches!(
            native_storage_method(receiver, name, args, types, context),
            Some(crate::call_plan::NativeCollectionMethod::ToArray {
                has_destination: true
            })
        )
        && let Some(source) = native_collection_storage_slot(receiver, facts, owner, types, context)
        && let (Some(source), Some(target)) = (source.child(&[0]), destination.child(&[0]))
    {
        // Like Arrays.copyOf below, toArray creates a fresh outer allocation
        // while transferring element references. Null padding belongs only to
        // the destination domain; never widen the source collection backwards.
        facts
            .nullable_collection_storage_flows
            .push((source.clone(), target.clone()));
        if !matches!(target.ty, JavaType::Primitive(_) | JavaType::Void) {
            let mut nullable = target.clone();
            nullable.ty = crate::types::optional_java_type(nullable.ty);
            facts
                .nullable_collection_storage_flows
                .push((nullable.clone(), nullable));
        }
        if native_storage_value_type(&source.ty, types, context) {
            facts.native_collection_storage_flows.push((source, target));
        }
        return;
    }
    let Some(crate::call_plan::CallTargetPlan::NativeCollectionStatic { method }) =
        crate::call_plan::plan_native_collection_static_call(
            receiver.as_deref(),
            name,
            resolved.as_ref(),
            types,
            context.facts,
        )
    else {
        return;
    };
    if matches!(
        method,
        crate::call_plan::NativeCollectionStaticMethod::ArraysCopyOf
            | crate::call_plan::NativeCollectionStaticMethod::ArraysCopyOfRange
    ) && let Some(array) = args.first()
        && let Some(mut source) =
            native_collection_storage_slot(array, facts, owner, types, context)
        && let Some(child) =
            crate::slot_shape::RustSlotShape::native_child_type_at_path(&source.ty, &[0])
        && crate::slot_shape::RustSlotShape::native_child_type_at_path(&destination.ty, &[0])
            .is_some()
    {
        // Shallow copies own a new outer allocation but retain each child
        // reference. Never connect the roots: writes to one array stay local.
        let mut target = destination;
        source.path.push(0);
        source.ty = child.clone();
        target.path.push(0);
        target.ty = child.clone();
        facts
            .nullable_collection_storage_flows
            .push((source.clone(), target.clone()));
        if native_storage_value_type(&child, types, context) {
            facts.native_collection_storage_flows.push((source, target));
        }
        return;
    }
    if matches!(
        method,
        crate::call_plan::NativeCollectionStaticMethod::CollectionsUnmodifiableList
            | crate::call_plan::NativeCollectionStaticMethod::CollectionsUnmodifiableSet
            | crate::call_plan::NativeCollectionStaticMethod::CollectionsUnmodifiableCollection
    ) && let [source] = args.as_slice()
        && crate::effective_facts::EffectiveFacts::new(context.facts)
            .retained_native_collection_kind(&destination.ty)
            .is_some()
    {
        // A view's policy is observable even if neither reference otherwise
        // escapes. Retain both allocations and share their element domain,
        // not their outer nullability: factory success proves a non-null view.
        facts
            .native_collection_retention_requirements
            .push(destination.clone());
        let storage_types = native_storage_type_env(&[source], facts, owner, types, context);
        if let Some(backing) =
            native_collection_storage_slot(source, facts, owner, &storage_types, context)
        {
            facts
                .native_collection_retention_requirements
                .push(backing.clone());
            facts
                .native_collection_storage_flows
                .push((backing.clone(), destination.clone()));
            if let (Some(source_ty), Some(target_ty)) = (
                crate::slot_shape::RustSlotShape::native_child_type_at_path(&backing.ty, &[0]),
                crate::slot_shape::RustSlotShape::native_child_type_at_path(&destination.ty, &[0]),
            ) {
                let mut child = backing;
                child.path.push(0);
                child.ty = source_ty;
                let mut view = destination;
                view.path.push(0);
                view.ty = target_ty;
                facts
                    .nullable_collection_storage_flows
                    .push((child.clone(), view.clone()));
                facts.nullable_collection_storage_flows.push((view, child));
            }
        }
        return;
    }
    if method == crate::call_plan::NativeCollectionStaticMethod::ArraysAsList
        && let [array] = args.as_slice()
        && let JavaType::Array(element) = crate::types::optional_element_type(
            &native_storage_expr_type(array, facts, owner, types, context),
        )
        .unwrap_or_else(|| native_storage_expr_type(array, facts, owner, types, context))
        && !matches!(element.as_ref(), JavaType::Primitive(_))
    {
        // The view is a distinct fixed-size collection, but its storage must
        // retain the original array allocation. Root placement requirements
        // cross this edge; outer Java-nullability does not (the factory throws
        // on a null array). Element storage is shared in both directions.
        let backing = native_collection_storage_slot(array, facts, owner, types, context);
        if let Some(backing) = &backing {
            facts
                .native_collection_retention_requirements
                .push(backing.clone());
        }
        if crate::stdlib::java_collection_kind_for_type(&destination.ty).is_some() {
            facts
                .native_collection_retention_requirements
                .push(destination.clone());
            if let Some(backing) = &backing {
                facts
                    .native_collection_storage_flows
                    .push((backing.clone(), destination.clone()));
            }
            if let Some(child) =
                crate::slot_shape::RustSlotShape::native_child_type_at_path(&destination.ty, &[0])
            {
                let mut view = destination;
                // Reference arrays have a canonical nullable element carrier
                // even when their initializer contains no Java nulls. Reuse
                // indexed storage projection rather than the nominal T.
                let physical = crate::shape_projection::indexed_assignment_shape_with_facts(
                    &crate::slot_shape::RustSlotShape::for_storage(JavaType::Array(element)),
                    context.facts,
                )
                .map(|shape| shape.java_ty)
                .unwrap_or_else(|| child.clone());
                view.path.push(0);
                view.ty = physical.clone();
                // A fresh inline array has no source storage node, but its
                // canonical element carrier constrains the view just as well.
                facts
                    .nullable_collection_storage_flows
                    .push((view.clone(), view.clone()));
                if let Some(mut backing) = backing {
                    backing.path.push(0);
                    backing.ty = physical;
                    facts
                        .nullable_collection_storage_flows
                        .push((backing.clone(), view.clone()));
                    facts
                        .nullable_collection_storage_flows
                        .push((view, backing));
                }
            }
        }
        return;
    }
    let array_varargs = method == crate::call_plan::NativeCollectionStaticMethod::ArraysAsList;
    if array_varargs && crate::stdlib::java_collection_kind_for_type(&destination.ty).is_some()
        || matches!(
            method,
            crate::call_plan::NativeCollectionStaticMethod::CollectionsEmptyList
                | crate::call_plan::NativeCollectionStaticMethod::CollectionsEmptySet
                | crate::call_plan::NativeCollectionStaticMethod::CollectionsSingletonList
                | crate::call_plan::NativeCollectionStaticMethod::CollectionsSingleton
        )
    {
        // Immutability is observable even for a unique reference. Keep the
        // factory's runtime policy instead of degrading its result to Vec/Set.
        facts
            .native_collection_retention_requirements
            .push(destination.clone());
    }
    let stored_arguments = if array_varargs {
        (0..args.len())
            .map(|argument| (argument, 0))
            .collect::<Vec<_>>()
    } else {
        method.stored_argument_children().to_vec()
    };
    for (argument, child) in stored_arguments {
        let Some(arg) = args.get(argument) else {
            continue;
        };
        let Some(ty) =
            crate::slot_shape::RustSlotShape::native_child_type_at_path(&destination.ty, &[child])
        else {
            continue;
        };
        if !native_storage_value_type(&ty, types, context) {
            continue;
        }
        let mut stored = destination.clone();
        stored.path.push(child);
        stored.ty = ty;
        record_native_factory_storage(arg, stored.clone(), facts, owner, types, context);
        if array_varargs {
            facts
                .native_collection_retention_requirements
                .push(stored.clone());
        }
        if let Some(value) = native_collection_storage_slot(arg, facts, owner, types, context) {
            facts.native_collection_storage_flows.push((stored, value));
        }
    }
}

fn native_collection_storage_slot(
    expr: &Expr,
    facts: &MethodUsageFacts,
    owner: &str,
    types: &TypeEnv,
    context: &UsageAnalysisContext<'_>,
) -> Option<NativeCollectionStorageSlot> {
    if let Expr::Binary {
        op: BinaryOp::Assign,
        left,
        ..
    } = expr
    {
        return native_collection_storage_slot(left, facts, owner, types, context);
    }
    if let Some(value) =
        crate::call_plan::checked_identity_source_argument(expr, types, context.facts)
    {
        return native_collection_storage_slot(value, facts, owner, types, context);
    }
    if let Expr::Call {
        receiver: Some(receiver),
        name,
        args,
        ..
    } = expr
        && args.is_empty()
        && let Expr::Var(entry) = receiver.as_ref()
        && let Some(index) = crate::call_plan::native_map_entry_accessor_index(name)
        && let Some(binding) = facts.visible_binding_id_for_name(entry)
        && let Some(origin) = facts.native_map_entry_backing_storage.get(&binding)
    {
        let mut slot = origin.clone();
        slot.path.push(index);
        slot.ty = crate::types::generic_read_type_preserving_optional(
            &crate::slot_shape::RustSlotShape::native_child_type_at_path(&origin.ty, &[index])?,
        );
        return Some(slot);
    }
    if let Expr::Call {
        receiver: Some(receiver),
        name,
        args,
        ..
    } = expr
        && args.is_empty()
        && let Some(index) = crate::call_plan::native_map_entry_accessor_index(name)
        && crate::effective_facts::EffectiveFacts::new(context.facts)
            .native_map_entry_type_args(&native_storage_expr_type(
                receiver, facts, owner, types, context,
            ))
            .is_some()
    {
        let mut slot = native_collection_storage_slot(receiver, facts, owner, types, context)?;
        slot.ty = crate::types::generic_read_type_preserving_optional(
            &crate::slot_shape::RustSlotShape::native_child_type_at_path(&slot.ty, &[index])?,
        );
        slot.path.push(index);
        return Some(slot);
    }
    let root = match expr {
        Expr::Var(name) => {
            if let Some(binding) = facts.visible_binding_id_for_name(name) {
                return Some(NativeCollectionStorageSlot {
                    root: NativeCollectionStorageRoot::Binding(binding),
                    path: vec![],
                    ty: crate::types::infer_expr_type(expr, types),
                    root_ty: crate::types::infer_expr_type(expr, types),
                });
            }
            let (owner, name, _) = context
                .source_symbols
                .as_ref()?
                .source_field_identity(owner, name)?;
            NativeCollectionStorageRoot::Origin(CollectionArgumentOrigin::Field { owner, name })
        }
        Expr::FieldAccess(receiver, field) => {
            let ty = crate::types::infer_expr_type(receiver, types);
            let ty = crate::types::optional_element_type(&ty).unwrap_or(ty);
            let owner = if matches!(receiver.as_ref(), Expr::This) {
                owner
            } else {
                usage_java_type_name(&ty)?
            };
            let (owner, name, _) = context
                .source_symbols
                .as_ref()?
                .source_field_identity(owner, field)?;
            NativeCollectionStorageRoot::Origin(CollectionArgumentOrigin::Field { owner, name })
        }
        Expr::Call {
            receiver: Some(receiver),
            name,
            args,
            ..
        } if crate::call_plan::native_cursor_returned_child(
            &native_storage_expr_type(receiver, facts, owner, types, context),
            name,
            args.len(),
            types,
            context.facts,
        )
        .is_some() =>
        {
            let mut slot = native_collection_storage_slot(receiver, facts, owner, types, context)?;
            slot.ty = crate::slot_shape::RustSlotShape::native_child_type_at_path(&slot.ty, &[0])?;
            slot.path.push(0);
            return Some(slot);
        }
        Expr::Call {
            receiver: Some(receiver),
            name,
            args,
            ..
        } if crate::effective_facts::EffectiveFacts::new(context.facts)
            .native_functional_carrier(&native_storage_expr_type(
                receiver, facts, owner, types, context,
            ))
            .is_some_and(|functional| {
                functional.method() == name && functional.inputs().len() == args.len()
            }) =>
        {
            let ty = native_storage_expr_type(receiver, facts, owner, types, context);
            let functional = crate::effective_facts::EffectiveFacts::new(context.facts)
                .native_functional_carrier(&ty)?;
            let crate::stdlib::FunctionalTypeSource::TypeArgument(child) = functional.returns()?
            else {
                return None;
            };
            let mut slot = native_collection_storage_slot(receiver, facts, owner, types, context)?;
            slot.path.push(child);
            slot.ty = crate::types::infer_expr_type(expr, types);
            return Some(slot);
        }
        Expr::Call {
            receiver: Some(receiver),
            name,
            args,
            ..
        } if native_storage_method(receiver, name, args, types, context)
            .and_then(|method| method.result_child())
            .is_some() =>
        {
            let child =
                native_storage_method(receiver, name, args, types, context)?.result_child()?;
            let mut slot = native_collection_storage_slot(receiver, facts, owner, types, context)?;
            slot.path.push(child);
            slot.ty = crate::types::infer_expr_type(expr, types);
            return Some(slot);
        }
        Expr::Call {
            resolved: Some(call),
            ..
        } => NativeCollectionStorageRoot::Origin(CollectionArgumentOrigin::MethodReturn(
            call.clone(),
        )),
        Expr::ArrayAccess(array, _) => {
            let mut slot = native_collection_storage_slot(array, facts, owner, types, context)?;
            slot.path.push(0);
            slot.ty = crate::types::infer_expr_type(expr, types);
            return Some(slot);
        }
        Expr::Cast { expr, .. } => {
            return native_collection_storage_slot(expr, facts, owner, types, context);
        }
        _ => return None,
    };
    Some(NativeCollectionStorageSlot {
        root,
        path: vec![],
        ty: crate::types::infer_expr_type(expr, types),
        root_ty: crate::types::infer_expr_type(expr, types),
    })
}

fn collection_argument_origins(
    args: &[Expr],
    types: &TypeEnv,
    context: &UsageAnalysisContext<'_>,
) -> Vec<(usize, CollectionArgumentOrigin)> {
    fn visit(
        expr: &Expr,
        index: usize,
        types: &TypeEnv,
        context: &UsageAnalysisContext<'_>,
        out: &mut Vec<(usize, CollectionArgumentOrigin)>,
    ) {
        match expr {
            Expr::Cast { expr, .. } => {
                visit(expr, index, types, context, out);
                return;
            }
            Expr::Ternary {
                then_expr,
                else_expr,
                ..
            } => {
                visit(then_expr, index, types, context, out);
                visit(else_expr, index, types, context, out);
                return;
            }
            _ => {}
        }
        let (origin, ty) = match expr {
            Expr::Call {
                resolved: Some(call),
                ..
            } => (
                CollectionArgumentOrigin::MethodReturn(call.clone()),
                call.return_type.clone(),
            ),
            Expr::FieldAccess(receiver, field) => {
                let owner = crate::types::infer_expr_type(receiver, types);
                let owner = crate::types::optional_element_type(&owner).unwrap_or(owner);
                let (JavaType::Named { name, .. } | JavaType::Unknown(name)) = owner else {
                    return;
                };
                let Some((owner, name, ty)) = context
                    .source_symbols
                    .as_ref()
                    .and_then(|symbols| symbols.source_field_identity(&name, field))
                else {
                    return;
                };
                (CollectionArgumentOrigin::Field { owner, name }, ty)
            }
            _ => return,
        };
        let ty = crate::types::optional_element_type(&ty).unwrap_or(ty);
        if crate::stdlib::java_collection_kind_for_type(&ty).is_none() {
            return;
        }
        if let JavaType::Named { name, .. } | JavaType::Unknown(name) = &ty
            && crate::call_plan::java_type_owner_is_parsed_source(name, types, context.facts)
        {
            return;
        }
        out.push((index, origin));
    }
    let mut out = Vec::new();
    for (index, arg) in args.iter().enumerate() {
        visit(arg, index, types, context, &mut out);
    }
    out
}

fn expr_is_null_literal(expr: &Expr) -> bool {
    matches!(expr, Expr::Literal(crate::java_ir::Literal::Null))
}

fn nullable_source_call_arg_indices(
    args: &[Expr],
    source_return_nullability: &HashMap<(String, usize), NullabilityState>,
    semantic_facts: &UsageAnalysisContext<'_>,
    facts: &MethodUsageFacts,
    current_type: &str,
    type_env: &TypeEnv,
) -> Vec<usize> {
    args.iter()
        .enumerate()
        .filter_map(|(index, arg)| {
            source_call_arg_nullability(
                arg,
                source_return_nullability,
                semantic_facts,
                facts,
                current_type,
                type_env,
            )
            .is_some_and(|state| {
                matches!(
                    state,
                    NullabilityState::DefinitelyNull | NullabilityState::MaybeNull
                )
            })
            .then_some(index)
        })
        .collect()
}

/// Keep expression-result producers until signature saturation. A syntactic
/// source-nullability summary can still be unknown before nullable field or
/// parameter storage reaches the producer's solved return contract.
fn native_return_argument_calls(
    args: &[Expr],
    internal_methods: &HashSet<String>,
    facts: &MethodUsageFacts,
    current_type: &str,
    current_super_type: Option<&str>,
    field_names: &HashSet<String>,
    field_types: &HashMap<String, JavaType>,
    semantic_facts: &UsageAnalysisContext<'_>,
    type_env: &TypeEnv,
) -> Vec<(usize, CallResultBoundaryPolicy)> {
    // Retain only native storage producers, including unresolved SDK calls.
    // The normal source-result records continue to drive source contract joins.
    args.iter()
        .enumerate()
        .flat_map(|(index, arg)| {
            call_result_boundary_policies_with_context(
                arg,
                internal_methods,
                facts,
                current_type,
                current_super_type,
                field_names,
                field_types,
                semantic_facts,
                type_env,
            )
            .into_iter()
            .filter(|call| {
                let owner = call
                    .receiver_type()
                    .and_then(|ty| match ty {
                        JavaType::Named { name, .. } | JavaType::Unknown(name) => {
                            Some(name.as_str())
                        }
                        _ => None,
                    })
                    .or_else(|| call.resolved().map(|call| call.declaring_type.as_str()));
                owner.is_some_and(|owner| {
                    semantic_facts
                        .facts
                        .native_return_allocation_for(owner, call.name(), call.arg_count())
                        .is_some()
                })
            })
            .map(move |call| (index, call))
        })
        .collect()
}

fn argument_result_calls(args: &[Expr]) -> Vec<(usize, ResolvedCall)> {
    fn visit(expr: &Expr, index: usize, out: &mut Vec<(usize, ResolvedCall)>) {
        match expr {
            Expr::Cast {
                ty: JavaType::Primitive(_),
                ..
            } => {}
            Expr::Cast { expr, .. } => visit(expr, index, out),
            Expr::Ternary {
                then_expr,
                else_expr,
                ..
            } => {
                visit(then_expr, index, out);
                visit(else_expr, index, out);
            }
            Expr::Call {
                resolved: Some(call),
                ..
            } if !matches!(call.return_type, JavaType::Void | JavaType::Primitive(_)) => {
                out.push((index, call.clone()));
            }
            _ => {}
        }
    }
    let mut results = Vec::new();
    for (index, arg) in args.iter().enumerate() {
        visit(arg, index, &mut results);
    }
    results
}

fn source_call_arg_nullability(
    arg: &Expr,
    source_return_nullability: &HashMap<(String, usize), NullabilityState>,
    semantic_facts: &UsageAnalysisContext<'_>,
    facts: &MethodUsageFacts,
    current_type: &str,
    type_env: &TypeEnv,
) -> Option<NullabilityState> {
    if matches!(arg, Expr::Literal(crate::java_ir::Literal::Null)) {
        return Some(NullabilityState::DefinitelyNull);
    }
    if let Expr::ArrayAccess(array, _) = arg {
        let types =
            native_storage_type_env(&[array], facts, current_type, type_env, semantic_facts);
        let ty = native_storage_expr_type(array, facts, current_type, &types, semantic_facts);
        if crate::shape_projection::reference_array_read_may_be_null(&ty) {
            // Reference-array elements (including rows) may be null even if
            // the array itself is non-null. No literal-null call is needed
            // to establish the source parameter's nullable carrier.
            return Some(NullabilityState::MaybeNull);
        }
    }
    if let Expr::Ternary {
        then_expr,
        else_expr,
        ..
    } = arg
    {
        let then_state = source_call_arg_nullability(
            then_expr,
            source_return_nullability,
            semantic_facts,
            facts,
            current_type,
            type_env,
        )
        .unwrap_or(NullabilityState::DefinitelyNonNull);
        let else_state = source_call_arg_nullability(
            else_expr,
            source_return_nullability,
            semantic_facts,
            facts,
            current_type,
            type_env,
        )
        .unwrap_or(NullabilityState::DefinitelyNonNull);
        return Some(crate::lattice::field_nullability_join(
            then_state, else_state,
        ));
    }
    if let Expr::Cast { ty, expr } = arg {
        if matches!(ty, JavaType::Primitive(_)) {
            return Some(NullabilityState::DefinitelyNonNull);
        }
        return source_call_arg_nullability(
            expr,
            source_return_nullability,
            semantic_facts,
            facts,
            current_type,
            type_env,
        );
    }
    let Expr::Call {
        receiver,
        name,
        args,
        resolved,
        ..
    } = arg
    else {
        return None;
    };
    if let Some(call) = resolved
        && let Some(table) = semantic_facts.source_return_table(&call.declaring_type)
    {
        return table.get(&(call.rust_name.clone(), args.len())).copied();
    }
    if !matches!(
        receiver.as_deref(),
        None | Some(Expr::This) | Some(Expr::Super)
    ) {
        return None;
    }
    let key = resolved
        .as_ref()
        .map(|resolved| (resolved.rust_name.clone(), args.len()))
        .unwrap_or_else(|| (name.clone(), args.len()));
    source_return_nullability.get(&key).copied()
}

fn source_return_nullability_by_call_name(
    ty: &TypeDecl,
    semantic_facts: &UsageAnalysisContext<'_>,
) -> HashMap<(String, usize), NullabilityState> {
    ty.members
        .iter()
        .filter_map(|member| {
            let Member::Method(method) = member else {
                return None;
            };
            // Usage analysis needs the same body- and API-aware return
            // nullability that signature solving will use. The syntax-only
            // pass cannot see through returns such as `return values.get(i)`,
            // so a direct `out.add(first(values))` would otherwise miss the
            // nullable element carrier and emit `Vec<T>::push(Option<T>)`.
            let constraints = crate::signature::typed_source_method_return_nullability_constraints(
                ty,
                method,
                semantic_facts,
            );
            let state = constraints
                .into_iter()
                .filter_map(|constraint| match constraint {
                    crate::signature::SignatureConstraint::RequiredStorageNullability(state) => {
                        Some(state)
                    }
                    _ => None,
                })
                .reduce(crate::lattice::field_nullability_join)
                .or_else(|| {
                    // An abstract reference-returning API does not promise a
                    // non-null value. Preserve its Java value domain in
                    // containers even when no implementation body is present.
                    (method.body.is_none()
                        && !matches!(method.return_type, JavaType::Void | JavaType::Primitive(_)))
                    .then_some(NullabilityState::MaybeNull)
                })?;
            Some((source_method_rust_name(method), method.params.len(), state))
        })
        .flat_map(|(rust_name, arity, state)| {
            let mut entries = vec![((rust_name.clone(), arity), state)];
            if let Some(method) = ty.members.iter().find_map(|member| match member {
                Member::Method(method)
                    if source_method_rust_name(method) == rust_name
                        && method.params.len() == arity =>
                {
                    Some(method)
                }
                _ => None,
            }) {
                entries.push(((method.name.clone(), arity), state));
            }
            entries
        })
        .collect()
}

/// Feed deferred source invocations into the ordinary source-call constraint
/// propagation. Nullable callback inputs are not literal null arguments and
/// do not execute at reference creation, but the eventual callee must accept
/// the complete input domain.
fn record_stream_collection_element_transfers(
    stream: &crate::stream::StreamPlan<'_>,
    usage: &mut MethodUsageFacts,
    owner: &str,
    context: &UsageAnalysisContext<'_>,
    types: &TypeEnv,
) {
    fn forwards_reference(expr: &Expr, param: &str) -> bool {
        match expr {
            Expr::Var(name) => name == param,
            Expr::Cast { ty, expr } if !matches!(ty, JavaType::Primitive(_)) => {
                forwards_reference(expr, param)
            }
            _ => false,
        }
    }
    // Only identity-preserving pipelines retain the source element domain.
    // Non-null filters discharge this root-nullability obligation. Arbitrary
    // maps and flat maps instead have their own result domains.
    if !stream.forwards_source_reference_domain() {
        return;
    }
    let crate::stream::StreamTerminal::ForEach(lambda) = stream.terminal() else {
        return;
    };
    let Some(mut source) =
        native_collection_storage_slot(stream.source().expr(), usage, owner, types, context)
    else {
        return;
    };
    let Some(element) =
        crate::slot_shape::RustSlotShape::native_child_type_at_path(&source.ty, &[0])
    else {
        return;
    };
    source.path.push(0);
    source.ty = element;
    let mut transfers = Vec::new();
    let mut visit = |expr: &Expr| {
        let Expr::Call {
            receiver: Some(receiver),
            name,
            args,
            ..
        } = expr
        else {
            return;
        };
        let receiver_types = native_storage_type_env(&[receiver], usage, owner, types, context);
        let Some(method) = native_storage_method(receiver, name, args, &receiver_types, context)
        else {
            return;
        };
        for (argument, child) in method.stored_argument_children() {
            if !args
                .get(*argument)
                .is_some_and(|arg| forwards_reference(arg, lambda.param_name()))
            {
                continue;
            }
            let Some(mut target) =
                native_collection_storage_slot(receiver, usage, owner, &receiver_types, context)
            else {
                continue;
            };
            let Some(ty) =
                crate::slot_shape::RustSlotShape::native_child_type_at_path(&target.ty, &[*child])
            else {
                continue;
            };
            target.path.push(*child);
            target.ty = ty;
            transfers.push((source.clone(), target));
        }
    };
    match lambda.body() {
        crate::stream::StreamLambdaBody::Expr(expr) => {
            crate::semantic_analysis::walk_expr_outside_lambdas(expr, &mut visit)
        }
        crate::stream::StreamLambdaBody::Block(block) => {
            crate::semantic_analysis::walk_block_exprs(block, &mut visit)
        }
        crate::stream::StreamLambdaBody::MethodReference {
            target,
            method,
            receiver,
        } => {
            let call = Expr::Call {
                receiver: Some(Box::new(
                    receiver
                        .cloned()
                        .unwrap_or_else(|| crate::parser::parse_expr(target)),
                )),
                name: method.to_string(),
                type_args: Vec::new(),
                args: vec![Expr::Var(lambda.param_name().to_string())],
                resolved: None,
            };
            visit(&call);
        }
    }
    usage.nullable_collection_storage_flows.extend(transfers);
}

fn record_nullable_source_callback_calls(
    expr: &Expr,
    usage: &mut MethodUsageFacts,
    current_type: &str,
    context: &UsageAnalysisContext<'_>,
    types: &TypeEnv,
) {
    let Some(source) = crate::stream::stream_chain_source_expression(expr) else {
        return;
    };
    let source_types = native_storage_type_env(&[source], usage, current_type, types, context);
    let types = &source_types;
    let Some(stream) = crate::stream::build_stream_plan(expr, types, context.facts) else {
        return;
    };
    record_stream_collection_element_transfers(&stream, usage, current_type, context, types);
    let Some(symbols) = context.source_symbols.as_ref() else {
        return;
    };
    let reference_argument = |ty: &JavaType| !matches!(ty, JavaType::Primitive(_) | JavaType::Void);
    let mut record = |name: &str, call: ResolvedCall, indices: Vec<usize>| {
        if indices.is_empty() {
            return;
        }
        usage.call_sites.push(CallSiteUse {
            name: name.to_string(),
            argument_result_calls: Vec::new(),
            native_return_argument_calls: Vec::new(),
            arg_count: call.arg_target_types.len(),
            result_returned: false,
            receiver_is_self: false,
            receiver_targets_self_field: false,
            receiver_type: Some(JavaType::Named {
                name: call.declaring_type.clone(),
                args: Vec::new(),
            }),
            null_arg_indices: Vec::new(),
            nullable_source_call_arg_indices: indices,
            argument_bindings: Vec::new(),
            argument_self_fields: Vec::new(),
            collection_argument_origins: Vec::new(),
            boundary: CallArgumentBoundary::Unknown,
            resolved: Some(call),
        });
    };
    stream.visit_callbacks(&mut |lambda, input| {
        let Some(payload) = input.optional_element_type() else { return; };
        if let crate::stream::StreamLambdaBody::MethodReference { target, method, receiver } = lambda.body() {
            if let Some(call) = crate::resolve::resolve_source_method_reference_with_receiver(
                target, receiver, method, std::slice::from_ref(&payload), current_type, symbols, types, context.facts,
            ) {
                if call.arg_target_types.len() == 1 && reference_argument(&call.arg_target_types[0]) {
                    record(method, call, vec![0]);
                }
            }
            return;
        }
        let mut callback_types = types.clone();
        callback_types.insert_local(lambda.param_name(), payload.clone());
        callback_types.insert_local(lambda.rust_param_name(), payload.clone());
        let mut visit = |expr: &Expr| {
            let Expr::Call { receiver, name, args, resolved, .. } = expr else { return; };
            // Lambda parameter types are contextual, so the initial resolver
            // may not yet have selected a source overload. Resolve with the
            // pipeline's input type, using the ordinary applicability rules.
            let contextual_call = resolved.as_ref().filter(|call| {
                crate::call_plan::java_type_owner_is_parsed_source(
                    &call.declaring_type, &callback_types, context.facts,
                )
            }).cloned().or_else(|| {
                let arg_types = args.iter().map(|arg| crate::types::infer_expr_type(arg, &callback_types)).collect::<Vec<_>>();
                crate::resolve::resolve_source_call_with_types(
                    receiver.as_deref(), name, &arg_types, current_type, symbols,
                    &callback_types, context.facts,
                )
            });
            let Some(call) = contextual_call else { return; };
            let indices = args.iter().enumerate().filter_map(|(index, arg)| {
                (matches!(arg, Expr::Var(name) if name == lambda.param_name() || name == lambda.rust_param_name())
                    && call.arg_target_types.get(index).is_some_and(reference_argument)).then_some(index)
            }).collect();
            record(name, call, indices);
        };
        match lambda.body() {
            crate::stream::StreamLambdaBody::Expr(expr) => crate::semantic_analysis::walk_expr_outside_lambdas(expr, &mut visit),
            crate::stream::StreamLambdaBody::Block(block) => crate::semantic_analysis::walk_block_exprs(block, &mut visit),
            crate::stream::StreamLambdaBody::MethodReference { .. } => unreachable!(),
        }
    });
}

fn analyze_expr_with_context(
    expr: &Expr,
    facts: &mut MethodUsageFacts,
    current_type: &str,
    current_super_type: Option<&str>,
    internal_methods: &HashSet<String>,
    field_names: &HashSet<String>,
    field_types: &HashMap<String, JavaType>,
    semantic_facts: &UsageAnalysisContext<'_>,
    type_env: &TypeEnv,
    source_return_nullability: &HashMap<(String, usize), NullabilityState>,
    result_returned: bool,
) {
    if let Expr::Binary {
        op: BinaryOp::Assign,
        left,
        right,
    } = expr
    {
        record_null_source_field_assignment(
            left,
            right,
            facts,
            current_type,
            type_env,
            semantic_facts,
        );
        record_native_assignment_value_domains(
            left,
            right,
            facts,
            current_type,
            type_env,
            semantic_facts,
        );
    }
    if matches!(expr, Expr::Call { .. }) {
        record_nullable_source_callback_calls(expr, facts, current_type, semantic_facts, type_env);
        record_native_call_input_domains(expr, facts, current_type, type_env, semantic_facts);
        record_native_ordering_input_domains(expr, facts, current_type, type_env, semantic_facts);
    }
    if let Some((left, right)) =
        crate::call_plan::native_objects_equals_operands(expr, type_env, semantic_facts.facts)
    {
        record_native_comparison_retention(
            left,
            right,
            facts,
            current_type,
            type_env,
            semantic_facts,
        );
    }
    if let Expr::Call {
        receiver: Some(receiver),
        name,
        args,
        ..
    } = expr
        && name == "equals"
        && args.len() == 1
        && crate::effective_facts::EffectiveFacts::new(semantic_facts.facts)
            .retained_native_container_kind(&native_storage_expr_type(
                receiver,
                facts,
                current_type,
                type_env,
                semantic_facts,
            ))
            .is_some()
    {
        record_native_comparison_retention(
            receiver,
            &args[0],
            facts,
            current_type,
            type_env,
            semantic_facts,
        );
    }
    if matches!(expr, Expr::This | Expr::Super)
        || self_field_name(expr, facts, field_names).is_some()
    {
        facts.uses_receiver = true;
    }
    if expr_mutates_self_field(
        expr,
        facts,
        current_type,
        current_super_type,
        field_names,
        field_types,
        semantic_facts,
    ) {
        facts.mutates_self = true;
    }
    match expr {
        Expr::MethodReference {
            receiver: Some(receiver),
            ..
        } => {
            analyze_expr(
                receiver,
                facts,
                current_type,
                current_super_type,
                internal_methods,
                field_names,
                field_types,
                semantic_facts,
                type_env,
                source_return_nullability,
            );
        }
        Expr::FieldAccess(receiver, field) => {
            if field == "length"
                && let Expr::Var(name) = receiver.as_ref()
            {
                record_use(facts, name, BindingUse::ContentRead);
            }
            analyze_expr(
                receiver,
                facts,
                current_type,
                current_super_type,
                internal_methods,
                field_names,
                field_types,
                semantic_facts,
                type_env,
                source_return_nullability,
            );
        }
        Expr::Call {
            receiver,
            name,
            args,
            resolved,
            ..
        } => {
            if matches!(name.as_str(), "set" | "add")
                && args.len() == 1
                && let Some(receiver) = receiver.as_deref()
            {
                let receiver_ty = native_storage_expr_type(
                    receiver,
                    facts,
                    current_type,
                    type_env,
                    semantic_facts,
                );
                if let Some(child) = crate::call_plan::native_list_cursor_written_child(
                    &receiver_ty,
                    name,
                    args.len(),
                    type_env,
                    semantic_facts.facts,
                ) {
                    // Reject ordinary collection mutations before cloning
                    // their environment; only a selected cursor needs this.
                    let storage_types = native_storage_type_env(
                        &[expr],
                        facts,
                        current_type,
                        type_env,
                        semantic_facts,
                    );
                    if let Some(mut stored) = native_collection_storage_slot(
                        receiver,
                        facts,
                        current_type,
                        &storage_types,
                        semantic_facts,
                    ) {
                        stored.path.push(0);
                        stored.ty = child.clone();
                        if source_call_arg_nullability(
                            &args[0],
                            source_return_nullability,
                            semantic_facts,
                            facts,
                            current_type,
                            &storage_types,
                        )
                        .is_some_and(|state| {
                            matches!(
                                state,
                                NullabilityState::DefinitelyNull | NullabilityState::MaybeNull
                            )
                        }) {
                            let mut nullable = stored.clone();
                            nullable.ty = crate::types::optional_java_type(child.clone());
                            facts
                                .nullable_collection_storage_flows
                                .push((nullable.clone(), nullable));
                        }
                        if let Some(value) = native_collection_storage_slot(
                            &args[0],
                            facts,
                            current_type,
                            &storage_types,
                            semantic_facts,
                        ) {
                            facts
                                .nullable_collection_storage_flows
                                .push((value.clone(), stored.clone()));
                            if native_storage_value_type(&child, &storage_types, semantic_facts) {
                                facts.native_collection_storage_flows.push((value, stored));
                            }
                        }
                    }
                }
            }
            if let Some(call) = resolved {
                for (actual_index, arg) in args.iter().enumerate() {
                    if !has_projectable_native_value_domain(arg) {
                        continue;
                    }
                    let Some((index, path, ty, root_ty)) =
                        crate::java_signature::resolved_argument_storage_projection(
                            call,
                            actual_index,
                        )
                    else {
                        continue;
                    };
                    record_native_value_domain_storage(
                        arg,
                        NativeCollectionStorageSlot {
                            root: NativeCollectionStorageRoot::MethodArgument {
                                call: call.clone(),
                                index,
                            },
                            path,
                            ty,
                            root_ty,
                        },
                        facts,
                        current_type,
                        type_env,
                        semantic_facts,
                    );
                }
            }
            let policy = call_argument_boundary_policy(receiver.as_deref(), name, internal_methods);
            let receiver_type = call_receiver_java_type_with_context(
                receiver.as_deref(),
                facts,
                current_type,
                current_super_type,
                field_names,
                field_types,
                semantic_facts,
            );
            let call_boundary = resolved_receiver_call_boundary(
                policy.boundary,
                receiver.as_deref(),
                resolved.as_ref(),
            );
            let call_boundary = modeled_receiver_call_boundary(
                call_boundary,
                receiver_type.as_ref(),
                name,
                args.len(),
                semantic_facts,
                type_env,
            );
            let call_boundary = source_receiver_call_boundary(
                call_boundary,
                receiver_type.as_ref(),
                semantic_facts,
            );
            let usage_resolved =
                resolved_call_for_usage(receiver.as_deref(), resolved.as_ref(), current_super_type);
            record_trivial_accessor_call_use(
                facts,
                current_type,
                receiver.as_deref(),
                name,
                args,
                resolved.as_ref(),
                semantic_facts,
                result_returned,
            );
            let argument_bindings = call_argument_bindings(args, facts);
            if !receiver.as_deref().is_some_and(|receiver| matches!(receiver, Expr::Var(name) if facts.visible_binding_id_for_name(name).is_some()))
                && let Some(crate::call_plan::CallTargetPlan::NativeCollectionStatic { method }) =
                crate::call_plan::plan_native_collection_static_call(
                    receiver.as_deref(),
                    name,
                    resolved.as_ref(),
                    type_env,
                    semantic_facts.facts,
                )
            {
                for (index, binding) in &argument_bindings {
                    if method.stored_argument_children().iter().any(|(argument, _)| argument == index) {
                        if let Some(name) = facts
                            .binding_facts_for_declaration_id(binding)
                            .map(|binding| binding.name().to_string())
                        {
                            facts.record_declared_binding_use(
                                &name,
                                Some(binding),
                                BindingUse::RetainedByCall,
                            );
                        }
                    }
                }
            }
            let non_null_arg_indices = non_null_argument_indices(args, facts);
            let nullable_source_call_arg_indices = nullable_source_call_arg_indices(
                args,
                source_return_nullability,
                semantic_facts,
                facts,
                current_type,
                type_env,
            );
            facts.call_sites.push(CallSiteUse {
                native_return_argument_calls: native_return_argument_calls(
                    args,
                    internal_methods,
                    facts,
                    current_type,
                    current_super_type,
                    field_names,
                    field_types,
                    semantic_facts,
                    type_env,
                ),
                argument_result_calls: argument_result_calls(args),
                name: name.clone(),
                arg_count: args.len(),
                result_returned,
                receiver_is_self: call_receiver_is_self(receiver.as_deref()),
                receiver_targets_self_field: receiver
                    .as_deref()
                    .is_some_and(|receiver| expr_targets_self_field(receiver, facts, field_names)),
                receiver_type: receiver_type.clone(),
                null_arg_indices: null_argument_indices(args),
                nullable_source_call_arg_indices: nullable_source_call_arg_indices.clone(),
                argument_bindings: argument_bindings.clone(),
                argument_self_fields: call_argument_self_fields(args, facts, field_names),
                collection_argument_origins: collection_argument_origins(
                    args,
                    type_env,
                    semantic_facts,
                ),
                boundary: call_boundary,
                resolved: usage_resolved.clone(),
            });
            // Stream traversal transfers Java references, not copies of the
            // child collection's contents. Seed the same canonical storage
            // graph used by enhanced-for and collection argument flow.
            if args.is_empty()
                && matches!(
                    name.as_str(),
                    "stream" | "parallelStream" | "parallel_stream"
                )
                && let Some(receiver) = receiver.as_deref()
                && let receiver_ty = native_storage_expr_type(
                    receiver,
                    facts,
                    current_type,
                    type_env,
                    semantic_facts,
                )
                && crate::effective_facts::EffectiveFacts::new(semantic_facts.facts)
                    .retained_native_collection_kind(&receiver_ty)
                    .is_some()
                && let Some(child_ty) =
                    crate::slot_shape::RustSlotShape::native_child_type_at_path(&receiver_ty, &[0])
                && native_storage_value_type(&child_ty, type_env, semantic_facts)
            {
                let storage_types = native_storage_type_env(
                    &[receiver],
                    facts,
                    current_type,
                    type_env,
                    semantic_facts,
                );
                if let Some(mut source) = native_collection_storage_slot(
                    receiver,
                    facts,
                    current_type,
                    &storage_types,
                    semantic_facts,
                ) {
                    source.path.push(0);
                    source.ty = child_ty;
                    facts.native_collection_retention_requirements.push(source);
                }
            }
            // Reject scalar/non-storage operations before enriching nested
            // expression types. This must not turn every call in a deep
            // expression into another full subtree traversal.
            if let Some(receiver) = receiver.as_deref()
                && let receiver_ty = native_storage_expr_type(
                    receiver,
                    facts,
                    current_type,
                    type_env,
                    semantic_facts,
                )
                && let Some(method) = native_storage_method_for_type(
                    receiver,
                    name,
                    args,
                    &receiver_ty,
                    type_env,
                    semantic_facts,
                )
                && method.argument_children().iter().any(|(_, child)| {
                    crate::slot_shape::RustSlotShape::native_child_type_at_path(
                        &receiver_ty,
                        &[*child],
                    )
                    .is_some_and(|ty| native_storage_value_type(&ty, type_env, semantic_facts))
                })
            {
                let storage_types =
                    native_storage_type_env(&[expr], facts, current_type, type_env, semantic_facts);
                if let Some(method) =
                    native_storage_method(receiver, name, args, &storage_types, semantic_facts)
                    && let Some(container) = native_collection_storage_slot(
                        receiver,
                        facts,
                        current_type,
                        &storage_types,
                        semantic_facts,
                    )
                {
                    for (argument, child) in method.argument_children() {
                        let Some(child_ty) =
                            crate::slot_shape::RustSlotShape::native_child_type_at_path(
                                &container.ty,
                                &[*child],
                            )
                        else {
                            continue;
                        };
                        if !native_storage_value_type(&child_ty, &storage_types, semantic_facts) {
                            continue;
                        }
                        let Some(value) = args.get(*argument) else {
                            continue;
                        };
                        if !native_storage_value_type(
                            &crate::types::infer_expr_type(value, &storage_types),
                            &storage_types,
                            semantic_facts,
                        ) {
                            continue;
                        }
                        let value_slot = native_collection_storage_slot(
                            value,
                            facts,
                            current_type,
                            &storage_types,
                            semantic_facts,
                        );
                        let mut stored = container.clone();
                        stored.path.push(*child);
                        stored.ty = child_ty;
                        if method
                            .stored_argument_children()
                            .contains(&(*argument, *child))
                        {
                            if let Some(value) = value_slot {
                                facts.native_collection_storage_flows.push((stored, value));
                            }
                        } else {
                            // Equal physical representations do not imply an
                            // alias or a nullable write flowing into storage.
                            facts.native_collection_retention_requirements.push(stored);
                            if let Some(value) = value_slot {
                                facts.native_collection_retention_requirements.push(value);
                            }
                        }
                    }
                }
            }
            if is_content_read_method(name)
                && let Some(receiver) = receiver
                && let Expr::Var(binding) = receiver.as_ref()
            {
                record_use(facts, binding, BindingUse::ContentRead);
            }
            if let Some(field_name) = receiver
                .as_deref()
                .and_then(|receiver| self_field_name(receiver, facts, field_names))
            {
                facts.field_uses.push(FieldUse::ReceiverForCall {
                    field_name: field_name.to_string(),
                    receiver_type: receiver_type.clone(),
                    name: name.clone(),
                    arg_count: args.len(),
                    literal_null_arg_indices: null_argument_indices(args),
                    argument_bindings: argument_bindings.clone(),
                    non_null_arg_indices: non_null_arg_indices.clone(),
                    nullable_source_call_arg_indices: nullable_source_call_arg_indices.clone(),
                });
            }
            if let Some(receiver) = receiver
                && let Expr::Var(binding) = receiver.as_ref()
            {
                record_use(
                    facts,
                    binding,
                    BindingUse::ReceiverForCall {
                        name: name.clone(),
                        arg_count: args.len(),
                        boundary: call_boundary,
                        has_literal_null_arg: args.iter().any(expr_is_null_literal),
                        literal_null_arg_indices: null_argument_indices(args),
                        argument_bindings,
                        non_null_arg_indices,
                        nullable_source_call_arg_indices,
                    },
                );
            }
            if let Some(receiver) = receiver.as_deref() {
                for (arg_index, arg) in args.iter().enumerate() {
                    record_collection_element_subtype_use(
                        facts,
                        receiver,
                        name,
                        args.len(),
                        arg_index,
                        arg,
                        receiver_type.as_ref(),
                        field_names,
                        type_env,
                        semantic_facts,
                    );
                }
            }
            if call_mut_borrow_arg_targets_self_field(
                receiver.as_deref(),
                name,
                args,
                facts,
                field_names,
                semantic_facts,
            ) {
                facts.mutates_self = true;
            }
            for (arg_index, arg) in args.iter().enumerate() {
                let arg_boundary =
                    call_argument_use_boundary_policy(name, arg_index, call_boundary);
                record_call_argument_subtype_use(
                    facts,
                    arg,
                    name,
                    args.len(),
                    usage_resolved.as_ref(),
                    arg_index,
                    type_env,
                    semantic_facts,
                );
                record_call_arg_use(
                    facts,
                    current_type,
                    field_names,
                    field_types,
                    semantic_facts,
                    arg,
                    name,
                    receiver_type.clone(),
                    args.len(),
                    arg_index,
                    arg_boundary,
                );
                analyze_nested_argument_expr(
                    arg,
                    facts,
                    current_type,
                    current_super_type,
                    internal_methods,
                    field_names,
                    field_types,
                    semantic_facts,
                    type_env,
                    source_return_nullability,
                );
            }
            if let Some(receiver) = receiver {
                analyze_expr(
                    receiver,
                    facts,
                    current_type,
                    current_super_type,
                    internal_methods,
                    field_names,
                    field_types,
                    semantic_facts,
                    type_env,
                    source_return_nullability,
                );
            }
        }
        Expr::Binary {
            op: BinaryOp::And,
            left,
            right,
        } => {
            analyze_expr(
                left,
                facts,
                current_type,
                current_super_type,
                internal_methods,
                field_names,
                field_types,
                semantic_facts,
                type_env,
                source_return_nullability,
            );
            let right_pattern_bindings = save_true_pattern_bindings(facts, left);
            analyze_expr(
                right,
                facts,
                current_type,
                current_super_type,
                internal_methods,
                field_names,
                field_types,
                semantic_facts,
                type_env,
                source_return_nullability,
            );
            restore_scoped_binding_uses(facts, right_pattern_bindings);
        }
        Expr::Binary {
            op: BinaryOp::Or,
            left,
            right,
        } => {
            analyze_expr(
                left,
                facts,
                current_type,
                current_super_type,
                internal_methods,
                field_names,
                field_types,
                semantic_facts,
                type_env,
                source_return_nullability,
            );
            let right_pattern_bindings = save_false_pattern_bindings(facts, left);
            analyze_expr(
                right,
                facts,
                current_type,
                current_super_type,
                internal_methods,
                field_names,
                field_types,
                semantic_facts,
                type_env,
                source_return_nullability,
            );
            restore_scoped_binding_uses(facts, right_pattern_bindings);
        }
        Expr::Binary {
            op: BinaryOp::Eq | BinaryOp::Ne,
            left,
            right,
        } => {
            record_indexed_null_comparison_use(facts, left, right);
            record_indexed_null_comparison_use(facts, right, left);
            record_native_comparison_retention(
                left,
                right,
                facts,
                current_type,
                type_env,
                semantic_facts,
            );
            analyze_expr(
                left,
                facts,
                current_type,
                current_super_type,
                internal_methods,
                field_names,
                field_types,
                semantic_facts,
                type_env,
                source_return_nullability,
            );
            analyze_expr(
                right,
                facts,
                current_type,
                current_super_type,
                internal_methods,
                field_names,
                field_types,
                semantic_facts,
                type_env,
                source_return_nullability,
            );
        }
        Expr::Binary { left, right, .. } => {
            analyze_expr(
                left,
                facts,
                current_type,
                current_super_type,
                internal_methods,
                field_names,
                field_types,
                semantic_facts,
                type_env,
                source_return_nullability,
            );
            analyze_expr(
                right,
                facts,
                current_type,
                current_super_type,
                internal_methods,
                field_names,
                field_types,
                semantic_facts,
                type_env,
                source_return_nullability,
            );
        }
        Expr::Cast { expr, ty } => {
            if let Expr::Var(binding) = expr.as_ref() {
                record_use(
                    facts,
                    binding,
                    BindingUse::CastTarget {
                        target_ty: ty.clone(),
                    },
                );
            }
            analyze_expr_with_context(
                expr,
                facts,
                current_type,
                current_super_type,
                internal_methods,
                field_names,
                field_types,
                semantic_facts,
                type_env,
                source_return_nullability,
                result_returned,
            )
        }
        Expr::Unary { expr, .. } => analyze_expr(
            expr,
            facts,
            current_type,
            current_super_type,
            internal_methods,
            field_names,
            field_types,
            semantic_facts,
            type_env,
            source_return_nullability,
        ),
        Expr::InstanceOf { expr, ty, .. } => {
            if let Expr::Var(binding) = expr.as_ref() {
                record_use(
                    facts,
                    binding,
                    BindingUse::InstanceOfTarget {
                        target_ty: ty.clone(),
                    },
                );
            }
            analyze_expr(
                expr,
                facts,
                current_type,
                current_super_type,
                internal_methods,
                field_names,
                field_types,
                semantic_facts,
                type_env,
                source_return_nullability,
            );
        }
        Expr::ArrayAccess(array, index) => {
            analyze_expr(
                array,
                facts,
                current_type,
                current_super_type,
                internal_methods,
                field_names,
                field_types,
                semantic_facts,
                type_env,
                source_return_nullability,
            );
            analyze_expr(
                index,
                facts,
                current_type,
                current_super_type,
                internal_methods,
                field_names,
                field_types,
                semantic_facts,
                type_env,
                source_return_nullability,
            );
        }
        Expr::ArrayCreation {
            dimensions,
            initializer,
            ..
        } => {
            for expr in dimensions {
                analyze_expr(
                    expr,
                    facts,
                    current_type,
                    current_super_type,
                    internal_methods,
                    field_names,
                    field_types,
                    semantic_facts,
                    type_env,
                    source_return_nullability,
                );
            }
            if let Some(initializer) = initializer {
                for expr in initializer {
                    analyze_expr(
                        expr,
                        facts,
                        current_type,
                        current_super_type,
                        internal_methods,
                        field_names,
                        field_types,
                        semantic_facts,
                        type_env,
                        source_return_nullability,
                    );
                }
            }
        }
        Expr::ArrayInitializer(dimensions) => {
            for expr in dimensions {
                analyze_expr(
                    expr,
                    facts,
                    current_type,
                    current_super_type,
                    internal_methods,
                    field_names,
                    field_types,
                    semantic_facts,
                    type_env,
                    source_return_nullability,
                );
            }
        }
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => {
            analyze_expr(
                condition,
                facts,
                current_type,
                current_super_type,
                internal_methods,
                field_names,
                field_types,
                semantic_facts,
                type_env,
                source_return_nullability,
            );
            let then_pattern_bindings = save_true_pattern_bindings(facts, condition);
            analyze_expr_with_context(
                then_expr,
                facts,
                current_type,
                current_super_type,
                internal_methods,
                field_names,
                field_types,
                semantic_facts,
                type_env,
                source_return_nullability,
                result_returned,
            );
            restore_scoped_binding_uses(facts, then_pattern_bindings);
            let else_pattern_bindings = save_false_pattern_bindings(facts, condition);
            analyze_expr_with_context(
                else_expr,
                facts,
                current_type,
                current_super_type,
                internal_methods,
                field_names,
                field_types,
                semantic_facts,
                type_env,
                source_return_nullability,
                result_returned,
            );
            restore_scoped_binding_uses(facts, else_pattern_bindings);
        }
        Expr::New {
            ty, args, resolved, ..
        } => {
            if let Some(call) = resolved {
                record_constructor_argument_value_domains(
                    args,
                    call,
                    facts,
                    current_type,
                    type_env,
                    semantic_facts,
                );
            }
            facts.constructor_call_sites.push(ConstructorCallSiteUse {
                name: "new".to_string(),
                native_return_argument_calls: native_return_argument_calls(
                    args,
                    internal_methods,
                    facts,
                    current_type,
                    current_super_type,
                    field_names,
                    field_types,
                    semantic_facts,
                    type_env,
                ),
                argument_result_calls: argument_result_calls(args),
                arg_count: args.len(),
                null_arg_indices: null_argument_indices(args),
                nullable_source_call_arg_indices: nullable_source_call_arg_indices(
                    args,
                    source_return_nullability,
                    semantic_facts,
                    facts,
                    current_type,
                    type_env,
                ),
                argument_bindings: call_argument_bindings(args, facts),
                argument_self_fields: call_argument_self_fields(args, facts, field_names),
                boundary: constructor_invocation_boundary_policy_for_expr(resolved.as_ref()),
                collection_argument_origins: collection_argument_origins(
                    args,
                    type_env,
                    semantic_facts,
                ),
                resolved: resolved.clone(),
            });
            for (arg_index, arg) in args.iter().enumerate() {
                record_call_arg_use(
                    facts,
                    current_type,
                    field_names,
                    field_types,
                    semantic_facts,
                    arg,
                    "new",
                    Some(ty.clone()),
                    args.len(),
                    arg_index,
                    constructor_argument_boundary_policy(
                        current_type,
                        resolved.as_ref(),
                        arg_index,
                    ),
                );
                record_constructor_argument_subtype_use(
                    facts,
                    arg,
                    resolved.as_ref(),
                    arg_index,
                    type_env,
                    semantic_facts,
                );
                analyze_nested_argument_expr(
                    arg,
                    facts,
                    current_type,
                    current_super_type,
                    internal_methods,
                    field_names,
                    field_types,
                    semantic_facts,
                    type_env,
                    source_return_nullability,
                );
            }
        }
        Expr::Lambda { params, body } => analyze_lambda_body(
            params,
            body,
            facts,
            current_type,
            current_super_type,
            internal_methods,
            field_names,
            field_types,
            semantic_facts,
            type_env,
            source_return_nullability,
        ),
        Expr::Switch { selector, cases } => {
            analyze_expr(
                selector,
                facts,
                current_type,
                current_super_type,
                internal_methods,
                field_names,
                field_types,
                semantic_facts,
                type_env,
                source_return_nullability,
            );
            for case in cases {
                for label in &case.labels {
                    if let SwitchLabel::Expr(expr) = label {
                        analyze_expr(
                            expr,
                            facts,
                            current_type,
                            current_super_type,
                            internal_methods,
                            field_names,
                            field_types,
                            semantic_facts,
                            type_env,
                            source_return_nullability,
                        );
                    }
                }
                let case_pattern_bindings = save_switch_expr_case_pattern_bindings(facts, case);
                analyze_expr(
                    &case.value,
                    facts,
                    current_type,
                    current_super_type,
                    internal_methods,
                    field_names,
                    field_types,
                    semantic_facts,
                    type_env,
                    source_return_nullability,
                );
                restore_scoped_binding_uses(facts, case_pattern_bindings);
            }
        }
        Expr::Var(name) => record_use(facts, name, BindingUse::ContentRead),
        Expr::Literal(_)
        | Expr::This
        | Expr::Super
        | Expr::MethodReference { .. }
        | Expr::ClassLiteral(_)
        | Expr::Raw(_)
        | Expr::Unsupported { .. } => {}
    }
}

fn analyze_lambda_body(
    params: &[crate::java_ir::LambdaParam],
    body: &LambdaBody,
    facts: &mut MethodUsageFacts,
    current_type: &str,
    current_super_type: Option<&str>,
    internal_methods: &HashSet<String>,
    field_names: &HashSet<String>,
    field_types: &HashMap<String, JavaType>,
    semantic_facts: &UsageAnalysisContext<'_>,
    type_env: &TypeEnv,
    source_return_nullability: &HashMap<(String, usize), NullabilityState>,
) {
    let previous_result = facts.active_callback_result.take();
    facts.active_callback_result = facts
        .callback_value_domains
        .remove(&SourceLambdaBodyKey::for_lambda(body).domain_key())
        .and_then(|domain| domain.result);
    let mut lambda_env = type_env.clone();
    let saved_params = params
        .iter()
        .map(|param| {
            let saved = facts.hide_visible_binding_for_scope(&param.source_name);
            let ty = param
                .ty
                .clone()
                .unwrap_or_else(|| JavaType::Unknown("lambda parameter".to_string()));
            lambda_env.insert_local(param.name.clone(), ty.clone());
            lambda_env.insert_local(param.source_name.clone(), ty.clone());
            insert_binding_use_facts(facts, param.source_name.clone(), param.binding.clone(), ty);
            (param.source_name.clone(), saved)
        })
        .collect::<Vec<_>>();

    match body {
        LambdaBody::Expr(expr) => {
            if let Some(result) = facts.active_callback_result.clone() {
                record_native_value_domain_storage(
                    expr,
                    result,
                    facts,
                    current_type,
                    &lambda_env,
                    semantic_facts,
                );
            }
            analyze_expr(
                expr,
                facts,
                current_type,
                current_super_type,
                internal_methods,
                field_names,
                field_types,
                semantic_facts,
                &lambda_env,
                source_return_nullability,
            )
        }
        LambdaBody::Block(block) => {
            for stmt in &block.statements {
                analyze_stmt(
                    stmt,
                    facts,
                    current_type,
                    current_super_type,
                    None,
                    internal_methods,
                    field_names,
                    field_types,
                    semantic_facts,
                    &lambda_env,
                    source_return_nullability,
                );
            }
        }
    }

    for (name, saved) in saved_params {
        restore_binding_use_facts(facts, name, saved);
    }
    facts.active_callback_result = previous_result;
}

fn record_returned_call_result_use(
    expr: &Expr,
    facts: &mut MethodUsageFacts,
    internal_methods: &HashSet<String>,
    current_type: &str,
    current_super_type: Option<&str>,
    field_names: &HashSet<String>,
    field_types: &HashMap<String, JavaType>,
    semantic_facts: &UsageAnalysisContext<'_>,
    type_env: &TypeEnv,
) {
    match expr {
        Expr::Call { .. } | Expr::New { .. } => {
            let Some(policy) = call_result_boundary_policy_with_context(
                expr,
                internal_methods,
                facts,
                current_type,
                current_super_type,
                field_names,
                field_types,
                semantic_facts,
                type_env,
            ) else {
                return;
            };
            let boundary = match expr {
                Expr::New {
                    resolved: Some(resolved),
                    ..
                } if resolved.declaring_type != current_type => CallArgumentBoundary::Unknown,
                _ => policy.boundary,
            };
            facts.returned_call_results.push(ReturnedCallResultUse {
                name: policy.name,
                arg_count: policy.arg_count,
                receiver_type: policy.receiver_type,
                boundary,
                resolved: policy.resolved,
            });
        }
        Expr::Cast { expr, .. } => record_returned_call_result_use(
            expr,
            facts,
            internal_methods,
            current_type,
            current_super_type,
            field_names,
            field_types,
            semantic_facts,
            type_env,
        ),
        Expr::Ternary {
            then_expr,
            else_expr,
            ..
        } => {
            record_returned_call_result_use(
                then_expr,
                facts,
                internal_methods,
                current_type,
                current_super_type,
                field_names,
                field_types,
                semantic_facts,
                type_env,
            );
            record_returned_call_result_use(
                else_expr,
                facts,
                internal_methods,
                current_type,
                current_super_type,
                field_names,
                field_types,
                semantic_facts,
                type_env,
            );
        }
        _ => {}
    }
}

fn record_returned_subtype_use(
    expr: &Expr,
    declared_return_type: Option<&JavaType>,
    facts: &mut MethodUsageFacts,
    type_env: &TypeEnv,
    semantic_facts: &UsageAnalysisContext<'_>,
) {
    let Some(declared_return_type) = declared_return_type else {
        return;
    };
    let target_type = crate::types::optional_element_type(declared_return_type)
        .unwrap_or_else(|| declared_return_type.clone());
    let value_expr = optional_of_present_argument(expr).unwrap_or(expr);
    if let Expr::Ternary {
        then_expr,
        else_expr,
        ..
    } = value_expr
    {
        record_returned_subtype_use(
            then_expr,
            Some(declared_return_type),
            facts,
            type_env,
            semantic_facts,
        );
        record_returned_subtype_use(
            else_expr,
            Some(declared_return_type),
            facts,
            type_env,
            semantic_facts,
        );
        return;
    }

    // `TypeEnv` intentionally carries type parameters, not the method's full
    // lexical binding table. For `Sub value = ...; return value;`, use the
    // authoritative live binding fact so the declared supertype return slot is
    // marked as polymorphic just as it is for `return new Sub(...)`.
    let actual_type = match value_expr {
        Expr::Var(name) => facts
            .visible_binding_java_type_for_name(name)
            .and_then(|actual| {
                assigned_subtype_actual_type(actual, &target_type, type_env, semantic_facts)
            }),
        _ => assigned_subtype_local_actual_type(
            value_expr,
            &target_type,
            type_env,
            semantic_facts,
            facts,
        ),
    };
    let Some(actual_type) = actual_type else {
        return;
    };
    facts.field_uses.push(FieldUse::AssignedSubtypeToReturn {
        target_type,
        actual_type,
    });
}

fn optional_of_present_argument(expr: &Expr) -> Option<&Expr> {
    let Expr::Call {
        receiver: Some(receiver),
        name,
        args,
        ..
    } = expr
    else {
        return None;
    };
    if name == "of"
        && args.len() == 1
        && matches!(receiver.as_ref(), Expr::Var(owner) if matches!(owner.as_str(), "Optional" | "optional"))
    {
        args.first()
    } else {
        None
    }
}

fn resolved_call_for_usage(
    receiver: Option<&Expr>,
    resolved: Option<&ResolvedCall>,
    current_super_type: Option<&str>,
) -> Option<ResolvedCall> {
    let mut resolved = resolved.cloned()?;
    if matches!(receiver, Some(Expr::Super))
        && resolved.declaring_type == "super"
        && let Some(super_type) = current_super_type
    {
        // Normal resolved calls already name their lexical declaration, which
        // may be above the direct parent. Never overwrite that identity with
        // a parameterized extends spelling such as `Base<B>`.
        resolved.declaring_type = match crate::parser::parse_hierarchy_type(super_type) {
            JavaType::Named { name, .. } => name,
            _ => super_type.to_string(),
        };
    }
    Some(resolved)
}

fn record_call_arg_use(
    facts: &mut MethodUsageFacts,
    current_type: &str,
    field_names: &HashSet<String>,
    _field_types: &HashMap<String, JavaType>,
    semantic_facts: &UsageAnalysisContext<'_>,
    arg: &Expr,
    call_name: &str,
    receiver_type: Option<JavaType>,
    arg_count: usize,
    arg_index: usize,
    boundary: CallArgumentBoundary,
) {
    let mutates_argument = call_argument_effect_mutates_argument(
        semantic_facts,
        receiver_type.as_ref(),
        call_name,
        arg_count,
        arg_index,
    );
    if let Some(field_name) = trivial_getter_self_field(arg, current_type, semantic_facts) {
        facts.field_uses.push(FieldUse::PassedToCall {
            field_name,
            name: call_name.to_string(),
            receiver_type,
            arg_count,
            arg_index,
            boundary,
        });
        return;
    }
    match arg {
        Expr::This => {
            facts.self_argument_uses.push(SelfArgumentUse {
                name: call_name.to_string(),
                arg_count,
                boundary,
            });
        }
        Expr::Var(name) if is_unshadowed_field_name(facts, field_names, name) => {
            if mutates_argument && !facts.field_is_static(name) {
                facts.mutates_self = true;
            }
            facts.field_uses.push(FieldUse::PassedToCall {
                field_name: name.clone(),
                name: call_name.to_string(),
                receiver_type,
                arg_count,
                arg_index,
                boundary,
            });
        }
        Expr::Var(binding) => {
            if mutates_argument {
                record_use(facts, binding, BindingUse::Mutated);
            }
            record_use(
                facts,
                binding,
                BindingUse::PassedToCall {
                    name: call_name.to_string(),
                    receiver_type,
                    arg_count,
                    arg_index,
                    boundary,
                },
            );
        }
        Expr::FieldAccess(receiver, field_name) if matches!(receiver.as_ref(), Expr::This) => {
            if mutates_argument && !facts.field_is_static(field_name) {
                facts.mutates_self = true;
            }
            facts.field_uses.push(FieldUse::PassedToCall {
                field_name: field_name.clone(),
                name: call_name.to_string(),
                receiver_type,
                arg_count,
                arg_index,
                boundary,
            });
        }
        _ => {}
    }
}

fn record_collection_element_subtype_use(
    facts: &mut MethodUsageFacts,
    receiver: &Expr,
    call_name: &str,
    arg_count: usize,
    arg_index: usize,
    arg: &Expr,
    receiver_type: Option<&JavaType>,
    field_names: &HashSet<String>,
    type_env: &TypeEnv,
    semantic_facts: &UsageAnalysisContext<'_>,
) {
    let (collection_name, targets_field) = match receiver {
        Expr::Var(name) => (name, is_unshadowed_field_name(facts, field_names, name)),
        Expr::FieldAccess(owner, name) if matches!(owner.as_ref(), Expr::This) => (name, true),
        _ => return,
    };
    let Some(receiver_type) = receiver_type else {
        return;
    };
    let Some(element_type) =
        collection_stored_argument_type(receiver_type, call_name, arg_count, arg_index)
    else {
        return;
    };
    let Some(actual_type) =
        assigned_subtype_local_actual_type(arg, &element_type, type_env, semantic_facts, facts)
    else {
        return;
    };
    if targets_field {
        facts
            .field_uses
            .push(FieldUse::AssignedSubtypeToCollectionElement {
                field_name: collection_name.clone(),
                element_type,
                actual_type,
            });
        return;
    }
    let collection_binding = facts.visible_binding_id_for_name(collection_name);
    record_use(
        facts,
        collection_name,
        BindingUse::AssignedSubtypeToCollectionElement {
            collection_name: collection_name.clone(),
            collection_binding,
            element_type,
            actual_type,
        },
    );
}

fn record_call_argument_subtype_use(
    facts: &mut MethodUsageFacts,
    arg: &Expr,
    call_name: &str,
    arg_count: usize,
    resolved: Option<&ResolvedCall>,
    arg_index: usize,
    type_env: &TypeEnv,
    semantic_facts: &UsageAnalysisContext<'_>,
) {
    let Some(target_type) = resolved
        .and_then(|resolved| crate::call_plan::resolved_call_arg_target_type(resolved, arg_index))
        .or_else(|| facts.same_type_call_arg_target_type(call_name, arg_count, arg_index))
    else {
        return;
    };
    let local_actual = match arg {
        Expr::Var(binding_name) => agreed_binding_java_type(facts, binding_name),
        _ => None,
    };
    let Some(actual_type) = local_actual
        .as_ref()
        .and_then(|actual| {
            assigned_subtype_actual_type(actual, target_type, type_env, semantic_facts)
        })
        .or_else(|| {
            assigned_subtype_local_actual_type(arg, target_type, type_env, semantic_facts, facts)
        })
    else {
        return;
    };
    if let Expr::Var(binding_name) = arg {
        record_use(
            facts,
            binding_name,
            BindingUse::AssignedSubtypeToCallArgument {
                target_type: target_type.clone(),
                actual_type,
            },
        );
    } else {
        facts
            .field_uses
            .push(FieldUse::AssignedSubtypeToCallArgument {
                target_type: target_type.clone(),
                actual_type,
            });
    }
}

fn record_constructor_argument_subtype_use(
    facts: &mut MethodUsageFacts,
    arg: &Expr,
    resolved: Option<&ResolvedConstructor>,
    arg_index: usize,
    type_env: &TypeEnv,
    semantic_facts: &UsageAnalysisContext<'_>,
) {
    let Some(target_type) = resolved.and_then(|resolved| {
        resolved
            .params
            .get(arg_index)
            .map(|param| &param.ty)
            .or_else(|| {
                resolved
                    .arg_coercions
                    .iter()
                    .find(|coercion| coercion.arg_index == arg_index)
                    .map(|coercion| &coercion.target)
            })
    }) else {
        return;
    };
    let Some(actual_type) =
        assigned_subtype_local_actual_type(arg, target_type, type_env, semantic_facts, facts)
    else {
        return;
    };

    if let Expr::Var(binding_name) = arg {
        record_use(
            facts,
            binding_name,
            BindingUse::AssignedSubtypeToCallArgument {
                target_type: target_type.clone(),
                actual_type,
            },
        );
    } else {
        facts
            .field_uses
            .push(FieldUse::AssignedSubtypeToCallArgument {
                target_type: target_type.clone(),
                actual_type,
            });
    }
}

fn collection_stored_argument_type(
    receiver_type: &JavaType,
    call_name: &str,
    arg_count: usize,
    arg_index: usize,
) -> Option<JavaType> {
    let JavaType::Named { args, .. } = receiver_type else {
        return None;
    };
    let kind = crate::stdlib::java_collection_kind_for_type(receiver_type)?;
    match kind {
        crate::stdlib::JavaCollectionKind::List if call_name == "add" => {
            let element_arg_index = match arg_count {
                1 => 0,
                2 => 1,
                _ => return None,
            };
            (arg_index == element_arg_index)
                .then(|| args.first().cloned())
                .flatten()
        }
        crate::stdlib::JavaCollectionKind::List if call_name == "set" && arg_index == 1 => {
            args.first().cloned()
        }
        kind if kind.is_set_like() && call_name == "add" && arg_index == 0 => args.first().cloned(),
        kind if kind.is_map_like() && call_name == "put" && matches!(arg_index, 0 | 1) => {
            args.get(arg_index).cloned()
        }
        _ => None,
    }
}

fn call_argument_effect_mutates_argument(
    facts: &FactSet,
    receiver_type: Option<&JavaType>,
    call_name: &str,
    arg_count: usize,
    arg_index: usize,
) -> bool {
    if let Some(receiver_key) = receiver_type.and_then(java_type_name_for_usage)
        && !facts.model_fact_applies_to_owner(
            receiver_key,
            crate::facts::FactProvenance::BuiltinExternalModel,
        )
    {
        // A source definition overrides the platform owner, including the
        // legacy unscoped method-name fallback below. Explicit non-built-in
        // argument facts can still describe an intentional source boundary.
        return facts
            .external_api_argument_effect_for_receiver_with_arity(
                Some(receiver_key),
                call_name,
                Some(arg_count),
                arg_index,
            )
            .is_some_and(|effect| {
                effect.provenance != crate::facts::FactProvenance::BuiltinExternalModel
                    && effect.argument_mutability
                        == crate::facts::ReceiverMutabilityFact::MutatesReceiver
            });
    }
    if let Some(receiver_key) = receiver_type.and_then(java_type_name_for_usage)
        && (facts
            .external_api_argument_effect_for_receiver_with_arity(
                Some(receiver_key),
                call_name,
                Some(arg_count),
                arg_index,
            )
            .is_some_and(|effect| {
                effect.argument_mutability == crate::facts::ReceiverMutabilityFact::MutatesReceiver
            })
            || crate::external_packs::external_api_argument_effect_for_receiver_with_arity(
                Some(receiver_key),
                call_name,
                Some(arg_count),
                arg_index,
            )
            .is_some_and(|effect| {
                effect.argument_mutability == crate::facts::ReceiverMutabilityFact::MutatesReceiver
            }))
    {
        return true;
    }
    facts
        .external_api_argument_effect_facts
        .iter()
        .any(|effect| {
            effect.method_name == call_name
                && effect.arg_count == Some(arg_count)
                && effect.parameter_index == arg_index
                && effect.argument_mutability
                    == crate::facts::ReceiverMutabilityFact::MutatesReceiver
        })
}

fn call_mut_borrow_arg_targets_self_field(
    receiver: Option<&Expr>,
    name: &str,
    args: &[Expr],
    usage: &MethodUsageFacts,
    field_names: &HashSet<String>,
    facts: &FactSet,
) -> bool {
    facts
        .facade_abi_facts
        .iter()
        .filter(|abi| abi.key.member_name() == name && abi.key.arity() == args.len())
        .filter(|abi| facade_abi_matches_call_receiver(abi.key.owner_type(), receiver, usage))
        .any(|abi| {
            abi.args.iter().enumerate().any(|(index, slot)| {
                slot.contains_mut_borrow()
                    && args
                        .get(index)
                        .is_some_and(|arg| expr_targets_self_field(arg, usage, field_names))
            })
        })
}

fn facade_abi_matches_call_receiver(
    owner: &str,
    receiver: Option<&Expr>,
    usage: &MethodUsageFacts,
) -> bool {
    let Some(Expr::Var(receiver_name)) = receiver else {
        return true;
    };
    if let Some(receiver_ty) = agreed_binding_java_type(usage, receiver_name) {
        let Some(receiver_type_name) = usage_java_type_name(&receiver_ty) else {
            return false;
        };
        return crate::stdlib::canonical_external_receiver_type_key(receiver_type_name)
            == crate::stdlib::canonical_external_receiver_type_key(owner);
    }
    static_owner_receiver_name(receiver_name).is_some_and(|static_owner| {
        crate::stdlib::canonical_external_receiver_type_key(static_owner)
            == crate::stdlib::canonical_external_receiver_type_key(owner)
    })
}

fn expr_targets_self_field(
    expr: &Expr,
    facts: &MethodUsageFacts,
    field_names: &HashSet<String>,
) -> bool {
    match expr {
        Expr::Var(name) => is_unshadowed_field_name(facts, field_names, name),
        Expr::FieldAccess(receiver, _) => {
            matches!(receiver.as_ref(), Expr::This | Expr::Super)
        }
        Expr::ArrayAccess(array, _) => expr_targets_self_field(array, facts, field_names),
        Expr::Call {
            receiver: Some(receiver),
            name,
            args,
            ..
        } if name == "get" && args.len() == 1 => {
            expr_targets_self_field(receiver, facts, field_names)
        }
        _ => false,
    }
}

fn self_field_name<'a>(
    expr: &'a Expr,
    facts: &MethodUsageFacts,
    field_names: &HashSet<String>,
) -> Option<&'a str> {
    match expr {
        Expr::Var(name) if is_unshadowed_field_name(facts, field_names, name) => Some(name),
        Expr::FieldAccess(receiver, field_name)
            if matches!(receiver.as_ref(), Expr::This | Expr::Super) =>
        {
            Some(field_name)
        }
        _ => None,
    }
}

fn call_receiver_java_type(receiver: Option<&Expr>, facts: &MethodUsageFacts) -> Option<JavaType> {
    let receiver = receiver?;
    if let Expr::Var(name) = receiver {
        return agreed_binding_java_type(facts, name).or_else(|| {
            static_owner_receiver_name(name).map(|owner| JavaType::Unknown(owner.to_string()))
        });
    }
    let owner = crate::stdlib::qualified_static_owner_java_name(receiver)?;
    // A package-shaped expression rooted in a live binding is an instance
    // expression, not a fully qualified static owner.
    let root = owner.split('.').next()?;
    if agreed_binding_java_type(facts, root).is_some() {
        return None;
    }
    Some(JavaType::Unknown(owner))
}

fn agreed_binding_java_type(facts: &MethodUsageFacts, name: &str) -> Option<JavaType> {
    facts
        .visible_binding_java_type_for_name(name)
        .cloned()
        .or_else(|| idless_compatibility_binding_java_type_for_name(facts, name))
}

fn idless_compatibility_binding_java_type_for_name(
    facts: &MethodUsageFacts,
    name: &str,
) -> Option<JavaType> {
    facts
        .idless_compatibility_binding_facts_for_source_name(name)
        .map(|binding| binding.java_ty.clone())
}

fn usage_java_type_name(java_ty: &JavaType) -> Option<&str> {
    match java_ty {
        JavaType::Named { name, .. } | JavaType::Unknown(name) => Some(name),
        JavaType::String => Some("String"),
        _ => None,
    }
}

fn call_receiver_java_type_with_context(
    receiver: Option<&Expr>,
    facts: &MethodUsageFacts,
    current_type: &str,
    current_super_type: Option<&str>,
    field_names: &HashSet<String>,
    field_types: &HashMap<String, JavaType>,
    semantic_facts: &UsageAnalysisContext<'_>,
) -> Option<JavaType> {
    match receiver {
        Some(Expr::Var(name)) if is_unshadowed_field_name(facts, field_names, name) => {
            source_field_type(current_type, name, field_types, semantic_facts)
        }
        Some(Expr::FieldAccess(inner, field_name)) if matches!(inner.as_ref(), Expr::This) => {
            source_field_type(current_type, field_name, field_types, semantic_facts)
        }
        Some(Expr::Super) => current_super_type.map(|name| JavaType::Unknown(name.to_string())),
        _ => call_receiver_java_type(receiver, facts),
    }
}

fn stream_of_forwarded_array_source<'a>(
    expr: &'a Expr,
    facts: &MethodUsageFacts,
    field_names: &HashSet<String>,
    field_types: &HashMap<String, JavaType>,
    type_env: &TypeEnv,
) -> Option<&'a Expr> {
    let Expr::Call {
        receiver: Some(receiver),
        name,
        args,
        ..
    } = expr
    else {
        return None;
    };
    let [source] = args.as_slice() else {
        return None;
    };
    if !matches!(receiver.as_ref(), Expr::Var(owner)
        if crate::stdlib::canonical_external_receiver_type_key(owner) == "Stream")
        || name != "of"
        || !matches!(
            returned_expr_type(source, facts, field_names, field_types, type_env),
            Some(JavaType::Array(_))
        )
    {
        return None;
    }
    Some(source)
}

fn sequence_element_preserving_return_source(expr: &Expr) -> Option<&Expr> {
    let Expr::Call {
        receiver: Some(source),
        name,
        args,
        ..
    } = expr
    else {
        return None;
    };
    if matches!(name.as_str(), "iterator" | "spliterator") && args.is_empty() {
        return Some(source);
    }
    if name != "collect" || args.len() != 1 {
        return None;
    }
    let Expr::Call {
        receiver: Some(collector_owner),
        name: collector_name,
        args: collector_args,
        ..
    } = &args[0]
    else {
        return None;
    };
    (collector_args.is_empty()
        && matches!(
            collector_name.as_str(),
            "toList" | "to_list" | "toSet" | "to_set"
        )
        && matches!(collector_owner.as_ref(), Expr::Var(owner)
            if crate::stdlib::canonical_external_receiver_type_key(owner) == "Collectors"))
    .then_some(source)
}

fn returned_expr_type(
    expr: &Expr,
    facts: &MethodUsageFacts,
    field_names: &HashSet<String>,
    field_types: &HashMap<String, JavaType>,
    type_env: &TypeEnv,
) -> Option<JavaType> {
    match expr {
        Expr::Var(name) if is_unshadowed_field_name(facts, field_names, name) => {
            declared_field_type(field_types, name).cloned()
        }
        Expr::FieldAccess(receiver, field_name) if matches!(receiver.as_ref(), Expr::This) => {
            declared_field_type(field_types, field_name).cloned()
        }
        _ => Some(crate::types::infer_expr_type(expr, type_env)),
    }
}

fn source_field_type(
    current_type: &str,
    field_name: &str,
    field_types: &HashMap<String, JavaType>,
    semantic_facts: &UsageAnalysisContext<'_>,
) -> Option<JavaType> {
    declared_field_type(field_types, field_name)
        .cloned()
        .or_else(|| {
            semantic_facts
                .external_field_for(current_type, field_name)
                .or_else(|| {
                    semantic_facts
                        .external_field_for(current_type, &crate::parser::to_snake_case(field_name))
                })
                .and_then(|field| field.field_type.clone())
        })
}

fn declared_field_type<'a>(
    field_types: &'a HashMap<String, JavaType>,
    source_name: &str,
) -> Option<&'a JavaType> {
    field_types.get(source_name).or_else(|| {
        let rust_name = crate::parser::to_snake_case(source_name);
        field_types.get(&rust_name)
    })
}

fn static_owner_receiver_name(name: &str) -> Option<&str> {
    let first = name.chars().next()?;
    if first.is_ascii_uppercase() {
        Some(name)
    } else {
        match crate::stdlib::canonical_external_receiver_type_key(name).as_str() {
            "Arrays" => Some("Arrays"),
            "Collections" => Some("Collections"),
            _ => None,
        }
    }
}

fn record_trivial_accessor_call_use(
    facts: &mut MethodUsageFacts,
    current_type: &str,
    receiver: Option<&Expr>,
    name: &str,
    args: &[Expr],
    resolved: Option<&ResolvedCall>,
    semantic_facts: &UsageAnalysisContext<'_>,
    result_returned: bool,
) {
    let Some(accessor) =
        trivial_accessor_for_self_call(current_type, receiver, name, resolved, semantic_facts)
    else {
        return;
    };
    match &accessor.kind {
        TrivialAccessorKind::Getter { field } if args.is_empty() && result_returned => {
            facts.field_uses.push(FieldUse::Returned {
                field_name: field.field_name().to_string(),
            });
        }
        TrivialAccessorKind::Setter { field, .. } if args.len() == 1 => {
            facts.mutates_self = true;
            analyze_stored_expr(&args[0], facts, field.field_name());
        }
        _ => {}
    }
}

fn trivial_getter_self_field(
    expr: &Expr,
    current_type: &str,
    semantic_facts: &UsageAnalysisContext<'_>,
) -> Option<String> {
    let Expr::Call {
        receiver,
        name,
        args,
        resolved,
        ..
    } = expr
    else {
        return None;
    };
    if !args.is_empty() {
        return None;
    }
    let accessor = trivial_accessor_for_self_call(
        current_type,
        receiver.as_deref(),
        name,
        resolved.as_ref(),
        semantic_facts,
    )?;
    match &accessor.kind {
        TrivialAccessorKind::Getter { field } => Some(field.field_name().to_string()),
        TrivialAccessorKind::Setter { .. } => None,
    }
}

fn trivial_accessor_for_self_call<'a>(
    current_type: &str,
    receiver: Option<&Expr>,
    name: &str,
    resolved: Option<&ResolvedCall>,
    semantic_facts: &'a FactSet,
) -> Option<&'a crate::facts::TrivialAccessorFact> {
    if !receiver.is_none_or(|receiver| matches!(receiver, Expr::This)) {
        return None;
    }
    if !resolved
        .as_ref()
        .is_none_or(|resolved| resolved.declaring_type == current_type)
    {
        return None;
    }
    semantic_facts.trivial_accessor_for(current_type, name)
}

fn target_is_this_field(expr: &Expr) -> bool {
    matches!(expr, Expr::FieldAccess(receiver, _) if matches!(receiver.as_ref(), Expr::This | Expr::Super))
}

fn array_assignment_targets_self_field(
    expr: &Expr,
    facts: &MethodUsageFacts,
    field_names: &HashSet<String>,
) -> bool {
    let Expr::ArrayAccess(array, _) = expr else {
        return false;
    };
    array_expr_targets_self_field(array, facts, field_names)
}

fn array_expr_targets_self_field(
    expr: &Expr,
    facts: &MethodUsageFacts,
    field_names: &HashSet<String>,
) -> bool {
    match expr {
        Expr::Var(name) => {
            is_unshadowed_field_name(facts, field_names, name) && !facts.field_is_static(name)
        }
        Expr::FieldAccess(receiver, field) => {
            matches!(receiver.as_ref(), Expr::This | Expr::Super) && !facts.field_is_static(field)
        }
        Expr::ArrayAccess(array, _) => array_expr_targets_self_field(array, facts, field_names),
        _ => false,
    }
}

fn analyze_scoped_statements(
    statements: &[Stmt],
    facts: &mut MethodUsageFacts,
    current_type: &str,
    current_super_type: Option<&str>,
    declared_return_type: Option<&JavaType>,
    internal_methods: &HashSet<String>,
    field_names: &HashSet<String>,
    field_types: &HashMap<String, JavaType>,
    semantic_facts: &UsageAnalysisContext<'_>,
    type_env: &TypeEnv,
    source_return_nullability: &HashMap<(String, usize), NullabilityState>,
) {
    let saved_bindings = save_direct_var_decl_bindings(facts, statements);
    for stmt in statements {
        analyze_stmt(
            stmt,
            facts,
            current_type,
            current_super_type,
            declared_return_type,
            internal_methods,
            field_names,
            field_types,
            semantic_facts,
            type_env,
            source_return_nullability,
        );
    }
    restore_scoped_binding_uses(facts, saved_bindings);
}

fn save_direct_var_decl_bindings(
    facts: &mut MethodUsageFacts,
    statements: &[Stmt],
) -> Vec<(String, Option<BindingUseFacts>)> {
    let mut saved = Vec::new();
    let mut seen = HashSet::new();
    for stmt in statements {
        if let Stmt::VarDecl { source_name, .. } = stmt
            && seen.insert(source_name.clone())
        {
            saved.push((
                source_name.clone(),
                facts.snapshot_visible_binding_for_scope(source_name),
            ));
        }
    }
    saved
}

fn restore_scoped_binding_uses(
    facts: &mut MethodUsageFacts,
    saved_bindings: Vec<(String, Option<BindingUseFacts>)>,
) {
    for (name, saved) in saved_bindings.into_iter().rev() {
        restore_binding_use_facts(facts, name, saved);
    }
}

#[derive(Debug)]
enum ActiveNonNullBinding {
    Binding(BindingId),
    IdlessName(String),
}

fn activate_branch_non_null_binding(
    facts: &mut MethodUsageFacts,
    condition: &Expr,
    true_branch: bool,
) -> Option<ActiveNonNullBinding> {
    let TernaryNullRefinementPlan::LocalNullTest {
        local_name,
        then_non_null,
        else_non_null,
    } = plan_ternary_null_refinement(condition)
    else {
        return None;
    };
    if !(if true_branch {
        then_non_null
    } else {
        else_non_null
    }) {
        return None;
    }
    if let Some(binding) = facts.visible_binding_id_for_name(local_name) {
        return facts
            .active_non_null_bindings
            .insert(binding.clone())
            .then_some(ActiveNonNullBinding::Binding(binding));
    }
    facts
        .active_non_null_idless_names
        .insert(local_name.to_string())
        .then_some(ActiveNonNullBinding::IdlessName(local_name.to_string()))
}

fn restore_branch_non_null_binding(
    facts: &mut MethodUsageFacts,
    activated: Option<ActiveNonNullBinding>,
) {
    match activated {
        Some(ActiveNonNullBinding::Binding(binding)) => {
            facts.active_non_null_bindings.remove(&binding);
        }
        Some(ActiveNonNullBinding::IdlessName(name)) => {
            facts.active_non_null_idless_names.remove(&name);
        }
        None => {}
    }
}

fn save_true_pattern_bindings(
    facts: &mut MethodUsageFacts,
    expr: &Expr,
) -> Vec<(String, Option<BindingUseFacts>)> {
    let mut saved = Vec::new();
    let mut seen = HashSet::new();
    save_true_pattern_bindings_inner(facts, expr, &mut seen, &mut saved);
    saved
}

fn save_true_pattern_bindings_inner(
    facts: &mut MethodUsageFacts,
    expr: &Expr,
    seen: &mut HashSet<String>,
    saved: &mut Vec<(String, Option<BindingUseFacts>)>,
) {
    match expr {
        Expr::InstanceOf {
            binding: Some(name),
            ty,
            ..
        } => save_pattern_binding(facts, name, ty, seen, saved),
        Expr::Unary {
            op: UnaryOp::Not,
            expr,
        } => save_false_pattern_bindings_inner(facts, expr, seen, saved),
        Expr::Binary {
            op: BinaryOp::And,
            left,
            right,
        } => {
            save_true_pattern_bindings_inner(facts, left, seen, saved);
            save_true_pattern_bindings_inner(facts, right, seen, saved);
        }
        _ => {}
    }
}

fn save_false_pattern_bindings(
    facts: &mut MethodUsageFacts,
    expr: &Expr,
) -> Vec<(String, Option<BindingUseFacts>)> {
    let mut saved = Vec::new();
    let mut seen = HashSet::new();
    save_false_pattern_bindings_inner(facts, expr, &mut seen, &mut saved);
    saved
}

fn save_false_pattern_bindings_inner(
    facts: &mut MethodUsageFacts,
    expr: &Expr,
    seen: &mut HashSet<String>,
    saved: &mut Vec<(String, Option<BindingUseFacts>)>,
) {
    match expr {
        Expr::Unary {
            op: UnaryOp::Not,
            expr,
        } => save_true_pattern_bindings_inner(facts, expr, seen, saved),
        Expr::Binary {
            op: BinaryOp::Or,
            left,
            right,
        } => {
            save_false_pattern_bindings_inner(facts, left, seen, saved);
            save_false_pattern_bindings_inner(facts, right, seen, saved);
        }
        _ => {}
    }
}

fn save_switch_case_pattern_bindings(
    facts: &mut MethodUsageFacts,
    case: &crate::java_ir::SwitchCase,
) -> Vec<(String, Option<BindingUseFacts>)> {
    let mut saved = Vec::new();
    let mut seen = HashSet::new();
    for label in &case.labels {
        if let SwitchLabel::Expr(expr) = label {
            save_true_pattern_bindings_inner(facts, expr, &mut seen, &mut saved);
        }
    }
    saved
}

fn save_switch_expr_case_pattern_bindings(
    facts: &mut MethodUsageFacts,
    case: &crate::java_ir::SwitchExprCase,
) -> Vec<(String, Option<BindingUseFacts>)> {
    let mut saved = Vec::new();
    let mut seen = HashSet::new();
    for label in &case.labels {
        if let SwitchLabel::Expr(expr) = label {
            save_true_pattern_bindings_inner(facts, expr, &mut seen, &mut saved);
        }
    }
    saved
}

fn save_pattern_binding(
    facts: &mut MethodUsageFacts,
    name: &str,
    java_ty: &JavaType,
    seen: &mut HashSet<String>,
    saved: &mut Vec<(String, Option<BindingUseFacts>)>,
) {
    if !seen.insert(name.to_string()) {
        return;
    }
    let saved_binding = facts.snapshot_visible_binding_for_scope(name);
    insert_binding_use_facts(facts, name.to_string(), None, java_ty.clone());
    saved.push((name.to_string(), saved_binding));
}

fn expr_mutates_self_field(
    expr: &Expr,
    facts: &MethodUsageFacts,
    current_type: &str,
    current_super_type: Option<&str>,
    field_names: &HashSet<String>,
    field_types: &HashMap<String, JavaType>,
    semantic_facts: &UsageAnalysisContext<'_>,
) -> bool {
    // Do not conflate mutation of class storage with mutation of `this`.
    // Keep ordinary field-use recording intact for nullable/static storage.
    let target = match expr {
        Expr::Binary {
            op: crate::java_ir::BinaryOp::Assign,
            left,
            ..
        } => Some(left.as_ref()),
        Expr::Unary { expr, .. } => Some(expr.as_ref()),
        Expr::Call {
            receiver: Some(receiver),
            ..
        } => Some(receiver.as_ref()),
        _ => None,
    };
    if target
        .and_then(|target| self_field_name(target, facts, field_names))
        .is_some_and(|name| facts.field_is_static(name))
    {
        return false;
    }
    match expr {
        Expr::Binary {
            op: crate::java_ir::BinaryOp::Assign,
            left,
            ..
        } => {
            array_assignment_targets_self_field(left, facts, field_names)
                || match left.as_ref() {
                    Expr::Var(name) => is_unshadowed_field_name(facts, field_names, name),
                    target => target_is_this_field(target),
                }
        }
        Expr::Unary { expr, .. } => match expr.as_ref() {
            Expr::Var(name) => is_unshadowed_field_name(facts, field_names, name),
            target => {
                target_is_this_field(target)
                    || array_assignment_targets_self_field(target, facts, field_names)
            }
        },
        Expr::Call {
            receiver: Some(receiver),
            name,
            ..
        } if crate::stdlib::native_collection_method_name_mutates_receiver(name)
            && matches!(receiver.as_ref(), Expr::Call { name, args, .. } if name == "get" && args.len() == 1) =>
        {
            expr_targets_self_field(receiver, facts, field_names)
        }
        Expr::Call {
            receiver: Some(receiver),
            name,
            ..
        } if call_receiver_java_type_with_context(
            Some(receiver.as_ref()),
            facts,
            current_type,
            current_super_type,
            field_names,
            field_types,
            semantic_facts,
        )
        .is_some_and(|ty| crate::stdlib::native_collection_receiver_mutates(&ty, name)) =>
        {
            match receiver.as_ref() {
                Expr::Var(name) => is_unshadowed_field_name(facts, field_names, name),
                target => {
                    target_is_this_field(target)
                        || expr_targets_self_field(target, facts, field_names)
                }
            }
        }
        Expr::Call {
            receiver,
            name,
            args,
            ..
        } => args.iter().enumerate().any(|(arg_index, arg)| {
            crate::stdlib::static_collection_argument_mutates(
                receiver.as_deref().and_then(static_receiver_name),
                name,
                args.len(),
                arg_index,
            ) && expr_targets_self_field(arg, facts, field_names)
        }),
        _ => false,
    }
}

fn static_receiver_name(receiver: &Expr) -> Option<&str> {
    match receiver {
        Expr::Var(name) => Some(name),
        _ => None,
    }
}

fn analyze_stored_expr(expr: &Expr, facts: &mut MethodUsageFacts, field_name: &str) {
    if let Expr::Var(name) = expr {
        record_use(
            facts,
            name,
            BindingUse::StoredInField {
                field_name: field_name.to_string(),
            },
        );
    }
}

pub fn call_argument_boundary_policy(
    receiver: Option<&Expr>,
    name: &str,
    internal_methods: &HashSet<String>,
) -> CallArgumentBoundaryPolicy {
    let boundary = if is_content_read_method(name) {
        CallArgumentBoundary::KnownContentRead
    } else if internal_methods.contains(name)
        && receiver.is_none_or(|receiver| matches!(receiver, Expr::This))
    {
        CallArgumentBoundary::InternalSameType
    } else {
        CallArgumentBoundary::Unknown
    };
    CallArgumentBoundaryPolicy {
        name: name.to_string(),
        boundary,
    }
}

fn constructor_result_boundary_policy(
    resolved: Option<&ResolvedConstructor>,
) -> CallArgumentBoundary {
    if resolved.is_some() {
        CallArgumentBoundary::InternalSameType
    } else {
        CallArgumentBoundary::Unknown
    }
}

fn constructor_argument_boundary_policy(
    current_type: &str,
    resolved: Option<&ResolvedConstructor>,
    arg_index: usize,
) -> CallArgumentBoundary {
    let Some(resolved) = resolved else {
        return CallArgumentBoundary::Unknown;
    };
    if resolved.declaring_type == current_type && resolved.params.get(arg_index).is_some() {
        CallArgumentBoundary::InternalSameType
    } else {
        CallArgumentBoundary::Unknown
    }
}

fn constructor_invocation_boundary_policy_for_expr(
    resolved: Option<&ResolvedConstructor>,
) -> CallArgumentBoundary {
    if resolved.is_some() {
        CallArgumentBoundary::InternalSameType
    } else {
        CallArgumentBoundary::Unknown
    }
}

fn resolved_receiver_call_boundary(
    boundary: CallArgumentBoundary,
    receiver: Option<&Expr>,
    resolved: Option<&ResolvedCall>,
) -> CallArgumentBoundary {
    if boundary != CallArgumentBoundary::Unknown {
        return boundary;
    }
    if matches!(receiver, Some(Expr::Super)) {
        return CallArgumentBoundary::InternalSameType;
    }
    let _ = resolved;
    boundary
}

fn modeled_receiver_call_boundary(
    boundary: CallArgumentBoundary,
    receiver_type: Option<&JavaType>,
    name: &str,
    arg_count: usize,
    semantic_facts: &UsageAnalysisContext<'_>,
    type_env: &TypeEnv,
) -> CallArgumentBoundary {
    if boundary != CallArgumentBoundary::Unknown {
        return boundary;
    }
    let Some(receiver_key) = receiver_type.and_then(|receiver_type| {
        crate::stdlib::external_api_receiver_owner_key(receiver_type, type_env)
    }) else {
        return boundary;
    };
    if semantic_facts
        .external_lookup
        .external_api_effect_for_receiver_with_arity(Some(&receiver_key), name, Some(arg_count))
        .is_some_and(|effect| {
            effect.receiver_mutability == crate::facts::ReceiverMutabilityFact::ReadOnly
        })
    {
        return CallArgumentBoundary::KnownContentRead;
    }
    if crate::external_packs::external_api_effect_for_receiver_with_arity(
        Some(&receiver_key),
        name,
        Some(arg_count),
    )
    .is_some_and(|effect| {
        effect.receiver_mutability == crate::facts::ReceiverMutabilityFact::ReadOnly
    }) {
        return CallArgumentBoundary::KnownContentRead;
    }
    let receiver_simple = receiver_key.rsplit('.').next().unwrap_or(&receiver_key);
    if matches!(
        receiver_simple,
        "Math" | "StrictMath" | "Integer" | "Float" | "Double" | "String" | "Objects"
    ) && crate::stdlib::known_external_method_return_type(Some(&receiver_key), name, arg_count)
        .is_some()
    {
        return CallArgumentBoundary::KnownContentRead;
    }
    boundary
}

fn source_receiver_call_boundary(
    boundary: CallArgumentBoundary,
    receiver_type: Option<&JavaType>,
    semantic_facts: &UsageAnalysisContext<'_>,
) -> CallArgumentBoundary {
    if boundary != CallArgumentBoundary::Unknown {
        return boundary;
    };
    let Some(receiver_name) = receiver_type.and_then(usage_java_type_name) else {
        return boundary;
    };
    if semantic_facts
        .type_capability_overrides
        .iter()
        .any(|capability| {
            capability.provenance == crate::facts::FactProvenance::ParsedSource
                && crate::facts::external_type_name_matches(&capability.type_name, receiver_name)
        })
    {
        return CallArgumentBoundary::KnownContentRead;
    }
    boundary
}

pub fn call_argument_use_boundary_policy(
    name: &str,
    arg_index: usize,
    call_boundary: CallArgumentBoundary,
) -> CallArgumentBoundary {
    if crate::stdlib::is_content_read_argument_method(name, arg_index) {
        CallArgumentBoundary::KnownContentRead
    } else {
        call_boundary
    }
}

fn record_use(facts: &mut MethodUsageFacts, name: &str, usage: BindingUse) {
    facts.record_visible_binding_use(name, usage);
}

fn insert_binding_use_facts(
    facts: &mut MethodUsageFacts,
    name: String,
    binding: Option<BindingId>,
    java_ty: JavaType,
) -> Option<BindingUseFacts> {
    facts.insert_visible_binding_for_scope(name, binding, java_ty)
}

fn restore_binding_use_facts(
    facts: &mut MethodUsageFacts,
    name: String,
    saved: Option<BindingUseFacts>,
) {
    facts.restore_visible_binding_for_scope(name, saved);
}

#[cfg(test)]
mod tests {
    #[test]
    fn exact_method_inventory_and_lookup_keep_overloaded_bodies_distinct() {
        let parsed = crate::parser::parse_java_source(
            "class Overloaded { int state; int value() { return state; } int value(int x) { state = x; return state; } }",
        );
        let resolved = crate::resolve::resolve_program(&parsed.program, parsed.diagnostics);
        let usage = super::analyze_usage_with_facts(&resolved.program, &resolved.facts);
        assert_eq!(usage.method_entries().count(), 2);
        assert_eq!(usage.method_usages().count(), 2);
        assert!(
            !usage
                .current_rust_method_usage("Overloaded", "value")
                .unwrap()
                .mutates_self()
        );
        assert!(
            usage
                .method_entries()
                .any(|(key, body)| key.method_name() != "value" && body.mutates_self())
        );
    }

    #[test]
    fn anonymous_sibling_calls_keep_identity_and_exception_edges() {
        let parsed = crate::parser::parse_java_source(include_str!(
            "../tests/corpus/numbers/AnonymousHelperCallProbe.java"
        ));
        let resolved = crate::resolve::resolve_program(&parsed.program, parsed.diagnostics);
        assert!(
            !resolved.diagnostics.has_errors(),
            "{:?}",
            resolved.diagnostics
        );
        let graph = super::build_call_graph_with_facts(&resolved.program, &resolved.facts);
        let helper = graph
            .anonymous_methods()
            .iter()
            .find(|(_, method)| method.name == "helper")
            .unwrap()
            .0;
        let getter = graph
            .anonymous_methods()
            .iter()
            .find(|(_, method)| method.name == "get")
            .unwrap()
            .0;
        assert!(
            graph
                .edges()
                .iter()
                .any(|edge| edge.caller() == getter && edge.callee() == helper),
            "{:?}",
            graph.edges()
        );
        let plan = crate::exception_plan::plan_program_fallibility(
            &resolved.program,
            &graph,
            &resolved.facts,
        );
        assert!(plan.returns_result(helper));
        assert!(plan.returns_result(getter));
        assert!(!plan.returns_result(&super::CallableKey::method_identity(
            "AnonymousHelperCallProbe",
            "make"
        )));
    }

    #[test]
    fn constructed_mapped_receiver_retains_external_callee_identity() {
        let packs = crate::external_packs::parse_external_pack_manifest(
            "external-pack dep.value dep.Value rust-crate-mapping mapped target=value_runtime\nexternal-pack-constructor dep.value dep.Value 1 mapped rust-type=Value rust=new\nexternal-pack-api dep.value dep.Value read mapped getter read-only definitely-non-null args=0 return=int rust=read rust-result=true\n",
        );
        assert!(!packs.diagnostics().has_errors());
        let parsed = crate::parser::parse_java_source(
            "import dep.Value; class Probe { static int run() { return new Value(1).read(); } }",
        );
        let resolved = crate::resolve::resolve_program(&parsed.program, parsed.diagnostics);
        let mut facts = resolved.facts.clone();
        facts.extend_from(packs.facts().clone());
        let graph = super::build_call_graph_with_facts(&resolved.program, &facts);
        assert!(
            graph
                .edges()
                .iter()
                .any(|edge| edge.callee().external_method_parts().is_some_and(
                    |(owner, name, arity)| owner
                        .is_some_and(|owner| owner == "dep.Value" || owner == "Value")
                        && name == "read"
                        && arity == 0
                )),
            "{:?}",
            graph.edges()
        );
    }

    use super::*;
    use crate::binding_id::BindingKind;
    use crate::facts::{
        BorrowSource, BorrowedReturnRecovery, CallableKey as FactCallableKey, FacadeAbiFact,
        FacadeSlot, FactProvenance, ReceiverAbi, ResultAbi, Staticness,
    };
    use crate::java_ir::{PrimitiveType, TypeKind};

    #[test]
    fn unmodifiable_native_views_retain_policy_without_other_alias_requirements() {
        let parsed = crate::parser::parse_java_source(
            r#"
            import java.util.*;
            class Views {
                static void run() {
                    List<String> source = new ArrayList<String>();
                    List<String> view = Collections.unmodifiableList(source);
                }
            }
            "#,
        );
        let resolved = crate::resolve::resolve_program(&parsed.program, parsed.diagnostics);
        let usage = analyze_usage_with_facts(&resolved.program, &resolved.facts);
        let owner = &resolved.program.types[0];
        let method = owner
            .members
            .iter()
            .find_map(|member| match member {
                Member::Method(method) => Some(method),
                _ => None,
            })
            .unwrap();
        let facts = usage.source_method_usage(owner, method).unwrap();
        for name in ["source", "view"] {
            assert!(facts.native_collection_retention_requirements().iter().any(|slot|
                slot.path.is_empty()
                    && matches!(&slot.root, NativeCollectionStorageRoot::Binding(binding) if binding.name() == name)
            ), "{name}");
        }
        assert!(facts.native_collection_storage_flows().iter().any(|(source, target)|
            matches!(&source.root, NativeCollectionStorageRoot::Binding(binding) if binding.name() == "source")
                && matches!(&target.root, NativeCollectionStorageRoot::Binding(binding) if binding.name() == "view")
        ));
    }

    #[test]
    fn native_container_queries_require_carriers_without_alias_or_value_flow() {
        let parsed = crate::parser::parse_java_source(
            r#"
            import java.util.*;
            class Queries {
                static boolean probe(List<Set<String>> values, Set<String> needle) {
                    return values.contains(needle);
                }
            }
        "#,
        );
        let resolved = crate::resolve::resolve_program(&parsed.program, parsed.diagnostics);
        let usage = analyze_usage_with_facts(&resolved.program, &resolved.facts);
        let owner = &resolved.program.types[0];
        let method = owner
            .members
            .iter()
            .find_map(|member| match member {
                Member::Method(m) => Some(m),
                _ => None,
            })
            .unwrap();
        let facts = usage.source_method_usage(owner, method).unwrap();
        assert!(facts.native_collection_storage_flows().is_empty());
        assert!(facts.nullable_collection_storage_flows().is_empty());
        assert!(facts.native_collection_reference_transfers().is_empty());
        assert!(
            facts
                .native_collection_retention_requirements()
                .iter()
                .any(|slot| slot.path == [0])
        );
        assert!(
            facts
                .native_collection_retention_requirements()
                .iter()
                .any(|slot| slot.path.is_empty())
        );
    }

    #[test]
    fn native_container_boundary_flow_does_not_capture_a_source_set() {
        let parsed = crate::parser::parse_java_source(
            r#"
            class Set<T> {}
            class SourceTransfers {
                static Set<String> pass(Set<String> value) { return value; }
            }
        "#,
        );
        let resolved = crate::resolve::resolve_program(&parsed.program, parsed.diagnostics);
        let usage = analyze_usage_with_facts(&resolved.program, &resolved.facts);
        let owner = resolved
            .program
            .types
            .iter()
            .find(|ty| ty.name == "SourceTransfers")
            .unwrap();
        let method = owner
            .members
            .iter()
            .find_map(|member| match member {
                Member::Method(m) => Some(m),
                _ => None,
            })
            .unwrap();
        let facts = usage.source_method_usage(owner, method).unwrap();
        assert!(facts.native_collection_storage_flows().is_empty());
        assert!(facts.native_collection_retention_requirements().is_empty());
        assert!(facts.native_collection_reference_transfers().is_empty());
    }

    #[test]
    fn source_functional_array_domains_connect_parameters_and_returns() {
        let parsed = crate::parser::parse_java_source(include_str!(
            "../tests/corpus/statistics/FunctionalArrayDomainProbe.java"
        ));
        let resolved = crate::resolve::resolve_program(&parsed.program, parsed.diagnostics);
        let usage = analyze_usage_with_facts(&resolved.program, &resolved.facts);
        let owner = &resolved.program.types[0];
        let method = owner
            .members
            .iter()
            .find_map(|member| match member {
                Member::Method(method) if method.name == "retain" => Some(method),
                _ => None,
            })
            .unwrap();
        let facts = usage.source_method_usage(owner, method).unwrap();
        assert!(
            facts
                .native_collection_storage_flows()
                .iter()
                .any(|(source, target)| {
                    source.path == [0]
                        && matches!(
                            target.root,
                            NativeCollectionStorageRoot::MethodArgument { index: 0, .. }
                        )
                }),
            "{facts:#?}"
        );
        assert!(
            facts
                .native_collection_storage_flows()
                .iter()
                .any(|(source, target)| {
                    target.path == [1]
                        && matches!(
                            source.root,
                            NativeCollectionStorageRoot::Origin(
                                CollectionArgumentOrigin::MethodReturn(_)
                            )
                        )
                }),
            "{facts:#?}"
        );
    }

    #[test]
    fn native_container_references_transfer_across_children_and_source_boundaries() {
        let parsed = crate::parser::parse_java_source(
            r#"
            import java.util.*;
            class Receiver { Receiver(Set<String> values) {} }
            class Transfers {
                static Set<String> pass(Set<String> value) { return value; }
                static void send(Map<String, Set<String>> values) {
                    Set<String> child = values.get("key");
                    new Receiver(child);
                    pass(child);
                }
            }
        "#,
        );
        let resolved = crate::resolve::resolve_program(&parsed.program, parsed.diagnostics);
        let usage = analyze_usage_with_facts(&resolved.program, &resolved.facts);
        let owner = resolved
            .program
            .types
            .iter()
            .find(|ty| ty.name == "Transfers")
            .unwrap();
        for member in &owner.members {
            let Member::Method(method) = member else {
                continue;
            };
            let facts = usage.source_method_usage(owner, method).unwrap();
            let flows = facts.native_collection_storage_flows();
            let transfers = facts.native_collection_reference_transfers();
            if method.name == "pass" {
                assert!(
                    transfers.iter().any(|(_, target)| matches!(
                        target.root,
                        NativeCollectionStorageRoot::Return
                    ))
                );
            } else {
                assert!(
                    flows
                        .iter()
                        .any(|(source, target)| source.path == [1] && target.path.is_empty())
                );
                assert!(transfers.iter().any(|(_, target)| matches!(
                    target.root,
                    NativeCollectionStorageRoot::ConstructorArgument { index: 0, .. }
                )));
                assert!(transfers.iter().any(|(_, target)| matches!(
                    target.root,
                    NativeCollectionStorageRoot::MethodArgument { index: 0, .. }
                )));
            }
        }
    }

    #[test]
    fn native_entry_storage_projects_backing_children_but_not_unknown_or_rebound_entries() {
        let parsed = crate::parser::parse_java_source(
            r#"
            import java.util.*;
            class EntryFlows {
                static void read(Map<String, List<String>> map) {
                    for (Map.Entry<String, List<String>> entry : map.entrySet()) {
                        List<String> alias = entry.getValue(); Iterable<String> retained = alias;
                    }
                }
                static void unknown(Map.Entry<String, List<String>> entry) {
                    List<String> alias = entry.getValue(); Iterable<String> retained = alias;
                }
                static void rebound(Map<String, List<String>> map, Map.Entry<String, List<String>> replacement) {
                    for (Map.Entry<String, List<String>> entry : map.entrySet()) {
                        entry = replacement;
                        List<String> alias = entry.getValue(); Iterable<String> retained = alias;
                    }
                }
            }
        "#,
        );
        let resolved = crate::resolve::resolve_program(&parsed.program, parsed.diagnostics);
        let semantic = crate::semantic_analysis::analyze_program(
            &resolved.program,
            &resolved.facts,
            &FactSet::default(),
        );
        let owner = &resolved.program.types[0];
        for method in owner.members.iter().filter_map(|member| match member {
            Member::Method(method) => Some(method),
            _ => None,
        }) {
            let usage = semantic.usage.source_method_usage(owner, method).unwrap();
            if method.name == "read" {
                assert_eq!(usage.native_map_entry_backing_storage.len(), 1);
                assert!(usage.native_collection_storage_flows.iter().any(|(source, target)|
                    source.path == vec![1] && target.path.is_empty()
                    && matches!(&source.root, NativeCollectionStorageRoot::Binding(id) if id.name() == "map")
                    && matches!(&target.root, NativeCollectionStorageRoot::Binding(id) if id.name() == "alias")));
            } else {
                assert!(
                    usage.native_map_entry_backing_storage.is_empty(),
                    "{}",
                    method.name
                );
            }
        }
    }

    #[test]
    fn stored_native_values_link_checked_effects_only_at_callback_invocation() {
        let parsed = crate::parser::parse_java_source(
            r#"
import java.util.function.IntSupplier;
class InlineValues {
    static IntSupplier stored;
    static int invoke(IntSupplier callback) { return callback.getAsInt(); }
    static int unused(IntSupplier callback) { return 1; }
    static int run() { return unused(stored) + invoke(stored); }
}
"#,
        );
        let resolved = crate::resolve::resolve_program(&parsed.program, parsed.diagnostics);
        let callbacks =
            crate::source_callbacks::SourceCallbackPlan::build(&resolved.program, &resolved.facts);
        let graph =
            build_call_graph_with_source_callbacks(&resolved.program, &resolved.facts, &callbacks);
        let owner = &resolved.program.types[0];
        let method_key = |name: &str| {
            let method = owner
                .members
                .iter()
                .find_map(|member| match member {
                    Member::Method(method) if method.name == name => Some(method),
                    _ => None,
                })
                .unwrap();
            CallableKey::source_method_for_type(owner, method)
        };
        let formal = CallableKey::CallbackParameter {
            owner: Box::new(method_key("invoke")),
            index: 0,
        };
        assert!(
            graph.edges().iter().any(|edge| {
                edge.caller() == &formal
                    && edge
                        .callee()
                        .external_method_parts()
                        .is_some_and(|(_, method, arity)| method == "getAsInt_result" && arity == 0)
            }),
            "{graph:#?}"
        );
        assert!(
            graph
                .edges()
                .iter()
                .any(|edge| { edge.caller() == &method_key("invoke") && edge.callee() == &formal }),
            "{graph:#?}"
        );
        assert!(
            !graph.edges().iter().any(|edge| {
                edge.caller() == &method_key("unused")
                    && matches!(edge.callee(), CallableKey::CallbackParameter { .. })
            }),
            "{graph:#?}"
        );
    }

    #[test]
    fn native_callback_fields_retain_the_checked_runtime_call_graph_entry() {
        let parsed = crate::parser::parse_java_source(
            r#"
import java.util.function.IntSupplier;
class Holder {
    private IntSupplier callback;
    static int invoke(Holder holder) { return holder.callback.getAsInt(); }
}
"#,
        );
        let resolved = crate::resolve::resolve_program(&parsed.program, parsed.diagnostics);
        let graph = build_call_graph_with_facts(&resolved.program, &FactSet::default());
        assert!(
            graph.edges().iter().any(|edge| {
                edge.callee()
                    .external_method_parts()
                    .is_some_and(|(owner, method, arity)| {
                        owner.is_some_and(|owner| owner.rsplit('.').next() == Some("IntSupplier"))
                            && method == "getAsInt_result"
                            && arity == 0
                    })
            }),
            "{graph:#?}"
        );
    }

    #[test]
    fn collector_reference_domains_are_directional_and_stop_at_non_null_filters() {
        let parsed = crate::parser::parse_java_source(
            r#"
import java.util.*;
import java.util.stream.Collectors;
class CollectorDomains {
    static Set<String> copy(List<String> source) {
        return source.stream().collect(Collectors.toSet());
    }
    static Set<String> nonNull(List<String> source) {
        return source.stream().filter(Objects::nonNull).collect(Collectors.toSet());
    }
    static Set<String> nullableCopy(List<String> source) {
        Set<String> result = source.stream().collect(Collectors.toSet());
        Iterable<String> retained = result;
        result.add(null);
        return result;
    }
}
"#,
        );
        let resolved = crate::resolve::resolve_program(&parsed.program, parsed.diagnostics);
        assert!(
            !resolved.diagnostics.has_errors(),
            "{:?}",
            resolved.diagnostics
        );
        let semantic = crate::semantic_analysis::analyze_program(
            &resolved.program,
            &resolved.facts,
            &FactSet::default(),
        );
        let representations =
            crate::representation::plan_representations(&resolved.program, &semantic.facts);
        let signatures = crate::signature::solve_signatures_with_facts(
            &resolved.program,
            &representations,
            &semantic.usage,
            &semantic.facts,
        );
        let owner = &resolved.program.types[0];
        for method in owner.members.iter().filter_map(|member| match member {
            Member::Method(method) => Some(method),
            _ => None,
        }) {
            let usage = semantic.usage.source_method_usage(owner, method).unwrap();
            let signature = signatures.method_signature(owner, method).unwrap();
            assert_eq!(
                signature.params()[0].shape().sequence_element_type(),
                Some(JavaType::String),
                "{}",
                method.name
            );
            if method.name == "copy" {
                let flows = usage.nullable_collection_storage_flows();
                assert_eq!(flows.len(), 1, "{flows:?}");
                assert_eq!(flows[0].0.path, vec![0]);
                assert_eq!(flows[0].1.path, vec![0]);
                assert!(matches!(
                    flows[0].1.root,
                    NativeCollectionStorageRoot::Return
                ));
                assert!(usage.native_collection_storage_flows().is_empty());
            } else if method.name == "nonNull" {
                assert!(usage.nullable_collection_storage_flows().is_empty());
            } else {
                assert!(
                    signature
                        .return_shape()
                        .unwrap()
                        .sequence_element_type()
                        .is_some_and(|element| crate::types::optional_element_type(&element)
                            == Some(JavaType::String))
                );
            }
        }
    }

    #[test]
    fn list_view_cast_nullability_does_not_equate_outer_storage_contracts() {
        let parsed = crate::parser::parse_java_source(
            r#"
import java.util.List;
class CastContracts {
    static <T> List<T> cast(Iterable<T> source) { return (List<T>) source; }
    static List<String> identity(List<String> source) {
        List<String> alias = source;
        return alias;
    }
}
"#,
        );
        let resolved = crate::resolve::resolve_program(&parsed.program, parsed.diagnostics);
        assert!(
            !resolved.diagnostics.has_errors(),
            "{:?}",
            resolved.diagnostics
        );
        let semantic = crate::semantic_analysis::analyze_program(
            &resolved.program,
            &resolved.facts,
            &FactSet::default(),
        );
        let owner = &resolved.program.types[0];
        for method in owner.members.iter().filter_map(|member| match member {
            Member::Method(method) => Some(method),
            _ => None,
        }) {
            let usage = semantic.usage.source_method_usage(owner, method).unwrap();
            if method.name == "cast" {
                assert!(
                    !usage
                        .all_binding_facts()
                        .into_iter()
                        .any(|binding| binding.uses().contains(&BindingUse::Returned))
                );
                let flows = usage.nullable_collection_storage_flows();
                assert_eq!(flows.len(), 3, "{flows:?}");
                assert!(flows.iter().any(|(source, target)| source.path.is_empty()
                    && target.path.is_empty()
                    && matches!(target.root, NativeCollectionStorageRoot::Return)));
                assert!(!flows.iter().any(|(source, target)| source.path.is_empty()
                    && target.path.is_empty()
                    && matches!(source.root, NativeCollectionStorageRoot::Return)));
            } else {
                let flows = usage.nullable_collection_storage_flows();
                assert_eq!(flows.len(), 2, "{flows:?}");
                assert!(flows.iter().all(|(source, target)| source.path.is_empty()
                    && target.path.is_empty()
                    && !matches!(source.root, NativeCollectionStorageRoot::Return)));
                assert!(
                    flows.iter().any(|(_, target)| matches!(
                        target.root,
                        NativeCollectionStorageRoot::Return
                    ))
                );
            }
        }
    }

    #[test]
    fn stored_sam_result_flows_use_canonical_destination_slots() {
        let parsed = crate::parser::parse_java_source(include_str!(
            "../tests/fixtures/stored_nullable_callback_results.java"
        ));
        let resolved = crate::resolve::resolve_program(&parsed.program, parsed.diagnostics);
        assert!(
            !resolved.diagnostics.has_errors(),
            "{:?}",
            resolved.diagnostics
        );
        let semantic = crate::semantic_analysis::analyze_program(
            &resolved.program,
            &resolved.facts,
            &FactSet::default(),
        );
        let owner = &resolved.program.types[1];
        let mut checked = 0;
        for member in &owner.members {
            let Member::Method(method) = member else {
                continue;
            };
            if !matches!(
                method.name.as_str(),
                "construct" | "local" | "assigned" | "forwarded" | "stored" | "function"
            ) {
                continue;
            }
            let usage = semantic.usage.source_method_usage(owner, method).unwrap();
            let all_flows = usage.nullable_collection_storage_flows();
            let null_seeds = all_flows
                .iter()
                .filter(|(source, target)| {
                    source == target
                        && source.path.is_empty()
                        && crate::types::optional_element_type(&source.ty).is_some()
                })
                .collect::<Vec<_>>();
            assert_eq!(
                null_seeds.len(),
                usize::from(matches!(method.name.as_str(), "assigned" | "stored"))
            );
            let mut callback_params = Vec::new();
            crate::semantic_analysis::walk_block_exprs(
                method.body.as_ref().unwrap(),
                &mut |expr| {
                    if let Expr::Lambda { params, .. } = expr {
                        callback_params
                            .extend(params.iter().filter_map(|param| param.binding.clone()));
                    }
                },
            );
            let callback_param = |slot: &NativeCollectionStorageSlot| matches!(&slot.root, NativeCollectionStorageRoot::Binding(id) if callback_params.contains(id));
            let formal_flows = all_flows
                .iter()
                .filter(|(source, target)| callback_param(source) || callback_param(target))
                .collect::<Vec<_>>();
            assert_eq!(formal_flows.len(), 2 * callback_params.len());
            for id in &callback_params {
                let incoming = formal_flows.iter().find(|(_, target)| matches!(&target.root, NativeCollectionStorageRoot::Binding(binding) if binding == id)).unwrap();
                assert!(incoming.1.path.is_empty());
                assert_eq!(incoming.0.path, [0]);
                assert_eq!(incoming.0.ty, JavaType::String);
                assert_eq!(incoming.1.ty, JavaType::String);
                assert!(
                    formal_flows
                        .iter()
                        .any(|(source, target)| source == &incoming.1 && target == &incoming.0)
                );
            }
            let returns_callback =
                matches!(method.name.as_str(), "local" | "assigned" | "function");
            // Source-object returns now have ordinary directional presence
            // edges too. The rows object is not itself a native callback.
            let returns_rows = matches!(method.name.as_str(), "stored" | "forwarded");
            let callback_roots = all_flows
                .iter()
                .filter(|(source, target)| {
                    source.path.is_empty() && target.path.is_empty() && source != target
                })
                .collect::<Vec<_>>();
            assert_eq!(
                callback_roots.len(),
                usize::from(returns_callback || returns_rows)
            );
            if returns_callback || returns_rows {
                let (source, target) = callback_roots[0];
                if method.name == "forwarded" {
                    assert!(
                        matches!(&source.root, NativeCollectionStorageRoot::Origin(CollectionArgumentOrigin::MethodReturn(call)) if call.rust_name == "wrap")
                    );
                } else {
                    assert!(matches!(
                        source.root,
                        NativeCollectionStorageRoot::Binding(_)
                    ));
                }
                assert!(matches!(target.root, NativeCollectionStorageRoot::Return));
                assert_eq!(
                    usage
                        .native_collection_reference_transfers()
                        .contains(&(source.clone(), target.clone())),
                    returns_callback
                );
                if method.name == "stored" {
                    assert!(
                        matches!(&source.root, NativeCollectionStorageRoot::Binding(id) if id.name() == "rows")
                    );
                }
            }
            // Function also forwards its input domain from the returned view
            // into the stored callback. Suppliers have no input to forward.
            let input_flows = all_flows
                .iter()
                .filter(|(source, _)| matches!(source.root, NativeCollectionStorageRoot::Return))
                .collect::<Vec<_>>();
            assert_eq!(input_flows.len(), usize::from(method.name == "function"));
            if let Some((source, target)) = input_flows.first() {
                assert_eq!(source.path, [0]);
                assert_eq!(target.path, [0]);
                assert_eq!(source.ty, JavaType::String);
                assert_eq!(target.ty, JavaType::String);
                assert!(matches!(
                    target.root,
                    NativeCollectionStorageRoot::Binding(_)
                ));
            }
            // Check the original result-domain edges separately from the
            // callback allocation reference and contravariant input flow.
            let flows = all_flows
                .iter()
                .filter(|(source, target)| !callback_param(source) && !callback_param(target))
                .filter(|(source, target)| !source.path.is_empty() || !target.path.is_empty())
                .filter(|(source, _)| !matches!(source.root, NativeCollectionStorageRoot::Return))
                .collect::<Vec<_>>();
            assert_eq!(
                flows.len(),
                3 + usize::from(returns_callback),
                "{}: {flows:?}",
                method.name
            );
            if returns_callback {
                let (source, target) = &flows[3];
                assert!(matches!(
                    source.root,
                    NativeCollectionStorageRoot::Binding(_)
                ));
                assert!(matches!(target.root, NativeCollectionStorageRoot::Return));
                let path = vec![usize::from(method.name == "function")];
                assert_eq!(source.path, path);
                assert_eq!(target.path, path);
            }
            let (source, target) = &flows[0];
            // The outer result flows forward; only the shared Iterable's
            // descendant element storage domain is joined in both directions.
            assert_eq!(flows[1].0, flows[2].1);
            assert_eq!(flows[1].1, flows[2].0);
            assert_eq!(flows[1].0.root, source.root);
            assert_eq!(flows[1].1.root, target.root);
            assert_eq!(flows[1].0.path, vec![0]);
            assert_eq!(
                flows[1].1.path,
                vec![usize::from(method.name == "function"), 0]
            );
            assert!(matches!(
                source.root,
                NativeCollectionStorageRoot::Binding(_)
            ));
            assert!(source.path.is_empty());
            assert_eq!(target.path, vec![usize::from(method.name == "function")]);
            assert!(
                match method.name.as_str() {
                    "construct" => matches!(
                        target.root,
                        NativeCollectionStorageRoot::ConstructorArgument { index: 0, .. }
                    ),
                    "forwarded" => matches!(
                        target.root,
                        NativeCollectionStorageRoot::MethodArgument { index: 0, .. }
                    ),
                    "stored" => matches!(
                        target.root,
                        NativeCollectionStorageRoot::Origin(CollectionArgumentOrigin::Field { .. })
                    ),
                    _ => matches!(target.root, NativeCollectionStorageRoot::Binding(_)),
                },
                "{}: {target:?}",
                method.name
            );
            checked += 1;
        }
        assert_eq!(checked, 6);
    }

    #[test]
    fn generic_composition_storage_preserves_hidden_values_and_nullable_local_boundaries() {
        let parsed = crate::parser::parse_java_source(
            r#"
import java.util.function.*;
class CompositionStorage {
    static Function<String, String> after(Function<String, int[]> parse, Function<int[], String> format) {
        return parse.andThen(format);
    }
    static Function<String, String> before(Function<String, int[]> parse, Function<int[], String> format) {
        return format.compose(parse);
    }
    static BiFunction<String, String, String> binary(BiFunction<String, String, int[]> parse, Function<int[], String> format) {
        return parse.andThen(format);
    }
    static Function<String, int[]> nested(Function<String, Supplier<int[]>> source, Function<Supplier<int[]>, int[]> read) {
        return source.andThen(read);
    }
    static Function<String, String> rebound(Function<String, int[]> parse) {
        return parse.andThen(value -> { int[] saved = value; value = null; return String.valueOf(saved[0]); });
    }
    static Function<String, Supplier<int[]>> delayed(Function<String, int[]> parse) {
        return parse.andThen(value -> () -> value);
    }
}
"#,
        );
        let resolved = crate::resolve::resolve_program(&parsed.program, parsed.diagnostics);
        assert!(
            !resolved.diagnostics.has_errors(),
            "{:?}",
            resolved.diagnostics
        );
        let semantic = crate::semantic_analysis::analyze_program(
            &resolved.program,
            &resolved.facts,
            &FactSet::default(),
        );
        let representations =
            crate::representation::plan_representations(&resolved.program, &semantic.facts);
        let signatures = crate::signature::solve_signatures_with_facts(
            &resolved.program,
            &representations,
            &semantic.usage,
            &semantic.facts,
        );
        let owner = &resolved.program.types[0];
        let binding_named = |slot: &NativeCollectionStorageSlot, name: &str, path: &[usize]| {
            matches!(&slot.root, NativeCollectionStorageRoot::Binding(id) if id.name() == name)
                && slot.path == path
        };
        for method in owner.members.iter().filter_map(|member| match member {
            Member::Method(method) => Some(method),
            _ => None,
        }) {
            let usage = semantic.usage.source_method_usage(owner, method).unwrap();
            let flows = usage.native_collection_storage_flows();
            if method.name == "nested" {
                assert!(
                    usage
                        .native_collection_reference_transfers()
                        .iter()
                        .any(|(source, target)| binding_named(source, "source", &[1])
                            && binding_named(target, "read", &[0])),
                    "nested SAM invocation values retain their own facade ABI"
                );
            } else if matches!(method.name.as_str(), "after" | "before" | "binary") {
                let result_index = if method.name == "binary" { 2 } else { 1 };
                assert!(
                    flows.iter().any(|(source, target)| binding_named(
                        source,
                        "parse",
                        &[result_index]
                    ) && binding_named(target, "format", &[0])),
                    "{}: {flows:#?}",
                    method.name
                );
                assert_eq!(usage.native_callback_environment_domains().len(), 2);
                assert!(
                    usage
                        .native_collection_reference_transfers()
                        .iter()
                        .all(|(source, target)| !(source.path.is_empty()
                            && matches!(target.root, NativeCollectionStorageRoot::Return))),
                    "factory results are fresh objects"
                );
                if method.name == "binary" {
                    for index in [0, 1] {
                        assert!(usage.nullable_collection_storage_flows().iter().any(
                            |(source, target)| matches!(
                                source.root,
                                NativeCollectionStorageRoot::Return
                            ) && source.path == [index]
                                && binding_named(target, "parse", &[index])
                        ));
                    }
                }
            } else {
                assert!(
                    flows
                        .iter()
                        .any(|(source, target)| binding_named(source, "parse", &[1])
                            && binding_named(target, "value", &[])),
                    "{}: {flows:#?}",
                    method.name
                );
                assert!(
                    !usage
                        .nullable_collection_storage_flows()
                        .iter()
                        .any(|(source, target)| binding_named(source, "value", &[])
                            && binding_named(target, "parse", &[1])),
                    "consumer local rebinding must not widen its producer"
                );
                let signature = signatures.method_signature(owner, method).unwrap();
                let producer_result = crate::slot_shape::RustSlotShape::native_child_type_at_path(
                    &signature.params()[0].shape().java_ty,
                    &[1],
                )
                .unwrap();
                assert!(
                    crate::types::optional_element_type(&producer_result).is_none(),
                    "producer: {signature:#?}"
                );
                let parameter = signature
                    .locals()
                    .iter()
                    .find(|local| local.name() == "value")
                    .unwrap();
                assert_eq!(
                    parameter.shape().optional_element_type().is_some(),
                    method.name == "rebound",
                    "consumer: {signature:#?}"
                );
                if method.name == "delayed" {
                    assert!(
                        flows
                            .iter()
                            .any(|(source, target)| binding_named(source, "value", &[])
                                && matches!(target.root, NativeCollectionStorageRoot::Return)
                                && target.path == [1, 0]),
                        "{flows:#?}"
                    );
                }
            }
        }
    }

    #[test]
    fn native_sam_result_qualifiers_use_result_argument_paths_and_capture_identities() {
        let parsed = crate::parser::parse_java_source(
            r#"
import java.util.*;
import java.util.function.*;
class Captures<T> {
    List<T> values = new ArrayList<>();
    static <T> Supplier<Iterable<T>> supplier(List<T> values) {
        values.add(null); return () -> values;
    }
    static <T> Function<String, Iterable<T>> function(List<T> values) {
        values.add(null); return ignored -> values;
    }
    Function<List<String>, Iterable<String>> shadow() {
        values.add(null); return values -> values;
    }
}
"#,
        );
        let resolved = crate::resolve::resolve_program(&parsed.program, parsed.diagnostics);
        let semantic = crate::semantic_analysis::analyze_program(
            &resolved.program,
            &resolved.facts,
            &FactSet::default(),
        );
        let representations =
            crate::representation::plan_representations(&resolved.program, &semantic.facts);
        let signatures = crate::signature::solve_signatures_with_facts(
            &resolved.program,
            &representations,
            &semantic.usage,
            &semantic.facts,
        );
        let owner = &resolved.program.types[0];
        for method in owner.members.iter().filter_map(|member| match member {
            Member::Method(method) => Some(method),
            _ => None,
        }) {
            let usage = semantic.usage.source_method_usage(owner, method).unwrap();
            let signature = signatures.method_signature(owner, method).unwrap();
            if method.name == "shadow" {
                // The lambda now has its own input and result domains. None
                // of those edges may resolve its parameter to the outer field.
                let flows = usage.nullable_collection_storage_flows();
                assert_eq!(flows.len(), 5, "{flows:?}");
                assert!(
                    flows.iter().all(|(source, target)| {
                        [&source.root, &target.root].iter().all(|root| {
                            !matches!(
                                root,
                                NativeCollectionStorageRoot::Origin(
                                    CollectionArgumentOrigin::Field { .. }
                                )
                            )
                        })
                    }),
                    "{flows:?}"
                );
                assert!(flows.iter().any(|(source, target)| {
                    matches!(&source.root, NativeCollectionStorageRoot::Binding(id) if id.kind() == crate::binding_id::BindingKind::Local && id.name() == "values")
                        && source.path.is_empty() && target.path == [1]
                }));
                continue;
            }
            assert_eq!(
                usage.nullable_collection_storage_flows().len(),
                if method.name == "supplier" { 3 } else { 5 }
            );
            let result_index = if method.name == "supplier" { 0 } else { 1 };
            let shape = signature.return_shape().unwrap();
            let ty = crate::types::optional_element_type(&shape.java_ty)
                .unwrap_or_else(|| shape.java_ty.clone());
            let JavaType::Named { args, .. } = ty else {
                panic!("{shape:?}");
            };
            let JavaType::Named {
                args: iterable_args,
                ..
            } = &args[result_index]
            else {
                panic!("{shape:?}");
            };
            assert!(
                crate::types::optional_element_type(&iterable_args[0]).is_some(),
                "{}: {shape:?}",
                method.name
            );
        }
    }

    #[test]
    fn checked_sam_field_transfers_preserve_nullable_results_not_outer_allocations() {
        let parsed = crate::parser::parse_java_source(include_str!(
            "../tests/fixtures/deferred_source_views.java"
        ));
        let resolved = crate::resolve::resolve_program(&parsed.program, parsed.diagnostics);
        assert!(
            !resolved.diagnostics.has_errors(),
            "{:?}",
            resolved.diagnostics
        );
        let semantic = crate::semantic_analysis::analyze_program(
            &resolved.program,
            &resolved.facts,
            &FactSet::default(),
        );
        let representations =
            crate::representation::plan_representations(&resolved.program, &semantic.facts);
        let signatures = crate::signature::solve_signatures_with_facts(
            &resolved.program,
            &representations,
            &semantic.usage,
            &semantic.facts,
        );
        let shape = signatures.required_field_shape("CheckedDeferred", "source");
        let facade_owner = resolved
            .program
            .types
            .iter()
            .find(|ty| ty.name == "DeferredSourceViews")
            .unwrap();
        let forwarded = facade_owner
            .members
            .iter()
            .find_map(|member| match member {
                Member::Method(method) if method.name == "forwarded" => Some(method),
                _ => None,
            })
            .unwrap();
        let forwarded_usage = semantic
            .usage
            .source_method_usage(facade_owner, forwarded)
            .unwrap();
        let binding = forwarded_usage
            .all_binding_facts()
            .into_iter()
            .find(|binding| binding.name() == "values")
            .unwrap();
        assert!(
            binding.uses().contains(&BindingUse::RetainedByCall),
            "{binding:?}"
        );
        assert!(!shape.native_allocation.shared(), "{shape:?}");
        let result =
            crate::slot_shape::RustSlotShape::native_child_type_at_path(&shape.java_ty, &[0])
                .expect("Supplier result child");
        assert!(
            crate::types::optional_element_type(&result).is_some(),
            "{shape:?}"
        );
        let owner = &resolved.program.types[0];
        let iterator = owner
            .members
            .iter()
            .find_map(|member| match member {
                Member::Method(method) if method.name == "iterator" => Some(method),
                _ => None,
            })
            .unwrap();
        let result = signatures
            .method_signature(owner, iterator)
            .unwrap()
            .return_shape()
            .unwrap();
        let element =
            crate::slot_shape::RustSlotShape::native_child_type_at_path(&result.java_ty, &[0])
                .unwrap();
        assert!(
            crate::types::optional_element_type(&element).is_some(),
            "{result:?}"
        );
    }

    #[test]
    fn nullable_stream_destinations_do_not_widen_source_element_domains() {
        let parsed = crate::parser::parse_java_source(
            r#"
import java.util.*;
class DirectedTransfers {
    static void copy(List<String> source, List<String> target) {
        target.add(null);
        source.stream().forEach(target::add);
    }
    static void bulk(List<String> source, List<String> target) {
        target.add(null);
        target.addAll(source);
    }
}
"#,
        );
        let resolved = crate::resolve::resolve_program(&parsed.program, parsed.diagnostics);
        let semantic = crate::semantic_analysis::analyze_program(
            &resolved.program,
            &resolved.facts,
            &FactSet::default(),
        );
        let representations =
            crate::representation::plan_representations(&resolved.program, &semantic.facts);
        let signatures = crate::signature::solve_signatures_with_facts(
            &resolved.program,
            &representations,
            &semantic.usage,
            &semantic.facts,
        );
        let ty = &resolved.program.types[0];
        for method in ty.members.iter().filter_map(|member| match member {
            Member::Method(method) => Some(method),
            _ => None,
        }) {
            let signature = signatures.method_signature(ty, method).unwrap();
            assert_eq!(
                signature.params()[0].shape().sequence_element_type(),
                Some(JavaType::String),
                "{}: {:?}",
                method.name,
                signature.params()[0].shape()
            );
            assert!(
                matches!(signature.params()[1].shape().sequence_element_type(), Some(JavaType::Named { name, .. }) if name == "Optional"),
                "{}: {:?}",
                method.name,
                signature.params()[1].shape()
            );
        }
    }

    #[test]
    fn stream_reference_transfers_keep_nullable_elements_in_destination_storage() {
        let parsed = crate::parser::parse_java_source(include_str!(
            "../tests/fixtures/stream_generic_values.java"
        ));
        let resolved = crate::resolve::resolve_program(&parsed.program, parsed.diagnostics);
        let semantic = crate::semantic_analysis::analyze_program(
            &resolved.program,
            &resolved.facts,
            &FactSet::default(),
        );
        let ty = &resolved.program.types[0];
        let method = ty
            .members
            .iter()
            .find_map(|member| match member {
                Member::Method(method) if method.name == "copyAll" => Some(method),
                _ => None,
            })
            .unwrap();
        let facts = semantic.usage.source_method_usage(ty, method).unwrap();
        assert_eq!(
            facts.nullable_collection_storage_flows().len(),
            1,
            "{:?}",
            method.body
        );
        let non_null = ty
            .members
            .iter()
            .find_map(|member| match member {
                Member::Method(method) if method.name == "copyNonNull" => Some(method),
                _ => None,
            })
            .unwrap();
        assert!(
            semantic
                .usage
                .source_method_usage(ty, non_null)
                .unwrap()
                .nullable_collection_storage_flows()
                .is_empty()
        );
        let representations =
            crate::representation::plan_representations(&resolved.program, &semantic.facts);
        let signatures = crate::signature::solve_signatures_with_facts(
            &resolved.program,
            &representations,
            &semantic.usage,
            &semantic.facts,
        );
        let signature = signatures.method_signature(ty, method).unwrap();
        assert!(
            matches!(signature.params()[0].shape().sequence_element_type(), Some(JavaType::Named { name, .. }) if name == "Optional"),
            "{:?}",
            signature.params()[0].shape()
        );
    }

    #[test]
    fn nullable_callback_input_flows_to_source_calls_but_not_past_non_null_filter() {
        let parsed = crate::parser::parse_java_source(
            r#"
            import java.util.Arrays;
            import java.util.Iterator;
            import java.util.Objects;
            class Contracts {
                static boolean accepts(String value) { return true; }
                static boolean accepts(int value) { return false; }
                static boolean reference(String[] values) {
                    return Arrays.stream(values).anyMatch(Contracts::accepts);
                }
                static boolean lambda(String[] values) {
                    return Arrays.stream(values).anyMatch(value -> accepts(value));
                }
                static boolean block(String[] values) {
                    return Arrays.stream(values).anyMatch(value -> { return accepts(value); });
                }
                static Iterator<Boolean> deferred(String[] values) {
                    return Arrays.stream(values).map(Contracts::accepts).iterator();
                }
                static boolean filtered(String[] values) {
                    return Arrays.stream(values).filter(Objects::nonNull).anyMatch(Contracts::accepts);
                }
            }
        "#,
        );
        assert!(!parsed.diagnostics.has_errors(), "{:?}", parsed.diagnostics);
        let resolved = crate::resolve::resolve_program(&parsed.program, parsed.diagnostics);
        let usage = analyze_usage_with_facts(&resolved.program, &resolved.facts);
        let owner = &resolved.program.types[0];
        for member in &owner.members {
            let Member::Method(method) = member else {
                continue;
            };
            if method.name == "accepts" {
                continue;
            }
            let calls = usage
                .source_method_usage(owner, method)
                .unwrap()
                .call_sites();
            let nullable_calls = calls
                .iter()
                .filter(|call| {
                    call.nullable_source_call_arg_indices() == [0]
                        && call
                            .resolved
                            .as_ref()
                            .is_some_and(|resolved| resolved.arg_target_types == [JavaType::String])
                })
                .count();
            assert_eq!(
                nullable_calls > 0,
                method.name != "filtered",
                "{}: {calls:#?}",
                method.name
            );
        }
    }

    #[test]
    fn qualified_collection_static_owner_preserves_argument_effects_without_binding_shadowing() {
        let receiver = Expr::FieldAccess(
            Box::new(Expr::FieldAccess(
                Box::new(Expr::Var("java".to_string())),
                "util".to_string(),
            )),
            "Collections".to_string(),
        );
        let mut usage = MethodUsageFacts::default();
        let owner = call_receiver_java_type(Some(&receiver), &usage);
        assert_eq!(
            owner,
            Some(JavaType::Unknown("java.util.Collections".to_string()))
        );
        let platform_facts = FactSet {
            external_api_argument_effect_facts:
                crate::external_packs::java_platform_api_argument_effect_facts(),
            ..FactSet::default()
        };
        assert!(call_argument_effect_mutates_argument(
            &platform_facts,
            owner.as_ref(),
            "reverse",
            1,
            0,
        ));
        let parsed = crate::parser::parse_java_source("class Collections {}");
        let resolved = crate::resolve::resolve_program(&parsed.program, parsed.diagnostics);
        let mut source_facts = resolved.facts;
        source_facts
            .external_api_argument_effect_facts
            .extend(platform_facts.external_api_argument_effect_facts.clone());
        assert!(
            !call_argument_effect_mutates_argument(
                &source_facts,
                Some(&JavaType::Named {
                    name: "Collections".to_string(),
                    args: Vec::new()
                }),
                "sort",
                2,
                0,
            ),
            "parsed source must not inherit platform mutation effects"
        );
        insert_binding_use_facts(
            &mut usage,
            "java".to_string(),
            None,
            JavaType::Unknown("PackageLikeObject".to_string()),
        );
        assert!(call_receiver_java_type(Some(&receiver), &usage).is_none());
    }

    #[test]
    fn package_prefixed_summary_owner_matches_emitted_owner_but_peer_packages_do_not() {
        assert!(!usage_distinct_qualified_type_owners(
            "org.example.Outer.Inner",
            "Outer.Inner"
        ));
        assert!(usage_distinct_qualified_type_owners("a.Item", "b.Item"));
    }

    #[test]
    fn exact_callable_mutability_crosses_only_package_prefix_owner_aliases() {
        let parsed = crate::parser::parse_java_source(
            "class Outer { class Inner { void touch(int value) {} } }",
        );
        let outer = &parsed.program.types[0];
        let inner = outer
            .members
            .iter()
            .find_map(|member| match member {
                Member::Type(inner) => Some(inner),
                _ => None,
            })
            .expect("nested type");
        let method = inner
            .members
            .iter()
            .find_map(|member| match member {
                Member::Method(method) => Some(method),
                _ => None,
            })
            .expect("nested method");

        let mut usage = UsageFacts::default();
        let mut mutating = MethodUsageFacts::default();
        mutating.mutates_self = true;
        usage.methods_by_java_key.insert(
            JavaCallableKey::source_method_for_owner(
                JavaOwnerKey::resolved_source("org.example.Outer.Inner"),
                method,
            ),
            mutating,
        );

        assert!(usage.source_method_or_alias_mutates(inner, method));

        let mut flattened = inner.clone();
        flattened.name = "Outer_Inner".to_string();
        assert!(usage.source_method_or_alias_mutates(&flattened, method));

        let mut peer = inner.clone();
        peer.name = "other.Outer.Inner".to_string();
        assert!(!usage.source_method_or_alias_mutates(&peer, method));
    }

    fn binding_fact(name: &str, binding: Option<BindingId>) -> BindingUseFacts {
        BindingUseFacts {
            name: name.to_string(),
            binding,
            java_ty: JavaType::String,
            uses: Vec::new(),
        }
    }

    fn resolved_instance_call(declaring_type: &str) -> ResolvedCall {
        ResolvedCall {
            type_args: Vec::new(),
            declaring_type: declaring_type.to_string(),
            rust_name: "read".to_string(),
            kind: crate::java_ir::CallKind::Instance,
            return_type: JavaType::Void,
            arg_target_types: Vec::new(),
            arg_coercions: Vec::new(),
        }
    }

    fn receiver_call_count(binding: &BindingUseFacts, names: &[&str]) -> usize {
        binding
            .uses
            .iter()
            .filter(|usage| {
                matches!(
                    usage,
                    BindingUse::ReceiverForCall { name, .. } if names.contains(&name.as_str())
                )
            })
            .count()
    }

    #[test]
    fn usage_source_key_constructors_name_canonical_and_legacy_paths() {
        let parsed = crate::parser::parse_java_source(
            r#"
class KeyOwner {
    KeyOwner(int value) {}

    void camelCase() {}
}
"#,
        );
        let resolved = crate::resolve::resolve_program(&parsed.program, parsed.diagnostics);
        let ty = &resolved.program.types[0];
        let constructor = ty
            .members
            .iter()
            .find_map(|member| match member {
                Member::Constructor(constructor) => Some(constructor),
                _ => None,
            })
            .expect("constructor");
        let method = ty
            .members
            .iter()
            .find_map(|member| match member {
                Member::Method(method) if method.name == "camelCase" => Some(method),
                _ => None,
            })
            .expect("method");

        assert_eq!(
            UsageMethodKey::source_method(ty, method),
            UsageMethodKey::current_rust_method_name("KeyOwner", "camel_case")
        );
        assert_eq!(
            UsageMethodKey::legacy_java_name_source_method(ty, method),
            UsageMethodKey::current_rust_method_name("KeyOwner", "camelCase")
        );
        assert_eq!(
            UsageMethodKey::current_rust_method_name("KeyOwner", "camel_case"),
            UsageMethodKey::current_rust_method_name("KeyOwner", "camel_case")
        );
        assert_eq!(
            UsageConstructorKey::source_constructor(ty, constructor),
            UsageConstructorKey::current_rust_constructor_name("KeyOwner", "new")
        );
        assert_eq!(
            UsageConstructorKey::current_rust_constructor_name("KeyOwner", "new"),
            UsageConstructorKey::current_rust_constructor_name("KeyOwner", "new")
        );
    }

    #[test]
    fn local_initialized_from_known_subtype_records_subtype_dispatch_use() {
        let parsed = crate::parser::parse_java_source(
            r#"
class Use {
    void run() {
        Account a = new Savings();
        a.label();
    }
}
class Account {
    String label() { return "account"; }
}
class Savings extends Account {
    String label() { return "savings"; }
}
"#,
        );
        let resolved = crate::resolve::resolve_program(&parsed.program, parsed.diagnostics);
        let usage = analyze_usage_with_facts(&resolved.program, &resolved.facts);
        let method = UsageMethodKey::current_rust_method_name("Use", "run");
        let binding = usage
            .method_usage_by_key(&method)
            .and_then(|method| method.current_visible_binding_facts_for_name("a"))
            .expect("local binding");

        assert!(binding.uses().iter().any(|use_site| {
            matches!(
                use_site,
                BindingUse::AssignedSubtypeToLocal {
                    local_name,
                    actual_type: JavaType::Named { name, .. },
                    ..
                } if local_name == "a" && name == "Savings"
            )
        }));
    }

    #[test]
    fn collection_add_known_subtype_records_element_subtype_dispatch_use() {
        let parsed = crate::parser::parse_java_source(
            r#"
class Use {
    void run() {
        java.util.ArrayList<Account> accounts = new java.util.ArrayList<Account>();
        accounts.add(new Savings());
    }
}
class Account {
    String label() { return "account"; }
}
class Savings extends Account {
    String label() { return "savings"; }
}
"#,
        );
        let resolved = crate::resolve::resolve_program(&parsed.program, parsed.diagnostics);
        let usage = analyze_usage_with_facts(&resolved.program, &resolved.facts);
        let method = UsageMethodKey::current_rust_method_name("Use", "run");
        let binding = usage
            .method_usage_by_key(&method)
            .and_then(|method| method.current_visible_binding_facts_for_name("accounts"))
            .expect("collection binding");

        assert!(binding.uses().iter().any(|use_site| {
            matches!(
                use_site,
                BindingUse::AssignedSubtypeToCollectionElement {
                    collection_name,
                    element_type: JavaType::Named { name: element_name, .. },
                    actual_type: JavaType::Named { name: actual_name, .. },
                    ..
                } if collection_name == "accounts"
                    && element_name == "Account"
                    && actual_name == "Savings"
            )
        }));
    }

    #[test]
    fn collection_field_add_bound_subtype_records_field_element_dispatch_use() {
        let parsed = crate::parser::parse_java_source(
            r#"
class Base {}
class Mid extends Base {}
class Standards<T extends Mid> {
    java.util.List<Base> lines = new java.util.ArrayList<Base>();

    void add(T line) {
        this.lines.add(line);
    }
}
"#,
        );
        let resolved = crate::resolve::resolve_program(&parsed.program, parsed.diagnostics);
        let usage = analyze_usage_with_facts(&resolved.program, &resolved.facts);
        let method = UsageMethodKey::current_rust_method_name("Standards", "add");
        let method_usage = usage
            .method_usage_by_key(&method)
            .expect("method usage facts");

        assert!(method_usage.field_uses().iter().any(|use_site| {
            matches!(
                use_site,
                FieldUse::AssignedSubtypeToCollectionElement {
                    field_name,
                    element_type: JavaType::Named { name: element_name, .. },
                    actual_type: JavaType::Named { name: actual_name, .. },
                } if field_name == "lines" && element_name == "Base" && actual_name == "Mid"
            )
        }));
    }

    #[test]
    fn static_storage_updates_do_not_mutate_instance_receiver() {
        let parsed = crate::parser::parse_java_source(
            r#"
class StaticUsageBase { static int inheritedCount; }
class StaticUsage extends StaticUsageBase {
    static int totalCount;
    int value;
    int unary() { totalCount++; inheritedCount++; return value; }
    void assign() { totalCount = 3; this.totalCount = 4; }
    void instance() { value++; }
    int shadowed() { int totalCount = 0; totalCount++; return totalCount; }
}
class HidesStatic extends StaticUsageBase {
    int inheritedCount;
    void instance() { inheritedCount++; }
}
"#,
        );
        let resolved = crate::resolve::resolve_program(&parsed.program, parsed.diagnostics);
        let usage = analyze_usage_with_facts(&resolved.program, &resolved.facts);
        for name in ["unary", "assign", "shadowed"] {
            let key = UsageMethodKey::current_rust_method_name("StaticUsage", name);
            assert!(
                !usage.method_usage_by_key(&key).unwrap().mutates_self(),
                "{name}"
            );
        }
        for owner in ["StaticUsage", "HidesStatic"] {
            let key = UsageMethodKey::current_rust_method_name(owner, "instance");
            assert!(
                usage.method_usage_by_key(&key).unwrap().mutates_self(),
                "{owner}"
            );
        }
    }

    #[test]
    fn inherited_collection_field_mutation_marks_subclass_method_mutating() {
        let parsed = crate::parser::parse_java_source(
            r#"
class CacheBase {
    java.util.Map<String, String> cache = new java.util.HashMap<String, String>();
}
class CacheChild extends CacheBase {
    void store(String key, String value) {
        cache.put(key, value);
    }
}
"#,
        );
        let resolved = crate::resolve::resolve_program(&parsed.program, parsed.diagnostics);
        let usage = analyze_usage_with_facts(&resolved.program, &resolved.facts);
        let method = UsageMethodKey::current_rust_method_name("CacheChild", "store");
        let method_usage = usage
            .method_usage_by_key(&method)
            .expect("subclass method usage facts");

        assert!(method_usage.mutates_self);
    }

    #[test]
    fn source_method_known_subtype_argument_records_target_subtype_dispatch_use() {
        let parsed = crate::parser::parse_java_source(
            r#"
class Base {
    int id;
}
class Sub extends Base {
}
class Use {
    Base keep(Base value) {
        return value;
    }
    void run(Sub value) {
        keep(value);
    }
}
"#,
        );
        let resolved = crate::resolve::resolve_program(&parsed.program, parsed.diagnostics);
        let usage = analyze_usage_with_facts(&resolved.program, &resolved.facts);
        let method = UsageMethodKey::current_rust_method_name("Use", "run");
        let binding = usage
            .method_usage_by_key(&method)
            .and_then(|method| method.current_visible_binding_facts_for_name("value"))
            .expect("argument binding");

        assert!(binding.uses().iter().any(|use_site| {
            matches!(
                use_site,
                BindingUse::AssignedSubtypeToCallArgument {
                    target_type: JavaType::Named { name: target_name, .. },
                    actual_type: JavaType::Named { name: actual_name, .. },
                } if target_name == "Base" && actual_name == "Sub"
            )
        }));
    }

    #[test]
    fn erased_downcast_local_passed_to_source_supertype_records_dispatch_use() {
        let parsed = crate::parser::parse_java_source(
            r#"
class HeaderLine {}
class CompoundLine extends HeaderLine {}
class FormatLine extends CompoundLine {}
class Use {
    static void accept(CompoundLine line) {}
    static void run() {
        Object erased = new FormatLine();
        FormatLine format = (FormatLine) erased;
        accept(format);
    }
}
"#,
        );
        let resolved = crate::resolve::resolve_program(&parsed.program, parsed.diagnostics);
        let usage = analyze_usage_with_facts(&resolved.program, &resolved.facts);
        let method = UsageMethodKey::current_rust_method_name("Use", "run");
        let binding = usage
            .method_usage_by_key(&method)
            .and_then(|method| method.current_visible_binding_facts_for_name("format"))
            .expect("downcast local binding");

        assert!(
            binding.uses().iter().any(|use_site| {
                matches!(
                    use_site,
                    BindingUse::AssignedSubtypeToCallArgument {
                        target_type: JavaType::Named { name: target_name, .. },
                        actual_type: JavaType::Named { name: actual_name, .. },
                    } if target_name == "CompoundLine" && actual_name == "FormatLine"
                )
            }),
            "uses={:?}",
            binding.uses()
        );
    }

    #[test]
    fn fresh_source_subtype_argument_records_target_subtype_dispatch_use() {
        let parsed = crate::parser::parse_java_source(
            r#"
class Base {
    int id;
}
class Sub extends Base {
}
class Use {
    Base keep(Base value) {
        return value;
    }
    void run() {
        keep(new Sub());
    }
}
"#,
        );
        let resolved = crate::resolve::resolve_program(&parsed.program, parsed.diagnostics);
        let usage = analyze_usage_with_facts(&resolved.program, &resolved.facts);
        let method = UsageMethodKey::current_rust_method_name("Use", "run");
        let usage = usage
            .method_usage_by_key(&method)
            .expect("method usage facts");

        assert!(usage.field_uses().iter().any(|use_site| {
            matches!(
                use_site,
                FieldUse::AssignedSubtypeToCallArgument {
                    target_type: JavaType::Named { name: target_name, .. },
                    actual_type: JavaType::Named { name: actual_name, .. },
                } if target_name == "Base" && actual_name == "Sub"
            )
        }));
    }

    #[test]
    fn type_param_bound_supertype_assignment_records_bound_subtype_dispatch_use() {
        let parsed = crate::parser::parse_java_source(
            r#"
class HeaderLine {
    String id;
}
class CompoundLine extends HeaderLine {
}
class Standards<T extends CompoundLine> {
    void accept(HeaderLine line) {
    }
    void run(T line) {
        accept(line);
    }
}
"#,
        );
        let resolved = crate::resolve::resolve_program(&parsed.program, parsed.diagnostics);
        let usage = analyze_usage_with_facts(&resolved.program, &resolved.facts);
        let method = UsageMethodKey::current_rust_method_name("Standards", "run");
        let binding = usage
            .method_usage_by_key(&method)
            .and_then(|method| method.current_visible_binding_facts_for_name("line"))
            .expect("argument binding");

        assert!(binding.uses().iter().any(|use_site| {
            matches!(
                use_site,
                BindingUse::AssignedSubtypeToCallArgument {
                    target_type: JavaType::Named { name: target_name, .. },
                    actual_type: JavaType::Named { name: actual_name, .. },
                    ..
                } if target_name == "HeaderLine" && actual_name == "CompoundLine"
            )
        }));
    }

    #[test]
    fn erased_object_cast_records_target_type_on_source_binding() {
        let parsed = crate::parser::parse_java_source(
            r#"
class Use {
    void run(Object value) {
        Base base = (Base) value;
    }
}
class Base {
}
"#,
        );
        let resolved = crate::resolve::resolve_program(&parsed.program, parsed.diagnostics);
        let usage = analyze_usage_with_facts(&resolved.program, &resolved.facts);
        let method = UsageMethodKey::current_rust_method_name("Use", "run");
        let binding = usage
            .method_usage_by_key(&method)
            .and_then(|method| method.current_visible_binding_facts_for_name("value"))
            .expect("erased binding");

        assert!(binding.uses().iter().any(|use_site| {
            matches!(
                use_site,
                BindingUse::CastTarget {
                    target_ty: JavaType::Named { name, .. },
                } if name == "Base"
            )
        }));
    }

    #[test]
    fn source_constructor_direct_known_subtype_argument_records_target_subtype_dispatch_use() {
        let parsed = crate::parser::parse_java_source(
            r#"
class Base {
    int id;
}
class Sub extends Base {
}
class Holder {
    Base value;
    Holder(Base value) {
        this.value = value;
    }
}
class Use {
    Holder make() {
        return new Holder(new Sub());
    }
}
"#,
        );
        let resolved = crate::resolve::resolve_program(&parsed.program, parsed.diagnostics);
        let usage = analyze_usage_with_facts(&resolved.program, &resolved.facts);
        let method = UsageMethodKey::current_rust_method_name("Use", "make");
        let method_usage = usage
            .method_usage_by_key(&method)
            .expect("method usage should be recorded");

        assert!(method_usage.field_uses().iter().any(|use_site| {
            matches!(
                use_site,
                FieldUse::AssignedSubtypeToCallArgument {
                    target_type: JavaType::Named { name: target_name, .. },
                    actual_type: JavaType::Named { name: actual_name, .. },
                } if target_name == "Base" && actual_name == "Sub"
            )
        }));
    }

    #[test]
    fn optional_return_known_subtype_records_target_subtype_dispatch_use() {
        let parsed = crate::parser::parse_java_source(
            r#"
import java.util.Optional;
class Base {
    int id;
}
class Sub extends Base {
}
class Use {
    Optional<Base> make() {
        return Optional.of(new Sub());
    }
}
"#,
        );
        let resolved = crate::resolve::resolve_program(&parsed.program, parsed.diagnostics);
        let usage = analyze_usage_with_facts(&resolved.program, &resolved.facts);
        let method = UsageMethodKey::current_rust_method_name("Use", "make");
        let method_usage = usage
            .method_usage_by_key(&method)
            .expect("method usage should be recorded");

        assert!(
            method_usage.field_uses().iter().any(|use_site| {
                matches!(
                    use_site,
                    FieldUse::AssignedSubtypeToReturn {
                        target_type: JavaType::Named { name: target_name, .. },
                        actual_type: JavaType::Named { name: actual_name, .. },
                    } if target_name == "Base" && actual_name == "Sub"
                )
            }),
            "field_uses={:?}",
            method_usage.field_uses()
        );
    }

    #[test]
    fn resolved_receiver_boundary_does_not_turn_source_instance_calls_into_content_reads() {
        let resolved = resolved_instance_call("a.Item");

        assert_eq!(
            resolved_receiver_call_boundary(CallArgumentBoundary::Unknown, None, Some(&resolved)),
            CallArgumentBoundary::Unknown
        );
    }

    #[test]
    fn all_binding_facts_includes_id_facts_and_legacy_name_only_facts_once() {
        let first = BindingId::new(BindingKind::Local, "value", 0);
        let second = BindingId::new(BindingKind::Local, "value", 1);
        let mut facts = MethodUsageFacts::default();
        facts
            .bindings_by_id
            .insert(first.clone(), binding_fact("value", Some(first.clone())));
        facts
            .bindings_by_id
            .insert(second.clone(), binding_fact("value", Some(second.clone())));
        facts
            .bindings
            .insert("value".to_string(), binding_fact("value", Some(second)));
        facts
            .bindings
            .insert("legacy".to_string(), binding_fact("legacy", None));

        let mut seen = facts
            .all_binding_facts()
            .into_iter()
            .map(|binding| {
                (
                    binding.name.clone(),
                    binding.binding.as_ref().map(|binding| binding.occurrence()),
                )
            })
            .collect::<Vec<_>>();
        seen.sort();

        assert_eq!(
            seen,
            vec![
                ("legacy".to_string(), None),
                ("value".to_string(), Some(0)),
                ("value".to_string(), Some(1)),
            ]
        );
    }

    #[test]
    fn binding_fact_constructors_separate_exact_and_idless_surfaces() {
        let binding = BindingId::new(BindingKind::Local, "value", 0);
        let bound = binding_fact("value", Some(binding.clone()));
        let idless = binding_fact("legacy", None);

        let exact = MethodUsageFacts::from_exact_binding_facts([bound.clone(), idless.clone()]);
        assert!(exact.binding_facts_for_id(&binding).is_some());
        assert!(
            exact
                .idless_compatibility_binding_facts_for_source_name("legacy")
                .is_none()
        );
        assert!(exact.visible_binding_id_for_name("value").is_none());

        let compatibility =
            MethodUsageFacts::from_idless_compatibility_facts([bound.clone(), idless.clone()]);
        assert!(compatibility.binding_facts_for_id(&binding).is_none());
        assert!(
            compatibility
                .idless_compatibility_binding_facts_for_source_name("legacy")
                .is_some()
        );
        assert!(compatibility.visible_binding_id_for_name("value").is_none());

        let visible = MethodUsageFacts::from_visible_binding_fixture_facts([bound]);
        assert!(visible.binding_facts_for_id(&binding).is_some());
        assert_eq!(visible.visible_binding_id_for_name("value"), Some(binding));
    }

    #[test]
    fn all_binding_facts_deduplicates_by_binding_id_key() {
        let binding = BindingId::new(BindingKind::Local, "value", 1);
        let mut facts = MethodUsageFacts::default();
        facts.bindings_by_id.insert(
            binding.clone(),
            BindingUseFacts {
                binding: None,
                ..binding_fact("value", None)
            },
        );
        facts
            .bindings
            .insert("value".to_string(), binding_fact("value", Some(binding)));

        let all = facts.all_binding_facts();

        assert_eq!(
            all.iter().filter(|fact| fact.name == "value").count(),
            1,
            "{all:?}"
        );
        assert_eq!(all[0].binding, None);
    }

    #[test]
    fn binding_facts_for_declaration_id_does_not_fall_back_by_name() {
        let first = BindingId::new(BindingKind::Local, "value", 0);
        let second = BindingId::new(BindingKind::Local, "value", 1);
        let mut facts = MethodUsageFacts::default();
        facts
            .bindings_by_id
            .insert(first.clone(), binding_fact("value", Some(first)));
        facts.bindings.insert(
            "value".to_string(),
            binding_fact("value", Some(second.clone())),
        );

        assert!(
            facts.binding_facts_for_declaration_id(&second).is_none(),
            "a resolver-assigned declaration id must not silently rejoin by name"
        );
        assert!(
            facts
                .idless_compatibility_binding_facts_for_source_name("value")
                .is_none(),
            "id-less compatibility callers must reject stale name facts that point at a missing binding id"
        );
    }

    #[test]
    fn id_less_name_fallback_rejects_same_name_bound_history() {
        let bound = BindingId::new(BindingKind::Local, "value", 0);
        let mut facts = MethodUsageFacts::default();
        facts.bindings_by_id.insert(
            bound.clone(),
            BindingUseFacts {
                java_ty: JavaType::String,
                ..binding_fact("value", Some(bound))
            },
        );
        facts.bindings.insert(
            "value".to_string(),
            BindingUseFacts {
                binding: None,
                java_ty: JavaType::Primitive(PrimitiveType::Int),
                ..binding_fact("value", None)
            },
        );

        assert!(
            facts
                .idless_compatibility_binding_facts_for_source_name("value")
                .is_none(),
            "id-less fallback cannot prove which same-name declaration it represents"
        );
        assert_eq!(
            call_receiver_java_type(Some(&Expr::Var("value".to_string())), &facts),
            None,
            "call receiver type must fail closed instead of using an id-less stale name fact"
        );
    }

    #[test]
    fn ambiguous_id_less_name_map_entry_is_not_current_or_aggregated() {
        let bound = BindingId::new(BindingKind::Local, "value", 0);
        let mut facts = MethodUsageFacts::default();
        facts
            .bindings_by_id
            .insert(bound.clone(), binding_fact("value", Some(bound)));
        facts
            .bindings
            .insert("value".to_string(), binding_fact("value", None));

        assert!(!facts.name_binding_is_current("value"));
        assert_eq!(
            facts.all_binding_facts().len(),
            1,
            "ambiguous id-less name-map facts must not leak through aggregate readers"
        );
        assert!(
            is_unshadowed_field_name(&facts, &HashSet::from(["value".to_string()]), "value"),
            "an ambiguous id-less local cannot shadow a same-name field"
        );

        record_use(&mut facts, "value", BindingUse::Returned);

        assert!(
            facts
                .bindings
                .get("value")
                .is_some_and(|binding| binding.uses.is_empty()),
            "ambiguous id-less name-map facts must not collect new uses"
        );
        assert_eq!(
            facts
                .bindings_by_id
                .values()
                .flat_map(|binding| binding.uses.iter())
                .count(),
            0,
            "without a visible binding id, record_use must fail closed"
        );
    }

    #[test]
    fn visible_binding_id_for_name_centralizes_compatibility_lookup() {
        let binding = BindingId::new(BindingKind::Local, "value", 1);
        let mut facts = MethodUsageFacts::default();
        facts.bindings.insert(
            "value".to_string(),
            binding_fact("value", Some(binding.clone())),
        );
        facts.bindings_by_id.insert(
            binding.clone(),
            binding_fact("value", Some(binding.clone())),
        );

        assert_eq!(facts.visible_binding_id_for_name("value"), Some(binding));
        assert_eq!(
            facts.visible_binding_java_type_for_name("value"),
            Some(&JavaType::String)
        );
        assert_eq!(
            facts.receiver_handle_java_type("value"),
            Some(&JavaType::String)
        );
        assert_eq!(facts.visible_binding_id_for_name("missing"), None);
        assert_eq!(facts.visible_binding_java_type_for_name("missing"), None);

        let compatibility =
            MethodUsageFacts::from_idless_compatibility_facts([binding_fact("legacy", None)]);
        assert!(
            compatibility
                .idless_compatibility_binding_facts_for_source_name("legacy")
                .is_some()
        );
        assert_eq!(compatibility.visible_binding_id_for_name("legacy"), None);
        assert_eq!(compatibility.receiver_handle_java_type("legacy"), None);
    }

    #[test]
    fn visible_binding_id_for_name_rejects_stale_name_map_binding() {
        let binding = BindingId::new(BindingKind::Local, "value", 1);
        let mut facts = MethodUsageFacts::default();
        facts
            .bindings
            .insert("value".to_string(), binding_fact("value", Some(binding)));

        assert_eq!(facts.visible_binding_id_for_name("value"), None);
        assert!(!facts.name_binding_is_current("value"));
        assert!(
            facts
                .idless_compatibility_binding_facts_for_source_name("value")
                .is_none()
        );
        assert!(facts.all_binding_facts().is_empty());
    }

    #[test]
    fn binding_name_fallback_returns_authoritative_live_id_fact() {
        let binding = BindingId::new(BindingKind::Local, "value", 1);
        let mut facts = MethodUsageFacts::default();
        facts.bindings_by_id.insert(
            binding.clone(),
            BindingUseFacts {
                java_ty: JavaType::String,
                uses: vec![BindingUse::Returned],
                ..binding_fact("value", Some(binding.clone()))
            },
        );
        facts.bindings.insert(
            "value".to_string(),
            BindingUseFacts {
                java_ty: JavaType::Primitive(PrimitiveType::Int),
                uses: Vec::new(),
                ..binding_fact("value", Some(binding))
            },
        );

        let selected = facts
            .idless_compatibility_binding_facts_for_source_name("value")
            .expect("live binding-id fallback should resolve through bindings_by_id");

        assert_eq!(selected.java_ty, JavaType::String);
        assert_eq!(selected.uses, vec![BindingUse::Returned]);
    }

    #[test]
    fn binding_facts_for_param_uses_source_name_and_binding_id() {
        let bound = BindingId::new(BindingKind::Local, "type", 1);
        let shadow = BindingId::new(BindingKind::Local, "type", 2);
        let mut facts = MethodUsageFacts::default();
        facts.bindings_by_id.insert(
            bound.clone(),
            BindingUseFacts {
                java_ty: JavaType::String,
                uses: vec![BindingUse::Returned],
                ..binding_fact("type", Some(bound.clone()))
            },
        );
        facts.bindings_by_id.insert(
            shadow.clone(),
            BindingUseFacts {
                java_ty: JavaType::Primitive(PrimitiveType::Int),
                uses: Vec::new(),
                ..binding_fact("type", Some(shadow.clone()))
            },
        );
        facts.bindings.insert(
            "type".to_string(),
            BindingUseFacts {
                java_ty: JavaType::Primitive(PrimitiveType::Long),
                uses: Vec::new(),
                ..binding_fact("type", Some(shadow))
            },
        );

        let bound_param = Param {
            name: "type_".to_string(),
            source_name: "type".to_string(),
            ty: JavaType::String,
            is_varargs: false,
            binding: Some(bound),
        };
        let idless_param = Param {
            name: "type_".to_string(),
            source_name: "type".to_string(),
            ty: JavaType::Primitive(PrimitiveType::Long),
            is_varargs: false,
            binding: None,
        };

        assert_eq!(
            facts
                .binding_facts_for_param(&bound_param)
                .map(|binding| (&binding.java_ty, &binding.uses)),
            Some((&JavaType::String, &vec![BindingUse::Returned])),
            "bound parameters must use the resolver binding id instead of the legacy name slot"
        );
        assert_eq!(
            facts
                .binding_facts_for_param(&idless_param)
                .map(|binding| &binding.java_ty),
            Some(&JavaType::Primitive(PrimitiveType::Int)),
            "id-less compatibility parameters fall back through the source-name policy"
        );
    }

    #[test]
    fn record_use_records_live_binding_id_once() {
        let binding = BindingId::new(BindingKind::Local, "value", 1);
        let mut facts = MethodUsageFacts::default();
        facts.bindings.insert(
            "value".to_string(),
            binding_fact("value", Some(binding.clone())),
        );
        facts.bindings_by_id.insert(
            binding.clone(),
            binding_fact("value", Some(binding.clone())),
        );

        record_use(&mut facts, "value", BindingUse::Returned);

        assert_eq!(
            facts
                .bindings_by_id
                .get(&binding)
                .expect("live binding should collect the use")
                .uses,
            vec![BindingUse::Returned]
        );
        assert!(
            facts
                .bindings
                .get("value")
                .expect("compatibility name entry remains present")
                .uses
                .is_empty(),
            "live binding-id use recording should not duplicate through the name map"
        );
    }

    #[test]
    fn keyword_renamed_bound_local_records_uses_by_source_binding() {
        let binding = BindingId::new(BindingKind::Local, "self", 0);
        let stmt = Stmt::VarDecl {
            ty: JavaType::String,
            name: "self__".to_string(),
            source_name: "self".to_string(),
            initializer: Some(Expr::Var("input".to_string())),
            binding: Some(binding.clone()),
            is_final: false,
        };
        let mut facts = MethodUsageFacts::default();
        insert_binding_use_facts(
            &mut facts,
            "input".to_string(),
            Some(BindingId::new(BindingKind::Param, "input", 0)),
            JavaType::String,
        );
        let semantic_facts = FactSet::default();
        let semantic_context = UsageAnalysisContext::new(&semantic_facts);

        analyze_stmt(
            &stmt,
            &mut facts,
            "Owner",
            None,
            None,
            &HashSet::new(),
            &HashSet::new(),
            &HashMap::new(),
            &semantic_context,
            &TypeEnv::new(),
            &HashMap::new(),
        );
        record_use(&mut facts, "self", BindingUse::Returned);

        assert_eq!(
            facts.visible_binding_id_for_name("self"),
            Some(binding.clone())
        );
        assert_eq!(facts.visible_binding_id_for_name("self__"), None);
        assert_eq!(
            facts
                .bindings_by_id
                .get(&binding)
                .expect("source binding id should be visible")
                .uses,
            vec![BindingUse::Returned]
        );
    }

    #[test]
    fn name_map_binding_id_must_match_the_queried_name() {
        let other = BindingId::new(BindingKind::Local, "other", 1);
        let mut facts = MethodUsageFacts::default();
        facts
            .bindings_by_id
            .insert(other.clone(), binding_fact("other", Some(other.clone())));
        facts
            .bindings
            .insert("value".to_string(), binding_fact("value", Some(other)));

        assert_eq!(facts.visible_binding_id_for_name("value"), None);
        assert!(!facts.name_binding_is_current("value"));
        assert!(
            facts
                .idless_compatibility_binding_facts_for_source_name("value")
                .is_none(),
            "name-map compatibility must not rejoin value to an unrelated live binding id"
        );
        assert!(
            facts.snapshot_visible_binding_for_scope("value").is_none(),
            "scope snapshots must not preserve a name entry whose binding id belongs to another name"
        );
    }

    #[test]
    fn name_map_payload_name_must_match_the_lookup_key() {
        let mut facts = MethodUsageFacts::default();
        facts.bindings.insert(
            "value".to_string(),
            BindingUseFacts {
                name: "other".to_string(),
                binding: None,
                java_ty: JavaType::String,
                uses: Vec::new(),
            },
        );

        assert!(!facts.name_binding_is_current("value"));
        assert!(
            facts
                .idless_compatibility_binding_facts_for_source_name("value")
                .is_none(),
            "id-less compatibility must not accept a stale payload stored under another name"
        );
        assert!(
            facts.snapshot_visible_binding_for_scope("value").is_none(),
            "scope snapshots must not preserve mismatched name-map payloads"
        );
        assert!(
            facts.all_binding_facts().is_empty(),
            "aggregate readers must not expose mismatched name-map payloads"
        );

        record_use(&mut facts, "value", BindingUse::Returned);
        assert!(
            facts
                .bindings
                .get("value")
                .is_some_and(|binding| binding.uses.is_empty()),
            "stale id-less payloads must not collect uses by lookup key"
        );
    }

    #[test]
    fn bound_name_map_payload_name_must_match_the_lookup_key() {
        let binding = BindingId::new(BindingKind::Local, "value", 1);
        let mut facts = MethodUsageFacts::default();
        facts.bindings_by_id.insert(
            binding.clone(),
            binding_fact("value", Some(binding.clone())),
        );
        facts.bindings.insert(
            "value".to_string(),
            BindingUseFacts {
                name: "other".to_string(),
                binding: Some(binding.clone()),
                java_ty: JavaType::String,
                uses: Vec::new(),
            },
        );

        assert_eq!(facts.visible_binding_id_for_name("value"), None);
        assert!(
            facts
                .idless_compatibility_binding_facts_for_source_name("value")
                .is_none(),
            "binding-id recovery must ignore a name-map payload whose stored name drifted"
        );
        assert!(!facts.name_binding_is_current("value"));
        assert!(
            facts.snapshot_visible_binding_for_scope("value").is_none(),
            "scope snapshots must not preserve bound payloads whose stored name drifted"
        );
        assert_eq!(
            facts
                .all_binding_facts()
                .iter()
                .filter(|fact| fact.name == "value")
                .count(),
            1,
            "aggregate readers should expose only the authoritative binding-id fact"
        );
    }

    #[test]
    fn binding_id_payload_name_must_match_the_binding_key() {
        let binding = BindingId::new(BindingKind::Local, "value", 1);
        let mut facts = MethodUsageFacts::default();
        facts.bindings_by_id.insert(
            binding.clone(),
            BindingUseFacts {
                name: "other".to_string(),
                binding: Some(binding.clone()),
                java_ty: JavaType::String,
                uses: Vec::new(),
            },
        );
        facts.bindings.insert(
            "value".to_string(),
            binding_fact("value", Some(binding.clone())),
        );

        assert_eq!(facts.visible_binding_id_for_name("value"), None);
        assert!(
            facts.binding_facts_for_declaration_id(&binding).is_none(),
            "binding-id lookup must reject a payload whose stored name drifted"
        );
        assert!(
            facts.all_binding_facts().is_empty(),
            "aggregate readers must not expose a binding-id payload whose name disagrees with its key"
        );
    }

    #[test]
    fn binding_facts_for_id_rejects_stale_payload_name() {
        let binding = BindingId::new(BindingKind::Local, "value", 1);
        let mut facts = MethodUsageFacts::default();
        facts.bindings_by_id.insert(
            binding.clone(),
            BindingUseFacts {
                name: "other".to_string(),
                binding: Some(binding.clone()),
                java_ty: JavaType::String,
                uses: Vec::new(),
            },
        );

        assert_eq!(facts.binding_facts_for_id(&binding), None);
    }

    #[test]
    fn record_use_rejects_stale_name_map_binding() {
        let binding = BindingId::new(BindingKind::Local, "value", 1);
        let mut facts = MethodUsageFacts::default();
        facts.bindings.insert(
            "value".to_string(),
            binding_fact("value", Some(binding.clone())),
        );

        record_use(&mut facts, "value", BindingUse::Returned);

        assert!(
            facts
                .bindings
                .get("value")
                .expect("stale compatibility name entry remains stored")
                .uses
                .is_empty()
        );
        assert!(facts.bindings_by_id.get(&binding).is_none());
    }

    #[test]
    fn restore_binding_use_facts_rejects_stale_saved_binding_id() {
        let binding = BindingId::new(BindingKind::Local, "value", 1);
        let mut facts = MethodUsageFacts::default();

        restore_binding_use_facts(
            &mut facts,
            "value".to_string(),
            Some(binding_fact("value", Some(binding.clone()))),
        );

        assert!(facts.bindings.get("value").is_none());
        assert!(facts.bindings_by_id.get(&binding).is_none());
    }

    #[test]
    fn restore_binding_use_facts_keeps_legacy_idless_binding() {
        let mut facts = MethodUsageFacts::default();

        restore_binding_use_facts(
            &mut facts,
            "value".to_string(),
            Some(binding_fact("value", None)),
        );

        assert!(facts.bindings.get("value").is_some());
    }

    #[test]
    fn visible_binding_scope_helpers_reject_stale_saved_binding_id() {
        let binding = BindingId::new(BindingKind::Local, "value", 1);
        let mut facts = MethodUsageFacts::default();
        facts.bindings.insert(
            "value".to_string(),
            binding_fact("value", Some(binding.clone())),
        );

        let saved = facts.hide_visible_binding_for_scope("value");

        assert_eq!(saved, None);
        assert!(facts.bindings.get("value").is_none());
        facts.restore_visible_binding_for_scope(
            "value".to_string(),
            Some(binding_fact("value", Some(binding.clone()))),
        );
        assert!(facts.bindings.get("value").is_none());
        assert!(facts.bindings_by_id.get(&binding).is_none());
    }

    #[test]
    fn restore_visible_binding_rejects_mismatched_saved_payload_name() {
        let mut facts = MethodUsageFacts::default();

        facts.restore_visible_binding_for_scope(
            "value".to_string(),
            Some(binding_fact("other", None)),
        );

        assert!(facts.bindings.get("value").is_none());
        assert!(facts.bindings.get("other").is_none());
    }

    #[test]
    fn stale_name_map_binding_does_not_shadow_field() {
        let binding = BindingId::new(BindingKind::Local, "value", 1);
        let mut facts = MethodUsageFacts::default();
        facts
            .bindings
            .insert("value".to_string(), binding_fact("value", Some(binding)));
        let fields = HashSet::from(["value".to_string()]);

        assert!(is_unshadowed_field_name(&facts, &fields, "value"));
    }

    #[test]
    fn usage_name_binding_id_recovery_goes_through_visible_binding_helper() {
        let source = include_str!("usage.rs");
        let analyze_stmt_source = source
            .split("fn analyze_stmt")
            .nth(1)
            .and_then(|tail| tail.split("fn analyze_expr").next())
            .expect("analyze_stmt source should be discoverable");
        let record_use_source = source
            .split("fn record_use")
            .nth(1)
            .and_then(|tail| tail.split("fn insert_binding_use_facts").next())
            .expect("record_use source should be discoverable");
        let unshadowed_field_source = source
            .split("fn is_unshadowed_field_name")
            .nth(1)
            .and_then(|tail| {
                tail.split("fn call_result_boundary_policy_with_context")
                    .next()
            })
            .expect("is_unshadowed_field_name source should be discoverable");

        assert!(analyze_stmt_source.contains("facts.visible_binding_id_for_name(name)"));
        assert!(record_use_source.contains("facts.record_visible_binding_use(name, usage)"));
        assert!(!record_use_source.contains("facts.visible_binding_id_for_name(name)"));
        assert!(unshadowed_field_source.contains("!facts.name_binding_is_current(name)"));
        assert!(!unshadowed_field_source.contains("!facts.bindings.contains_key(name)"));
        assert!(
            !analyze_stmt_source.contains(".bindings\n                            .get(name)"),
            "assignment aliasing should not recover target binding ids by open-coded map lookup"
        );
        assert!(
            !record_use_source.contains(".bindings\n        .get(name)"),
            "record_use should not recover binding ids by open-coded map lookup"
        );
    }

    #[test]
    fn usage_scope_shadowing_goes_through_visible_binding_helpers() {
        let source = include_str!("usage.rs");
        let production_source = source
            .split("\n#[cfg(test)]\nmod tests")
            .next()
            .expect("production usage source should be discoverable");
        let analyze_stmt_source = production_source
            .split("fn analyze_stmt")
            .nth(1)
            .and_then(|tail| tail.split("fn analyze_expr").next())
            .expect("analyze_stmt source should be discoverable");
        let analyze_lambda_source = production_source
            .split("fn analyze_lambda_body")
            .nth(1)
            .and_then(|tail| tail.split("fn analyze_scoped_statements").next())
            .expect("analyze_lambda_body source should be discoverable");
        let save_direct_source = production_source
            .split("fn save_direct_var_decl_bindings")
            .nth(1)
            .and_then(|tail| tail.split("fn restore_scoped_binding_uses").next())
            .expect("save_direct_var_decl_bindings source should be discoverable");

        assert!(
            analyze_stmt_source.contains("facts.hide_visible_binding_for_scope(source_var_name)")
        );
        assert!(
            analyze_stmt_source
                .contains("facts.hide_visible_binding_for_scope(&catch.source_name)")
        );
        assert!(
            analyze_lambda_source
                .contains("facts.hide_visible_binding_for_scope(&param.source_name)")
        );
        assert!(
            save_direct_source.contains("facts.snapshot_visible_binding_for_scope(source_name)")
        );
        assert!(
            !analyze_stmt_source.contains("facts.bindings.remove")
                && !analyze_lambda_source.contains("facts.bindings.remove")
                && !save_direct_source.contains("facts.bindings.get"),
            "scope shadowing should use MethodUsageFacts visible-binding helpers"
        );
    }

    #[test]
    fn block_scoped_local_does_not_shadow_field_after_scope() {
        let parsed = crate::parser::parse_java_source(
            r#"
class ScopedFieldReturn {
    int value;

    int f(boolean b) {
        if (b) {
            int value = 1;
        }
        return value;
    }
}
"#,
        );
        let resolved = crate::resolve::resolve_program(&parsed.program, parsed.diagnostics);
        let usage = analyze_usage(&resolved.program);
        let method_usage = usage
            .methods
            .get(&UsageMethodKey::current_rust_method_name(
                "ScopedFieldReturn",
                "f",
            ))
            .expect("method usage should be recorded");

        assert!(
            method_usage.field_uses().iter().any(|use_fact| matches!(
                use_fact,
                FieldUse::Returned { field_name } if field_name == "value"
            )),
            "{method_usage:?}"
        );
        assert!(
            !method_usage
                .all_binding_facts()
                .iter()
                .any(|binding| binding
                    .uses
                    .iter()
                    .any(|use_fact| matches!(use_fact, BindingUse::Returned))),
            "the out-of-scope local must not receive the return use: {method_usage:?}"
        );
    }

    #[test]
    fn instanceof_pattern_binding_read_does_not_use_outer_same_name_binding() {
        let parsed = crate::parser::parse_java_source(
            r#"
class InstanceofPatternUsageScope {
    void f(Object o, String s) {
        if (o instanceof String s) {
            s.toString();
        }
        s.toString();
    }
}
"#,
        );
        let resolved = crate::resolve::resolve_program(&parsed.program, parsed.diagnostics);
        let usage = analyze_usage(&resolved.program);
        let method_usage = usage
            .methods
            .get(&UsageMethodKey::current_rust_method_name(
                "InstanceofPatternUsageScope",
                "f",
            ))
            .expect("method usage should be recorded");
        let outer = method_usage
            .all_binding_facts()
            .into_iter()
            .find(|binding| binding.name == "s" && binding.binding.is_some())
            .expect("outer parameter binding should be recorded");
        let receiver_calls = outer
            .uses
            .iter()
            .filter(|usage| {
                matches!(
                    usage,
                    BindingUse::ReceiverForCall { name, .. } if name == "toString"
                )
            })
            .count();

        assert_eq!(
            receiver_calls, 1,
            "pattern variable reads must not be charged to the outer same-name parameter: {outer:?}"
        );
    }

    #[test]
    fn negated_instanceof_else_pattern_binding_read_does_not_use_outer_same_name_binding() {
        let parsed = crate::parser::parse_java_source(
            r#"
class NegatedInstanceofPatternUsageScope {
    void f(Object o, String s) {
        if (!(o instanceof String s)) {
            s.toString();
        } else {
            s.toString();
        }
        s.toString();
    }
}
"#,
        );
        let resolved = crate::resolve::resolve_program(&parsed.program, parsed.diagnostics);
        let usage = analyze_usage(&resolved.program);
        let method_usage = usage
            .methods
            .get(&UsageMethodKey::current_rust_method_name(
                "NegatedInstanceofPatternUsageScope",
                "f",
            ))
            .expect("method usage should be recorded");
        let outer = method_usage
            .all_binding_facts()
            .into_iter()
            .find(|binding| binding.name == "s" && binding.binding.is_some())
            .expect("outer parameter binding should be recorded");
        let receiver_calls = outer
            .uses
            .iter()
            .filter(|usage| {
                matches!(
                    usage,
                    BindingUse::ReceiverForCall { name, .. } if name == "toString"
                )
            })
            .count();

        assert_eq!(
            receiver_calls, 2,
            "only the condition-false branch should hide the outer same-name parameter: {outer:?}"
        );
    }

    #[test]
    fn and_pattern_binding_read_does_not_use_outer_same_name_binding() {
        let parsed = crate::parser::parse_java_source(
            r#"
class AndPatternUsageScope {
    void f(Object o, String s) {
        boolean b = o instanceof String s && s.isEmpty();
        s.toString();
    }
}
"#,
        );
        let resolved = crate::resolve::resolve_program(&parsed.program, parsed.diagnostics);
        let usage = analyze_usage(&resolved.program);
        let method_usage = usage
            .methods
            .get(&UsageMethodKey::current_rust_method_name(
                "AndPatternUsageScope",
                "f",
            ))
            .expect("method usage should be recorded");
        let outer = method_usage
            .all_binding_facts()
            .into_iter()
            .find(|binding| binding.name == "s" && binding.binding.is_some())
            .expect("outer parameter binding should be recorded");
        let receiver_calls = outer
            .uses
            .iter()
            .filter(|usage| {
                matches!(
                    usage,
                    BindingUse::ReceiverForCall { name, .. } if name == "toString" || name == "isEmpty"
                )
            })
            .count();

        assert_eq!(
            receiver_calls, 1,
            "short-circuit pattern variable reads must not be charged to the outer same-name parameter: {outer:?}"
        );
    }

    #[test]
    fn or_negated_pattern_binding_read_does_not_use_outer_same_name_binding() {
        let parsed = crate::parser::parse_java_source(
            r#"
class OrNegatedPatternUsageScope {
    void f(Object o, String s) {
        boolean b = !(o instanceof String s) || s.isEmpty();
        s.toString();
    }
}
"#,
        );
        let resolved = crate::resolve::resolve_program(&parsed.program, parsed.diagnostics);
        let usage = analyze_usage(&resolved.program);
        let method_usage = usage
            .methods
            .get(&UsageMethodKey::current_rust_method_name(
                "OrNegatedPatternUsageScope",
                "f",
            ))
            .expect("method usage should be recorded");
        let outer = method_usage
            .all_binding_facts()
            .into_iter()
            .find(|binding| binding.name == "s" && binding.binding.is_some())
            .expect("outer parameter binding should be recorded");
        let receiver_calls = outer
            .uses
            .iter()
            .filter(|usage| {
                matches!(
                    usage,
                    BindingUse::ReceiverForCall { name, .. } if name == "toString" || name == "isEmpty"
                )
            })
            .count();

        assert_eq!(
            receiver_calls, 1,
            "negated short-circuit pattern variable reads must not be charged to the outer same-name parameter: {outer:?}"
        );
    }

    #[test]
    fn ternary_pattern_binding_read_does_not_use_outer_same_name_binding() {
        let parsed = crate::parser::parse_java_source(
            r#"
class TernaryPatternUsageScope {
    void f(Object o, String s) {
        int n = (o instanceof String s) ? s.length() : 0;
        s.toString();
    }
}
"#,
        );
        let resolved = crate::resolve::resolve_program(&parsed.program, parsed.diagnostics);
        let usage = analyze_usage(&resolved.program);
        let method_usage = usage
            .methods
            .get(&UsageMethodKey::current_rust_method_name(
                "TernaryPatternUsageScope",
                "f",
            ))
            .expect("method usage should be recorded");
        let outer = method_usage
            .all_binding_facts()
            .into_iter()
            .find(|binding| binding.name == "s" && binding.binding.is_some())
            .expect("outer parameter binding should be recorded");
        let receiver_calls = outer
            .uses
            .iter()
            .filter(|usage| {
                matches!(
                    usage,
                    BindingUse::ReceiverForCall { name, .. } if name == "toString" || name == "length"
                )
            })
            .count();

        assert_eq!(
            receiver_calls, 1,
            "ternary pattern variable reads must not be charged to the outer same-name parameter: {outer:?}"
        );
    }

    #[test]
    fn switch_expression_pattern_binding_read_does_not_use_outer_same_name_binding() {
        let s_binding = BindingId::new(BindingKind::Param, "s", 0);
        let o_binding = BindingId::new(BindingKind::Param, "o", 0);
        let method = crate::java_ir::MethodDecl {
            name: "f".to_string(),
            rust_name: None,
            type_params: Vec::new(),
            type_param_bounds: Vec::new(),
            return_type: JavaType::Void,
            throws: Vec::new(),
            params: vec![
                crate::java_ir::Param {
                    name: "o".to_string(),
                    source_name: "o".to_string(),
                    ty: JavaType::Unknown("Object".to_string()),
                    is_varargs: false,
                    binding: Some(o_binding),
                },
                crate::java_ir::Param {
                    name: "s".to_string(),
                    source_name: "s".to_string(),
                    ty: JavaType::String,
                    is_varargs: false,
                    binding: Some(s_binding),
                },
            ],
            modifiers: Vec::new(),
            body: Some(Block {
                statements: vec![
                    Stmt::VarDecl {
                        is_final: false,
                        ty: JavaType::String,
                        name: "x".to_string(),
                        source_name: "x".to_string(),
                        initializer: Some(Expr::Switch {
                            selector: Box::new(Expr::Var("o".to_string())),
                            cases: vec![
                                crate::java_ir::SwitchExprCase {
                                    labels: vec![SwitchLabel::Expr(Expr::InstanceOf {
                                        expr: Box::new(Expr::Var("o".to_string())),
                                        ty: JavaType::String,
                                        binding: Some("s".to_string()),
                                    })],
                                    value: Expr::Call {
                                        type_args: Vec::new(),
                                        receiver: Some(Box::new(Expr::Var("s".to_string()))),
                                        name: "trim".to_string(),
                                        args: Vec::new(),
                                        resolved: None,
                                    },
                                },
                                crate::java_ir::SwitchExprCase {
                                    labels: vec![SwitchLabel::Default],
                                    value: Expr::Literal(crate::java_ir::Literal::String(
                                        String::new(),
                                    )),
                                },
                            ],
                        }),
                        binding: None,
                    },
                    Stmt::Expr(Expr::Call {
                        type_args: Vec::new(),
                        receiver: Some(Box::new(Expr::Var("s".to_string()))),
                        name: "toString".to_string(),
                        args: Vec::new(),
                        resolved: None,
                    }),
                ],
            }),
            span: crate::diagnostics::Span::default(),
        };
        let ty = TypeDecl {
            source_names: None,
            kind: TypeKind::Class,
            name: "SwitchExpressionPatternUsageScope".to_string(),
            type_params: Vec::new(),
            type_param_bounds: Vec::new(),
            modifiers: Vec::new(),
            extends: None,
            implements: Vec::new(),
            permits: Vec::new(),
            members: vec![Member::Method(method)],
            static_init_blocks: Vec::new(),
            span: crate::diagnostics::Span::default(),
        };
        let usage = analyze_usage(&Program {
            package: None,
            imports: Vec::new(),
            types: vec![ty],
        });
        let method_usage = usage
            .methods
            .get(&UsageMethodKey::current_rust_method_name(
                "SwitchExpressionPatternUsageScope",
                "f",
            ))
            .expect("method usage should be recorded");
        let outer = method_usage
            .all_binding_facts()
            .into_iter()
            .find(|binding| binding.name == "s" && binding.binding.is_some())
            .expect("outer parameter binding should be recorded");

        assert_eq!(
            receiver_call_count(outer, &["toString", "trim"]),
            1,
            "switch expression pattern variable reads must not be charged to the outer same-name parameter: {outer:?}"
        );
    }

    #[test]
    fn content_read_method_receiver_also_records_receiver_call_use() {
        let parsed = crate::parser::parse_java_source(
            r#"
class ContentReadReceiverUse {
    String f(String value) {
        return value.toString();
    }
}
"#,
        );
        let resolved = crate::resolve::resolve_program(&parsed.program, parsed.diagnostics);
        let usage = analyze_usage(&resolved.program);
        let method_usage = usage
            .methods
            .get(&UsageMethodKey::current_rust_method_name(
                "ContentReadReceiverUse",
                "f",
            ))
            .expect("method usage should be recorded");
        let value = method_usage
            .all_binding_facts()
            .into_iter()
            .find(|binding| binding.name == "value")
            .expect("value binding should be recorded");

        assert!(
            value
                .uses
                .iter()
                .any(|usage| matches!(usage, BindingUse::ContentRead)),
            "{value:?}"
        );
        assert!(
            value.uses.iter().any(|usage| matches!(
                usage,
                BindingUse::ReceiverForCall { name, .. } if name == "toString"
            )),
            "{value:?}"
        );
    }

    #[test]
    fn lambda_keyword_param_scope_uses_source_name() {
        let parsed = crate::parser::parse_java_source(
            r#"
class LambdaKeywordParamScope {
    void f(String self) {
        java.util.function.Function<String, String> mapper = self -> self.toString();
        self.toString();
    }
}
"#,
        );
        let resolved = crate::resolve::resolve_program(&parsed.program, parsed.diagnostics);
        let usage = analyze_usage(&resolved.program);
        let method_usage = usage
            .methods
            .get(&UsageMethodKey::current_rust_method_name(
                "LambdaKeywordParamScope",
                "f",
            ))
            .expect("method usage should be recorded");
        let method_param = method_usage
            .all_binding_facts()
            .into_iter()
            .find(|binding| binding.name == "self" && binding.binding.is_some())
            .expect("source-named method parameter should be recorded");
        let receiver_calls = method_param
            .uses
            .iter()
            .filter(|usage| {
                matches!(
                    usage,
                    BindingUse::ReceiverForCall { name, .. } if name == "toString"
                )
            })
            .count();

        assert_eq!(
            receiver_calls, 1,
            "lambda parameter source name must shadow the outer parameter inside the lambda body"
        );
    }

    #[test]
    fn call_receiver_type_uses_current_visible_binding_before_same_name_history() {
        let first = BindingId::new(BindingKind::Local, "value", 0);
        let second = BindingId::new(BindingKind::Local, "value", 1);
        let mut facts = MethodUsageFacts::default();
        facts.bindings_by_id.insert(
            first.clone(),
            BindingUseFacts {
                java_ty: JavaType::String,
                ..binding_fact("value", Some(first))
            },
        );
        facts.bindings_by_id.insert(
            second.clone(),
            BindingUseFacts {
                java_ty: JavaType::Primitive(PrimitiveType::Int),
                ..binding_fact("value", Some(second.clone()))
            },
        );
        facts.bindings.insert(
            "value".to_string(),
            BindingUseFacts {
                java_ty: JavaType::Primitive(PrimitiveType::Int),
                ..binding_fact("value", Some(second))
            },
        );

        assert_eq!(
            call_receiver_java_type(Some(&Expr::Var("value".to_string())), &facts),
            Some(JavaType::Primitive(PrimitiveType::Int))
        );
        assert_eq!(
            call_result_receiver_type(&Expr::Var("value".to_string()), &facts),
            Some(JavaType::Primitive(PrimitiveType::Int))
        );
    }

    #[test]
    fn call_receiver_type_reads_authoritative_binding_id_fact() {
        let binding = BindingId::new(BindingKind::Local, "value", 1);
        let mut facts = MethodUsageFacts::default();
        facts.bindings_by_id.insert(
            binding.clone(),
            BindingUseFacts {
                java_ty: JavaType::Primitive(PrimitiveType::Int),
                ..binding_fact("value", Some(binding.clone()))
            },
        );
        facts.bindings.insert(
            "value".to_string(),
            BindingUseFacts {
                java_ty: JavaType::String,
                ..binding_fact("value", Some(binding))
            },
        );

        assert_eq!(
            call_receiver_java_type(Some(&Expr::Var("value".to_string())), &facts),
            Some(JavaType::Primitive(PrimitiveType::Int))
        );
        assert_eq!(
            call_result_receiver_type(&Expr::Var("value".to_string()), &facts),
            Some(JavaType::Primitive(PrimitiveType::Int))
        );
    }

    #[test]
    fn call_receiver_type_rejects_stale_visible_name_binding() {
        let binding = BindingId::new(BindingKind::Local, "value", 1);
        let mut facts = MethodUsageFacts::default();
        facts.bindings.insert(
            "value".to_string(),
            BindingUseFacts {
                java_ty: JavaType::Primitive(PrimitiveType::Int),
                ..binding_fact("value", Some(binding))
            },
        );

        assert_eq!(
            call_receiver_java_type(Some(&Expr::Var("value".to_string())), &facts),
            None
        );
        assert_eq!(
            call_result_receiver_type(&Expr::Var("value".to_string()), &facts),
            None
        );
    }

    #[test]
    fn call_receiver_type_uses_legacy_name_when_unambiguous() {
        let mut facts = MethodUsageFacts::default();
        facts
            .bindings
            .insert("value".to_string(), binding_fact("value", None));

        assert_eq!(
            call_receiver_java_type(Some(&Expr::Var("value".to_string())), &facts),
            Some(JavaType::String)
        );
        assert_eq!(
            call_result_receiver_type(&Expr::Var("value".to_string()), &facts),
            Some(JavaType::String)
        );
    }

    #[test]
    fn call_receiver_type_rejects_same_name_history_without_visible_binding_even_when_types_agree()
    {
        let first = BindingId::new(BindingKind::Local, "value", 0);
        let second = BindingId::new(BindingKind::Local, "value", 1);
        let mut facts = MethodUsageFacts::default();
        facts.bindings_by_id.insert(
            first.clone(),
            BindingUseFacts {
                java_ty: JavaType::String,
                ..binding_fact("value", Some(first))
            },
        );
        facts.bindings_by_id.insert(
            second.clone(),
            BindingUseFacts {
                java_ty: JavaType::String,
                ..binding_fact("value", Some(second))
            },
        );

        assert_eq!(
            call_receiver_java_type(Some(&Expr::Var("value".to_string())), &facts),
            None
        );
        assert_eq!(
            call_result_receiver_type(&Expr::Var("value".to_string()), &facts),
            None
        );
    }

    fn mut_borrow_abi(owner: &str) -> FacadeAbiFact {
        FacadeAbiFact {
            key: FactCallableKey::method(owner, "copyInto", Staticness::Instance, 1),
            rust_path: "copy_into".to_string(),
            receiver: ReceiverAbi::BorrowedSelf,
            args: vec![FacadeSlot::MutBorrowed],
            return_slot: FacadeSlot::Owned,
            result: ResultAbi::Plain,
            borrow_source: None::<BorrowSource>,
            borrowed_return_recovery: None::<BorrowedReturnRecovery>,
            provenance: FactProvenance::UserOverride,
        }
    }

    #[test]
    fn facade_abi_receiver_match_rejects_wrong_local_receiver_type() {
        let binding_id = BindingId::new(BindingKind::Local, "helper", 0);
        let mut usage = MethodUsageFacts::default();
        let helper = BindingUseFacts {
            java_ty: JavaType::Named {
                name: "OtherHelper".to_string(),
                args: Vec::new(),
            },
            ..binding_fact("helper", Some(binding_id.clone()))
        };
        usage.bindings_by_id.insert(binding_id, helper.clone());
        usage.bindings.insert("helper".to_string(), helper);

        let mut facts = FactSet::default();
        facts.facade_abi_facts.push(mut_borrow_abi("CopyHelper"));
        let fields = HashSet::from(["out".to_string()]);

        assert!(
            !call_mut_borrow_arg_targets_self_field(
                Some(&Expr::Var("helper".to_string())),
                "copyInto",
                &[Expr::Var("out".to_string())],
                &usage,
                &fields,
                &facts,
            ),
            "a local receiver of a different Java type must not claim CopyHelper's mut-borrow ABI"
        );
    }

    #[test]
    fn facade_abi_receiver_match_accepts_matching_local_receiver_type() {
        let binding_id = BindingId::new(BindingKind::Local, "helper", 0);
        let mut usage = MethodUsageFacts::default();
        let helper = BindingUseFacts {
            java_ty: JavaType::Named {
                name: "CopyHelper".to_string(),
                args: Vec::new(),
            },
            ..binding_fact("helper", Some(binding_id.clone()))
        };
        usage.bindings_by_id.insert(binding_id, helper.clone());
        usage.bindings.insert("helper".to_string(), helper);

        let mut facts = FactSet::default();
        facts.facade_abi_facts.push(mut_borrow_abi("CopyHelper"));
        let fields = HashSet::from(["out".to_string()]);

        assert!(call_mut_borrow_arg_targets_self_field(
            Some(&Expr::Var("helper".to_string())),
            "copyInto",
            &[Expr::Var("out".to_string())],
            &usage,
            &fields,
            &facts,
        ));
    }

    #[test]
    fn facade_abi_receiver_match_rejects_same_name_history_without_visible_binding() {
        let first = BindingId::new(BindingKind::Local, "helper", 0);
        let second = BindingId::new(BindingKind::Local, "helper", 1);
        let mut usage = MethodUsageFacts::default();
        for binding in [first, second] {
            usage.bindings_by_id.insert(
                binding.clone(),
                BindingUseFacts {
                    java_ty: JavaType::Named {
                        name: "CopyHelper".to_string(),
                        args: Vec::new(),
                    },
                    ..binding_fact("helper", Some(binding))
                },
            );
        }

        let mut facts = FactSet::default();
        facts.facade_abi_facts.push(mut_borrow_abi("CopyHelper"));
        let fields = HashSet::from(["out".to_string()]);

        assert!(
            !call_mut_borrow_arg_targets_self_field(
                Some(&Expr::Var("helper".to_string())),
                "copyInto",
                &[Expr::Var("out".to_string())],
                &usage,
                &fields,
                &facts,
            ),
            "receiver ABI matching must not recover a type from non-visible same-name history"
        );
    }

    #[test]
    fn facade_abi_receiver_match_restricts_idless_fallback_to_static_owners() {
        let usage = MethodUsageFacts::default();

        assert!(
            !facade_abi_matches_call_receiver(
                "StringUtils",
                Some(&Expr::Var("string_utils".to_string())),
                &usage,
            ),
            "id-less facade ABI matching must not promote arbitrary local-looking names to owners"
        );
        assert!(
            facade_abi_matches_call_receiver(
                "StringUtils",
                Some(&Expr::Var("StringUtils".to_string())),
                &usage,
            ),
            "explicit static-owner pseudo receivers remain valid id-less compatibility input"
        );
    }

    #[test]
    fn resolved_camel_case_field_receiver_retains_declared_collection_type() {
        let parsed = crate::parser::parse_java_source(
            r#"
            import java.util.ArrayList;
            import java.util.List;
            class Values {
                private final List<String> recordList = new ArrayList<>();
                String maybeValue(String input) { return input.isEmpty() ? null : input; }
                void addValue(String input) { this.recordList.add(maybeValue(input)); }
            }
            "#,
        );
        let resolved = crate::resolve::resolve_program(&parsed.program, parsed.diagnostics);
        let ty = &resolved.program.types[0];
        let method = ty
            .members
            .iter()
            .find_map(|member| match member {
                Member::Method(method) if method.name == "addValue" => Some(method),
                _ => None,
            })
            .expect("addValue");
        let usage = analyze_usage(&resolved.program);
        let method_usage = usage.source_method_usage(ty, method).expect("method usage");
        assert!(
            method_usage.field_uses().iter().any(|usage| matches!(
                usage,
                FieldUse::ReceiverForCall {
                    field_name,
                    receiver_type: Some(JavaType::Named { name, args }),
                    name: method_name,
                    nullable_source_call_arg_indices,
                    ..
                } if field_name == "recordList"
                    && name == "List"
                    && args == &[JavaType::String]
                    && method_name == "add"
                    && nullable_source_call_arg_indices == &[0]
            )),
            "{:#?}",
            method_usage.field_uses()
        );
    }

    #[test]
    fn direct_collection_add_sees_typed_nullable_source_call_return() {
        let parsed = crate::parser::parse_java_source(
            r#"
            import java.util.ArrayList;
            import java.util.List;
            class Values {
                String first(List<String> values) { return values.get(0); }
                void copyFirst(List<String> values) {
                    List<String> copied = new ArrayList<String>();
                    copied.add(first(values));
                }
            }
            "#,
        );
        let resolved = crate::resolve::resolve_program(&parsed.program, parsed.diagnostics);
        let ty = &resolved.program.types[0];
        let method = ty
            .members
            .iter()
            .find_map(|member| match member {
                Member::Method(method) if method.name == "copyFirst" => Some(method),
                _ => None,
            })
            .expect("copyFirst");
        let usage = analyze_usage_with_facts(&resolved.program, &resolved.facts);
        let method_usage = usage.source_method_usage(ty, method).expect("method usage");
        let copied = method_usage
            .all_binding_facts()
            .into_iter()
            .find(|binding| binding.name() == "copied")
            .expect("copied binding");
        assert!(
            copied.uses().iter().any(|usage| matches!(
                usage,
                BindingUse::ReceiverForCall {
                    name,
                    nullable_source_call_arg_indices,
                    ..
                } if name == "add" && nullable_source_call_arg_indices == &[0]
            )),
            "{:#?}",
            copied.uses()
        );

        let semantic = crate::semantic_analysis::analyze_program(
            &resolved.program,
            &resolved.facts,
            &FactSet::default(),
        );
        let representations =
            crate::representation::plan_representations(&resolved.program, &semantic.facts);
        let signatures = crate::signature::solve_signatures_with_facts(
            &resolved.program,
            &representations,
            &semantic.usage,
            &semantic.facts,
        );
        let copied_shape = signatures
            .method_signature(ty, method)
            .expect("copyFirst signature")
            .locals()
            .iter()
            .find(|local| local.name() == "copied")
            .expect("copied local")
            .shape();
        assert!(
            copied_shape
                .sequence_element_type()
                .and_then(|element| crate::types::optional_element_type(&element))
                .is_some(),
            "{copied_shape:#?}"
        );
    }
}
