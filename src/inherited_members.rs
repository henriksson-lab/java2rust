use std::collections::{HashMap, HashSet};

use crate::facts::{
    ExternalApiEffectFact, ExternalFieldFact, ExternalFieldKind, FactSet, NullabilityState,
};
use crate::java_ir::{
    FieldDecl, JavaType, Member, MethodDecl, Modifier, Program, TypeDecl, WildcardBound,
};
use crate::java_signature::{
    JavaCallableSlotKey, source_method_overload_rust_name, source_method_rust_name,
};

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ExternalInheritedInstanceField {
    declaring_type: String,
    field_name: String,
    rust_name: String,
    field_type: JavaType,
    field_nullability: Option<NullabilityState>,
}

impl ExternalInheritedInstanceField {
    pub(crate) fn field_name(&self) -> &str {
        &self.field_name
    }

    pub(crate) fn new(
        declaring_type: String,
        field_name: String,
        rust_name: String,
        field_type: JavaType,
        field_nullability: Option<NullabilityState>,
    ) -> Self {
        Self {
            declaring_type,
            field_name,
            rust_name,
            field_type,
            field_nullability,
        }
    }

    pub(crate) fn rust_name(&self) -> &str {
        &self.rust_name
    }

    pub(crate) fn field_type(&self) -> &JavaType {
        &self.field_type
    }

    pub(crate) fn field_nullability(&self) -> Option<NullabilityState> {
        self.field_nullability
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ExternalInheritedStaticField {
    declaring_type: String,
    field_name: String,
    field_type: JavaType,
}

impl ExternalInheritedStaticField {
    pub(crate) fn new(declaring_type: String, field_name: String, field_type: JavaType) -> Self {
        Self {
            declaring_type,
            field_name,
            field_type,
        }
    }

    pub(crate) fn declaring_type(&self) -> &str {
        &self.declaring_type
    }

    pub(crate) fn field_name(&self) -> &str {
        &self.field_name
    }

    pub(crate) fn field_type(&self) -> &JavaType {
        &self.field_type
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ExternalInheritedInstanceMethod {
    declaring_type: String,
    method_name: String,
    rust_name: String,
    arg_count: usize,
    fact: ExternalApiEffectFact,
}

impl ExternalInheritedInstanceMethod {
    pub(crate) fn new(
        declaring_type: String,
        method_name: String,
        rust_name: String,
        arg_count: usize,
        fact: ExternalApiEffectFact,
    ) -> Self {
        Self {
            declaring_type,
            method_name,
            rust_name,
            arg_count,
            fact,
        }
    }

    pub(crate) fn declaring_type(&self) -> &str {
        &self.declaring_type
    }

    pub(crate) fn method_name(&self) -> &str {
        &self.method_name
    }

    pub(crate) fn rust_name(&self) -> &str {
        &self.rust_name
    }

    pub(crate) fn arg_count(&self) -> usize {
        self.arg_count
    }

    pub(crate) fn fact(&self) -> &ExternalApiEffectFact {
        &self.fact
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct InheritedInstanceField {
    declaring_type: String,
    field: FieldDecl,
    rust_name: String,
}

impl InheritedInstanceField {
    pub(crate) fn new(declaring_type: String, field: FieldDecl, rust_name: String) -> Self {
        Self {
            declaring_type,
            field,
            rust_name,
        }
    }

    pub(crate) fn declaring_type(&self) -> &str {
        &self.declaring_type
    }

    pub(crate) fn field(&self) -> &FieldDecl {
        &self.field
    }

    pub(crate) fn rust_name(&self) -> &str {
        &self.rust_name
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct InheritedInstanceFieldRef<'a> {
    declaring_type: &'a str,
    field: &'a FieldDecl,
}

impl<'a> InheritedInstanceFieldRef<'a> {
    pub(crate) fn new(declaring_type: &'a str, field: &'a FieldDecl) -> Self {
        Self {
            declaring_type,
            field,
        }
    }

    pub(crate) fn declaring_type(&self) -> &'a str {
        self.declaring_type
    }

    pub(crate) fn field(&self) -> &'a FieldDecl {
        self.field
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct InheritedInstanceMethodRef<'a> {
    declaring_type: &'a str,
    declaring_ty: &'a TypeDecl,
    method: &'a crate::java_ir::MethodDecl,
    rust_name: String,
}

impl<'a> InheritedInstanceMethodRef<'a> {
    pub(crate) fn new(
        declaring_type: &'a str,
        declaring_ty: &'a TypeDecl,
        method: &'a crate::java_ir::MethodDecl,
        rust_name: String,
    ) -> Self {
        Self {
            declaring_type,
            declaring_ty,
            method,
            rust_name,
        }
    }

    pub(crate) fn declaring_type(&self) -> &'a str {
        self.declaring_type
    }

    pub(crate) fn declaring_ty(&self) -> &'a TypeDecl {
        self.declaring_ty
    }

    pub(crate) fn method(&self) -> &'a crate::java_ir::MethodDecl {
        self.method
    }

    pub(crate) fn rust_name(&self) -> &str {
        &self.rust_name
    }
}

pub(crate) fn inherited_instance_fields_for_type(
    ty: &TypeDecl,
    support_program: &Program,
) -> Vec<InheritedInstanceField> {
    inherited_instance_field_refs_for_type(ty, support_program)
        .into_iter()
        .map(|inherited| {
            InheritedInstanceField::new(
                inherited.declaring_type().to_string(),
                inherited.field().clone(),
                inherited_instance_field_rust_name_for_type_in_programs(
                    ty,
                    &inherited,
                    &[support_program],
                ),
            )
        })
        .collect()
}

pub(crate) fn inherited_instance_method_refs_for_type<'a>(
    ty: &TypeDecl,
    support_program: &'a Program,
) -> Vec<InheritedInstanceMethodRef<'a>> {
    let mut methods = Vec::new();
    let mut seen_methods = direct_instance_method_slots(ty);
    let direct_method_rust_names = direct_instance_method_rust_names(ty);
    let mut current = ty.extends.as_deref();
    let mut owner_context = ty.name.clone();
    let mut visited_types = HashSet::new();

    while let Some(type_name) = current {
        let Some(super_ty) =
            find_type_decl_for_owner_context(support_program, &owner_context, type_name)
        else {
            break;
        };
        if !visited_types.insert(super_ty.name.clone()) {
            break;
        }

        let substitutions =
            source_ancestor_type_arguments(support_program, ty, super_ty).unwrap_or_default();
        for member in &super_ty.members {
            let Member::Method(method) = member else {
                continue;
            };
            if !is_inheritable_instance_method(method) {
                continue;
            }
            let signature =
                JavaCallableSlotKey::source_method_with_substitutions(method, &substitutions);
            if seen_methods.insert(signature) {
                let rust_name =
                    inherited_method_rust_name_for_child(method, &direct_method_rust_names);
                methods.push(InheritedInstanceMethodRef::new(
                    &super_ty.name,
                    super_ty,
                    method,
                    rust_name,
                ));
            }
        }

        owner_context = super_ty.name.clone();
        current = super_ty.extends.as_deref();
    }

    collect_inherited_interface_default_methods(
        ty,
        support_program,
        &direct_method_rust_names,
        &mut seen_methods,
        &mut methods,
    );
    // Interface dispatch resolves an abstract method inherited through *any*
    // parent as a member of the child interface.  The default-method walk
    // above deliberately excludes abstract methods for classes, but doing the
    // same for an interface loses the signature alias for a second parent in
    // `interface C extends A, B`.  Lowering then asks for `C::bMethod` even
    // though only `B::bMethod` was planned.
    if ty.kind == crate::java_ir::TypeKind::Interface {
        collect_inherited_interface_contract_methods(
            ty,
            support_program,
            &direct_method_rust_names,
            &mut seen_methods,
            &mut methods,
        );
    }

    methods
}

/// Private methods are not inherited Java members, but a concrete inherited
/// method body may call private implementation helpers declared by its owner.
/// Because source inheritance is flattened by copying that body into the
/// emitted child impl, copy those implementation dependencies under an
/// owner-qualified Rust name.  They deliberately stay out of the ordinary
/// inherited-member set (and therefore out of traits and external dispatch).
pub(crate) fn inherited_private_implementation_method_refs_for_type<'a>(
    ty: &TypeDecl,
    support_program: &'a Program,
) -> Vec<InheritedInstanceMethodRef<'a>> {
    let mut owners = Vec::new();
    let mut current = ty.extends.as_deref();
    let mut owner_context = ty.name.clone();
    let mut visited_classes = HashSet::new();
    while let Some(type_name) = current {
        let Some(super_ty) =
            find_type_decl_for_owner_context(support_program, &owner_context, type_name)
        else {
            break;
        };
        if !visited_classes.insert(super_ty.name.clone()) {
            break;
        }
        owners.push(super_ty);
        owner_context = super_ty.name.clone();
        current = super_ty.extends.as_deref();
    }

    // Interface default methods can call private interface helpers too. Add
    // their declaring interfaces to the same owner set while retaining the
    // explicit class walk above for constructor-only superclasses.
    owners.extend(
        inherited_instance_method_refs_for_type(ty, support_program)
            .into_iter()
            .map(|inherited| inherited.declaring_ty()),
    );

    let mut declaring_types = HashSet::new();
    let mut helpers = Vec::new();

    for declaring_ty in owners {
        if !declaring_types.insert(declaring_ty.name.clone()) {
            continue;
        }
        for member in &declaring_ty.members {
            let Member::Method(method) = member else {
                continue;
            };
            if is_static_method(method)
                || !method.modifiers.contains(&Modifier::Private)
                || method.modifiers.contains(&Modifier::Abstract)
                || method.body.is_none()
            {
                continue;
            }
            helpers.push(InheritedInstanceMethodRef::new(
                &declaring_ty.name,
                declaring_ty,
                method,
                inherited_private_implementation_method_rust_name(&declaring_ty.name, method),
            ));
        }
    }

    helpers
}

