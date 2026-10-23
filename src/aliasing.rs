//! Which inserted clones stand in for a Java alias that is actually observed.
//!
//! Java assigns object *references*. Rust's `=` on a struct moves the value, so
//! wherever the source of a reference assignment is still read afterwards,
//! lowering cannot move it and inserts a `clone()` instead. That substitutes
//! copy semantics for alias semantics.
//!
//! The substitution is invisible as long as nobody writes through either handle
//! afterwards, and a silently wrong answer as soon as somebody does:
//!
//! ```java
//! Node b = a;   // Java: one object, two names
//! a.v = 5;      // Rust:  two objects, one write
//! return b.v;   // Java 5, translation 0
//! ```
//!
//! Deciding which case a given clone is takes two facts that already exist
//! separately: *is a second handle created here* (syntax), and *is the other
//! handle still read after the write* (the backward liveness fixpoint in
//! [`crate::liveness`]). This module joins them.
//!
//! It is the detection half of audit 0002. The repair half needs a shared
//! representation — `Rc<RefCell<_>>` or an arena — that `ObjectRepresentation`
//! does not have, so until then the honest output is a diagnostic rather than a
//! clone that compiles and lies.

use std::collections::{HashMap, HashSet};

use crate::diagnostics::{DiagnosticBag, Severity};
use crate::facts::{FactSet, ReceiverMutabilityFact};
use crate::java_ir::{Block, Expr, JavaType, Member, Program, Stmt, TypeDecl, UnaryOp};
use crate::liveness::LivenessAnalysis;
use crate::representation::RepresentationPlan;
use crate::types::TypeEnv;
use crate::usage::{MethodUsageFacts, UsageFacts};

/// A clone that Java would not have made, where the difference is observable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ObservableAlias {
    native_allocation_preserved: bool,
    /// `Type.member`, for the diagnostic message.
    callable: String,
    /// The binding whose value is copied because it is still needed.
    original: String,
    /// The second handle Java would have created.
    alias: String,
    /// The handle the observed write goes through.
    mutated_through: String,
    /// The handle that still reads the object after that write.
    observed_through: String,
    /// The Java type whose reference was copied.
    aliased_ty: JavaType,
    /// The declared type of the original local, before an interface upcast.
    original_ty: Option<JavaType>,
    original_is_live_native_map_entry: bool,
    /// What the write was, for the message.
    mutation: MutationKind,
}

impl ObservableAlias {
    pub(crate) fn new(
        callable: String,
        original: String,
        alias: String,
        mutated_through: String,
        observed_through: String,
        aliased_ty: JavaType,
        original_ty: Option<JavaType>,
        original_is_live_native_map_entry: bool,
        mutation: MutationKind,
    ) -> Self {
        Self {
            native_allocation_preserved: false,
            callable,
            original,
            alias,
            mutated_through,
            observed_through,
            aliased_ty,
            original_ty,
            original_is_live_native_map_entry,
            mutation,
        }
    }

    pub(crate) fn callable(&self) -> &str {
        &self.callable
    }

    pub(crate) fn original(&self) -> &str {
        &self.original
    }

    pub(crate) fn alias(&self) -> &str {
        &self.alias
    }

    pub(crate) fn mutated_through(&self) -> &str {
        &self.mutated_through
    }

    pub(crate) fn observed_through(&self) -> &str {
        &self.observed_through
    }

    pub(crate) fn mutation(&self) -> &MutationKind {
        &self.mutation
    }

    fn aliased_ty(&self) -> &JavaType {
        &self.aliased_ty
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum MutationKind {
    /// `x.f = …`
    FieldWrite { field: String },
    /// `x[i] = …`
    ElementWrite,
    /// `x.m(…)` where `m` was solved to write through its receiver.
    MutatingCall { method: String },
}

impl MutationKind {
    fn describe(&self, handle: &str) -> String {
        match self {
            MutationKind::FieldWrite { field } => format!("`{handle}.{field} = …`"),
            MutationKind::ElementWrite => format!("`{handle}[…] = …`"),
            MutationKind::MutatingCall { method } => {
                format!("`{handle}.{method}(…)`, which writes through its receiver")
            }
        }
    }
}

/// Every observable alias in the program, in source order per callable.
#[cfg(test)]
fn observable_aliases(program: &Program, usage: &UsageFacts) -> Vec<ObservableAlias> {
    observable_aliases_with_facts(program, usage, &FactSet::default())
}

/// Every observable alias, including fact-backed external receiver mutations.
#[cfg(test)]
pub(crate) fn observable_aliases_with_facts(
    program: &Program,
    usage: &UsageFacts,
    facts: &FactSet,
) -> Vec<ObservableAlias> {
    observable_aliases_with_signatures(program, usage, facts, None)
}

fn observable_aliases_with_signatures(
    program: &Program,
    usage: &UsageFacts,
    facts: &FactSet,
    signatures: Option<&crate::signature::SignaturePlan>,
) -> Vec<ObservableAlias> {
    let mut found = Vec::new();
    for ty in &program.types {
        collect_in_type(ty, usage, facts, signatures, &mut found);
    }
    found
}

/// Report observable aliases as errors. The message names both handles and
/// the write, because the fix is a source-level or representation-level
/// decision that a human has to make.
pub(crate) fn report_observable_aliases(
    program: &Program,
    usage: &UsageFacts,
    facts: &FactSet,
    representations: &RepresentationPlan,
    signatures: &crate::signature::SignaturePlan,
    into: &mut DiagnosticBag,
) {
    for alias in observable_aliases_with_signatures(program, usage, facts, Some(signatures)) {
        if alias.native_allocation_preserved
            || direct_local_alias_mutation_is_represented(&alias, facts)
            || source_map_entry_alias_mutation_is_represented(&alias, program, representations)
        {
            continue;
        }
        into.push(
            Severity::Error,
            "observable_alias",
            format!(
                "{}: `{} = {}` aliases one object in Java but is \
                 translated as a copy, and {} is still read through `{observed_through}` \
                 afterwards, so the two sides diverge",
                alias.callable(),
                alias.alias(),
                alias.original(),
                alias.mutation().describe(alias.mutated_through()),
                observed_through = alias.observed_through(),
            ),
            None,
        );
    }
}

fn source_map_entry_alias_mutation_is_represented(
    alias: &ObservableAlias,
    program: &Program,
    representations: &RepresentationPlan,
) -> bool {
    if !representations.source_map_entry_uses_value_carrier()
        || !matches!(alias.mutation(), MutationKind::MutatingCall { .. })
        || !matches!(alias.aliased_ty(), JavaType::Named { name, args } if args.len() == 2 && matches!(crate::stdlib::canonical_external_receiver_type_key(name).as_str(), "Entry" | "Map.Entry" | "java.util.Map.Entry"))
    {
        return false;
    }
    if alias.original_is_live_native_map_entry {
        return true;
    }
    let Some(JavaType::Named { name, .. }) = alias.original_ty.as_ref() else {
        return false;
    };
    fn find_source_type<'a>(ty: &'a TypeDecl, name: &str) -> Option<&'a TypeDecl> {
        if ty.name == name {
            return Some(ty);
        }
        ty.members.iter().find_map(|member| match member {
            Member::Type(nested) => find_source_type(nested, name),
            _ => None,
        })
    }
    let Some(source_type) = program
        .types
        .iter()
        .find_map(|ty| find_source_type(ty, name))
    else {
        return false;
    };
    if !crate::stdlib::source_type_directly_implements_jdk_map_entry(source_type) {
        return false;
    }
    let placement = representations.storage_placement_for(name);
    placement.shared() && placement.interior_mutable() && !placement.sync()
}

