pub fn java_getter_method_name(name: &str) -> bool {
    java_getter_property_suffix(name).is_some()
}

pub fn java_getter_field_name(name: &str) -> Option<String> {
    java_getter_property_suffix(name).map(crate::parser::to_snake_case)
}

fn java_getter_property_suffix(name: &str) -> Option<&str> {
    name.strip_prefix("get")
        .filter(|rest| !rest.is_empty())
        .or_else(|| name.strip_prefix("is").filter(|rest| !rest.is_empty()))
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
}
