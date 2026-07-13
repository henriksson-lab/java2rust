pub fn is_known_jdk_shadow_sensitive_owner(name: &str) -> bool {
    matches!(name, "Objects" | "Collections" | "Integer" | "Paths")
}

pub fn is_content_read_method(name: &str) -> bool {
    matches!(name, "length" | "isEmpty" | "trim")
}

pub fn is_borrowed_string_view_method(name: &str, arg_count: usize) -> bool {
    name == "trim" && arg_count == 0
}