fn direct_local_alias_mutation_is_represented(alias: &ObservableAlias, facts: &FactSet) -> bool {
    if !matches!(
        alias.mutation(),
        MutationKind::FieldWrite { .. } | MutationKind::MutatingCall { .. }
    ) {
        return false;
    }
    let effective = crate::effective_facts::EffectiveFacts::new(facts);
    if effective
        .native_list_cursor_element_type(alias.aliased_ty())
        .is_some()
        && alias
            .original_ty
            .as_ref()
            .is_some_and(|ty| effective.native_list_cursor_element_type(ty).is_some())
    {
        // This intrinsic handle works for local and field aliases. Parsed
        // source lookalikes are excluded by the shared carrier authority.
        return true;
    }
    if [
        alias.original(),
        alias.alias(),
        alias.mutated_through(),
        alias.observed_through(),
    ]
    .iter()
    .any(|handle| handle.contains('.'))
    {
        return false;
    }
    let JavaType::Named { name, .. } = alias.aliased_ty() else {
        return false;
    };
    if effective.native_text_reference_carrier(alias.aliased_ty())
        && alias.original_ty.as_ref().is_some_and(|original| {
            effective.native_text_reference_carrier(original)
                && facts
                    .hierarchy_subtype_policy(original, alias.aliased_ty())
                    .reaches()
        })
    {
        return true;
    }
    let placement = facts.storage_placement_for(name);
    if placement.shared() && placement.interior_mutable() && placement.is_implemented() {
        return true;
    }
    // A native SAM or object-safe source interface retains the implementor's
    // canonical handle; its facade type need not have source storage placement.
    // Require the same subtype and implemented mutable-handle proofs as the
    // conversion, rather than treating every nominal interface alias as safe.
    let Some(JavaType::Named {
        name: original_name,
        ..
    }) = alias.original_ty.as_ref()
    else {
        return false;
    };
    if crate::shape_projection::java_type_is_erased_object_storage(alias.aliased_ty())
        && !effective.parsed_source_controls_native_carrier(alias.aliased_ty())
        && effective.parsed_source_controls_native_carrier(alias.original_ty.as_ref().unwrap())
    {
        // Source-to-Object boxing stores the canonical source reference, not
        // a copy of its payload. Require the implemented mutable carrier that
        // this observed mutation needs; a plain boxed source value is not proof.
        let source_placement = facts.storage_placement_for(original_name);
        return source_placement.shared()
            && source_placement.interior_mutable()
            && source_placement.is_implemented();
    }
    let source_interface_adapter = effective.has_parsed_source_type_capability_for_name(name)
        && facts.object_safe_trait_fact_for_name(name) == Some(true);
    if (!source_interface_adapter
        && effective
            .native_functional_carrier(alias.aliased_ty())
            .is_none())
        || !effective.has_parsed_source_type_capability_for_name(original_name)
        || !facts
            .hierarchy_subtype_policy(alias.original_ty.as_ref().unwrap(), alias.aliased_ty())
            .reaches()
    {
        return false;
    }
    let source_placement = facts.storage_placement_for(original_name);
    source_placement.shared()
        && source_placement.interior_mutable()
        && source_placement.is_implemented()
}

#[derive(Default)]
struct NativeAliasStorage {
    allocations: HashMap<crate::binding_id::BindingId, crate::storage_strategy::StoragePlacement>,
    params: HashMap<String, crate::binding_id::BindingId>,
    fields: HashMap<String, crate::storage_strategy::StoragePlacement>,
}

impl NativeAliasStorage {
    fn from_slots(
        params: &[crate::java_ir::Param],
        slots: &[crate::signature::ParamPlan],
        locals: &[crate::signature::LocalPlan],
        facts: &FactSet,
    ) -> Self {
        let mut result = Self::default();
        for (param, slot) in params.iter().zip(slots) {
            if let Some(binding) = &param.binding {
                result.params.insert(param.name.clone(), binding.clone());
                result.record(binding, slot.shape(), facts);
            }
        }
        for local in locals {
            result.record(local.binding(), local.shape(), facts);
        }
        result
    }

    fn with_fields(
        mut self,
        owner: &TypeDecl,
        signatures: &crate::signature::SignaturePlan,
        facts: &FactSet,
    ) -> Self {
        for member in &owner.members {
            if let Member::Field(field) = member
                && let Some(shape) = signatures.field_shape_for_declared_field(owner, field)
                && let Some(placement) = Self::native_placement(shape, facts)
            {
                self.fields.insert(field.name.clone(), placement);
            }
        }
        self
    }

    fn record(
        &mut self,
        binding: &crate::binding_id::BindingId,
        shape: &crate::slot_shape::RustSlotShape,
        facts: &FactSet,
    ) {
        if let Some(placement) = Self::native_placement(shape, facts) {
            self.allocations.insert(binding.clone(), placement);
        }
    }

    fn native_placement(
        shape: &crate::slot_shape::RustSlotShape,
        facts: &FactSet,
    ) -> Option<crate::storage_strategy::StoragePlacement> {
        let ty = crate::types::optional_element_type(&shape.java_ty)
            .unwrap_or_else(|| shape.java_ty.clone());
        let effective = crate::effective_facts::EffectiveFacts::new(facts);
        if (matches!(ty, JavaType::Array(_))
            || effective.retained_native_container_kind(&ty).is_some()
            || effective.native_iterable_element_type(&ty).is_some())
            && shape.native_allocation.shared()
            && shape.native_allocation.interior_mutable()
            && shape.native_allocation.is_implemented()
        {
            Some(shape.native_allocation)
        } else {
            None
        }
    }
}

fn collect_in_type(
    ty: &TypeDecl,
    usage: &UsageFacts,
    facts: &FactSet,
    signatures: Option<&crate::signature::SignaturePlan>,
    found: &mut Vec<ObservableAlias>,
) {
    for member in &ty.members {
        match member {
            Member::Method(method) => {
                if let Some(body) = &method.body {
                    let storage = signatures
                        .and_then(|signatures| {
                            signatures.method_signature(ty, method).map(|plan| {
                                NativeAliasStorage::from_slots(
                                    &method.params,
                                    plan.params(),
                                    plan.locals(),
                                    facts,
                                )
                                .with_fields(ty, signatures, facts)
                            })
                        })
                        .unwrap_or_default();
                    let type_env =
                        aliasing_type_env(ty, &method.type_params, &method.type_param_bounds);
                    collect_in_body(
                        body,
                        &format!("{}.{}", ty.name, method.name),
                        usage,
                        facts,
                        &type_env,
                        usage.source_method_usage(ty, method),
                        &storage,
                        found,
                    );
                }
            }
            Member::Constructor(constructor) => {
                if let Some(body) = &constructor.body {
                    let storage = signatures
                        .and_then(|signatures| {
                            signatures
                                .constructor_signature(ty, constructor)
                                .map(|plan| {
                                    NativeAliasStorage::from_slots(
                                        &constructor.params,
                                        plan.params(),
                                        plan.locals(),
                                        facts,
                                    )
                                    .with_fields(ty, signatures, facts)
                                })
                        })
                        .unwrap_or_default();
                    let type_env = aliasing_type_env(
                        ty,
                        &constructor.type_params,
                        &constructor.type_param_bounds,
                    );
                    let constructor_usage = usage.source_constructor_usage(ty, constructor);
                    collect_in_body(
                        body,
                        &format!("{}.<init>", ty.name),
                        usage,
                        facts,
                        &type_env,
                        constructor_usage,
                        &storage,
                        found,
                    );
                }
            }
            Member::Type(nested) => collect_in_type(nested, usage, facts, signatures, found),
            _ => {}
        }
    }
}

fn aliasing_type_env(
    ty: &TypeDecl,
    member_type_params: &[String],
    member_type_param_bounds: &[Option<JavaType>],
) -> TypeEnv {
    let mut env = TypeEnv::new();
    insert_aliasing_type_param_bounds(&mut env, &ty.type_params, &ty.type_param_bounds);
    insert_aliasing_type_param_bounds(&mut env, member_type_params, member_type_param_bounds);
    env
}

