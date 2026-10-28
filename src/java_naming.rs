use std::collections::{HashMap, HashSet};

use crate::java_ir::{Expr, FieldDecl, Member, MethodDecl, Modifier, Program, Stmt, TypeDecl};

/// Checked implementation of the compiler-provided source enum lookup method.
/// Resolver identity and exception propagation must refer to the same body.
pub(crate) const ENUM_VALUE_OF_RESULT_METHOD: &str = "__java_value_of_result";

pub(crate) fn outer_handle_companion_name(method: &str) -> String {
    format!("{method}_with_outer")
}

/// All runtime class-token producers share original binary source identity.
pub(crate) fn source_binary_class_name(ty: &TypeDecl, package: Option<&str>) -> String {
    if let Some(names) = &ty.source_names {
        return names.binary.clone();
    }
    if ty.name.contains('.') {
        return ty.name.clone();
    }
    package
        .filter(|package| !package.is_empty())
        .map(|package| format!("{package}.{}", ty.name))
        .unwrap_or_else(|| ty.name.clone())
}

/// Capture named-declaration runtime identity before qualification, merging or
/// Rust-name collision repair. Java canonical and binary nesting are distinct;
/// a dollar sign inside a source identifier must not be mistaken for nesting.
pub(crate) fn record_source_type_names(package: Option<&str>, types: &mut [TypeDecl]) {
    fn record(ty: &mut TypeDecl, canonical_owner: &str, binary_owner: &str, nested: bool) {
        let canonical = if canonical_owner.is_empty() {
            ty.name.clone()
        } else {
            format!("{canonical_owner}.{}", ty.name)
        };
        let binary = if binary_owner.is_empty() {
            ty.name.clone()
        } else {
            format!(
                "{binary_owner}{}{}",
                if nested { "$" } else { "." },
                ty.name
            )
        };
        let names = ty
            .source_names
            .get_or_insert(crate::java_ir::SourceTypeNames { canonical, binary });
        let canonical = names.canonical.clone();
        let binary = names.binary.clone();
        for member in &mut ty.members {
            if let Member::Type(child) = member {
                record(child, &canonical, &binary, true);
            }
        }
    }
    let package = package.unwrap_or_default();
    for ty in types {
        record(ty, package, package, false);
    }
}

/// Reconcile relative and already-qualified declarations without duplicating
/// the lexical owner. Shared by emission inventories and declaration lookup.
pub(crate) fn source_type_name_in_owner(name: &str, enclosing: Option<&str>) -> String {
    match enclosing {
        Some(owner) if !name.starts_with(&format!("{owner}.")) => format!("{owner}.{name}"),
        _ => name.to_string(),
    }
}

/// Recover a declaration's lexical path by identity, not by guessing which of
/// several same-leaf nested types a summary intended.
pub(crate) fn source_type_declaration_path(program: &Program, target: &TypeDecl) -> Option<String> {
    fn visit(ty: &TypeDecl, target: &TypeDecl, enclosing: Option<&str>) -> Option<String> {
        let name = source_type_name_in_owner(&ty.name, enclosing);
        if std::ptr::eq(ty, target) {
            return Some(name);
        }
        ty.members.iter().find_map(|member| match member {
            Member::Type(nested) => visit(nested, target, Some(&name)),
            _ => None,
        })
    }
    program.types.iter().find_map(|ty| visit(ty, target, None))
}

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
        if name == "Result" {
            return "JavaResult".to_string();
        }
        if name.contains('$') {
            let mut out = String::with_capacity(name.len());
            push_upper_camel_identifier_part(&mut out, name);
            return out;
        }
        return name.to_string();
    }
    // Dots and identifier separators disappear (or become a single `_`), so
    // the source length is a tight lower bound for almost every output.  More
    // importantly, append each part directly: this helper is used in the
    // innermost external-facade matching loops and allocating a temporary
    // `String` for every package segment made qualified names surprisingly
    // expensive on large source contexts.
    let mut out = String::with_capacity(name.len());
    let mut previous_part_was_type = false;
    for part in name.split('.').filter(|part| !part.trim().is_empty()) {
        let part_was_type = java_name_part_looks_like_type(part);
        if previous_part_was_type && part_was_type {
            out.push('_');
        }
        push_upper_camel_identifier_part(&mut out, part);
        previous_part_was_type = part_was_type;
    }
    out
}

fn push_upper_camel_identifier_part(out: &mut String, part: &str) {
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
}

