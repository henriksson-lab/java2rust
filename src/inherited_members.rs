use std::collections::{HashMap, HashSet};

use crate::facts::{
    ExternalApiEffectFact, ExternalFieldFact, ExternalFieldKind, FactSet, NullabilityState,
};
use crate::java_ir::{FieldDecl, JavaType, Member, MethodDecl, Modifier, Program, TypeDecl};
use crate::java_signature::{JavaCallableSlotKey, source_method_overload_rust_name};

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ExternalInheritedInstanceField {
    declaring_type: String,
    field_name: String,
    rust_name: String,
    field_type: JavaType,
    field_nullability: Option<NullabilityState>,
}

impl ExternalInheritedInstanceField {
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
}

impl InheritedInstanceField {
    pub(crate) fn new(declaring_type: String, field: FieldDecl) -> Self {
        Self {
            declaring_type,
            field,
        }
    }

    pub(crate) fn declaring_type(&self) -> &str {
        &self.declaring_type
    }

    pub(crate) fn field(&self) -> &FieldDecl {
        &self.field
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

        for member in &super_ty.members {
            let Member::Method(method) = member else {
                continue;
            };
            if !is_inheritable_instance_method(method) {
                continue;
            }
            let signature = JavaCallableSlotKey::source_method(method);
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

    methods
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
            let signature = JavaCallableSlotKey::source_method(method);
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
    fields
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
            record_external_inherited_instance_method_from_fact(&mut methods, fact, &super_name);
        }
        if !facts.disable_builtin_external_packs {
            for fact in crate::external_packs::builtin_external_api_effect_facts() {
                record_external_inherited_instance_method_from_fact(
                    &mut methods,
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
    let rust_name = crate::parser::to_snake_case(&fact.method_name);
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
    let mut seen_fields = direct_instance_field_names(ty);
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
            if seen_fields.insert(rust_field_name(&field.name)) {
                fields.push(InheritedInstanceFieldRef::new(&super_ty.name, field));
            }
        }

        owner_context = super_ty.name.clone();
        current = super_ty.extends.as_deref();
    }

    fields
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

fn find_type_decl_for_owner_context<'a>(
    program: &'a Program,
    owner_name: &str,
    type_name: &str,
) -> Option<&'a TypeDecl> {
    find_exact_type_decl(program, type_name)
        .or_else(|| find_same_package_type_decl(program, owner_name, type_name))
        .or_else(|| find_type_decl(program, type_name))
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