fn insert_aliasing_type_param_bounds(
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

fn collect_in_body(
    body: &Block,
    callable: &str,
    usage: &UsageFacts,
    facts: &FactSet,
    type_env: &TypeEnv,
    method_usage: Option<&MethodUsageFacts>,
    native_storage: &NativeAliasStorage,
    found: &mut Vec<ObservableAlias>,
) {
    // Statement identity in the analysis is the address of the `Stmt`, so the
    // walk below must visit the very same tree that was analyzed.
    let liveness = LivenessAnalysis::for_body(body);

    let mut statements = Vec::new();
    collect_statements(body, &mut statements);

    let aliases = alias_points(&statements, &liveness);
    if aliases.is_empty() {
        return;
    }
    let mutations = mutation_sites(&statements, usage, facts, type_env, method_usage);

    for point in &aliases {
        // One report per alias point. A loop body that writes through the same
        // handle ten times is one defect, and ten copies of the same sentence
        // is how a diagnostic gets ignored.
        let Some(point_index) = statement_index(&statements, point.stmt) else {
            continue;
        };
        let Some((site, other)) = mutations.iter().find_map(|site| {
            let Some(site_index) = statement_index(&statements, site.stmt) else {
                return None;
            };
            if site_index <= point_index {
                return None;
            }
            let other = if site.handle == point.original {
                point.alias.clone()
            } else if point.alias.local_name() == Some(site.handle.as_str()) {
                AliasHandle::Local(point.original.clone())
            } else {
                return None;
            };
            // The write is only observable if the *other* handle is still read
            // after it. Ordering falls out of this: a write that precedes the
            // declaration of `alias` cannot have `alias` live after it.
            alias_handle_is_observed_after(&other, &statements, &liveness, site.stmt)
                .then_some((site, other))
        }) else {
            continue;
        };
        let mut alias = ObservableAlias::new(
            callable.to_string(),
            point.original.clone(),
            point.alias.display(),
            site.handle.clone(),
            other.display(),
            point.ty.clone(),
            point.original_ty.clone(),
            point.original_is_live_native_map_entry,
            site.kind.clone(),
        );
        if (!native_storage.allocations.is_empty() || !native_storage.fields.is_empty())
            && let Some(alias_name) = point.alias.local_name()
        {
            let binding_at = |name: &str| {
                if let Some(site) = liveness.declaration_site_at(point.stmt, name) {
                    statements.iter().find_map(|stmt| {
                        if std::ptr::from_ref::<Stmt>(*stmt) as usize != site {
                            return None;
                        }
                        match stmt {
                            Stmt::VarDecl { binding, .. } | Stmt::EnhancedFor { binding, .. } => {
                                binding.as_ref()
                            }
                            _ => None,
                        }
                    })
                } else {
                    native_storage.params.get(name)
                }
            };
            let placement_at = |name: &str| match binding_at(name) {
                Some(binding) => native_storage.allocations.get(binding),
                None => native_storage.fields.get(name),
            };
            alias.native_allocation_preserved = placement_at(&point.original)
                .zip(placement_at(alias_name))
                .is_some_and(|(original, alias)| original == alias);
        }
        found.push(alias);
    }
}

fn statement_index(statements: &[&Stmt], needle: &Stmt) -> Option<usize> {
    statements
        .iter()
        .position(|stmt| std::ptr::eq(*stmt, needle))
}

struct AliasPoint<'a> {
    stmt: &'a Stmt,
    original: String,
    alias: AliasHandle,
    ty: JavaType,
    original_ty: Option<JavaType>,
    original_is_live_native_map_entry: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum AliasHandle {
    Local(String),
    ThisField(String),
}

impl AliasHandle {
    fn local_name(&self) -> Option<&str> {
        match self {
            AliasHandle::Local(name) => Some(name),
            AliasHandle::ThisField(_) => None,
        }
    }

    fn display(&self) -> String {
        match self {
            AliasHandle::Local(name) => name.clone(),
            AliasHandle::ThisField(field) => format!("this.{field}"),
        }
    }
}

/// `T b = a;` where `a` is read again afterwards — exactly the shape that makes
/// lowering insert a clone rather than move.
///
/// Restricted to declarations: the new handle is introduced here, which is what
/// lets a liveness query stand in for "after this point".
fn alias_points<'a>(statements: &[&'a Stmt], liveness: &LivenessAnalysis) -> Vec<AliasPoint<'a>> {
    let mut points = Vec::new();
    let mut local_types = HashMap::new();
    let mut live_native_map_entries = HashSet::new();
    for stmt in statements {
        match stmt {
            Stmt::VarDecl {
                ty,
                name,
                initializer,
                ..
            } => {
                if let Some(Expr::Var(source)) = initializer
                    && !java_type_is_value_like(ty)
                    && source != name
                    && liveness.binding_is_live_after(stmt, source) == Some(true)
                {
                    points.push(AliasPoint {
                        stmt,
                        original: source.clone(),
                        alias: AliasHandle::Local(name.clone()),
                        ty: ty.clone(),
                        original_ty: local_types.get(source).cloned(),
                        original_is_live_native_map_entry: live_native_map_entries.contains(source),
                    });
                }
                if initializer.as_ref().is_some_and(|initializer| {
                    native_live_map_entry_initializer(initializer, &local_types)
                        || matches!(initializer, Expr::Var(source) if live_native_map_entries.contains(source))
                }) {
                    live_native_map_entries.insert(name.clone());
                }
                local_types.insert(name.clone(), ty.clone());
            }
            Stmt::Assign {
                target,
                value: Expr::Var(source),
            } => {
                let Some(source_ty) = local_types.get(source) else {
                    continue;
                };
                let Some(alias) = alias_storage_receiver(target) else {
                    continue;
                };
                if java_type_is_value_like(source_ty) || alias.local_name() == Some(source) {
                    continue;
                }
                if liveness.binding_is_live_after(stmt, source) != Some(true) {
                    continue;
                }
                points.push(AliasPoint {
                    stmt,
                    original: source.clone(),
                    alias,
                    ty: source_ty.clone(),
                    original_ty: Some(source_ty.clone()),
                    original_is_live_native_map_entry: live_native_map_entries.contains(source),
                });
            }
            Stmt::Assign {
                target: Expr::Var(name),
                value,
            } => {
                if !matches!(value, Expr::Var(source) if live_native_map_entries.contains(source)) {
                    live_native_map_entries.remove(name);
                }
            }
            _ => {}
        }
    }
    points
}

fn native_live_map_entry_initializer(
    initializer: &Expr,
    local_types: &HashMap<String, JavaType>,
) -> bool {
    let Expr::Call {
        receiver: Some(iterator),
        name: next_name,
        args: next_args,
        ..
    } = initializer
    else {
        return false;
    };
    let Expr::Call {
        receiver: Some(entry_set),
        name: iterator_name,
        args: iterator_args,
        ..
    } = iterator.as_ref()
    else {
        return false;
    };
    let Expr::Call {
        receiver: Some(map),
        name: entry_set_name,
        args: entry_set_args,
        ..
    } = entry_set.as_ref()
    else {
        return false;
    };
    let Expr::Var(map_name) = map.as_ref() else {
        return false;
    };
    next_name == "next"
        && next_args.is_empty()
        && iterator_name == "iterator"
        && iterator_args.is_empty()
        && matches!(entry_set_name.as_str(), "entrySet" | "entry_set")
        && entry_set_args.is_empty()
        && local_types.get(map_name).is_some_and(|ty| {
            crate::stdlib::java_collection_kind_for_type(ty)
                == Some(crate::stdlib::JavaCollectionKind::InsertionOrderedMap)
                || matches!(ty, JavaType::Named { name, .. } | JavaType::Unknown(name)
                    if matches!(name.as_str(), "HashMap" | "java.util.HashMap" | "TreeMap" | "java.util.TreeMap"))
        })
}

