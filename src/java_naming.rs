use std::collections::{HashMap, HashSet};

use crate::java_ir::{FieldDecl, Member, Modifier, TypeDecl};

pub(crate) fn java_getter_method_name(name: &str) -> bool {
    java_getter_property_suffix(name).is_some()
}

pub(crate) fn java_getter_field_name(name: &str) -> Option<String> {
    java_getter_property_suffix(name).map(crate::parser::to_snake_case)
}

pub(crate) fn java_method_name_aliases(name: &str) -> Vec<String> {
    let mut aliases = vec![name.to_string()];
    let snake = crate::parser::to_snake_case(name);
    push_unique_alias(&mut aliases, snake.clone());
    if let Some(java_spelling) = snake_method_name_to_java_spelling(&snake) {
        push_unique_alias(&mut aliases, java_spelling);
    }
    aliases
}

pub(crate) fn java_accessor_method_reference_fallback_name(name: &str) -> bool {
    name == "get" || name.starts_with("get_")
}

pub(crate) fn rust_name_for_java_named_type(name: &str) -> String {
    if !name.contains('.') {
        return name.to_string();
    }
    let mut out = String::new();
    let mut previous_part_was_type = false;
    for part in name.split('.').filter(|part| !part.trim().is_empty()) {
        let part_was_type = java_name_part_looks_like_type(part);
        if previous_part_was_type && part_was_type {
            out.push('_');
        }
        out.push_str(&upper_camel_identifier_part(part));
        previous_part_was_type = part_was_type;
    }
    out
}

fn java_name_part_looks_like_type(part: &str) -> bool {
    part.chars()
        .find(|ch| ch.is_ascii_alphanumeric())
        .is_some_and(|ch| ch.is_ascii_uppercase())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RustKeywordEscape {
    Raw,
    Rename,
}

pub(crate) fn rust_keyword_escape(name: &str) -> Option<RustKeywordEscape> {
    match name {
        "crate" | "self" | "Self" | "super" => Some(RustKeywordEscape::Rename),
        "as" | "async" | "await" | "break" | "const" | "continue" | "dyn" | "else" | "enum"
        | "extern" | "false" | "fn" | "for" | "if" | "impl" | "in" | "let" | "loop" | "match"
        | "mod" | "move" | "mut" | "pub" | "ref" | "return" | "static" | "struct" | "trait"
        | "true" | "type" | "unsafe" | "use" | "where" | "while" | "abstract" | "become"
        | "box" | "do" | "final" | "macro" | "override" | "priv" | "try" | "typeof" | "unsized"
        | "virtual" | "yield" => Some(RustKeywordEscape::Raw),
        _ => None,
    }
}

pub(crate) fn rust_keyword_is_rawable_identifier(name: &str) -> bool {
    rust_keyword_escape(name) == Some(RustKeywordEscape::Raw)
}

pub(crate) fn rust_keyword_needs_rename(name: &str) -> bool {
    rust_keyword_escape(name) == Some(RustKeywordEscape::Rename)
}

pub(crate) fn rename_non_rawable_rust_keyword(name: &str) -> String {
    debug_assert!(rust_keyword_needs_rename(name));
    format!("{name}__")
}

pub(crate) fn allocate_rust_ident(base: &str, occupied: &HashSet<String>) -> String {
    let mut candidate = if rust_keyword_needs_rename(base) {
        rename_non_rawable_rust_keyword(base)
    } else {
        base.to_string()
    };
    while occupied.contains(&candidate) {
        candidate.push('_');
    }
    candidate
}

pub(crate) fn source_binding_rust_names<'a>(
    source_names: impl IntoIterator<Item = &'a str>,
) -> HashMap<String, String> {
    let source_names = source_names.into_iter().collect::<Vec<_>>();
    let mut occupied = source_names
        .iter()
        .filter(|name| !rust_keyword_needs_rename(name))
        .map(|name| (*name).to_string())
        .collect::<HashSet<_>>();
    let mut names = HashMap::new();
    for source_name in source_names {
        let rust_name = if rust_keyword_needs_rename(source_name) {
            allocate_rust_ident(source_name, &occupied)
        } else {
            source_name.to_string()
        };
        occupied.insert(rust_name.clone());
        names.insert(source_name.to_string(), rust_name);
    }
    names
}