pub(crate) fn java_name_part_looks_like_type(part: &str) -> bool {
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

/// Enum fields are projected through methods, so their getters share Rust's
/// method namespace even though Java fields and methods have separate names.
/// Reserve other field spellings as well, to keep collision repair stable.
pub(crate) fn source_enum_field_accessor_name(ty: &TypeDecl, field_name: &str) -> String {
    let base = crate::parser::to_snake_case(
        source_instance_field_decl(ty, field_name)
            .map(|field| field.name.as_str())
            .unwrap_or(field_name),
    );
    let mut occupied = [
        "name",
        "ordinal",
        "values",
        "value_of",
        ENUM_VALUE_OF_RESULT_METHOD,
        "to_string",
        "get_class",
        "__java_enum_constant",
        "__java_publish",
        "__java_payloads",
        "__java_values_ready",
        "__java_class_init",
    ]
    .into_iter()
    .map(str::to_string)
    .collect::<HashSet<_>>();
    occupied.extend(ty.members.iter().filter_map(|member| match member {
        Member::Method(method) => {
            let name = crate::java_signature::source_method_rust_name(method);
            // An exact field-only getter can be the synthetic accessor itself.
            // Keep its public Java spelling and its owned publication result;
            // a second forwarding method could borrow from a temporary token.
            let reuses_field = name == base
                && method.params.is_empty()
                && method.type_params.is_empty()
                && !method.modifiers.contains(&Modifier::Static)
                && !method.modifiers.contains(&Modifier::Synchronized)
                && trivial_returned_instance_field_name(method).as_deref() == Some(base.as_str())
                && source_instance_field_decl(ty, field_name)
                    .is_some_and(|field| field.ty == method.return_type);
            (!reuses_field).then_some(name)
        }
        _ => None,
    }));
    if !occupied.contains(&base) && !rust_keyword_needs_rename(&base) {
        return base;
    }
    occupied.extend(ty.members.iter().filter_map(|member| match member {
        Member::Field(field) if !field.modifiers.contains(&Modifier::Static) => {
            Some(crate::parser::to_snake_case(&field.name))
        }
        _ => None,
    }));
    allocate_rust_ident(&base, &occupied)
}

pub(crate) fn trivial_returned_instance_field_name(method: &MethodDecl) -> Option<String> {
    let body = method.body.as_ref()?;
    let [Stmt::Return(Some(expr))] = body.statements.as_slice() else {
        return None;
    };
    match expr {
        Expr::Var(name) => Some(crate::parser::to_snake_case(
            name.strip_prefix("this.").unwrap_or(name),
        )),
        Expr::FieldAccess(receiver, field)
            if matches!(receiver.as_ref(), Expr::This)
                || matches!(receiver.as_ref(), Expr::Var(name) if name == "this") =>
        {
            Some(crate::parser::to_snake_case(field))
        }
        _ => None,
    }
}

pub(crate) fn source_instance_field_rust_name(ty: &TypeDecl, field_name: &str) -> String {
    let field_name = source_instance_field_decl(ty, field_name)
        .map(|field| field.name.as_str())
        .unwrap_or(field_name);
    source_instance_field_rust_names(ty)
        .remove(field_name)
        .unwrap_or_else(|| allocate_rust_ident(field_name, &HashSet::new()))
}

pub(crate) fn source_static_field_rust_names(ty: &TypeDecl) -> HashMap<String, String> {
    source_field_rust_names(ty, |field| field.modifiers.contains(&Modifier::Static))
}

/// Resolve a static source field by its Java spelling or by the parser's
/// canonical alias.  Declaration emission and every reference-lowering path
/// must go through the same declaration before allocating a Rust identifier;
/// independently snake-casing a reference (for example `INST`) can otherwise
/// disagree with the spelling emitted for its storage accessor.
///
/// An alias is accepted only when it identifies exactly one declaration.  An
/// exact Java spelling always wins, while ambiguous canonical aliases remain
/// unresolved rather than depending on declaration order.
pub(crate) fn source_static_field_decl<'a>(ty: &'a TypeDecl, query: &str) -> Option<&'a FieldDecl> {
    source_field_decl(ty, query, true)
}

pub(crate) fn source_instance_field_decl<'a>(
    ty: &'a TypeDecl,
    query: &str,
) -> Option<&'a FieldDecl> {
    source_field_decl(ty, query, false)
}