fn alias_storage_receiver(target: &Expr) -> Option<AliasHandle> {
    match target {
        Expr::FieldAccess(receiver, field) if matches!(receiver.as_ref(), Expr::This) => {
            Some(AliasHandle::ThisField(field.clone()))
        }
        Expr::ArrayAccess(receiver, _) | Expr::FieldAccess(receiver, _) => {
            alias_handle_for_storage_receiver(receiver)
        }
        _ => None,
    }
}

fn alias_handle_for_storage_receiver(receiver: &Expr) -> Option<AliasHandle> {
    match receiver {
        Expr::Var(alias) => Some(AliasHandle::Local(alias.clone())),
        Expr::This => None,
        Expr::FieldAccess(inner, field) if matches!(inner.as_ref(), Expr::This) => {
            Some(AliasHandle::ThisField(field.clone()))
        }
        _ => None,
    }
}

fn alias_handle_is_observed_after(
    handle: &AliasHandle,
    statements: &[&Stmt],
    liveness: &LivenessAnalysis,
    after: &Stmt,
) -> bool {
    match handle {
        AliasHandle::Local(name) => liveness.binding_is_live_after(after, name) == Some(true),
        AliasHandle::ThisField(field) => statements
            .iter()
            .position(|stmt| std::ptr::eq(*stmt, after))
            .is_some_and(|index| {
                statements[index + 1..]
                    .iter()
                    .any(|stmt| stmt_reads_this_field(stmt, field))
            }),
    }
}

/// A primitive assignment copies in Java too, so it is never an alias.
///
/// Deliberately not a list of immutable library types: a type with no reachable
/// write cannot produce a mutation site, so it drops out of the join instead of
/// needing to be named here.
fn java_type_is_value_like(ty: &JavaType) -> bool {
    matches!(ty, JavaType::Primitive(_) | JavaType::Void)
}

struct MutationSite<'a> {
    stmt: &'a Stmt,
    handle: String,
    kind: MutationKind,
}

/// Writes that go *through* a local handle, rather than reassigning it.
///
/// Reassignment (`a = other`) rebinds the name and is already modelled by
/// liveness; it is the writes that reach the pointed-to object that a copy
/// fails to share.
fn mutation_sites<'a>(
    statements: &[&'a Stmt],
    usage: &UsageFacts,
    facts: &FactSet,
    type_env: &TypeEnv,
    method_usage: Option<&MethodUsageFacts>,
) -> Vec<MutationSite<'a>> {
    let mut sites = Vec::new();
    let stored_lambdas = stored_lambda_mutations(statements, usage, facts, type_env, method_usage);
    for stmt in statements {
        let mut here = Vec::new();
        collect_stmt_mutations(
            stmt,
            usage,
            facts,
            type_env,
            method_usage,
            &stored_lambdas,
            &mut here,
        );
        for (handle, kind) in here {
            sites.push(MutationSite { stmt, handle, kind });
        }
    }
    sites
}

fn stored_lambda_mutations(
    statements: &[&Stmt],
    usage: &UsageFacts,
    facts: &FactSet,
    type_env: &TypeEnv,
    method_usage: Option<&MethodUsageFacts>,
) -> HashMap<String, Vec<(String, MutationKind)>> {
    let mut stored = HashMap::new();
    for stmt in statements {
        if let Stmt::VarDecl {
            name,
            initializer: Some(Expr::Lambda { body, .. }),
            ..
        } = stmt
        {
            let mut mutations = Vec::new();
            collect_lambda_body_mutations(
                body,
                usage,
                facts,
                type_env,
                method_usage,
                &mut mutations,
            );
            if !mutations.is_empty() {
                stored.insert(name.clone(), mutations);
            }
        }
    }
    stored
}

fn collect_stmt_mutations(
    stmt: &Stmt,
    usage: &UsageFacts,
    facts: &FactSet,
    type_env: &TypeEnv,
    method_usage: Option<&MethodUsageFacts>,
    stored_lambdas: &HashMap<String, Vec<(String, MutationKind)>>,
    out: &mut Vec<(String, MutationKind)>,
) {
    match stmt {
        Stmt::Assign { target, value } => {
            push_write_target(target, out);
            if !matches!(value, Expr::Lambda { .. }) {
                collect_expr_mutations(
                    value,
                    usage,
                    facts,
                    type_env,
                    method_usage,
                    stored_lambdas,
                    out,
                );
            }
        }
        Stmt::VarDecl {
            initializer: Some(Expr::Lambda { .. }),
            ..
        } => {}
        _ => visit_stmt_exprs(stmt, &mut |expr| {
            collect_expr_mutations(
                expr,
                usage,
                facts,
                type_env,
                method_usage,
                stored_lambdas,
                out,
            )
        }),
    }
}

fn collect_lambda_body_mutations(
    body: &crate::java_ir::LambdaBody,
    usage: &UsageFacts,
    facts: &FactSet,
    type_env: &TypeEnv,
    method_usage: Option<&MethodUsageFacts>,
    out: &mut Vec<(String, MutationKind)>,
) {
    let stored_lambdas = HashMap::new();
    match body {
        crate::java_ir::LambdaBody::Expr(expr) => collect_expr_mutations(
            expr,
            usage,
            facts,
            type_env,
            method_usage,
            &stored_lambdas,
            out,
        ),
        crate::java_ir::LambdaBody::Block(block) => collect_block_mutations(
            block,
            usage,
            facts,
            type_env,
            method_usage,
            &stored_lambdas,
            out,
        ),
    }
}

fn collect_block_mutations(
    block: &Block,
    usage: &UsageFacts,
    facts: &FactSet,
    type_env: &TypeEnv,
    method_usage: Option<&MethodUsageFacts>,
    stored_lambdas: &HashMap<String, Vec<(String, MutationKind)>>,
    out: &mut Vec<(String, MutationKind)>,
) {
    let mut statements = Vec::new();
    collect_statements(block, &mut statements);
    for stmt in statements {
        collect_stmt_mutations(
            stmt,
            usage,
            facts,
            type_env,
            method_usage,
            stored_lambdas,
            out,
        );
    }
}

fn collect_expr_mutations(
    expr: &Expr,
    usage: &UsageFacts,
    facts: &FactSet,
    type_env: &TypeEnv,
    method_usage: Option<&MethodUsageFacts>,
    stored_lambdas: &HashMap<String, Vec<(String, MutationKind)>>,
    out: &mut Vec<(String, MutationKind)>,
) {
    match expr {
        Expr::Binary {
            op: crate::java_ir::BinaryOp::Assign,
            left,
            right,
        } => {
            push_write_target(left, out);
            collect_expr_mutations(
                right,
                usage,
                facts,
                type_env,
                method_usage,
                stored_lambdas,
                out,
            );
        }
        Expr::Unary {
            op: UnaryOp::PreInc | UnaryOp::PreDec | UnaryOp::PostInc | UnaryOp::PostDec,
            expr,
        } => {
            push_write_target(expr, out);
            collect_expr_mutations(
                expr,
                usage,
                facts,
                type_env,
                method_usage,
                stored_lambdas,
                out,
            );
        }
        Expr::Call {
            receiver: Some(receiver),
            name,
            args,
            resolved,
            ..
        } => {
            if let Expr::Var(handle) = receiver.as_ref() {
                if lambda_invocation_name(name)
                    && let Some(mutations) = stored_lambdas.get(handle)
                {
                    out.extend(mutations.iter().cloned());
                }
                if call_writes_through_receiver(
                    resolved.as_ref(),
                    name,
                    args.len(),
                    receiver_java_type_for_handle(handle, method_usage).as_ref(),
                    usage,
                    facts,
                    type_env,
                ) {
                    out.push((
                        handle.clone(),
                        MutationKind::MutatingCall {
                            method: name.clone(),
                        },
                    ));
                }
            }
            collect_expr_mutations(
                receiver,
                usage,
                facts,
                type_env,
                method_usage,
                stored_lambdas,
                out,
            );
            for arg in args {
                collect_expr_mutations(
                    arg,
                    usage,
                    facts,
                    type_env,
                    method_usage,
                    stored_lambdas,
                    out,
                );
            }
        }
        Expr::Lambda { body, .. } => {
            collect_lambda_body_mutations(body, usage, facts, type_env, method_usage, out)
        }
        _ => visit_subexpressions(expr, &mut |inner| {
            collect_expr_mutations(
                inner,
                usage,
                facts,
                type_env,
                method_usage,
                stored_lambdas,
                out,
            )
        }),
    }
}