pub(crate) fn source_instance_field_rust_names(ty: &TypeDecl) -> HashMap<String, String> {
    source_field_rust_names(ty, |field| !field.modifiers.contains(&Modifier::Static))
}

pub(crate) fn source_instance_field_rust_name(ty: &TypeDecl, field_name: &str) -> String {
    source_instance_field_rust_names(ty)
        .remove(field_name)
        .unwrap_or_else(|| allocate_rust_ident(field_name, &HashSet::new()))
}

pub(crate) fn source_static_field_rust_names(ty: &TypeDecl) -> HashMap<String, String> {
    source_field_rust_names(ty, |field| field.modifiers.contains(&Modifier::Static))
}

fn source_field_rust_names(
    ty: &TypeDecl,
    include_field: impl Fn(&FieldDecl) -> bool,
) -> HashMap<String, String> {
    let fields = ty
        .members
        .iter()
        .filter_map(|member| match member {
            Member::Field(field) if include_field(field) => Some(field),
            _ => None,
        })
        .collect::<Vec<_>>();
    let mut occupied = fields
        .iter()
        .filter(|field| !rust_keyword_needs_rename(&field.name))
        .map(|field| field.name.clone())
        .collect::<HashSet<_>>();
    let mut names = HashMap::new();
    for field in fields {
        let rust_name = if rust_keyword_needs_rename(&field.name) {
            allocate_rust_ident(&field.name, &occupied)
        } else {
            field.name.clone()
        };
        occupied.insert(rust_name.clone());
        names.insert(field.name.clone(), rust_name);
    }
    names
}

pub(crate) fn source_static_field_rust_name(ty: &TypeDecl, field_name: &str) -> String {
    source_static_field_rust_names(ty)
        .remove(field_name)
        .unwrap_or_else(|| allocate_rust_ident(field_name, &HashSet::new()))
}

fn java_getter_property_suffix(name: &str) -> Option<&str> {
    name.strip_prefix("get")
        .filter(|rest| !rest.is_empty())
        .or_else(|| name.strip_prefix("is").filter(|rest| !rest.is_empty()))
}

fn snake_method_name_to_java_spelling(name: &str) -> Option<String> {
    match name {
        "non_null" => Some("nonNull".to_string()),
        "is_null" => Some("isNull".to_string()),
        "to_string" => Some("toString".to_string()),
        "value_of" => Some("valueOf".to_string()),
        "int_value" => Some("intValue".to_string()),
        _ if name.contains('_') => Some(lower_camel_from_snake(name)),
        _ => None,
    }
}

fn lower_camel_from_snake(name: &str) -> String {
    let mut out = String::new();
    let mut uppercase_next = false;
    for ch in name.chars() {
        if ch == '_' {
            uppercase_next = true;
        } else if uppercase_next {
            out.extend(ch.to_uppercase());
            uppercase_next = false;
        } else {
            out.push(ch);
        }
    }
    out
}

fn upper_camel_identifier_part(part: &str) -> String {
    let mut out = String::new();
    let mut uppercase_next = true;
    for ch in part.chars() {
        if ch == '_' || ch == '-' || ch == '$' {
            uppercase_next = true;
        } else if uppercase_next {
            out.push(ch.to_ascii_uppercase());
            uppercase_next = false;
        } else if ch.is_ascii_alphanumeric() {
            out.push(ch);
        }
    }
    out
}