pub(crate) fn inherited_private_implementation_method_rust_name(
    declaring_type: &str,
    method: &MethodDecl,
) -> String {
    let owner = crate::java_naming::rust_name_for_java_named_type(declaring_type);
    let owner = crate::parser::to_snake_case_unescaped(&owner);
    let method_name = source_method_rust_name(method);
    format!("__java_private_{owner}_{method_name}")
}

/// Every inherited instance contract of an interface, including abstract
/// contracts.  This is intentionally narrower than the class default-method
/// walk: a class's abstract interface methods are an implementation obligation,
/// while a subinterface must expose them as its own callable surface.
fn collect_inherited_interface_contract_methods<'a>(
    ty: &TypeDecl,
    support_program: &'a Program,
    direct_method_rust_names: &HashSet<String>,
    seen_methods: &mut HashSet<JavaCallableSlotKey>,
    methods: &mut Vec<InheritedInstanceMethodRef<'a>>,
) {
    let mut pending = ty
        .extends
        .iter()
        .chain(ty.implements.iter())
        .cloned()
        .map(|interface| (ty.name.clone(), interface))
        .collect::<Vec<_>>();
    let mut visited = HashSet::new();
    while let Some((owner_name, name)) = pending.pop() {
        let Some(interface) = find_type_decl_for_owner_context(support_program, &owner_name, &name)
        else {
            continue;
        };
        if interface.kind != crate::java_ir::TypeKind::Interface
            || !visited.insert(interface.name.clone())
        {
            continue;
        }
        pending.extend(
            interface
                .extends
                .iter()
                .chain(interface.implements.iter())
                .cloned()
                .map(|parent| (interface.name.clone(), parent)),
        );
        let substitutions =
            source_ancestor_type_arguments(support_program, ty, interface).unwrap_or_default();
        for member in &interface.members {
            let Member::Method(method) = member else {
                continue;
            };
            if !is_inheritable_instance_method(method) {
                continue;
            }
            let signature =
                JavaCallableSlotKey::source_method_with_substitutions(method, &substitutions);
            if !seen_methods.insert(signature) {
                continue;
            }
            let rust_name = inherited_method_rust_name_for_child(method, direct_method_rust_names);
            methods.push(InheritedInstanceMethodRef::new(
                &interface.name,
                interface,
                method,
                rust_name,
            ));
        }
    }
}

/// Default methods a type inherits from the interfaces it implements.
///
/// This covers a type's own `implements` entries, their super-interfaces, and
/// the interfaces implemented by its superclasses.
///
/// The previous walk only followed `extends`, so direct interface defaults were
/// not inherited members at all. That mattered less for lowering (the trait
/// carries the default body) than for **signature planning**: with no signature
/// registered for the child, a call like `a.greet()` had no known return type
/// and took the erased-display path, so it compiled and returned `""` where
/// Java returned `"hi a"`.
///
/// Only methods with a body are inherited. An abstract interface method is the
/// implementor's own obligation and is handled by the interface-stub path.
fn collect_inherited_interface_default_methods<'a>(
    ty: &TypeDecl,
    support_program: &'a Program,
    direct_method_rust_names: &HashSet<String>,
    seen_methods: &mut HashSet<JavaCallableSlotKey>,
    methods: &mut Vec<InheritedInstanceMethodRef<'a>>,
) {
    // A class inherits the defaults of the interfaces its *superclasses*
    // implement as well as its own.
    let mut pending = ty
        .implements
        .iter()
        .cloned()
        .map(|interface| (ty.name.clone(), interface))
        .collect::<Vec<_>>();
    let mut super_name = ty.extends.as_deref();
    let mut super_owner_context = ty.name.clone();
    let mut visited_supers = HashSet::new();
    while let Some(name) = super_name {
        let Some(super_ty) =
            find_type_decl_for_owner_context(support_program, &super_owner_context, name)
        else {
            break;
        };
        if !visited_supers.insert(super_ty.name.clone()) {
            break;
        }
        pending.extend(
            super_ty
                .implements
                .iter()
                .cloned()
                .map(|interface| (super_ty.name.clone(), interface)),
        );
        super_owner_context = super_ty.name.clone();
        super_name = super_ty.extends.as_deref();
    }

    let mut visited = HashSet::new();
    while let Some((owner_name, name)) = pending.pop() {
        let Some(interface) = find_type_decl_for_owner_context(support_program, &owner_name, &name)
        else {
            continue;
        };
        if !visited.insert(interface.name.clone()) {
            continue;
        }
        // Super-interfaces contribute their defaults too.
        pending.extend(
            interface
                .extends
                .iter()
                .cloned()
                .map(|super_interface| (interface.name.clone(), super_interface)),
        );
        pending.extend(
            interface
                .implements
                .iter()
                .cloned()
                .map(|super_interface| (interface.name.clone(), super_interface)),
        );

        for member in &interface.members {
            let Member::Method(method) = member else {
                continue;
            };
            if method.body.is_none() || !is_inheritable_instance_method(method) {
                continue;
            }
            let substitutions =
                source_ancestor_type_arguments(support_program, ty, interface).unwrap_or_default();
            let signature =
                JavaCallableSlotKey::source_method_with_substitutions(method, &substitutions);
            if !seen_methods.insert(signature) {
                continue;
            }
            let rust_name = inherited_method_rust_name_for_child(method, direct_method_rust_names);
            methods.push(InheritedInstanceMethodRef::new(
                &interface.name,
                interface,
                method,
                rust_name,
            ));
        }
    }
}