fn lambda_invocation_name(name: &str) -> bool {
    matches!(name, "run")
}

fn push_write_target(target: &Expr, out: &mut Vec<(String, MutationKind)>) {
    match target {
        Expr::FieldAccess(receiver, field) => {
            if let Expr::Var(handle) = receiver.as_ref() {
                out.push((
                    handle.clone(),
                    MutationKind::FieldWrite {
                        field: field.clone(),
                    },
                ));
            }
        }
        Expr::ArrayAccess(receiver, _) => {
            if let Expr::Var(handle) = receiver.as_ref() {
                out.push((handle.clone(), MutationKind::ElementWrite));
            }
        }
        _ => {}
    }
}

/// Whether a call writes through its receiver, from the solved usage facts.
///
/// `mutates_self` is already a fixpoint over self-calls in [`crate::usage`], so
/// a method that only mutates by delegating is included. An unresolved call
/// answers `false`: over-reporting a divergence that is not there would train
/// readers to ignore the diagnostic, which costs more than the miss.
fn call_writes_through_receiver(
    resolved: Option<&crate::java_ir::ResolvedCall>,
    name: &str,
    arg_count: usize,
    receiver_ty: Option<&JavaType>,
    usage: &UsageFacts,
    facts: &FactSet,
    type_env: &TypeEnv,
) -> bool {
    if let Some(resolved) = resolved
        && usage
            .resolved_source_method_usage(resolved, name)
            .is_some_and(|facts| facts.mutates_self())
    {
        return true;
    }
    let receiver_type = receiver_ty.and_then(|receiver_ty| {
        crate::stdlib::external_api_receiver_owner_key(receiver_ty, type_env)
    });
    facts
        .external_api_effect_for_receiver_with_arity_or_rust_name(
            receiver_type.as_deref(),
            name,
            Some(arg_count),
        )
        .is_some_and(|effect| effect.receiver_mutability == ReceiverMutabilityFact::MutatesReceiver)
}

fn receiver_java_type_for_handle(
    handle: &str,
    method_usage: Option<&MethodUsageFacts>,
) -> Option<JavaType> {
    method_usage?.receiver_handle_java_type(handle).cloned()
}

fn collect_statements<'a>(block: &'a Block, out: &mut Vec<&'a Stmt>) {
    for stmt in &block.statements {
        out.push(stmt);
        collect_nested_statements(stmt, out);
    }
}

fn collect_nested_statements<'a>(stmt: &'a Stmt, out: &mut Vec<&'a Stmt>) {
    match stmt {
        Stmt::If {
            then_block,
            else_block,
            ..
        } => {
            collect_statements(then_block, out);
            if let Some(else_block) = else_block {
                collect_statements(else_block, out);
            }
        }
        Stmt::While { body, .. } | Stmt::DoWhile { body, .. } | Stmt::EnhancedFor { body, .. } => {
            collect_statements(body, out)
        }
        Stmt::For { init, body, .. } => {
            for stmt in init {
                out.push(stmt);
                collect_nested_statements(stmt, out);
            }
            collect_statements(body, out);
        }
        Stmt::Switch { cases, .. } => {
            for case in cases {
                for stmt in &case.statements {
                    out.push(stmt);
                    collect_nested_statements(stmt, out);
                }
            }
        }
        Stmt::Try {
            resources,
            body,
            catches,
            finally_block,
        } => {
            for stmt in resources {
                out.push(stmt);
                collect_nested_statements(stmt, out);
            }
            collect_statements(body, out);
            for catch in catches {
                collect_statements(&catch.body, out);
            }
            if let Some(finally_block) = finally_block {
                collect_statements(finally_block, out);
            }
        }
        Stmt::Block(block) | Stmt::LabeledBlock { body: block, .. } => {
            collect_statements(block, out)
        }
        _ => {}
    }
}

/// The expressions a statement evaluates directly, not counting nested blocks —
/// those are reached through [`collect_statements`], which keeps each mutation
/// attributed to the statement liveness knows about.
fn visit_stmt_exprs(stmt: &Stmt, visit: &mut impl FnMut(&Expr)) {
    match stmt {
        Stmt::VarDecl { initializer, .. } => {
            if let Some(expr) = initializer {
                visit(expr);
            }
        }
        Stmt::Assign { target, value } => {
            visit(target);
            visit(value);
        }
        Stmt::Expr(expr) | Stmt::Throw(expr) => visit(expr),
        Stmt::Return(expr) => {
            if let Some(expr) = expr {
                visit(expr);
            }
        }
        Stmt::If { condition, .. }
        | Stmt::While { condition, .. }
        | Stmt::DoWhile { condition, .. } => visit(condition),
        Stmt::For {
            condition, update, ..
        } => {
            if let Some(condition) = condition {
                visit(condition);
            }
            for expr in update {
                visit(expr);
            }
        }
        Stmt::EnhancedFor { iterable, .. } => visit(iterable),
        Stmt::Switch { selector, .. } => visit(selector),
        Stmt::ConstructorInvocation { args, .. } => {
            for arg in args {
                visit(arg);
            }
        }
        _ => {}
    }
}

fn visit_subexpressions(expr: &Expr, visit: &mut impl FnMut(&Expr)) {
    match expr {
        Expr::FieldAccess(inner, _) | Expr::Cast { expr: inner, .. } => visit(inner),
        Expr::Unary { expr: inner, .. } | Expr::InstanceOf { expr: inner, .. } => visit(inner),
        Expr::ArrayAccess(receiver, index) => {
            visit(receiver);
            visit(index);
        }
        Expr::Binary { left, right, .. } => {
            visit(left);
            visit(right);
        }
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => {
            visit(condition);
            visit(then_expr);
            visit(else_expr);
        }
        Expr::Switch { selector, cases } => {
            visit(selector);
            for case in cases {
                visit(&case.value);
            }
        }
        Expr::Call { receiver, args, .. } => {
            if let Some(receiver) = receiver {
                visit(receiver);
            }
            for arg in args {
                visit(arg);
            }
        }
        Expr::New {
            qualifier, args, ..
        } => {
            if let Some(qualifier) = qualifier {
                visit(qualifier);
            }
            for arg in args {
                visit(arg);
            }
        }
        Expr::ArrayCreation {
            dimensions,
            initializer,
            ..
        } => {
            for dimension in dimensions {
                visit(dimension);
            }
            for expr in initializer.iter().flatten() {
                visit(expr);
            }
        }
        Expr::ArrayInitializer(exprs) => {
            for expr in exprs {
                visit(expr);
            }
        }
        Expr::Lambda { body, .. } => match body {
            crate::java_ir::LambdaBody::Expr(expr) => visit(expr),
            crate::java_ir::LambdaBody::Block(_) => {}
        },
        Expr::Literal(_)
        | Expr::Var(_)
        | Expr::This
        | Expr::Super
        | Expr::MethodReference { .. }
        | Expr::ClassLiteral(_)
        | Expr::Unsupported { .. }
        | Expr::Raw(_) => {}
    }
}

