use std::collections::{HashMap, HashSet};

use crate::facts::{
    ExternalApiEffectFact, ExternalFieldFact, ExternalFieldKind, FactSet, NullabilityState,
};
use crate::java_ir::{FieldDecl, JavaType, Member, MethodDecl, Modifier, Program, TypeDecl};
use crate::java_signature::{JavaCallableSlotKey, source_method_overload_rust_name};

#[derive(Debug, Clone, PartialEq)]
pub struct ExternalInheritedInstanceField {
    pub declaring_type: String,
    pub field_name: String,
    pub rust_name: String,
    pub field_type: JavaType,
    pub field_nullability: Option<NullabilityState>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExternalInheritedStaticField {
    pub declaring_type: String,
    pub field_name: String,
    pub field_type: JavaType,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExternalInheritedInstanceMethod {
    pub declaring_type: String,
    pub method_name: String,
    pub rust_name: String,
    pub arg_count: usize,
    pub fact: ExternalApiEffectFact,
}

#[derive(Debug, Clone, PartialEq)]
pub struct InheritedInstanceField {
    pub declaring_type: String,
    pub field: FieldDecl,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InheritedInstanceFieldRef<'a> {
    pub declaring_type: &'a str,
    pub field: &'a FieldDecl,
}

#[derive(Debug, Clone, PartialEq)]
pub struct InheritedInstanceMethodRef<'a> {
    pub declaring_type: &'a str,
    pub declaring_ty: &'a TypeDecl,
    pub method: &'a crate::java_ir::MethodDecl,
    pub rust_name: String,
}

pub fn inherited_instance_fields_for_type(
    ty: &TypeDecl,
    support_program: &Program,
) -> Vec<InheritedInstanceField> {
    inherited_instance_field_refs_for_type(ty, support_program)
        .into_iter()
        .map(|inherited| InheritedInstanceField {
            declaring_type: inherited.declaring_type.to_string(),
            field: inherited.field.clone(),
        })
        .collect()
}

pub fn inherited_instance_method_refs_for_type<'a>(
    ty: &TypeDecl,
    support_program: &'a Program,
) -> Vec<InheritedInstanceMethodRef<'a>> {
    let mut methods = Vec::new();
    let mut seen_methods = direct_instance_method_slots(ty);
    let direct_method_rust_names = direct_instance_method_rust_names(ty);
    let mut current = ty.extends.as_deref();
    let mut visited_types = HashSet::new();

    while let Some(type_name) = current {
        let Some(super_ty) = find_type_decl(support_program, type_name) else {
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
                methods.push(InheritedInstanceMethodRef {
                    declaring_type: &super_ty.name,
                    declaring_ty: super_ty,
                    method,
                    rust_name,
                });
            }
        }

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
    let mut pending = ty.implements.clone();
    let mut super_name = ty.extends.as_deref();
    let mut visited_supers = HashSet::new();
    while let Some(name) = super_name {
        let Some(super_ty) = find_type_decl(support_program, name) else {
            break;
        };
        if !visited_supers.insert(super_ty.name.clone()) {
            break;
        }
        pending.extend(super_ty.implements.iter().cloned());
        super_name = super_ty.extends.as_deref();
    }

    let mut visited = HashSet::new();
    while let Some(name) = pending.pop() {
        let Some(interface) = find_type_decl(support_program, &name) else {
            continue;
        };
        if !visited.insert(interface.name.clone()) {
            continue;
        }
        // Super-interfaces contribute their defaults too.
        pending.extend(interface.extends.iter().cloned());
        pending.extend(interface.implements.iter().cloned());

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
            methods.push(InheritedInstanceMethodRef {
                declaring_type: &interface.name,
                declaring_ty: interface,
                method,
                rust_name,
            });
        }
    }
}