pub(crate) fn external_inherited_instance_fields_for_type(
    ty: &TypeDecl,
    facts: &FactSet,
) -> Vec<ExternalInheritedInstanceField> {
    let direct_fields = direct_instance_field_names(ty);
    let super_names = external_super_names_for_type(ty, facts);
    let mut fields = Vec::new();
    let mut seen_fields = direct_fields;
    for super_name in super_names {
        for fact in &facts.external_field_facts {
            push_external_inherited_instance_field_from_fact(
                &mut fields,
                &mut seen_fields,
                facts,
                fact,
                &super_name,
            );
        }
        if !facts.disable_builtin_external_packs {
            for fact in crate::external_packs::builtin_external_field_facts() {
                push_external_inherited_instance_field_from_fact(
                    &mut fields,
                    &mut seen_fields,
                    facts,
                    fact,
                    &super_name,
                );
            }
        }
    }
    if let Some(super_name) = embedded_external_super_name_for_type(ty, facts)
        && seen_fields.insert("__java_super".to_string())
    {
        fields.push(ExternalInheritedInstanceField::new(
            super_name.clone(),
            "__java_super".to_string(),
            "__java_super".to_string(),
            JavaType::Named {
                name: super_name,
                args: Vec::new(),
            },
            Some(NullabilityState::DefinitelyNonNull),
        ));
    }
    fields
}

/// Returns the concrete external base-class carrier that must remain embedded
/// in a flattened source class.
///
/// A Java class can inherit the external base indirectly through one or more
/// source classes. Those source layers are flattened by the Rust object model,
/// so the external base subobject must be flattened with them as well. Prefer
/// the direct spelling because it retains generic arguments; reachable facts
/// provide the erased transitive fallback.
pub(crate) fn embedded_external_super_name_for_type(
    ty: &TypeDecl,
    facts: &FactSet,
) -> Option<String> {
    let effective = crate::effective_facts::EffectiveFacts::new(facts);
    if let Some(super_name) = ty.extends.as_deref()
        && crate::external_packs::external_superclass_uses_embedded_carrier(super_name)
        && !effective.has_parsed_source_type_capability_for_name(super_name)
    {
        return Some(super_name.to_string());
    }
    external_super_names_for_type(ty, facts)
        .into_iter()
        .find(|super_name| {
            crate::external_packs::external_superclass_uses_embedded_carrier(super_name)
                && !effective.has_parsed_source_type_capability_for_name(super_name)
        })
}

fn push_external_inherited_instance_field_from_fact(
    fields: &mut Vec<ExternalInheritedInstanceField>,
    seen_fields: &mut HashSet<String>,
    facts: &FactSet,
    fact: &ExternalFieldFact,
    super_name: &str,
) {
    if !matches!(
        fact.field_kind,
        ExternalFieldKind::Unknown | ExternalFieldKind::InstanceField
    ) {
        return;
    }
    let field_owner = fact.field.type_name();
    let field_name = fact.field.field_name();
    if !external_field_fact_matches_super_type(field_owner, super_name, fact.provenance) {
        return;
    }
    let rust_name = rust_field_name(field_name);
    if !seen_fields.insert(rust_name.clone()) {
        return;
    }
    fields.push(ExternalInheritedInstanceField::new(
        field_owner.to_string(),
        field_name.to_string(),
        rust_name,
        fact.field_type
            .clone()
            .unwrap_or_else(|| JavaType::Unknown("Object".to_string())),
        inherited_external_field_nullability(facts, field_owner, field_name),
    ));
}

fn inherited_external_field_nullability(
    facts: &FactSet,
    type_name: &str,
    field_name: &str,
) -> Option<NullabilityState> {
    if !facts.disable_builtin_external_packs
        && let Some(state) =
            crate::external_packs::builtin_field_nullability_for(type_name, field_name)
    {
        return Some(state);
    }
    facts.field_nullability_for(type_name, field_name)
}

pub(crate) fn external_inherited_static_final_fields_for_type(
    ty: &TypeDecl,
    facts: &FactSet,
) -> Vec<ExternalInheritedStaticField> {
    let direct_fields = direct_static_final_field_names(ty);
    let super_names = external_super_names_for_type(ty, facts);
    let mut fields = Vec::new();
    let mut seen_fields = direct_fields;
    for super_name in super_names {
        for fact in &facts.external_field_facts {
            push_external_inherited_static_final_field_from_fact(
                &mut fields,
                &mut seen_fields,
                fact,
                &super_name,
            );
        }
        if !facts.disable_builtin_external_packs {
            for fact in crate::external_packs::builtin_external_field_facts() {
                push_external_inherited_static_final_field_from_fact(
                    &mut fields,
                    &mut seen_fields,
                    fact,
                    &super_name,
                );
            }
        }
    }
    fields
}

fn push_external_inherited_static_final_field_from_fact(
    fields: &mut Vec<ExternalInheritedStaticField>,
    seen_fields: &mut HashSet<String>,
    fact: &ExternalFieldFact,
    super_name: &str,
) {
    if fact.field_kind != ExternalFieldKind::StaticFinal {
        return;
    }
    let field_owner = fact.field.type_name();
    let field_name = fact.field.field_name();
    if !external_field_fact_matches_super_type(field_owner, super_name, fact.provenance) {
        return;
    }
    if !seen_fields.insert(field_name.to_string()) {
        return;
    }
    let Some(field_type) = fact.field_type.clone() else {
        return;
    };
    fields.push(ExternalInheritedStaticField::new(
        field_owner.to_string(),
        field_name.to_string(),
        field_type,
    ));
}

pub(crate) fn external_inherited_instance_methods_for_type(
    ty: &TypeDecl,
    facts: &FactSet,
) -> Vec<ExternalInheritedInstanceMethod> {
    let super_names = external_super_names_for_type(ty, facts);
    let mut methods = HashMap::<(String, usize), ExternalInheritedInstanceMethod>::new();
    for super_name in super_names {
        for fact in &facts.external_api_effect_facts {
            record_external_inherited_instance_method_from_fact(
                &mut methods,
                facts,
                fact,
                &super_name,
            );
        }
        if !facts.disable_builtin_external_packs {
            for fact in crate::external_packs::builtin_external_api_effect_facts() {
                record_external_inherited_instance_method_from_fact(
                    &mut methods,
                    facts,
                    fact,
                    &super_name,
                );
            }
        }
    }
    let mut methods = methods.into_values().collect::<Vec<_>>();
    methods.sort_by(|left, right| {
        left.rust_name
            .cmp(&right.rust_name)
            .then(left.arg_count.cmp(&right.arg_count))
    });
    methods
}

fn record_external_inherited_instance_method_from_fact(
    methods: &mut HashMap<(String, usize), ExternalInheritedInstanceMethod>,
    facts: &FactSet,
    fact: &ExternalApiEffectFact,
    super_name: &str,
) {
    let Some(receiver_type) = fact.receiver_type.as_deref() else {
        return;
    };
    if !type_name_matches(receiver_type, super_name) {
        return;
    }
    let Some(arg_count) = fact.arg_count else {
        return;
    };
    let rust_name = crate::call_plan::external_method_rust_name(
        receiver_type,
        &fact.method_name,
        arg_count,
        facts,
    );
    let method = ExternalInheritedInstanceMethod::new(
        receiver_type.to_string(),
        fact.method_name.clone(),
        rust_name.clone(),
        arg_count,
        fact.clone(),
    );
    methods
        .entry((rust_name, arg_count))
        .and_modify(|existing| {
            if external_inherited_method_fact_should_replace(&existing.fact, fact) {
                *existing = method.clone();
            }
        })
        .or_insert(method);
}