fn stmt_reads_this_field(stmt: &Stmt, field: &str) -> bool {
    match stmt {
        Stmt::VarDecl { initializer, .. } => initializer
            .as_ref()
            .is_some_and(|expr| expr_reads_this_field(expr, field)),
        Stmt::Assign { value, .. } => expr_reads_this_field(value, field),
        Stmt::Expr(expr) | Stmt::Throw(expr) => expr_reads_this_field(expr, field),
        Stmt::Return(expr) => expr
            .as_ref()
            .is_some_and(|expr| expr_reads_this_field(expr, field)),
        Stmt::If {
            condition,
            then_block,
            else_block,
        } => {
            expr_reads_this_field(condition, field)
                || block_reads_this_field(then_block, field)
                || else_block
                    .as_ref()
                    .is_some_and(|block| block_reads_this_field(block, field))
        }
        Stmt::While {
            condition, body, ..
        }
        | Stmt::DoWhile {
            condition, body, ..
        } => expr_reads_this_field(condition, field) || block_reads_this_field(body, field),
        Stmt::For {
            init,
            condition,
            update,
            body,
            ..
        } => {
            init.iter().any(|stmt| stmt_reads_this_field(stmt, field))
                || condition
                    .as_ref()
                    .is_some_and(|expr| expr_reads_this_field(expr, field))
                || update.iter().any(|expr| expr_reads_this_field(expr, field))
                || block_reads_this_field(body, field)
        }
        Stmt::EnhancedFor { iterable, body, .. } => {
            expr_reads_this_field(iterable, field) || block_reads_this_field(body, field)
        }
        Stmt::Switch { selector, cases } => {
            expr_reads_this_field(selector, field)
                || cases.iter().any(|case| {
                    case.labels.iter().any(|label| match label {
                        crate::java_ir::SwitchLabel::Expr(expr) => {
                            expr_reads_this_field(expr, field)
                        }
                        _ => false,
                    }) || case
                        .statements
                        .iter()
                        .any(|stmt| stmt_reads_this_field(stmt, field))
                })
        }
        Stmt::Try {
            resources,
            body,
            catches,
            finally_block,
        } => {
            resources
                .iter()
                .any(|stmt| stmt_reads_this_field(stmt, field))
                || block_reads_this_field(body, field)
                || catches
                    .iter()
                    .any(|catch| block_reads_this_field(&catch.body, field))
                || finally_block
                    .as_ref()
                    .is_some_and(|block| block_reads_this_field(block, field))
        }
        Stmt::ConstructorInvocation { args, .. } => {
            args.iter().any(|arg| expr_reads_this_field(arg, field))
        }
        Stmt::Block(block) | Stmt::LabeledBlock { body: block, .. } => {
            block_reads_this_field(block, field)
        }
        Stmt::LocalType(_) | Stmt::Break(_) | Stmt::Continue(_) | Stmt::Unsupported(_) => false,
    }
}

fn block_reads_this_field(block: &Block, field: &str) -> bool {
    block
        .statements
        .iter()
        .any(|stmt| stmt_reads_this_field(stmt, field))
}

