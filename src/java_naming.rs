pub fn java_getter_method_name(name: &str) -> bool {
    java_getter_property_suffix(name).is_some()
}

pub fn java_getter_field_name(name: &str) -> Option<String> {
    java_getter_property_suffix(name).map(crate::parser::to_snake_case)
}

pub fn java_method_name_aliases(name: &str) -> Vec<String> {
    let mut aliases = vec![name.to_string()];
    let snake = crate::parser::to_snake_case(name);
    push_unique_alias(&mut aliases, snake.clone());
    if let Some(java_spelling) = snake_method_name_to_java_spelling(&snake) {
        push_unique_alias(&mut aliases, java_spelling);
    }
    aliases
}

pub fn java_accessor_method_reference_fallback_name(name: &str) -> bool {
    name == "get" || name.starts_with("get_")
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
    fn java_accessor_method_reference_fallback_names_are_explicit() {
        assert!(java_accessor_method_reference_fallback_name("get"));
        assert!(java_accessor_method_reference_fallback_name("get_name"));
        assert!(!java_accessor_method_reference_fallback_name("getName"));
        assert!(!java_accessor_method_reference_fallback_name("is_active"));
    }
}