fn external_inherited_method_fact_should_replace(
    existing: &ExternalApiEffectFact,
    candidate: &ExternalApiEffectFact,
) -> bool {
    existing.return_type.is_none() && candidate.return_type.is_some()
}

fn external_super_names_for_type(ty: &TypeDecl, facts: &FactSet) -> Vec<String> {
    let mut super_names = ty.extends.iter().cloned().collect::<Vec<_>>();
    super_names.extend(
        facts
            .reachable_hierarchy_edges_for_type(&JavaType::Named {
                name: ty.name.clone(),
                args: Vec::new(),
            })
            .into_iter()
            .map(|edge| edge.super_name),
    );
    super_names.sort();
    super_names.dedup();
    super_names
}

pub(crate) fn inherited_instance_field_refs_for_type<'a>(
    ty: &TypeDecl,
    support_program: &'a Program,
) -> Vec<InheritedInstanceFieldRef<'a>> {
    inherited_instance_field_refs_for_type_in_programs(ty, &[support_program])
}

pub(crate) fn inherited_instance_field_refs_for_type_in_programs<'a>(
    ty: &TypeDecl,
    support_programs: &[&'a Program],
) -> Vec<InheritedInstanceFieldRef<'a>> {
    let mut fields = Vec::new();
    let mut current = ty.extends.as_deref();
    let mut owner_context = ty.name.clone();
    let mut visited_types = HashSet::new();

    while let Some(type_name) = current {
        let Some(super_ty) = find_unique_type_decl_for_owner_context_in_programs(
            support_programs,
            &owner_context,
            type_name,
        ) else {
            break;
        };
        if !visited_types.insert(super_ty.name.clone()) {
            break;
        }

        for member in &super_ty.members {
            let Member::Field(field) = member else {
                continue;
            };
            if is_static_field(field) {
                continue;
            }
            fields.push(InheritedInstanceFieldRef::new(&super_ty.name, field));
        }

        owner_context = super_ty.name.clone();
        current = super_ty.extends.as_deref();
    }

    fields
}

pub(crate) fn inherited_instance_field_rust_name_for_type_in_programs(
    ty: &TypeDecl,
    inherited: &InheritedInstanceFieldRef<'_>,
    support_programs: &[&Program],
) -> String {
    let base_name = find_unique_type_decl_for_owner_context_in_programs(
        support_programs,
        &ty.name,
        inherited.declaring_type(),
    )
    .map(|owner| {
        crate::java_naming::source_instance_field_rust_name(owner, &inherited.field().name)
    })
    .unwrap_or_else(|| rust_field_name(&inherited.field().name));
    let inherited_fields = inherited_instance_field_refs_for_type_in_programs(ty, support_programs);
    inherited_instance_field_rust_name(ty, inherited, &inherited_fields, &base_name)
}

pub(crate) fn inherited_instance_field_rust_name(
    ty: &TypeDecl,
    inherited: &InheritedInstanceFieldRef<'_>,
    inherited_fields: &[InheritedInstanceFieldRef<'_>],
    base_name: &str,
) -> String {
    let direct_count = ty
        .members
        .iter()
        .filter(|member| {
            matches!(member, Member::Field(field)
                if !is_static_field(field) && rust_field_name(&field.name) == base_name)
        })
        .count();
    let inherited_count = inherited_fields
        .iter()
        .filter(|candidate| rust_field_name(&candidate.field().name) == base_name)
        .count();
    if direct_count + inherited_count <= 1 {
        return base_name.to_string();
    }
    format!(
        "__java_hidden_{}_{}",
        crate::parser::to_snake_case(&crate::java_naming::rust_name_for_java_named_type(
            inherited.declaring_type()
        )),
        base_name
    )
}

fn find_unique_type_decl_in_programs<'a>(
    programs: &[&'a Program],
    type_name: &str,
) -> Option<&'a TypeDecl> {
    if let Some(ty) = find_unique_exact_type_decl_in_programs(programs, type_name) {
        return Some(ty);
    }
    let mut found = None;
    for program in programs {
        let Some(ty) = find_type_decl(program, type_name) else {
            continue;
        };
        if found.is_some_and(|found: &TypeDecl| found.name != ty.name) {
            return None;
        }
        found = Some(ty);
    }
    found
}

fn find_unique_type_decl_for_owner_context_in_programs<'a>(
    programs: &[&'a Program],
    owner_name: &str,
    type_name: &str,
) -> Option<&'a TypeDecl> {
    find_unique_exact_type_decl_in_programs(programs, type_name)
        .or_else(|| find_unique_same_package_type_decl_in_programs(programs, owner_name, type_name))
        .or_else(|| find_unique_type_decl_in_programs(programs, type_name))
}

fn find_unique_exact_type_decl_in_programs<'a>(
    programs: &[&'a Program],
    type_name: &str,
) -> Option<&'a TypeDecl> {
    let type_name = type_name.split('<').next().unwrap_or(type_name).trim();
    let mut found = None;
    for program in programs {
        let Some(ty) = program
            .types
            .iter()
            .find_map(|ty| find_exact_type_decl_in_type(ty, type_name))
        else {
            continue;
        };
        if found.is_some_and(|found: &TypeDecl| found.name != ty.name) {
            return None;
        }
        found = Some(ty);
    }
    found
}

fn find_unique_same_package_type_decl_in_programs<'a>(
    programs: &[&'a Program],
    owner_name: &str,
    type_name: &str,
) -> Option<&'a TypeDecl> {
    let type_name = type_name_without_args(type_name);
    if type_name.contains('.') || type_name.contains('$') {
        return None;
    }
    let package = owner_package_name(owner_name)?;
    let qualified = format!("{package}.{type_name}");
    find_unique_exact_type_decl_in_programs(programs, &qualified)
}

fn find_exact_type_decl_in_type<'a>(ty: &'a TypeDecl, type_name: &str) -> Option<&'a TypeDecl> {
    if ty.name == type_name {
        return Some(ty);
    }
    ty.members.iter().find_map(|member| match member {
        Member::Type(nested) => find_exact_type_decl_in_type(nested, type_name),
        _ => None,
    })
}

fn direct_instance_field_names(ty: &TypeDecl) -> HashSet<String> {
    ty.members
        .iter()
        .filter_map(|member| match member {
            Member::Field(field) if !is_static_field(field) => Some(rust_field_name(&field.name)),
            _ => None,
        })
        .collect()
}

fn direct_instance_method_slots(ty: &TypeDecl) -> HashSet<JavaCallableSlotKey> {
    ty.members
        .iter()
        .filter_map(|member| match member {
            Member::Method(method) if !is_static_method(method) => {
                Some(JavaCallableSlotKey::source_method(method))
            }
            _ => None,
        })
        .collect()
}

fn direct_instance_method_rust_names(ty: &TypeDecl) -> HashSet<String> {
    ty.members
        .iter()
        .filter_map(|member| match member {
            Member::Method(method) if !is_static_method(method) => Some(
                method
                    .rust_name
                    .clone()
                    .unwrap_or_else(|| crate::parser::to_snake_case(&method.name)),
            ),
            _ => None,
        })
        .collect()
}

fn inherited_method_rust_name_for_child(
    method: &MethodDecl,
    direct_method_rust_names: &HashSet<String>,
) -> String {
    let inherited_name = method
        .rust_name
        .clone()
        .unwrap_or_else(|| crate::parser::to_snake_case(&method.name));
    if !direct_method_rust_names.contains(&inherited_name) {
        return inherited_name;
    }
    source_method_overload_rust_name(method)
}