fn expr_reads_this_field(expr: &Expr, field: &str) -> bool {
    match expr {
        Expr::FieldAccess(receiver, name)
            if name == field && matches!(receiver.as_ref(), Expr::This) =>
        {
            true
        }
        Expr::Lambda { body, .. } => match body {
            crate::java_ir::LambdaBody::Expr(expr) => expr_reads_this_field(expr, field),
            crate::java_ir::LambdaBody::Block(block) => block
                .statements
                .iter()
                .any(|stmt| stmt_reads_this_field(stmt, field)),
        },
        _ => {
            let mut found = false;
            visit_subexpressions(expr, &mut |inner| {
                if expr_reads_this_field(inner, field) {
                    found = true;
                }
            });
            found
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::facts::{ExternalApiEffectFact, ExternalApiRole, FactProvenance, NullabilityState};
    use crate::parser::parse_java_source;

    /// Resolve first: a mutating-call mutation site is only visible once calls
    /// carry their declaring type, which is what the real pipeline feeds in.
    fn aliases_in(source: &str) -> Vec<ObservableAlias> {
        let parsed = parse_java_source(source);
        let resolved = crate::resolve::resolve_program(&parsed.program, parsed.diagnostics);
        let usage = crate::usage::analyze_usage(&resolved.program);
        observable_aliases(&resolved.program, &usage)
    }

    fn aliases_in_with_facts(source: &str, facts: &FactSet) -> Vec<ObservableAlias> {
        let parsed = parse_java_source(source);
        let resolved = crate::resolve::resolve_program(&parsed.program, parsed.diagnostics);
        let usage = crate::usage::analyze_usage_with_facts(&resolved.program, facts);
        observable_aliases_with_facts(&resolved.program, &usage, facts)
    }

    #[test]
    fn source_interface_alias_requires_canonical_mutable_handle_evidence() {
        let aliases = aliases_in(include_str!(
            "../tests/corpus/numbers/NullableInterfaceAliasProbe.java"
        ));
        assert!(!aliases.is_empty());
        for alias in aliases {
            assert!(!direct_local_alias_mutation_is_represented(
                &alias,
                &FactSet::default(),
            ));
        }
    }

    fn external_record_set_flag_fact(mutability: ReceiverMutabilityFact) -> FactSet {
        external_set_flag_fact("Record", mutability)
    }

    fn external_set_flag_fact(receiver_type: &str, mutability: ReceiverMutabilityFact) -> FactSet {
        let mut facts = FactSet::default();
        facts.external_api_effect_facts.push(ExternalApiEffectFact {
            method_name: "setFlag".to_string(),
            receiver_type: Some(receiver_type.to_string()),
            arg_count: Some(1),
            parameter_patterns: None,
            role: ExternalApiRole::Setter,
            receiver_mutability: mutability,
            return_type: Some(JavaType::Void),
            return_nullability: NullabilityState::DefinitelyNonNull,
            thrown: Vec::new(),
            provenance: FactProvenance::ExternalLibraryPack,
        });
        facts
    }

    #[test]
    fn subexpression_visitor_sees_qualified_new_receiver() {
        let expr = Expr::New {
            infer_type_arguments: false,
            ty: JavaType::Unknown("Inner".to_string()),
            qualifier: Some(Box::new(Expr::Var("outer".to_string()))),
            args: vec![Expr::Var("arg".to_string())],
            body: None,
            resolved: None,
        };
        let mut visited = Vec::new();

        visit_subexpressions(&expr, &mut |expr| {
            if let Expr::Var(name) = expr {
                visited.push(name.clone());
            }
        });

        assert_eq!(visited, vec!["outer".to_string(), "arg".to_string()]);
    }

    #[test]
    fn receiver_java_type_for_handle_uses_visible_binding_over_same_name_history() {
        let first = crate::binding_id::BindingId::new(
            crate::binding_id::BindingKind::Local,
            "handle".to_string(),
            0,
        );
        let second = crate::binding_id::BindingId::new(
            crate::binding_id::BindingKind::Local,
            "handle".to_string(),
            1,
        );
        let first_fact = crate::usage::BindingUseFacts::new(
            "handle",
            Some(first.clone()),
            JavaType::String,
            Vec::new(),
        );
        let second_fact = crate::usage::BindingUseFacts::new(
            "handle",
            Some(second.clone()),
            JavaType::Named {
                name: "Record".to_string(),
                args: Vec::new(),
            },
            Vec::new(),
        );
        let mut usage = MethodUsageFacts::default();
        usage.insert_binding_id_test_fact(first, first_fact);
        usage.insert_binding_id_test_fact(second, second_fact.clone());
        usage.insert_idless_compatibility_test_fact("handle", second_fact);

        assert_eq!(
            receiver_java_type_for_handle("handle", Some(&usage)),
            Some(JavaType::Named {
                name: "Record".to_string(),
                args: Vec::new(),
            })
        );
    }

    /// The archetype from audit 0002. Java prints 5; a copy prints 0.
    #[test]
    fn field_write_through_the_original_is_observed_through_the_alias() {
        let found = aliases_in(
            r#"
            public class Shared {
                static class Node { int v; }
                public int run() {
                    Node a = new Node();
                    Node b = a;
                    a.v = 5;
                    return b.v;
                }
            }
            "#,
        );
        assert_eq!(found.len(), 1, "expected one observable alias: {found:?}");
        assert_eq!(found[0].original(), "a");
        assert_eq!(found[0].alias(), "b");
        assert_eq!(found[0].mutated_through(), "a");
        assert_eq!(found[0].observed_through(), "b");
    }

    #[test]
    fn observable_alias_diagnostic_is_error_until_mutable_shared_handles_exist() {
        let source = r#"
            public class Shared {
                static class Node { int v; }
                public int run() {
                    Node a = new Node();
                    Node b = a;
                    a.v = 5;
                    return b.v;
                }
            }
            "#;
        let parsed = parse_java_source(source);
        let resolved = crate::resolve::resolve_program(&parsed.program, parsed.diagnostics);
        let usage = crate::usage::analyze_usage(&resolved.program);
        let mut diagnostics = DiagnosticBag::default();

        report_observable_aliases(
            &resolved.program,
            &usage,
            &FactSet::default(),
            &RepresentationPlan::default(),
            &crate::signature::SignaturePlan::default(),
            &mut diagnostics,
        );

        let diagnostic = diagnostics
            .iter()
            .find(|diagnostic| diagnostic.code() == "observable_alias")
            .unwrap_or_else(|| panic!("expected observable_alias diagnostic: {diagnostics:?}"));
        assert_eq!(diagnostic.severity(), Severity::Error);
    }

    #[test]
    fn live_native_map_entry_alias_exemption_requires_supported_map_origin() {
        fn call(receiver: Expr, name: &str) -> Expr {
            Expr::Call {
                type_args: Vec::new(),
                receiver: Some(Box::new(receiver)),
                name: name.to_string(),
                args: Vec::new(),
                resolved: None,
            }
        }
        let next = call(
            call(call(Expr::Var("map".to_string()), "entrySet"), "iterator"),
            "next",
        );
        let mut local_types = HashMap::new();
        local_types.insert(
            "map".to_string(),
            JavaType::Named {
                name: "java.util.LinkedHashMap".to_string(),
                args: Vec::new(),
            },
        );
        assert!(native_live_map_entry_initializer(&next, &local_types));
        local_types.insert(
            "map".to_string(),
            JavaType::Named {
                name: "java.util.TreeMap".to_string(),
                args: Vec::new(),
            },
        );
        assert!(native_live_map_entry_initializer(&next, &local_types));
        local_types.insert(
            "map".to_string(),
            JavaType::Named {
                name: "java.util.HashMap".to_string(),
                args: Vec::new(),
            },
        );
        assert!(native_live_map_entry_initializer(&next, &local_types));
        local_types.insert(
            "map".to_string(),
            JavaType::Named {
                name: "java.util.Map".to_string(),
                args: Vec::new(),
            },
        );
        assert!(!native_live_map_entry_initializer(&next, &local_types));
    }

    /// Symmetric: writing through the new handle is just as observable.
    #[test]
    fn field_write_through_the_alias_is_observed_through_the_original() {
        let found = aliases_in(
            r#"
            public class Shared {
                static class Node { int v; }
                public int run() {
                    Node a = new Node();
                    Node b = a;
                    b.v = 7;
                    return a.v;
                }
            }
            "#,
        );
        assert_eq!(found.len(), 1, "expected one observable alias: {found:?}");
        assert_eq!(found[0].mutated_through(), "b");
        assert_eq!(found[0].observed_through(), "a");
    }

    /// A copy and an alias are indistinguishable when nothing is written
    /// afterwards. Reporting these would bury the real ones.
    #[test]
    fn an_alias_nobody_writes_through_is_not_reported() {
        let found = aliases_in(
            r#"
            public class Shared {
                static class Node { int v; }
                public int run() {
                    Node a = new Node();
                    Node b = a;
                    return a.v + b.v;
                }
            }
            "#,
        );
        assert!(found.is_empty(), "no write, so no divergence: {found:?}");
    }

    /// The write happens after the last read of the other handle, so a copy
    /// still agrees with Java. This is the case the liveness join buys.
    #[test]
    fn a_write_after_the_other_handle_is_dead_is_not_reported() {
        let found = aliases_in(
            r#"
            public class Shared {
                static class Node { int v; }
                public int run() {
                    Node a = new Node();
                    Node b = a;
                    int seen = b.v;
                    a.v = 5;
                    return seen;
                }
            }
            "#,
        );
        assert!(found.is_empty(), "`b` is dead at the write: {found:?}");
    }

    /// Primitive assignment is a copy in Java too.
    #[test]
    fn primitive_locals_are_not_aliases() {
        let found = aliases_in(
            r#"
            public class Shared {
                public int run() {
                    int a = 1;
                    int b = a;
                    return a + b;
                }
            }
            "#,
        );
        assert!(found.is_empty(), "primitives copy in Java: {found:?}");
    }

    /// Storing a reference into an array element also creates a second live
    /// handle, even though the handle is the array carrier rather than a local
    /// declaration initialized from the source.
    #[test]
    fn array_element_alias_is_observed_after_original_field_write() {
        let found = aliases_in(
            r#"
            public class Shared {
                static class Node { int v; }
                public int run() {
                    Node a = new Node();
                    Node[] cell = new Node[1];
                    cell[0] = a;
                    a.v = 9;
                    return cell[0].v;
                }
            }
            "#,
        );
        assert_eq!(
            found.len(),
            1,
            "array element assignment should create an observable alias: {found:?}"
        );
        assert_eq!(found[0].original(), "a");
        assert_eq!(found[0].alias(), "cell");
        assert_eq!(found[0].mutated_through(), "a");
        assert_eq!(found[0].observed_through(), "cell");
    }

    #[test]
    fn primitive_array_element_assignment_is_not_an_object_alias() {
        let found = aliases_in(
            r#"
            public class Shared {
                public int run() {
                    int a = 1;
                    int[] cell = new int[1];
                    cell[0] = a;
                    a = 9;
                    return cell[0];
                }
            }
            "#,
        );
        assert!(found.is_empty(), "primitive values copy in Java: {found:?}");
    }

    /// Storing a reference through a local receiver's field is the same alias
    /// shape as an array element: the receiver local is the later observed
    /// handle.
    #[test]
    fn field_storage_alias_is_observed_after_original_field_write() {
        let found = aliases_in(
            r#"
            public class Shared {
                static class Node { int v; }
                static class Holder { Node node; }
                public int run() {
                    Node a = new Node();
                    Holder holder = new Holder();
                    holder.node = a;
                    a.v = 11;
                    return holder.node.v;
                }
            }
            "#,
        );
        assert_eq!(
            found.len(),
            1,
            "field storage should create an observable alias: {found:?}"
        );
        assert_eq!(found[0].original(), "a");
        assert_eq!(found[0].alias(), "holder");
        assert_eq!(found[0].mutated_through(), "a");
        assert_eq!(found[0].observed_through(), "holder");
    }

    #[test]
    fn primitive_field_storage_is_not_an_object_alias() {
        let found = aliases_in(
            r#"
            public class Shared {
                static class Holder { int value; }
                public int run() {
                    int a = 1;
                    Holder holder = new Holder();
                    holder.value = a;
                    a = 11;
                    return holder.value;
                }
            }
            "#,
        );
        assert!(
            found.is_empty(),
            "primitive field values copy in Java: {found:?}"
        );
    }

    #[test]
    fn this_field_storage_alias_is_observed_after_original_field_write() {
        let found = aliases_in(
            r#"
            public class Shared {
                static class Node { int v; }
                Node node;
                public int run() {
                    Node a = new Node();
                    this.node = a;
                    a.v = 13;
                    return this.node.v;
                }
            }
            "#,
        );
        assert_eq!(
            found.len(),
            1,
            "this-field storage should create an observable alias: {found:?}"
        );
        assert_eq!(found[0].original(), "a");
        assert_eq!(found[0].alias(), "this.node");
        assert_eq!(found[0].mutated_through(), "a");
        assert_eq!(found[0].observed_through(), "this.node");
    }

    #[test]
    fn this_field_assignment_target_is_not_a_read_observation() {
        let found = aliases_in(
            r#"
            public class Shared {
                static class Node { int v; }
                Node node;
                public int run() {
                    Node a = new Node();
                    this.node = a;
                    a.v = 13;
                    this.node = new Node();
                    return 0;
                }
            }
            "#,
        );
        assert!(
            found.is_empty(),
            "rewriting the this-field should not count as observing the old value: {found:?}"
        );
    }

    #[test]
    fn this_field_storage_after_original_write_is_not_reported() {
        let found = aliases_in(
            r#"
            public class Shared {
                static class Node { int v; }
                Node node;
                public int run() {
                    Node a = new Node();
                    a.v = 13;
                    this.node = a;
                    return this.node.v;
                }
            }
            "#,
        );
        assert!(
            found.is_empty(),
            "a copy made after the write still observes the written value: {found:?}"
        );
    }

    /// A method solved as writing through its receiver is a mutation site, so
    /// the same divergence is found without a syntactic field write.
    #[test]
    fn a_receiver_mutating_call_is_a_mutation_site() {
        let found = aliases_in(
            r#"
            public class Shared {
                static class Counter {
                    int v;
                    void bump() { this.v = this.v + 1; }
                    int get() { return this.v; }
                }
                public int run() {
                    Counter a = new Counter();
                    Counter b = a;
                    a.bump();
                    return b.get();
                }
            }
            "#,
        );
        assert_eq!(
            found.len(),
            1,
            "`bump` writes through its receiver: {found:?}"
        );
        assert!(matches!(
            found[0].mutation(),
            MutationKind::MutatingCall { .. }
        ));
    }

    #[test]
    fn receiver_mutation_lookup_uses_resolved_overload_identity() {
        let found = aliases_in(
            r#"
            public class Shared {
                static class Counter {
                    int v;
                    int bump(int n) { return this.v + n; }
                    int bump(String text) { this.v = text.length(); return this.v; }
                    int get() { return this.v; }
                }
                public int run() {
                    Counter a = new Counter();
                    Counter b = a;
                    a.bump(0);
                    return b.get();
                }
            }
            "#,
        );
        assert!(
            found.is_empty(),
            "the read-only int overload must not inherit mutability from the String overload: {found:?}"
        );
    }

    #[test]
    fn external_receiver_mutability_fact_marks_alias_mutation_site() {
        let source = r#"
            import dep.Record;
            public class Shared {
                public int run(Record a) {
                    Record b = a;
                    a.setFlag(true);
                    return b.hashCode();
                }
            }
            "#;
        let found = aliases_in_with_facts(
            source,
            &external_record_set_flag_fact(ReceiverMutabilityFact::MutatesReceiver),
        );
        assert_eq!(
            found.len(),
            1,
            "external mutator should create an observable alias: {found:?}"
        );
        assert_eq!(found[0].original(), "a");
        assert_eq!(found[0].alias(), "b");
        assert_eq!(found[0].mutated_through(), "a");
        assert_eq!(found[0].observed_through(), "b");
        assert_eq!(
            found[0].mutation(),
            &MutationKind::MutatingCall {
                method: "setFlag".to_string()
            }
        );

        let read_only = aliases_in_with_facts(
            source,
            &external_record_set_flag_fact(ReceiverMutabilityFact::ReadOnly),
        );
        assert!(
            read_only.is_empty(),
            "read-only external facts must not report aliases: {read_only:?}"
        );
    }

    #[test]
    fn external_receiver_mutability_fact_uses_type_param_bound_owner() {
        let source = r#"
            import dep.Record;
            public class Shared<T extends Record> {
                public int run(T a) {
                    T b = a;
                    a.setFlag(true);
                    return b.hashCode();
                }
            }
            "#;
        let found = aliases_in_with_facts(
            source,
            &external_record_set_flag_fact(ReceiverMutabilityFact::MutatesReceiver),
        );
        assert_eq!(
            found.len(),
            1,
            "external mutator should use the type-parameter bound owner: {found:?}"
        );
        assert_eq!(found[0].original(), "a");
        assert_eq!(found[0].alias(), "b");
        assert_eq!(found[0].mutated_through(), "a");
        assert_eq!(found[0].observed_through(), "b");
    }

    #[test]
    fn external_receiver_mutability_fact_uses_array_owner_key() {
        let source = r#"
            public class Shared {
                public int run(String[] a) {
                    String[] b = a;
                    a.setFlag(true);
                    return b.length;
                }
            }
            "#;
        let found = aliases_in_with_facts(
            source,
            &external_set_flag_fact("Array", ReceiverMutabilityFact::MutatesReceiver),
        );
        assert_eq!(
            found.len(),
            1,
            "external mutator should use the shared Array owner key: {found:?}"
        );
        assert_eq!(found[0].original(), "a");
        assert_eq!(found[0].alias(), "b");
        assert_eq!(found[0].mutated_through(), "a");
        assert_eq!(found[0].observed_through(), "b");
    }

    /// A block-bodied lambda is nested inside an expression statement, so the
    /// ordinary statement walk used to skip its field write completely.
    #[test]
    fn block_lambda_field_write_is_an_observable_alias_mutation_site() {
        let found = aliases_in(
            r#"
            import java.util.Arrays;
            public class Shared {
                static class Node { int v; }
                public int run() {
                    Node a = new Node();
                    Node b = a;
                    Arrays.asList(1).forEach(value -> { a.v = 5; });
                    return b.v;
                }
            }
            "#,
        );
        assert_eq!(
            found.len(),
            1,
            "block lambda field write should be observable: {found:?}"
        );
        assert_eq!(found[0].mutated_through(), "a");
        assert_eq!(found[0].observed_through(), "b");
        assert_eq!(
            found[0].mutation(),
            &MutationKind::FieldWrite {
                field: "v".to_string()
            }
        );
    }

    #[test]
    fn stored_lambda_declaration_is_not_a_mutation_site() {
        let found = aliases_in(
            r#"
            public class Shared {
                static class Node { int v; }
                public int run() {
                    Node a = new Node();
                    Node b = a;
                    Runnable r = () -> { a.v = 5; };
                    return b.v;
                }
            }
            "#,
        );
        assert!(
            found.is_empty(),
            "creating a callback must not execute its write: {found:?}"
        );
    }

    #[test]
    fn stored_runnable_run_is_an_observable_alias_mutation_site() {
        let found = aliases_in(
            r#"
            public class Shared {
                static class Node { int v; }
                public int run() {
                    Node a = new Node();
                    Node b = a;
                    Runnable r = () -> { a.v = 5; };
                    r.run();
                    return b.v;
                }
            }
            "#,
        );
        assert_eq!(
            found.len(),
            1,
            "calling a stored runnable should expose its captured write: {found:?}"
        );
        assert_eq!(found[0].mutated_through(), "a");
        assert_eq!(found[0].observed_through(), "b");
        assert_eq!(
            found[0].mutation(),
            &MutationKind::FieldWrite {
                field: "v".to_string()
            }
        );
    }
}
