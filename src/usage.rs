use std::collections::{HashMap, HashSet};

use crate::binding_id::BindingId;
use crate::facts::{ExceptionKind, FactSet, NullabilityState, TrivialAccessorKind};
use crate::java_ir::{
    Block, ConstructorInvocationKind, Expr, JavaType, LambdaBody, Member, Modifier, Param, Program,
    ResolvedCall, ResolvedConstructor, Stmt, SwitchLabel, TypeDecl,
};
use crate::java_signature::{JavaCallableKey, JavaOwnerKey, source_method_rust_name};
use crate::stdlib::is_content_read_method;
use crate::types::TypeEnv;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UsageFacts {
    methods: HashMap<UsageMethodKey, MethodUsageFacts>,
    methods_by_rust_name: HashMap<UsageMethodKey, MethodUsageFacts>,
    methods_by_java_key: HashMap<JavaCallableKey, MethodUsageFacts>,
    constructors: HashMap<UsageConstructorKey, MethodUsageFacts>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CallGraph {
    nodes: HashSet<CallableKey>,
    edges: Vec<CallGraphEdge>,
}

impl CallGraph {
    pub fn nodes(&self) -> &HashSet<CallableKey> {
        &self.nodes
    }

    pub fn edges(&self) -> &[CallGraphEdge] {
        &self.edges
    }

    #[cfg(test)]
    pub(crate) fn from_edges(edges: Vec<CallGraphEdge>) -> Self {
        Self {
            nodes: edges
                .iter()
                .flat_map(|edge| [edge.caller.clone(), edge.callee.clone()])
                .collect(),
            edges,
        }
    }

    pub fn from_nodes(nodes: impl IntoIterator<Item = CallableKey>) -> Self {
        Self {
            nodes: nodes.into_iter().collect(),
            edges: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum CallableKey {
    #[non_exhaustive]
    Method {
        type_name: String,
        method_name: String,
    },
    #[non_exhaustive]
    Constructor {
        type_name: String,
        rust_name: String,
    },
    #[non_exhaustive]
    Lambda {
        owner: Box<CallableKey>,
        ordinal: usize,
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
        Self::source_method(ty.name.clone(), method)
    }

    pub(crate) fn source_method(
        type_name: impl Into<String>,
        method: &crate::java_ir::MethodDecl,
    ) -> Self {
        CallableKey::Method {
            type_name: type_name.into(),
            method_name: crate::java_signature::source_method_rust_name(method),
        }
    }

    pub(crate) fn source_constructor_for_type(
        ty: &crate::java_ir::TypeDecl,
        constructor: &crate::java_ir::ConstructorDecl,
    ) -> Self {
        Self::source_constructor(ty.name.clone(), constructor)
    }

    pub(crate) fn source_constructor(
        type_name: impl Into<String>,
        constructor: &crate::java_ir::ConstructorDecl,
    ) -> Self {
        CallableKey::Constructor {
            type_name: type_name.into(),
            rust_name: constructor
                .rust_name
                .clone()
                .unwrap_or_else(|| "new".to_string()),
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
            type_name: type_name.into(),
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
            type_name: type_name.into(),
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
            } => Some((type_name, method_name)),
            _ => None,
        }
    }

    pub fn constructor_identity_parts(&self) -> Option<(&str, &str)> {
        match self {
            CallableKey::Constructor {
                type_name,
                rust_name,
            } => Some((type_name, rust_name)),
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
            } => format!("{type_name}::{method_name}"),
            CallableKey::Constructor {
                type_name,
                rust_name,
            } => format!("{type_name}::{rust_name}"),
            CallableKey::Lambda { owner, ordinal } => {
                format!("{}#lambda{ordinal}", owner.diagnostic_label())
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
    LambdaBody,
    HandledExceptionBoundary,
    UnresolvedExternalMethodCall,
    UnresolvedExternalConstructorCall,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct UsageMethodKey {
    type_name: String,
    method_name: String,
}

impl UsageMethodKey {
    pub(crate) fn source_method(ty: &TypeDecl, method: &crate::java_ir::MethodDecl) -> Self {
        Self {
            type_name: ty.name.clone(),
            method_name: source_method_rust_name(method),
        }
    }

    pub(crate) fn legacy_java_name_source_method(
        ty: &TypeDecl,
        method: &crate::java_ir::MethodDecl,
    ) -> Self {
        Self {
            type_name: ty.name.clone(),
            method_name: method.name.clone(),
        }
    }

    pub(crate) fn current_rust_method_name(
        type_name: impl Into<String>,
        method_name: impl Into<String>,
    ) -> Self {
        Self {
            type_name: type_name.into(),
            method_name: method_name.into(),
        }
    }

    pub(crate) fn type_name(&self) -> &str {
        &self.type_name
    }

    pub(crate) fn method_name(&self) -> &str {
        &self.method_name
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct UsageConstructorKey {
    type_name: String,
    rust_name: String,
}

impl UsageConstructorKey {
    pub(crate) fn source_constructor(
        ty: &TypeDecl,
        ctor: &crate::java_ir::ConstructorDecl,
    ) -> Self {
        Self {
            type_name: ty.name.clone(),
            rust_name: ctor.rust_name.clone().unwrap_or_else(|| "new".to_string()),
        }
    }

    pub(crate) fn current_rust_constructor_name(
        type_name: impl Into<String>,
        rust_name: impl Into<String>,
    ) -> Self {
        Self {
            type_name: type_name.into(),
            rust_name: rust_name.into(),
        }
    }

    pub(crate) fn type_name(&self) -> &str {
        &self.type_name
    }

    pub(crate) fn rust_name(&self) -> &str {
        &self.rust_name
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
    bindings: HashMap<String, BindingUseFacts>,
    bindings_by_id: HashMap<BindingId, BindingUseFacts>,
    type_params_in_scope: Vec<String>,
    call_sites: Vec<CallSiteUse>,
    constructor_call_sites: Vec<ConstructorCallSiteUse>,
    returned_call_results: Vec<ReturnedCallResultUse>,
    field_uses: Vec<FieldUse>,
    mutates_self: bool,
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
        self.method_usage_by_key(&UsageMethodKey::current_rust_method_name(
            type_name,
            method_name,
        ))
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
        self.methods.insert(key, usage);
    }

    pub(crate) fn method_entries(
        &self,
    ) -> impl Iterator<Item = (&UsageMethodKey, &MethodUsageFacts)> {
        self.methods.iter()
    }

    pub(crate) fn constructor_entries(
        &self,
    ) -> impl Iterator<Item = (&UsageConstructorKey, &MethodUsageFacts)> {
        self.constructors.iter()
    }

    pub(crate) fn method_usages(&self) -> impl Iterator<Item = &MethodUsageFacts> {
        self.methods.values()
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

impl MethodUsageFacts {
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

    pub fn mutates_self(&self) -> bool {
        self.mutates_self
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
        boundary: CallArgumentBoundary,
        has_literal_null_arg: bool,
        literal_null_arg_indices: Vec<usize>,
    },
    StoredInField {
        field_name: String,
    },
    Returned,
    InstanceOfTarget {
        target_ty: JavaType,
    },
    Mutated,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FieldUse {
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
    Returned {
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
pub struct CallSiteUse {
    name: String,
    arg_count: usize,
    result_returned: bool,
    receiver_is_self: bool,
    receiver_targets_self_field: bool,
    receiver_type: Option<JavaType>,
    null_arg_indices: Vec<usize>,
    nullable_source_call_arg_indices: Vec<usize>,
    boundary: CallArgumentBoundary,
    resolved: Option<ResolvedCall>,
}

impl CallSiteUse {
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
    boundary: CallArgumentBoundary,
    resolved: Option<ResolvedConstructor>,
}

impl ConstructorCallSiteUse {
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
    let mut facts = UsageFacts::default();
    for ty in &program.types {
        analyze_type_usage(ty, &mut facts, semantic_facts);
    }
    propagate_self_mutability_through_self_calls(&mut facts);
    facts
}

fn propagate_self_mutability_through_self_calls(facts: &mut UsageFacts) {
    loop {
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
                    .then(|| (key.type_name.clone(), key.method_name.clone()))
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
                    return contains_resolved_source_method_key(
                        &mutating_source_methods,
                        resolved,
                        &call_site.name,
                    );
                }
                if call_site
                    .receiver_type
                    .as_ref()
                    .is_some_and(|receiver_type| {
                        java_type_name_for_usage(receiver_type).is_some_and(|receiver_name| {
                            mutating_compat_methods
                                .contains(&(receiver_name.to_string(), call_site.name.clone()))
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
            mark(&key.type_name, usage);
        }
        for (key, usage) in facts.methods_by_rust_name.iter_mut() {
            mark(&key.type_name, usage);
        }
        for (key, usage) in facts.methods_by_java_key.iter_mut() {
            mark(key.owner(), usage);
        }
        if !changed {
            break;
        }
    }
}

pub fn build_call_graph(program: &Program) -> CallGraph {
    build_call_graph_with_facts(program, &FactSet::default())
}

pub fn build_call_graph_with_facts(program: &Program, facts: &FactSet) -> CallGraph {
    let mut graph = CallGraph::default();
    for ty in &program.types {
        build_type_call_graph(ty, &mut graph, facts);
    }
    graph
}

fn analyze_type_usage(ty: &TypeDecl, facts: &mut UsageFacts, semantic_facts: &FactSet) {
    let internal_methods = internal_method_names(ty);
    let source_return_nullability = source_return_nullability_by_call_name(ty);
    let field_names = field_names(ty);
    let field_types = field_types(ty);
    for member in &ty.members {
        match member {
            Member::Method(method) => {
                let mut method_facts = MethodUsageFacts::default();
                method_facts.type_params_in_scope =
                    usage_type_params_in_scope(&ty.type_params, &method.type_params);
                let type_env = usage_type_env(ty, &method.type_params, &method.type_param_bounds);
                for param in &method.params {
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
                            &internal_methods,
                            &field_names,
                            &field_types,
                            semantic_facts,
                            &type_env,
                            &source_return_nullability,
                        );
                    }
                }
                let rust_key = UsageMethodKey::source_method(ty, method);
                let java_key = UsageMethodKey::legacy_java_name_source_method(ty, method);
                facts
                    .methods_by_rust_name
                    .insert(rust_key, method_facts.clone());
                let rust_key = UsageMethodKey::source_method(ty, method);
                facts.methods.insert(rust_key, method_facts.clone());
                facts.methods_by_java_key.insert(
                    JavaCallableKey::source_method_for_type(ty, method),
                    method_facts.clone(),
                );
                facts.methods.insert(java_key, method_facts);
            }
            Member::Constructor(ctor) => {
                let mut ctor_facts = MethodUsageFacts::default();
                ctor_facts.type_params_in_scope =
                    usage_type_params_in_scope(&ty.type_params, &ctor.type_params);
                let type_env = usage_type_env(ty, &ctor.type_params, &ctor.type_param_bounds);
                for param in &ctor.params {
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
                            &internal_methods,
                            &field_names,
                            &field_types,
                            semantic_facts,
                            &type_env,
                            &source_return_nullability,
                        );
                    }
                }
                facts.constructors.insert(
                    UsageConstructorKey::source_constructor(ty, ctor),
                    ctor_facts,
                );
            }
            Member::Type(nested) => analyze_type_usage(nested, facts, semantic_facts),
            _ => {}
        }
    }
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

fn build_type_call_graph(ty: &TypeDecl, graph: &mut CallGraph, facts: &FactSet) {
    let internal_methods = all_method_names(ty);
    for member in &ty.members {
        match member {
            Member::Method(method) => {
                let caller = CallableKey::source_method_for_type(ty, method);
                graph.nodes.insert(caller.clone());
                if let Some(body) = &method.body {
                    let mut context = CallGraphBuildContext::new(
                        ty.name.clone(),
                        internal_methods.clone(),
                        caller,
                    );
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
                    );
                    collect_call_graph_edges_from_block(body, graph, &mut context, facts);
                }
            }
            Member::Type(nested) => build_type_call_graph(nested, graph, facts),
            _ => {}
        }
    }
}

#[derive(Debug, Clone)]
struct CallGraphBuildContext {
    current_type: String,
    internal_methods: HashSet<String>,
    current_callable: CallableKey,
    next_lambda_ordinal: usize,
    next_anonymous_ordinal: usize,
    handled_catch_types: Vec<JavaType>,
}

impl CallGraphBuildContext {
    fn new(
        current_type: String,
        internal_methods: HashSet<String>,
        current_callable: CallableKey,
    ) -> Self {
        Self {
            current_type,
            internal_methods,
            current_callable,
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
    context: &mut CallGraphBuildContext,
    facts: &FactSet,
) {
    for stmt in &block.statements {
        collect_call_graph_edges_from_stmt(stmt, graph, context, facts);
    }
}

fn collect_call_graph_edges_from_stmt(
    stmt: &Stmt,
    graph: &mut CallGraph,
    context: &mut CallGraphBuildContext,
    facts: &FactSet,
) {
    match stmt {
        Stmt::VarDecl { initializer, .. } => {
            if let Some(initializer) = initializer {
                collect_call_graph_edges_from_expr(initializer, graph, context, facts);
            }
        }
        Stmt::Assign { target, value } => {
            collect_call_graph_edges_from_expr(target, graph, context, facts);
            collect_call_graph_edges_from_expr(value, graph, context, facts);
        }
        Stmt::Expr(expr) | Stmt::Return(Some(expr)) | Stmt::Throw(expr) => {
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
                        .map(CallableKey::resolved_constructor)
                        .unwrap_or_else(|| {
                            CallableKey::constructor_identity(context.current_type.clone(), "new")
                        }),
                    CallGraphEdgeKind::ConstructorDelegationThis,
                ),
                ConstructorInvocationKind::Super => (
                    resolved
                        .as_ref()
                        .map(CallableKey::resolved_constructor)
                        .unwrap_or_else(|| CallableKey::external_constructor("super", args.len())),
                    CallGraphEdgeKind::ConstructorDelegationSuper,
                ),
            };
            add_call_graph_edge(graph, context, callee, edge_kind, facts);
            for arg in args {
                collect_call_graph_edges_from_expr(arg, graph, context, facts);
            }
        }
        Stmt::LocalType(ty) => build_type_call_graph(ty, graph, facts),
        Stmt::Block(block) | Stmt::LabeledBlock { body: block, .. } => {
            collect_call_graph_edges_from_block(block, graph, context, facts)
        }
    }
}

fn collect_call_graph_edges_from_expr(
    expr: &Expr,
    graph: &mut CallGraph,
    context: &mut CallGraphBuildContext,
    facts: &FactSet,
) {
    match expr {
        Expr::Call {
            receiver,
            name,
            args,
            resolved,
        } => {
            let (callee, edge_kind) =
                call_graph_method_target(receiver.as_deref(), name, args.len(), resolved, context);
            add_call_graph_edge(graph, context, callee, edge_kind, facts);
            if let Some(receiver) = receiver {
                collect_call_graph_edges_from_expr(receiver, graph, context, facts);
            }
            for arg in args {
                collect_call_graph_edges_from_expr(arg, graph, context, facts);
            }
        }
        Expr::New {
            ty,
            args,
            body,
            resolved,
        } => {
            let callee = resolved
                .as_ref()
                .map(CallableKey::resolved_constructor)
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
            for arg in args {
                collect_call_graph_edges_from_expr(arg, graph, context, facts);
            }
            if let Some(body) = body {
                for member in body {
                    if let Member::Method(method) = member {
                        let anonymous = context.next_anonymous_method(method.name.clone());
                        add_call_graph_edge(
                            graph,
                            context,
                            anonymous.clone(),
                            CallGraphEdgeKind::AnonymousClassMethod,
                            facts,
                        );
                        if let Some(body) = &method.body {
                            let mut child = context.child_for(anonymous);
                            collect_call_graph_edges_from_block(body, graph, &mut child, facts);
                        }
                    }
                }
            }
        }
        Expr::Lambda { body, .. } => {
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

fn call_graph_method_target(
    receiver: Option<&Expr>,
    name: &str,
    arg_count: usize,
    resolved: &Option<ResolvedCall>,
    context: &CallGraphBuildContext,
) -> (CallableKey, CallGraphEdgeKind) {
    if matches!(receiver, Some(Expr::Super)) {
        return (
            CallableKey::external_method(Some("super"), name, arg_count),
            CallGraphEdgeKind::UnresolvedExternalMethodCall,
        );
    }
    if let Some(resolved) = resolved {
        return (
            CallableKey::resolved_method(resolved),
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
    (
        CallableKey::external_method(
            receiver.and_then(expr_call_graph_type_name),
            name,
            arg_count,
        ),
        CallGraphEdgeKind::UnresolvedExternalMethodCall,
    )
}

fn add_call_graph_edge(
    graph: &mut CallGraph,
    context: &CallGraphBuildContext,
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
            .method_effect_for(type_name, method_name)
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
        | CallableKey::Lambda { .. }
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
        JavaType::Wildcard(_) => "?".to_string(),
    }
}

fn field_names(ty: &TypeDecl) -> HashSet<String> {
    ty.members
        .iter()
        .filter_map(|member| match member {
            Member::Field(field) => Some(field.name.clone()),
            _ => None,
        })
        .collect()
}

fn field_types(ty: &TypeDecl) -> HashMap<String, JavaType> {
    ty.members
        .iter()
        .filter_map(|member| match member {
            Member::Field(field) => Some((field.name.clone(), field.ty.clone())),
            _ => None,
        })
        .collect()
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
    internal_methods: &HashSet<String>,
    field_names: &HashSet<String>,
    field_types: &HashMap<String, JavaType>,
    semantic_facts: &FactSet,
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
            insert_binding_use_facts(facts, source_name.clone(), binding.clone(), ty.clone());
            if let Some(initializer) = initializer {
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
                if let Some(policy) = call_result_boundary_policy_with_context(
                    initializer,
                    internal_methods,
                    facts,
                    current_type,
                    field_names,
                    field_types,
                    semantic_facts,
                    type_env,
                ) {
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
            if let Expr::Var(name) = target {
                if is_unshadowed_field_name(facts, field_names, name) {
                    facts.mutates_self = true;
                    if let Some(policy) = call_result_boundary_policy_with_context(
                        value,
                        internal_methods,
                        facts,
                        current_type,
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
                    analyze_stored_expr(value, facts, name);
                } else {
                    record_use(facts, name, BindingUse::Mutated);
                    if let Expr::Var(source_name) = value {
                        let local_binding = facts.visible_binding_id_for_name(name);
                        record_use(
                            facts,
                            source_name,
                            BindingUse::AssignedToLocal {
                                local_name: name.clone(),
                                local_binding,
                            },
                        );
                    }
                    if let Some(policy) = call_result_boundary_policy_with_context(
                        value,
                        internal_methods,
                        facts,
                        current_type,
                        field_names,
                        field_types,
                        semantic_facts,
                        type_env,
                    ) {
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
                if target_is_this_field(target) {
                    facts.mutates_self = true;
                }
                if let Some(policy) = call_result_boundary_policy_with_context(
                    value,
                    internal_methods,
                    facts,
                    current_type,
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
                analyze_stored_expr(value, facts, field_name);
            }
            if array_assignment_targets_self_field(target, facts, field_names) {
                facts.mutates_self = true;
            }
            analyze_expr(
                target,
                facts,
                current_type,
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
                internal_methods,
                field_names,
                field_types,
                semantic_facts,
                type_env,
                source_return_nullability,
            );
        }
        Stmt::Return(Some(expr)) => {
            if let Expr::Var(name) = expr {
                if is_unshadowed_field_name(facts, field_names, name) {
                    facts.field_uses.push(FieldUse::Returned {
                        field_name: name.clone(),
                    });
                } else {
                    record_use(facts, name, BindingUse::Returned);
                }
            }
            record_returned_call_result_use(
                expr,
                facts,
                internal_methods,
                current_type,
                field_names,
                field_types,
                semantic_facts,
                type_env,
            );
            if let Expr::FieldAccess(receiver, field_name) = expr
                && matches!(receiver.as_ref(), Expr::This)
            {
                facts.field_uses.push(FieldUse::Returned {
                    field_name: field_name.clone(),
                });
            }
            analyze_return_expr(
                expr,
                facts,
                current_type,
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
                internal_methods,
                field_names,
                field_types,
                semantic_facts,
                type_env,
                source_return_nullability,
            );
            analyze_scoped_statements(
                &then_block.statements,
                facts,
                current_type,
                internal_methods,
                field_names,
                field_types,
                semantic_facts,
                type_env,
                source_return_nullability,
            );
            if let Some(else_block) = else_block {
                analyze_scoped_statements(
                    &else_block.statements,
                    facts,
                    current_type,
                    internal_methods,
                    field_names,
                    field_types,
                    semantic_facts,
                    type_env,
                    source_return_nullability,
                );
            }
        }
        Stmt::While {
            condition, body, ..
        }
        | Stmt::DoWhile {
            condition, body, ..
        } => {
            analyze_expr(
                condition,
                facts,
                current_type,
                internal_methods,
                field_names,
                field_types,
                semantic_facts,
                type_env,
                source_return_nullability,
            );
            analyze_scoped_statements(
                &body.statements,
                facts,
                current_type,
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
                internal_methods,
                field_names,
                field_types,
                semantic_facts,
                type_env,
                source_return_nullability,
            );
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
            analyze_expr(
                iterable,
                facts,
                current_type,
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
                internal_methods,
                field_names,
                field_types,
                semantic_facts,
                type_env,
                source_return_nullability,
            );
            for case in cases {
                analyze_scoped_statements(
                    &case.statements,
                    facts,
                    current_type,
                    internal_methods,
                    field_names,
                    field_types,
                    semantic_facts,
                    type_env,
                    source_return_nullability,
                );
            }
        }
        Stmt::Block(block) | Stmt::LabeledBlock { body: block, .. } => {
            analyze_scoped_statements(
                &block.statements,
                facts,
                current_type,
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
            let policy = constructor_invocation_boundary_policy(*kind);
            facts.constructor_call_sites.push(ConstructorCallSiteUse {
                name: "constructor".to_string(),
                arg_count: args.len(),
                null_arg_indices: null_argument_indices(args),
                nullable_source_call_arg_indices: nullable_source_call_arg_indices(
                    args,
                    source_return_nullability,
                ),
                boundary: policy.boundary,
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
    field_names: &HashSet<String>,
    field_types: &HashMap<String, JavaType>,
    semantic_facts: &FactSet,
    type_env: &TypeEnv,
) -> Option<CallResultBoundaryPolicy> {
    match expr {
        Expr::Call {
            receiver,
            name,
            args,
            resolved,
        } => {
            let receiver_type = receiver.as_deref().and_then(|receiver| {
                call_result_receiver_type_with_context(
                    receiver,
                    facts,
                    current_type,
                    field_names,
                    field_types,
                    semantic_facts,
                )
            });
            let boundary =
                call_argument_boundary_policy(receiver.as_deref(), name, internal_methods).boundary;
            let boundary = modeled_receiver_call_boundary(
                boundary,
                receiver_type.as_ref(),
                name,
                args.len(),
                semantic_facts,
                type_env,
            );
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

fn call_result_receiver_type_with_context(
    receiver: &Expr,
    facts: &MethodUsageFacts,
    current_type: &str,
    field_names: &HashSet<String>,
    field_types: &HashMap<String, JavaType>,
    semantic_facts: &FactSet,
) -> Option<JavaType> {
    match receiver {
        Expr::Var(name) if is_unshadowed_field_name(facts, field_names, name) => {
            source_field_type(current_type, name, field_types, semantic_facts)
        }
        Expr::FieldAccess(inner, field_name) if matches!(inner.as_ref(), Expr::This) => {
            source_field_type(current_type, field_name, field_types, semantic_facts)
        }
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

fn analyze_expr(
    expr: &Expr,
    facts: &mut MethodUsageFacts,
    current_type: &str,
    internal_methods: &HashSet<String>,
    field_names: &HashSet<String>,
    field_types: &HashMap<String, JavaType>,
    semantic_facts: &FactSet,
    type_env: &TypeEnv,
    source_return_nullability: &HashMap<(String, usize), NullabilityState>,
) {
    analyze_expr_with_context(
        expr,
        facts,
        current_type,
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
    internal_methods: &HashSet<String>,
    field_names: &HashSet<String>,
    field_types: &HashMap<String, JavaType>,
    semantic_facts: &FactSet,
    type_env: &TypeEnv,
    source_return_nullability: &HashMap<(String, usize), NullabilityState>,
) {
    analyze_expr_with_context(
        expr,
        facts,
        current_type,
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
    internal_methods: &HashSet<String>,
    field_names: &HashSet<String>,
    field_types: &HashMap<String, JavaType>,
    semantic_facts: &FactSet,
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

fn expr_is_null_literal(expr: &Expr) -> bool {
    matches!(expr, Expr::Literal(crate::java_ir::Literal::Null))
}

fn nullable_source_call_arg_indices(
    args: &[Expr],
    source_return_nullability: &HashMap<(String, usize), NullabilityState>,
) -> Vec<usize> {
    args.iter()
        .enumerate()
        .filter_map(|(index, arg)| {
            source_call_arg_nullability(arg, source_return_nullability)
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

fn source_call_arg_nullability(
    arg: &Expr,
    source_return_nullability: &HashMap<(String, usize), NullabilityState>,
) -> Option<NullabilityState> {
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
) -> HashMap<(String, usize), NullabilityState> {
    ty.members
        .iter()
        .filter_map(|member| {
            let Member::Method(method) = member else {
                return None;
            };
            let constraints =
                crate::signature::source_method_return_nullability_constraints(method);
            let state = constraints
                .into_iter()
                .find_map(|constraint| match constraint {
                    crate::signature::SignatureConstraint::RequiredStorageNullability(state) => {
                        Some(state)
                    }
                    _ => None,
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

fn analyze_expr_with_context(
    expr: &Expr,
    facts: &mut MethodUsageFacts,
    current_type: &str,
    internal_methods: &HashSet<String>,
    field_names: &HashSet<String>,
    field_types: &HashMap<String, JavaType>,
    semantic_facts: &FactSet,
    type_env: &TypeEnv,
    source_return_nullability: &HashMap<(String, usize), NullabilityState>,
    result_returned: bool,
) {
    if expr_mutates_self_field(
        expr,
        facts,
        current_type,
        field_names,
        field_types,
        semantic_facts,
    ) {
        facts.mutates_self = true;
    }
    match expr {
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
            let policy = call_argument_boundary_policy(receiver.as_deref(), name, internal_methods);
            let receiver_type = call_receiver_java_type_with_context(
                receiver.as_deref(),
                facts,
                current_type,
                field_names,
                field_types,
                semantic_facts,
            );
            let call_boundary = resolved_receiver_call_boundary(policy.boundary, resolved.as_ref());
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
            facts.call_sites.push(CallSiteUse {
                name: name.clone(),
                arg_count: args.len(),
                result_returned,
                receiver_is_self: call_receiver_is_self(receiver.as_deref()),
                receiver_targets_self_field: receiver
                    .as_deref()
                    .is_some_and(|receiver| expr_targets_self_field(receiver, facts, field_names)),
                receiver_type: receiver_type.clone(),
                null_arg_indices: null_argument_indices(args),
                nullable_source_call_arg_indices: nullable_source_call_arg_indices(
                    args,
                    source_return_nullability,
                ),
                boundary: call_boundary,
                resolved: resolved.clone(),
            });
            if is_content_read_method(name)
                && let Some(receiver) = receiver
                && let Expr::Var(binding) = receiver.as_ref()
            {
                record_use(facts, binding, BindingUse::ContentRead);
            }
            if let Some(receiver) = receiver
                && let Expr::Var(binding) = receiver.as_ref()
            {
                record_use(
                    facts,
                    binding,
                    BindingUse::ReceiverForCall {
                        name: name.clone(),
                        boundary: call_boundary,
                        has_literal_null_arg: args.iter().any(expr_is_null_literal),
                        literal_null_arg_indices: null_argument_indices(args),
                    },
                );
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
                    internal_methods,
                    field_names,
                    field_types,
                    semantic_facts,
                    type_env,
                    source_return_nullability,
                );
            }
        }
        Expr::Binary { left, right, .. } => {
            analyze_expr(
                left,
                facts,
                current_type,
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
                internal_methods,
                field_names,
                field_types,
                semantic_facts,
                type_env,
                source_return_nullability,
            );
        }
        Expr::Cast { expr, .. } => analyze_expr_with_context(
            expr,
            facts,
            current_type,
            internal_methods,
            field_names,
            field_types,
            semantic_facts,
            type_env,
            source_return_nullability,
            result_returned,
        ),
        Expr::Unary { expr, .. } => analyze_expr(
            expr,
            facts,
            current_type,
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
                internal_methods,
                field_names,
                field_types,
                semantic_facts,
                type_env,
                source_return_nullability,
            );
            analyze_expr_with_context(
                then_expr,
                facts,
                current_type,
                internal_methods,
                field_names,
                field_types,
                semantic_facts,
                type_env,
                source_return_nullability,
                result_returned,
            );
            analyze_expr_with_context(
                else_expr,
                facts,
                current_type,
                internal_methods,
                field_names,
                field_types,
                semantic_facts,
                type_env,
                source_return_nullability,
                result_returned,
            );
        }
        Expr::New {
            ty, args, resolved, ..
        } => {
            facts.constructor_call_sites.push(ConstructorCallSiteUse {
                name: "new".to_string(),
                arg_count: args.len(),
                null_arg_indices: null_argument_indices(args),
                nullable_source_call_arg_indices: nullable_source_call_arg_indices(
                    args,
                    source_return_nullability,
                ),
                boundary: constructor_invocation_boundary_policy_for_expr(resolved.as_ref()),
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
                    constructor_argument_boundary_policy(resolved.as_ref(), arg_index),
                );
                analyze_nested_argument_expr(
                    arg,
                    facts,
                    current_type,
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
                            internal_methods,
                            field_names,
                            field_types,
                            semantic_facts,
                            type_env,
                            source_return_nullability,
                        );
                    }
                }
                analyze_expr(
                    &case.value,
                    facts,
                    current_type,
                    internal_methods,
                    field_names,
                    field_types,
                    semantic_facts,
                    type_env,
                    source_return_nullability,
                );
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
    internal_methods: &HashSet<String>,
    field_names: &HashSet<String>,
    field_types: &HashMap<String, JavaType>,
    semantic_facts: &FactSet,
    type_env: &TypeEnv,
    source_return_nullability: &HashMap<(String, usize), NullabilityState>,
) {
    let saved_params = params
        .iter()
        .map(|param| {
            let saved = facts.hide_visible_binding_for_scope(&param.source_name);
            insert_binding_use_facts(
                facts,
                param.source_name.clone(),
                None,
                param
                    .ty
                    .clone()
                    .unwrap_or_else(|| JavaType::Unknown("lambda parameter".to_string())),
            );
            (param.source_name.clone(), saved)
        })
        .collect::<Vec<_>>();

    match body {
        LambdaBody::Expr(expr) => analyze_expr(
            expr,
            facts,
            current_type,
            internal_methods,
            field_names,
            field_types,
            semantic_facts,
            type_env,
            source_return_nullability,
        ),
        LambdaBody::Block(block) => {
            for stmt in &block.statements {
                analyze_stmt(
                    stmt,
                    facts,
                    current_type,
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

    for (name, saved) in saved_params {
        restore_binding_use_facts(facts, name, saved);
    }
}

fn record_returned_call_result_use(
    expr: &Expr,
    facts: &mut MethodUsageFacts,
    internal_methods: &HashSet<String>,
    current_type: &str,
    field_names: &HashSet<String>,
    field_types: &HashMap<String, JavaType>,
    semantic_facts: &FactSet,
    type_env: &TypeEnv,
) {
    match expr {
        Expr::Call { .. } | Expr::New { .. } => {
            let Some(policy) = call_result_boundary_policy_with_context(
                expr,
                internal_methods,
                facts,
                current_type,
                field_names,
                field_types,
                semantic_facts,
                type_env,
            ) else {
                return;
            };
            facts.returned_call_results.push(ReturnedCallResultUse {
                name: policy.name,
                arg_count: policy.arg_count,
                receiver_type: policy.receiver_type,
                boundary: policy.boundary,
                resolved: policy.resolved,
            });
        }
        Expr::Cast { expr, .. } => record_returned_call_result_use(
            expr,
            facts,
            internal_methods,
            current_type,
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
                field_names,
                field_types,
                semantic_facts,
                type_env,
            );
        }
        _ => {}
    }
}

fn record_call_arg_use(
    facts: &mut MethodUsageFacts,
    current_type: &str,
    field_names: &HashSet<String>,
    _field_types: &HashMap<String, JavaType>,
    semantic_facts: &FactSet,
    arg: &Expr,
    call_name: &str,
    receiver_type: Option<JavaType>,
    arg_count: usize,
    arg_index: usize,
    boundary: CallArgumentBoundary,
) {
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
        Expr::Var(name) if is_unshadowed_field_name(facts, field_names, name) => {
            facts.field_uses.push(FieldUse::PassedToCall {
                field_name: name.clone(),
                name: call_name.to_string(),
                receiver_type,
                arg_count,
                arg_index,
                boundary,
            });
        }
        Expr::Var(binding) => record_use(
            facts,
            binding,
            BindingUse::PassedToCall {
                name: call_name.to_string(),
                receiver_type,
                arg_count,
                arg_index,
                boundary,
            },
        ),
        Expr::FieldAccess(receiver, field_name) if matches!(receiver.as_ref(), Expr::This) => {
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
        Expr::FieldAccess(receiver, _) => matches!(receiver.as_ref(), Expr::This),
        Expr::ArrayAccess(array, _) => expr_targets_self_field(array, facts, field_names),
        _ => false,
    }
}

fn call_receiver_java_type(receiver: Option<&Expr>, facts: &MethodUsageFacts) -> Option<JavaType> {
    let Some(Expr::Var(name)) = receiver else {
        return None;
    };
    agreed_binding_java_type(facts, name).or_else(|| {
        static_owner_receiver_name(name).map(|owner| JavaType::Unknown(owner.to_string()))
    })
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
    field_names: &HashSet<String>,
    field_types: &HashMap<String, JavaType>,
    semantic_facts: &FactSet,
) -> Option<JavaType> {
    match receiver {
        Some(Expr::Var(name)) if is_unshadowed_field_name(facts, field_names, name) => {
            source_field_type(current_type, name, field_types, semantic_facts)
        }
        Some(Expr::FieldAccess(inner, field_name)) if matches!(inner.as_ref(), Expr::This) => {
            source_field_type(current_type, field_name, field_types, semantic_facts)
        }
        _ => call_receiver_java_type(receiver, facts),
    }
}

fn source_field_type(
    current_type: &str,
    field_name: &str,
    field_types: &HashMap<String, JavaType>,
    semantic_facts: &FactSet,
) -> Option<JavaType> {
    field_types.get(field_name).cloned().or_else(|| {
        semantic_facts
            .external_field_for(current_type, field_name)
            .and_then(|field| field.field_type.clone())
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
    semantic_facts: &FactSet,
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
    semantic_facts: &FactSet,
) -> Option<String> {
    let Expr::Call {
        receiver,
        name,
        args,
        resolved,
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
    matches!(expr, Expr::FieldAccess(receiver, _) if matches!(receiver.as_ref(), Expr::This))
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
        Expr::Var(name) => is_unshadowed_field_name(facts, field_names, name),
        Expr::FieldAccess(receiver, _) => matches!(receiver.as_ref(), Expr::This),
        Expr::ArrayAccess(array, _) => array_expr_targets_self_field(array, facts, field_names),
        _ => false,
    }
}

fn analyze_scoped_statements(
    statements: &[Stmt],
    facts: &mut MethodUsageFacts,
    current_type: &str,
    internal_methods: &HashSet<String>,
    field_names: &HashSet<String>,
    field_types: &HashMap<String, JavaType>,
    semantic_facts: &FactSet,
    type_env: &TypeEnv,
    source_return_nullability: &HashMap<(String, usize), NullabilityState>,
) {
    let saved_bindings = save_direct_var_decl_bindings(facts, statements);
    for stmt in statements {
        analyze_stmt(
            stmt,
            facts,
            current_type,
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

fn expr_mutates_self_field(
    expr: &Expr,
    facts: &MethodUsageFacts,
    current_type: &str,
    field_names: &HashSet<String>,
    field_types: &HashMap<String, JavaType>,
    semantic_facts: &FactSet,
) -> bool {
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
        } if call_receiver_java_type_with_context(
            Some(receiver.as_ref()),
            facts,
            current_type,
            field_names,
            field_types,
            semantic_facts,
        )
        .is_some_and(|ty| crate::stdlib::native_collection_receiver_mutates(&ty, name)) =>
        {
            match receiver.as_ref() {
                Expr::Var(name) => is_unshadowed_field_name(facts, field_names, name),
                target => target_is_this_field(target),
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
    resolved: Option<&ResolvedConstructor>,
    arg_index: usize,
) -> CallArgumentBoundary {
    let Some(resolved) = resolved else {
        return CallArgumentBoundary::Unknown;
    };
    if resolved.params.get(arg_index).is_some() {
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
    resolved: Option<&ResolvedCall>,
) -> CallArgumentBoundary {
    if boundary != CallArgumentBoundary::Unknown {
        return boundary;
    }
    let Some(resolved) = resolved else {
        return boundary;
    };
    if resolved.kind != crate::java_ir::CallKind::Instance {
        return boundary;
    }
    CallArgumentBoundary::KnownContentRead
}

fn modeled_receiver_call_boundary(
    boundary: CallArgumentBoundary,
    receiver_type: Option<&JavaType>,
    name: &str,
    arg_count: usize,
    semantic_facts: &FactSet,
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
    semantic_facts: &FactSet,
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
    use super::*;
    use crate::binding_id::BindingKind;
    use crate::facts::{
        BorrowSource, BorrowedReturnRecovery, CallableKey as FactCallableKey, FacadeAbiFact,
        FacadeSlot, FactProvenance, ReceiverAbi, ResultAbi, Staticness,
    };
    use crate::java_ir::PrimitiveType;

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
            declaring_type: declaring_type.to_string(),
            rust_name: "read".to_string(),
            kind: crate::java_ir::CallKind::Instance,
            return_type: JavaType::Void,
            arg_target_types: Vec::new(),
            arg_coercions: Vec::new(),
        }
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
    fn resolved_receiver_boundary_treats_source_instance_calls_as_content_reads() {
        let resolved = resolved_instance_call("a.Item");

        assert_eq!(
            resolved_receiver_call_boundary(CallArgumentBoundary::Unknown, Some(&resolved)),
            CallArgumentBoundary::KnownContentRead
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
        };
        let mut facts = MethodUsageFacts::default();
        insert_binding_use_facts(
            &mut facts,
            "input".to_string(),
            Some(BindingId::new(BindingKind::Param, "input", 0)),
            JavaType::String,
        );

        analyze_stmt(
            &stmt,
            &mut facts,
            "Owner",
            &HashSet::new(),
            &HashSet::new(),
            &HashMap::new(),
            &FactSet::default(),
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
}