fn direct_static_final_field_names(ty: &TypeDecl) -> HashSet<String> {
    ty.members
        .iter()
        .filter_map(|member| match member {
            Member::Field(field)
                if field.modifiers.contains(&Modifier::Static)
                    && field.modifiers.contains(&Modifier::Final) =>
            {
                Some(field.name.clone())
            }
            _ => None,
        })
        .collect()
}

fn rust_field_name(name: &str) -> String {
    crate::parser::to_snake_case(name)
}

fn find_type_decl<'a>(program: &'a Program, type_name: &str) -> Option<&'a TypeDecl> {
    let query = type_name_without_args(type_name);
    let mut exact_matches = Vec::new();
    for ty in &program.types {
        collect_type_decl_matches(ty, query, &mut exact_matches, |candidate, query| {
            type_name_without_args(candidate) == query
        });
    }
    if exact_matches.len() == 1 {
        return exact_matches.into_iter().next();
    }

    let mut alias_matches = Vec::new();
    for ty in &program.types {
        collect_type_decl_matches(ty, type_name, &mut alias_matches, type_name_matches);
    }
    if alias_matches.len() == 1 {
        return alias_matches.into_iter().next();
    }
    None
}

pub(crate) fn find_type_decl_for_owner_context<'a>(
    program: &'a Program,
    owner_name: &str,
    type_name: &str,
) -> Option<&'a TypeDecl> {
    find_exact_type_decl(program, type_name)
        .or_else(|| find_same_package_type_decl(program, owner_name, type_name))
        .or_else(|| find_type_decl(program, type_name))
}

/// Type arguments with which `target` reaches one source class ancestor.
///
/// Java inherited members are viewed after substituting every generic
/// `extends` edge.  Keep that operation in the shared inheritance module so
/// member discovery, signature planning, lowering, and hierarchy-carrier
/// dispatch cannot each invent a subtly different substitution walk.
pub(crate) fn source_class_ancestor_type_arguments(
    program: &Program,
    target: &TypeDecl,
    ancestor: &TypeDecl,
) -> Option<HashMap<String, JavaType>> {
    if target.name == ancestor.name {
        return Some(HashMap::new());
    }
    let mut current = target;
    let mut substitutions = HashMap::<String, JavaType>::new();
    let mut seen = HashSet::new();
    while seen.insert(current.name.clone()) {
        let parent_ref = current.extends.as_deref()?;
        let JavaType::Named { name, args } = crate::parser::parse_hierarchy_type(parent_ref) else {
            return None;
        };
        let parent = find_type_decl_for_owner_context(program, &current.name, &name)?;
        let args = parent
            .type_params
            .iter()
            .enumerate()
            .map(|(index, type_param)| {
                let actual = args.get(index).cloned().unwrap_or_else(|| JavaType::Named {
                    name: type_param.clone(),
                    args: Vec::new(),
                });
                crate::resolve::substitute_method_type_variables(&actual, &substitutions)
            })
            .collect::<Vec<_>>();
        substitutions = parent.type_params.iter().cloned().zip(args).collect();
        if parent.name == ancestor.name {
            return Some(substitutions);
        }
        current = parent;
    }
    None
}

/// Clone an inherited declaration as Java sees it from `target`.
///
/// Method type parameters shadow class type parameters, so those names are
/// deliberately removed before substituting the owner-level declaration.
pub(crate) fn specialized_inherited_method_for_type(
    program: &Program,
    target: &TypeDecl,
    inherited: &InheritedInstanceMethodRef<'_>,
) -> MethodDecl {
    let substitutions = source_ancestor_type_arguments(program, target, inherited.declaring_ty())
        .unwrap_or_default();
    crate::java_type_substitution::specialized_method(inherited.method(), &substitutions)
}

pub(crate) fn source_ancestor_type_arguments(
    program: &Program,
    target: &TypeDecl,
    owner: &TypeDecl,
) -> Option<HashMap<String, JavaType>> {
    if owner.kind == crate::java_ir::TypeKind::Interface {
        crate::representation_lowering::interface_type_arguments_in_target(program, target, owner)
            .map(|args| owner.type_params.iter().cloned().zip(args).collect())
    } else {
        source_class_ancestor_type_arguments(program, target, owner)
    }
}

/// Give source-generic wildcard input parameters a Rust-representable
/// existential ABI.
///
/// A Java parameter `Model<?>` accepts `Model<X>` for any admissible `X`.
/// Rendering the wildcard as `Box<dyn Any>` instead produces the invariant and
/// unrelated type `Model<Box<dyn Any>>`.  A fresh method type parameter is the
/// direct Rust encoding of that input existential: callers infer it from the
/// argument and the source generic's declared bound remains available inside
/// the method body.
pub(crate) fn method_with_synthesized_source_wildcard_params(
    method: &MethodDecl,
    emitted_owner: &TypeDecl,
    support_programs: &[&Program],
) -> MethodDecl {
    let (params, synthesized) =
        source_wildcard_input_reification(method, emitted_owner, support_programs);
    let mut effective = method.clone();
    effective.params = params;
    for (name, bound) in synthesized {
        effective.type_params.push(name);
        effective.type_param_bounds.push(bound);
    }
    effective
}

/// Capabilities demanded by the exact same captures as callable emission.
/// Inventory callers need neither a cloned method body nor a second wildcard
/// bound traversal with potentially different source-owner resolution.
pub(crate) fn synthesized_source_wildcard_bounds_for_method(
    method: &MethodDecl,
    emitted_owner: &TypeDecl,
    support_programs: &[&Program],
) -> Vec<Option<JavaType>> {
    source_wildcard_input_reification(method, emitted_owner, support_programs)
        .1
        .into_iter()
        .map(|(_, bound)| bound)
        .collect()
}

fn source_wildcard_input_reification(
    method: &MethodDecl,
    emitted_owner: &TypeDecl,
    support_programs: &[&Program],
) -> (Vec<crate::java_ir::Param>, Vec<(String, Option<JavaType>)>) {
    let mut params = method.params.clone();
    let mut used_names = emitted_owner
        .type_params
        .iter()
        .chain(method.type_params.iter())
        .cloned()
        .collect::<HashSet<_>>();
    let mut next_index = 0usize;
    let mut synthesized = Vec::<(String, Option<JavaType>)>::new();

    // `class C<H extends Model<?>> { f(H value) { ... } }` has the same
    // existential input problem as an explicit `f(Model<?> value)`.  Rust
    // cannot express the hidden `Model` argument through the invariant class
    // parameter `H`, and spelling its bound as `Model<Box<dyn Any>>` invents
    // a concrete, usually invalid instantiation.  Reify a direct input-only
    // class parameter at the method boundary, exactly as inheritance already
    // does after substituting `H` with `Model<?>` on a child.
    //
    // Reuse one synthesized wildcard for every `H` parameter in the method:
    // Java requires those inputs to have the same H.  Fail closed when H is
    // nested in another invariant carrier or escapes through the return/
    // throws surface; those cases need a richer existential representation.
    // An abstract generic owner defines a subtype/override contract: its
    // class parameter identifies the concrete payload accepted by each
    // implementation.  Turning that parameter into an independent method
    // generic would weaken `Writer<D>::write(D)` into `write<Product<W>>`
    // and disconnect hierarchy-carrier dispatch from D.  Concrete owners can
    // use the input-only erasure below after the escape checks prove that the
    // parameter has no other material role.
    let reify_owner_params = !emitted_owner.modifiers.contains(&Modifier::Abstract);
    for (index, owner_param) in emitted_owner
        .type_params
        .iter()
        .enumerate()
        .filter(|_| reify_owner_params)
    {
        if method.type_params.contains(owner_param)
            || java_type_contains_named_type_param(&method.return_type, owner_param)
            || method
                .throws
                .iter()
                .any(|ty| java_type_contains_named_type_param(ty, owner_param))
            || method
                .type_param_bounds
                .iter()
                .flatten()
                .any(|ty| java_type_contains_named_type_param(ty, owner_param))
        {
            continue;
        }
        let matching_params = params
            .iter()
            .enumerate()
            .filter(|(_, param)| java_type_contains_named_type_param(&param.ty, owner_param))
            .collect::<Vec<_>>();
        if matching_params.is_empty()
            || matching_params
                .iter()
                .any(|(_, param)| !java_type_is_named_type_param(&param.ty, owner_param))
        {
            continue;
        }
        let Some(Some(bound)) = emitted_owner.type_param_bounds.get(index) else {
            continue;
        };
        let synthesized_before = synthesized.len();
        let reified_bound = synthesize_source_wildcards_in_type(
            bound,
            &emitted_owner.name,
            support_programs,
            &mut used_names,
            &mut next_index,
            &mut synthesized,
        );
        if synthesized.len() == synthesized_before {
            continue;
        }
        for param in &mut params {
            if java_type_is_named_type_param(&param.ty, owner_param) {
                param.ty = reified_bound.clone();
            }
        }
    }
    for param in &mut params {
        param.ty = synthesize_source_wildcards_in_type(
            &param.ty,
            &emitted_owner.name,
            support_programs,
            &mut used_names,
            &mut next_index,
            &mut synthesized,
        );
    }
    (params, synthesized)
}