fn push_unique_alias(aliases: &mut Vec<String>, alias: String) {
    if !aliases.iter().any(|existing| existing == &alias) {
        aliases.push(alias);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostics::Span;
    use crate::java_ir::{JavaType, PrimitiveType, TypeKind};

    fn test_field(name: &str, modifiers: Vec<Modifier>) -> Member {
        Member::Field(FieldDecl {
            name: name.to_string(),
            ty: JavaType::Primitive(PrimitiveType::Int),
            modifiers,
            initializer: None,
            span: Span::default(),
        })
    }

    fn test_type(members: Vec<Member>) -> TypeDecl {
        TypeDecl {
            kind: TypeKind::Class,
            name: "NameCollision".to_string(),
            type_params: Vec::new(),
            type_param_bounds: Vec::new(),
            modifiers: Vec::new(),
            extends: None,
            implements: Vec::new(),
            permits: Vec::new(),
            members,
            static_init_blocks: Vec::new(),
            span: Span::default(),
        }
    }

    #[test]
    fn java_getter_name_policy_maps_property_suffixes() {
        assert!(java_getter_method_name("getName"));
        assert!(java_getter_method_name("isActive"));
        assert!(!java_getter_method_name("get"));
        assert!(java_getter_method_name("issue"));
        assert_eq!(
            java_getter_field_name("getSampleName").as_deref(),
            Some("sample_name")
        );
        assert_eq!(
            java_getter_field_name("isActive").as_deref(),
            Some("active")
        );
        assert_eq!(java_getter_field_name("get"), None);
    }

    #[test]
    fn java_method_name_aliases_preserve_explicit_java_spellings() {
        assert_eq!(
            java_method_name_aliases("non_null"),
            vec!["non_null".to_string(), "nonNull".to_string()]
        );
        assert_eq!(
            java_method_name_aliases("get_name"),
            vec!["get_name".to_string(), "getName".to_string()]
        );
        assert_eq!(
            java_method_name_aliases("toString"),
            vec!["toString".to_string(), "to_string".to_string()]
        );
    }

    #[test]
    fn nested_type_name_mangling_preserves_type_path_boundaries() {
        assert_eq!(rust_name_for_java_named_type("K.AB"), "K_AB");
        assert_eq!(rust_name_for_java_named_type("K.A.B"), "K_A_B");
        assert_eq!(
            rust_name_for_java_named_type("java.io.InputStream"),
            "JavaIoInputStream"
        );
    }

    #[test]
    fn java_accessor_method_reference_fallback_names_are_explicit() {
        assert!(java_accessor_method_reference_fallback_name("get"));
        assert!(java_accessor_method_reference_fallback_name("get_name"));
        assert!(!java_accessor_method_reference_fallback_name("getName"));
        assert!(!java_accessor_method_reference_fallback_name("is_active"));
    }

    #[test]
    fn rust_keyword_policy_separates_rawable_from_path_keywords() {
        for name in ["type", "match", "loop", "box", "try", "yield"] {
            assert_eq!(rust_keyword_escape(name), Some(RustKeywordEscape::Raw));
        }
        for name in ["crate", "self", "Self", "super"] {
            assert_eq!(rust_keyword_escape(name), Some(RustKeywordEscape::Rename));
            assert_eq!(rename_non_rawable_rust_keyword(name), format!("{name}__"));
        }
        assert_eq!(rust_keyword_escape("type_"), None);
    }

    #[test]
    fn non_rawable_keyword_allocator_avoids_occupied_suffixes() {
        let occupied = ["self__".to_string()].into_iter().collect();
        assert_eq!(allocate_rust_ident("self", &occupied), "self___");
    }

    #[test]
    fn source_binding_allocator_preserves_occupied_user_suffixes() {
        let names = source_binding_rust_names(["self", "self__"]);
        assert_eq!(names.get("self").map(String::as_str), Some("self___"));
        assert_eq!(names.get("self__").map(String::as_str), Some("self__"));
    }

    #[test]
    fn source_field_allocators_share_keyword_collision_policy() {
        let ty = test_type(vec![
            test_field("self", Vec::new()),
            test_field("self__", Vec::new()),
            test_field("crate", vec![Modifier::Static]),
            test_field("crate__", vec![Modifier::Static]),
        ]);

        let instance_names = source_instance_field_rust_names(&ty);
        assert_eq!(
            instance_names.get("self").map(String::as_str),
            Some("self___")
        );
        assert_eq!(
            instance_names.get("self__").map(String::as_str),
            Some("self__")
        );

        let static_names = source_static_field_rust_names(&ty);
        assert_eq!(
            static_names.get("crate").map(String::as_str),
            Some("crate___")
        );
        assert_eq!(
            static_names.get("crate__").map(String::as_str),
            Some("crate__")
        );
    }

    #[test]
    fn source_single_field_accessors_share_keyword_collision_policy() {
        let ty = test_type(vec![
            test_field("self", Vec::new()),
            test_field("self__", Vec::new()),
            test_field("crate", vec![Modifier::Static]),
            test_field("crate__", vec![Modifier::Static]),
        ]);

        assert_eq!(source_instance_field_rust_name(&ty, "self"), "self___");
        assert_eq!(source_instance_field_rust_name(&ty, "self__"), "self__");
        assert_eq!(source_static_field_rust_name(&ty, "crate"), "crate___");
        assert_eq!(source_static_field_rust_name(&ty, "crate__"), "crate__");
    }
}