fn source_field_decl<'a>(ty: &'a TypeDecl, query: &str, is_static: bool) -> Option<&'a FieldDecl> {
    let fields = || {
        ty.members.iter().filter_map(|member| match member {
            Member::Field(field) if field.modifiers.contains(&Modifier::Static) == is_static => {
                Some(field)
            }
            _ => None,
        })
    };
    if let Some(field) = fields().find(|field| field.name == query) {
        return Some(field);
    }

    let canonical = crate::parser::to_snake_case(query);
    let mut matches =
        fields().filter(|field| crate::parser::to_snake_case(&field.name) == canonical);
    let field = matches.next()?;
    matches.next().is_none().then_some(field)
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
    let field_name = source_static_field_decl(ty, field_name)
        .map(|field| field.name.as_str())
        .unwrap_or(field_name);
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

fn push_unique_alias(aliases: &mut Vec<String>, alias: String) {
    if !aliases.iter().any(|existing| existing == &alias) {
        aliases.push(alias);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parsed_runtime_names_preserve_packages_nesting_and_literal_dollars() {
        let parsed = crate::parser::parse_java_source(
            "package left.side; class Outer { static class Mid { enum Choice { ONE } } } class Cash$Value {}",
        );
        let outer = &parsed.program.types[0];
        let Member::Type(mid) = &outer.members[0] else {
            panic!("nested class");
        };
        let Member::Type(choice) = &mid.members[0] else {
            panic!("nested enum");
        };
        let names = choice.source_names.as_ref().unwrap();
        assert_eq!(names.canonical, "left.side.Outer.Mid.Choice");
        assert_eq!(names.binary, "left.side.Outer$Mid$Choice");
        assert_eq!(
            parsed.program.types[1]
                .source_names
                .as_ref()
                .unwrap()
                .binary,
            "left.side.Cash$Value"
        );
        let mut rewritten = parsed.program.clone();
        rewritten.types[0].name = "synthetic.JavaType.Outer".into();
        assert_eq!(rewritten.types[0].source_names, outer.source_names);
    }

    #[test]
    fn declaration_paths_distinguish_same_leaf_nested_summary_types() {
        let parsed = crate::parser::parse_java_source(
            "class First { enum Part { ONE } } class Second { enum Part { TWO } }",
        );
        for (index, expected) in [(0, "First.Part"), (1, "Second.Part")] {
            let nested = parsed.program.types[index]
                .members
                .iter()
                .find_map(|member| match member {
                    Member::Type(nested) => Some(nested),
                    _ => None,
                })
                .unwrap();
            assert_eq!(
                source_type_declaration_path(&parsed.program, nested).as_deref(),
                Some(expected)
            );
            assert!(source_type_declaration_path(&parsed.program, &nested.clone()).is_none());
        }
    }
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
            source_names: None,
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
            rust_name_for_java_named_type("Literal$Dollar"),
            "LiteralDollar"
        );
        assert_eq!(
            rust_name_for_java_named_type("K.Literal$Dollar"),
            "K_LiteralDollar"
        );
        assert_eq!(
            rust_name_for_java_named_type("java.io.InputStream"),
            "JavaIoInputStream"
        );
    }

    #[test]
    fn source_type_named_result_does_not_shadow_std_result() {
        assert_eq!(rust_name_for_java_named_type("Result"), "JavaResult");
        assert_eq!(rust_name_for_java_named_type("a.Result"), "AResult");
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

    #[test]
    fn static_reference_alias_reuses_the_declared_field_name_mapping() {
        let ty = test_type(vec![test_field("INST", vec![Modifier::Static])]);

        let declaration = source_static_field_decl(&ty, "i_n_s_t").expect("canonical alias");
        assert_eq!(declaration.name, "INST");
        assert_eq!(source_static_field_rust_name(&ty, "INST"), "INST");
        assert_eq!(source_static_field_rust_name(&ty, "i_n_s_t"), "INST");
    }

    #[test]
    fn ambiguous_static_reference_alias_does_not_depend_on_declaration_order() {
        let ty = test_type(vec![
            test_field("FOO", vec![Modifier::Static]),
            test_field("fOO", vec![Modifier::Static]),
        ]);

        assert!(source_static_field_decl(&ty, "f_o_o").is_none());
        assert_eq!(source_static_field_rust_name(&ty, "f_o_o"), "f_o_o");
        assert_eq!(source_static_field_rust_name(&ty, "FOO"), "FOO");
        assert_eq!(source_static_field_rust_name(&ty, "fOO"), "fOO");
    }
}