fn java_type_is_named_type_param(ty: &JavaType, type_param: &str) -> bool {
    matches!(ty, JavaType::Named { name, args } if name == type_param && args.is_empty())
}

fn java_type_contains_named_type_param(ty: &JavaType, type_param: &str) -> bool {
    match ty {
        JavaType::Named { name, args } => {
            (name == type_param && args.is_empty())
                || args
                    .iter()
                    .any(|arg| java_type_contains_named_type_param(arg, type_param))
        }
        JavaType::Array(element) => java_type_contains_named_type_param(element, type_param),
        JavaType::Wildcard(Some(crate::java_ir::WildcardBound::Extends(bound)))
        | JavaType::Wildcard(Some(crate::java_ir::WildcardBound::Super(bound))) => {
            java_type_contains_named_type_param(bound, type_param)
        }
        JavaType::Intersection(bounds) => bounds
            .iter()
            .any(|bound| java_type_contains_named_type_param(bound, type_param)),
        JavaType::Void
        | JavaType::Primitive(_)
        | JavaType::String
        | JavaType::Wildcard(None)
        | JavaType::Unknown(_) => false,
    }
}

/// Whether every material use of an owner parameter bounded by `Source<?>`
/// can be represented by the method-level wildcard reification above.
///
/// This is also the proof that the enclosing Rust `impl` must not carry the
/// false invariant bound `H: SourceBound<Box<dyn Any>>`: each usable H value
/// has moved to an input method generic, while fields, constructors, nested
/// invariant carriers, and escaping return types deliberately fail closed.
pub(crate) fn owner_wildcard_bound_is_fully_input_reified(
    owner: &TypeDecl,
    type_param: &str,
    bound: &JavaType,
    support_programs: &[&Program],
) -> bool {
    if owner.modifiers.contains(&Modifier::Abstract) {
        return false;
    }
    let JavaType::Named { name, args } = bound else {
        return false;
    };
    if args.is_empty()
        || !args.iter().any(|arg| matches!(arg, JavaType::Wildcard(_)))
        || support_programs
            .iter()
            .all(|program| find_type_decl_for_owner_context(program, &owner.name, name).is_none())
        || owner.members.iter().any(|member| match member {
            Member::Field(field) => java_type_contains_named_type_param(&field.ty, type_param),
            Member::Constructor(constructor) => constructor
                .params
                .iter()
                .any(|param| java_type_contains_named_type_param(&param.ty, type_param)),
            Member::Method(method) => {
                java_type_contains_named_type_param(&method.return_type, type_param)
                    || method
                        .throws
                        .iter()
                        .any(|ty| java_type_contains_named_type_param(ty, type_param))
                    || method
                        .type_param_bounds
                        .iter()
                        .flatten()
                        .any(|ty| java_type_contains_named_type_param(ty, type_param))
                    || method.params.iter().any(|param| {
                        java_type_contains_named_type_param(&param.ty, type_param)
                            && !java_type_is_named_type_param(&param.ty, type_param)
                    })
            }
            Member::Type(_) | Member::Unsupported { .. } => false,
        })
    {
        return false;
    }

    owner.members.iter().any(|member| match member {
        Member::Method(method) => method
            .params
            .iter()
            .any(|param| java_type_is_named_type_param(&param.ty, type_param)),
        _ => false,
    })
}

fn synthesize_source_wildcards_in_type(
    ty: &JavaType,
    owner_name: &str,
    support_programs: &[&Program],
    used_names: &mut HashSet<String>,
    next_index: &mut usize,
    synthesized: &mut Vec<(String, Option<JavaType>)>,
) -> JavaType {
    match ty {
        JavaType::Array(element) => JavaType::Array(Box::new(synthesize_source_wildcards_in_type(
            element,
            owner_name,
            support_programs,
            used_names,
            next_index,
            synthesized,
        ))),
        JavaType::Named { name, args } => {
            // Only direct wildcard arguments demand this declaration's
            // parameter bounds. Nested generic arguments resolve their own
            // owner recursively; ordinary named types need no source scan.
            let source_decl = args
                .iter()
                .any(|arg| {
                    matches!(
                        arg,
                        JavaType::Wildcard(None)
                            | JavaType::Wildcard(Some(crate::java_ir::WildcardBound::Extends(_)))
                    )
                })
                .then(|| {
                    support_programs.iter().find_map(|program| {
                        find_type_decl_for_owner_context(program, owner_name, name)
                    })
                })
                .flatten();
            let args = args
                .iter()
                .enumerate()
                .map(|(index, arg)| match arg {
                    JavaType::Wildcard(None) if source_decl.is_some() => {
                        let synthetic = fresh_wildcard_type_param(used_names, next_index);
                        let bound = source_decl
                            .and_then(|decl| decl.type_param_bounds.get(index))
                            .cloned()
                            .flatten();
                        synthesized.push((synthetic.clone(), bound));
                        JavaType::Named {
                            name: synthetic,
                            args: Vec::new(),
                        }
                    }
                    JavaType::Wildcard(Some(crate::java_ir::WildcardBound::Extends(bound)))
                        if source_decl.is_some() =>
                    {
                        let synthetic = fresh_wildcard_type_param(used_names, next_index);
                        synthesized.push((synthetic.clone(), Some(bound.as_ref().clone())));
                        JavaType::Named {
                            name: synthetic,
                            args: Vec::new(),
                        }
                    }
                    other => synthesize_source_wildcards_in_type(
                        other,
                        owner_name,
                        support_programs,
                        used_names,
                        next_index,
                        synthesized,
                    ),
                })
                .collect();
            JavaType::Named {
                name: name.clone(),
                args,
            }
        }
        JavaType::Intersection(bounds) => JavaType::Intersection(
            bounds
                .iter()
                .map(|bound| {
                    synthesize_source_wildcards_in_type(
                        bound,
                        owner_name,
                        support_programs,
                        used_names,
                        next_index,
                        synthesized,
                    )
                })
                .collect(),
        ),
        other => other.clone(),
    }
}

