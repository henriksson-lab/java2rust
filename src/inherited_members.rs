use std::collections::HashSet;

use crate::java_ir::{FieldDecl, Member, Modifier, Program, TypeDecl};

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

pub fn inherited_instance_field_refs_for_type<'a>(
    ty: &TypeDecl,
    support_program: &'a Program,
) -> Vec<InheritedInstanceFieldRef<'a>> {
    let mut fields = Vec::new();
    let mut seen_fields = direct_instance_field_names(ty);
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
            let Member::Field(field) = member else {
                continue;
            };
            if is_static_field(field) {
                continue;
            }
            if seen_fields.insert(field.name.clone()) {
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

fn direct_instance_field_names(ty: &TypeDecl) -> HashSet<String> {
    ty.members
        .iter()
        .filter_map(|member| match member {
            Member::Field(field) if !is_static_field(field) => Some(field.name.clone()),
            _ => None,
        })
        .collect()
}

fn find_type_decl<'a>(program: &'a Program, type_name: &str) -> Option<&'a TypeDecl> {
    program
        .types
        .iter()
        .find_map(|ty| find_type_decl_in_type(ty, type_name))
}

fn find_type_decl_in_type<'a>(ty: &'a TypeDecl, type_name: &str) -> Option<&'a TypeDecl> {
    if type_name_matches(&ty.name, type_name) {
        return Some(ty);
    }
    ty.members.iter().find_map(|member| match member {
        Member::Type(nested) => find_type_decl_in_type(nested, type_name),
        _ => None,
    })
}

fn type_name_matches(candidate: &str, query: &str) -> bool {
    let candidate_aliases = type_name_aliases(candidate);
    let query_aliases = type_name_aliases(query);
    candidate_aliases
        .iter()
        .any(|candidate| query_aliases.iter().any(|query| query == candidate))
}

fn type_name_aliases(type_name: &str) -> Vec<String> {
    let without_args = type_name.split('<').next().unwrap_or(type_name).trim();
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