pub fn external_inherited_instance_fields_for_type(
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
    if !external_field_fact_matches_super_type(&fact.field.type_name, super_name, fact.provenance) {
        return;
    }
    let rust_name = rust_field_name(&fact.field.field_name);
    if !seen_fields.insert(rust_name.clone()) {
        return;
    }
    fields.push(ExternalInheritedInstanceField {
        declaring_type: fact.field.type_name.clone(),
        field_name: fact.field.field_name.clone(),
        rust_name,
        field_type: fact
            .field_type
            .clone()
            .unwrap_or_else(|| JavaType::Unknown("Object".to_string())),
        field_nullability: inherited_external_field_nullability(
            facts,
            &fact.field.type_name,
            &fact.field.field_name,
        ),
    });
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

pub fn external_inherited_static_final_fields_for_type(
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
    if !external_field_fact_matches_super_type(&fact.field.type_name, super_name, fact.provenance) {
        return;
    }
    if !seen_fields.insert(fact.field.field_name.clone()) {
        return;
    }
    let Some(field_type) = fact.field_type.clone() else {
        return;
    };
    fields.push(ExternalInheritedStaticField {
        declaring_type: fact.field.type_name.clone(),
        field_name: fact.field.field_name.clone(),
        field_type,
    });
}

pub fn external_inherited_instance_methods_for_type(
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
    let method = ExternalInheritedInstanceMethod {
        declaring_type: receiver_type.to_string(),
        method_name: fact.method_name.clone(),
        rust_name: rust_name.clone(),
        arg_count,
        fact: fact.clone(),
    };
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

pub fn inherited_instance_field_refs_for_type<'a>(
    ty: &TypeDecl,
    support_program: &'a Program,
) -> Vec<InheritedInstanceFieldRef<'a>> {
    inherited_instance_field_refs_for_type_in_programs(ty, &[support_program])
}

pub fn inherited_instance_field_refs_for_type_in_programs<'a>(
    ty: &TypeDecl,
    support_programs: &[&'a Program],
) -> Vec<InheritedInstanceFieldRef<'a>> {
    let mut fields = Vec::new();
    let mut seen_fields = direct_instance_field_names(ty);
    let mut current = ty.extends.as_deref();
    let mut visited_types = HashSet::new();

    while let Some(type_name) = current {
        let Some(super_ty) = find_unique_type_decl_in_programs(support_programs, type_name) else {
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
                fields.push(InheritedInstanceFieldRef {
                    declaring_type: &super_ty.name,
                    field,
                });
            }
        }

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
    fn inherited_source_methods_refuse_ambiguous_simple_package_super() {
        let program = Program {
            package: None,
            imports: Vec::new(),
            types: vec![
                class("a.Base", None, vec![method("v", Vec::new(), "v")]),
                class("b.Base", None, vec![method("v", Vec::new(), "v")]),
                class("AmbiguousChild", Some("Base"), Vec::new()),
                class("ExactChild", Some("b.Base"), Vec::new()),
            ],
        };
        let ambiguous_child = find_type_decl(&program, "AmbiguousChild").unwrap();
        let exact_child = find_type_decl(&program, "ExactChild").unwrap();

        assert!(
            inherited_instance_method_refs_for_type(ambiguous_child, &program).is_empty(),
            "bare same-simple package super lookup must fail closed"
        );
        let inherited = inherited_instance_method_refs_for_type(exact_child, &program);
        assert_eq!(inherited.len(), 1);
        assert_eq!(inherited[0].declaring_type, "b.Base");
        assert_eq!(inherited[0].method.name, "v");
    }

    #[test]
    fn external_inherited_instance_methods_prefer_typed_builtin_facts() {
        let ty = class("GenomicsLikeTool", Some("GATKTool"), Vec::new());

        let inherited = external_inherited_instance_methods_for_type(&ty, &FactSet::default());
        let method = inherited
            .iter()
            .find(|method| method.rust_name == "get_default_tool_v_c_f_header_lines")
            .expect("GATKTool builtin method should be inherited");

        assert_eq!(method.arg_count, 0);
        assert_eq!(
            method.fact.return_type,
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