fn fresh_wildcard_type_param(used_names: &mut HashSet<String>, next_index: &mut usize) -> String {
    loop {
        let candidate = format!("__JavaWildcard{}", *next_index);
        *next_index += 1;
        if used_names.insert(candidate.clone()) {
            return candidate;
        }
    }
}

/// Whether a type parameter belongs to the reserved namespace used when a
/// public source wildcard is reified as a Rust callable generic.
pub(crate) fn is_synthesized_source_wildcard_type_param(name: &str) -> bool {
    name.strip_prefix("__JavaWildcard").is_some_and(|suffix| {
        !suffix.is_empty() && suffix.bytes().all(|byte| byte.is_ascii_digit())
    })
}

/// Whether an emitted Java type mentions a callable-generic introduced by
/// wildcard reification.
///
/// These parameters are inferred by Rust at the rewritten method boundary.
/// They are not Java resolver variables and therefore must not be replaced by
/// the resolver's pre-rewrite owner type parameter during call specialization.
pub(crate) fn java_type_contains_synthesized_source_wildcard(ty: &JavaType) -> bool {
    match ty {
        JavaType::Array(element) => {
            java_type_contains_synthesized_source_wildcard(element.as_ref())
        }
        JavaType::Named { name, args } => {
            is_synthesized_source_wildcard_type_param(name)
                || args
                    .iter()
                    .any(java_type_contains_synthesized_source_wildcard)
        }
        JavaType::Intersection(bounds) => bounds
            .iter()
            .any(java_type_contains_synthesized_source_wildcard),
        JavaType::Wildcard(Some(WildcardBound::Extends(bound)))
        | JavaType::Wildcard(Some(WildcardBound::Super(bound))) => {
            java_type_contains_synthesized_source_wildcard(bound.as_ref())
        }
        JavaType::Void
        | JavaType::Primitive(_)
        | JavaType::String
        | JavaType::Wildcard(None)
        | JavaType::Unknown(_) => false,
    }
}

fn find_exact_type_decl<'a>(program: &'a Program, type_name: &str) -> Option<&'a TypeDecl> {
    let query = type_name_without_args(type_name);
    let mut exact_matches = Vec::new();
    for ty in &program.types {
        collect_type_decl_matches(ty, query, &mut exact_matches, |candidate, query| {
            type_name_without_args(candidate) == query
        });
    }
    match exact_matches.as_slice() {
        [ty] => Some(*ty),
        _ => None,
    }
}

fn find_same_package_type_decl<'a>(
    program: &'a Program,
    owner_name: &str,
    type_name: &str,
) -> Option<&'a TypeDecl> {
    let query = type_name_without_args(type_name);
    if query.contains('.') || query.contains('$') {
        return None;
    }
    let package = owner_package_name(owner_name)?;
    let qualified = format!("{package}.{query}");
    find_exact_type_decl(program, &qualified)
}

fn owner_package_name(owner_name: &str) -> Option<&str> {
    owner_name.rsplit_once('.').map(|(package, _)| package)
}

fn collect_type_decl_matches<'a>(
    ty: &'a TypeDecl,
    type_name: &str,
    matches: &mut Vec<&'a TypeDecl>,
    predicate: fn(&str, &str) -> bool,
) {
    if predicate(&ty.name, type_name) {
        matches.push(ty);
    }
    for member in &ty.members {
        if let Member::Type(nested) = member {
            collect_type_decl_matches(nested, type_name, matches, predicate);
        }
    }
}

fn type_name_matches(candidate: &str, query: &str) -> bool {
    let candidate_aliases = type_name_aliases(candidate);
    let query_aliases = type_name_aliases(query);
    candidate_aliases
        .iter()
        .any(|candidate| query_aliases.iter().any(|query| query == candidate))
}

fn external_field_fact_matches_super_type(
    fact_type_name: &str,
    super_name: &str,
    provenance: crate::facts::FactProvenance,
) -> bool {
    if provenance == crate::facts::FactProvenance::BuiltinExternalModel
        && fact_type_name.contains('.')
        && !super_name.contains('.')
    {
        return false;
    }
    type_name_matches(fact_type_name, super_name)
}

fn type_name_without_args(type_name: &str) -> &str {
    type_name.split('<').next().unwrap_or(type_name).trim()
}

fn type_name_aliases(type_name: &str) -> Vec<String> {
    let without_args = type_name_without_args(type_name);
    let simple = without_args
        .rsplit('.')
        .next()
        .unwrap_or(without_args)
        .trim();
    let mut aliases = vec![
        without_args.to_string(),
        simple.to_string(),
        crate::parser::to_snake_case(without_args),
        crate::parser::to_snake_case(simple),
    ];
    aliases.retain(|alias| !alias.is_empty());
    aliases.sort();
    aliases.dedup();
    aliases
}

fn is_static_field(field: &FieldDecl) -> bool {
    field.modifiers.contains(&Modifier::Static)
}

fn is_static_method(method: &crate::java_ir::MethodDecl) -> bool {
    method.modifiers.contains(&Modifier::Static)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostics::Span;
    use crate::java_ir::{Block, MethodDecl, PrimitiveType, TypeKind};

    fn method(name: &str, params: Vec<JavaType>, rust_name: &str) -> Member {
        Member::Method(MethodDecl {
            name: name.to_string(),
            rust_name: Some(rust_name.to_string()),
            type_params: Vec::new(),
            type_param_bounds: Vec::new(),
            return_type: JavaType::Primitive(PrimitiveType::Int),
            throws: Vec::new(),
            params: params
                .into_iter()
                .enumerate()
                .map(|(index, ty)| crate::java_ir::Param {
                    name: format!("arg{index}"),
                    source_name: format!("arg{index}"),
                    ty,
                    is_varargs: false,
                    binding: None,
                })
                .collect(),
            modifiers: Vec::new(),
            body: Some(Block {
                statements: Vec::new(),
            }),
            span: Span::default(),
        })
    }

    fn field(name: &str) -> Member {
        Member::Field(FieldDecl {
            name: name.to_string(),
            ty: JavaType::Primitive(PrimitiveType::Int),
            modifiers: Vec::new(),
            initializer: None,
            span: Span::default(),
        })
    }

    fn class(name: &str, extends: Option<&str>, members: Vec<Member>) -> TypeDecl {
        TypeDecl {
            source_names: None,
            static_init_blocks: Vec::new(),
            kind: TypeKind::Class,
            name: name.to_string(),
            type_params: Vec::new(),
            type_param_bounds: Vec::new(),
            modifiers: Vec::new(),
            extends: extends.map(str::to_string),
            implements: Vec::new(),
            permits: Vec::new(),
            members,
            span: Span::default(),
        }
    }

    fn interface(name: &str, extends: Vec<&str>, members: Vec<Member>) -> TypeDecl {
        TypeDecl {
            kind: TypeKind::Interface,
            extends: extends.into_iter().map(str::to_string).next(),
            ..class(name, None, members)
        }
    }

    fn class_implementing(name: &str, implements: Vec<&str>) -> TypeDecl {
        TypeDecl {
            implements: implements.into_iter().map(str::to_string).collect(),
            ..class(name, None, Vec::new())
        }
    }

    #[test]
    fn inherited_override_slots_use_the_child_generic_frame() {
        let parsed = crate::parser::parse_java_source(
            r#"
            class Parent<A> {
                public boolean same(A left, A right) { return false; }
                public boolean same(int value) { return false; }
            }
            class Child<T> extends Parent<T> {
                public boolean same(T left, T right) { return true; }
            }
            class Fixed extends Parent<String> {
                public boolean same(String left, String right) { return true; }
            }
            interface First<A> { default A echo(A value) { return value; } }
            interface Second<T> extends First<T> {
                default T echo(T value) { return value; }
            }
            class Leaf implements Second<String> { }
        "#,
        );
        let program = &parsed.program;
        for name in ["Child", "Fixed"] {
            let child = find_type_decl(program, name).unwrap();
            let inherited = inherited_instance_method_refs_for_type(child, program);
            let same = inherited
                .iter()
                .filter(|method| method.method().name == "same")
                .collect::<Vec<_>>();
            assert_eq!(same.len(), 1, "{name}: {inherited:?}");
            assert_eq!(same[0].method().params.len(), 1);
        }
        let leaf = find_type_decl(program, "Leaf").unwrap();
        let inherited = inherited_instance_method_refs_for_type(leaf, program);
        let echo = inherited
            .iter()
            .filter(|method| method.method().name == "echo")
            .collect::<Vec<_>>();
        assert_eq!(echo.len(), 1, "{inherited:?}");
        assert_eq!(echo[0].declaring_type(), "Second");
    }

    #[test]
    fn inherited_methods_use_java_signature_slots_not_rust_name() {
        let int_ty = JavaType::Primitive(PrimitiveType::Int);
        let program = Program {
            package: None,
            imports: Vec::new(),
            types: vec![
                class(
                    "Base",
                    None,
                    vec![method("f", vec![int_ty.clone(), int_ty.clone()], "f")],
                ),
                class("Child", Some("Base"), vec![method("f", vec![int_ty], "f")]),
            ],
        };
        let child = find_type_decl(&program, "Child").unwrap();

        let inherited = inherited_instance_method_refs_for_type(child, &program);

        assert_eq!(inherited.len(), 1);
        assert_eq!(inherited[0].method.name, "f");
        assert_eq!(inherited[0].method.params.len(), 2);
        assert_eq!(inherited[0].rust_name, "f_int_int");
    }

    #[test]
    fn subinterface_inherits_abstract_contracts_from_every_parent() {
        let mut left = interface("Left", Vec::new(), vec![method("left", Vec::new(), "left")]);
        let mut right = interface(
            "Right",
            Vec::new(),
            vec![method("right", Vec::new(), "right")],
        );
        for ty in [&mut left, &mut right] {
            for member in &mut ty.members {
                if let Member::Method(method) = member {
                    method.body = None;
                }
            }
        }
        let child = TypeDecl {
            implements: vec!["Right".to_string()],
            ..interface("Child", vec!["Left"], Vec::new())
        };
        let program = Program {
            package: None,
            imports: Vec::new(),
            types: vec![left, right, child],
        };
        let child = find_type_decl(&program, "Child").expect("child interface");
        let inherited = inherited_instance_method_refs_for_type(child, &program);
        let names = inherited
            .iter()
            .map(|method| method.method.name.as_str())
            .collect::<HashSet<_>>();
        assert_eq!(names, HashSet::from(["left", "right"]));
    }

    #[test]
    fn inherited_source_methods_prefer_same_package_simple_super() {
        let program = Program {
            package: None,
            imports: Vec::new(),
            types: vec![
                class("a.Base", None, vec![method("v", Vec::new(), "v")]),
                class("b.Base", None, vec![method("v", Vec::new(), "v")]),
                class("AmbiguousChild", Some("Base"), Vec::new()),
                class("b.Child", Some("Base"), Vec::new()),
                class("ExactChild", Some("b.Base"), Vec::new()),
            ],
        };
        let ambiguous_child = find_type_decl(&program, "AmbiguousChild").unwrap();
        let same_package_child = find_type_decl(&program, "b.Child").unwrap();
        let exact_child = find_type_decl(&program, "ExactChild").unwrap();

        assert!(
            inherited_instance_method_refs_for_type(ambiguous_child, &program).is_empty(),
            "bare same-simple package super lookup must fail closed"
        );
        let same_package_inherited =
            inherited_instance_method_refs_for_type(same_package_child, &program);
        assert_eq!(same_package_inherited.len(), 1);
        assert_eq!(same_package_inherited[0].declaring_type, "b.Base");
        assert_eq!(same_package_inherited[0].method.name, "v");

        let inherited = inherited_instance_method_refs_for_type(exact_child, &program);
        assert_eq!(inherited.len(), 1);
        assert_eq!(inherited[0].declaring_type, "b.Base");
        assert_eq!(inherited[0].method.name, "v");
    }

    #[test]
    fn inherited_source_fields_prefer_same_package_simple_super() {
        let program = Program {
            package: None,
            imports: Vec::new(),
            types: vec![
                class("a.Base", None, vec![field("wrong")]),
                class("b.Base", None, vec![field("right")]),
                class("AmbiguousChild", Some("Base"), Vec::new()),
                class("b.Child", Some("Base"), Vec::new()),
            ],
        };
        let ambiguous_child = find_type_decl(&program, "AmbiguousChild").unwrap();
        let same_package_child = find_type_decl(&program, "b.Child").unwrap();

        assert!(
            inherited_instance_field_refs_for_type(ambiguous_child, &program).is_empty(),
            "bare same-simple package field inheritance must fail closed without owner package"
        );
        let inherited = inherited_instance_field_refs_for_type(same_package_child, &program);
        assert_eq!(inherited.len(), 1);
        assert_eq!(inherited[0].declaring_type(), "b.Base");
        assert_eq!(inherited[0].field().name, "right");
    }

    #[test]
    fn inherited_interface_defaults_prefer_same_package_simple_owner() {
        let program = Program {
            package: None,
            imports: Vec::new(),
            types: vec![
                interface(
                    "a.Named",
                    Vec::new(),
                    vec![method("wrong", Vec::new(), "wrong")],
                ),
                interface(
                    "b.Named",
                    Vec::new(),
                    vec![method("right", Vec::new(), "right")],
                ),
                class_implementing("AmbiguousChild", vec!["Named"]),
                class_implementing("b.Child", vec!["Named"]),
            ],
        };
        let ambiguous_child = find_type_decl(&program, "AmbiguousChild").unwrap();
        let same_package_child = find_type_decl(&program, "b.Child").unwrap();

        assert!(
            inherited_instance_method_refs_for_type(ambiguous_child, &program).is_empty(),
            "bare same-simple interface lookup must fail closed without owner package"
        );
        let inherited = inherited_instance_method_refs_for_type(same_package_child, &program);
        assert_eq!(inherited.len(), 1);
        assert_eq!(inherited[0].declaring_type, "b.Named");
        assert_eq!(inherited[0].method.name, "right");
    }

    #[test]
    fn external_inherited_instance_methods_prefer_typed_builtin_facts() {
        let ty = class("GenomicsLikeTool", Some("GATKTool"), Vec::new());

        let inherited = external_inherited_instance_methods_for_type(&ty, &FactSet::default());
        let method = inherited
            .iter()
            .find(|method| method.rust_name() == "get_default_tool_v_c_f_header_lines")
            .expect("GATKTool builtin method should be inherited");

        assert_eq!(method.arg_count(), 0);
        assert_eq!(
            method.fact().return_type,
            Some(JavaType::Named {
                name: "Set".to_string(),
                args: vec![JavaType::Named {
                    name: "VCFHeaderLine".to_string(),
                    args: Vec::new(),
                }],
            })
        );
    }
}

fn is_inheritable_instance_method(method: &crate::java_ir::MethodDecl) -> bool {
    !is_static_method(method)
        && !method.modifiers.contains(&Modifier::Private)
        && !method.modifiers.contains(&Modifier::Abstract)
}
