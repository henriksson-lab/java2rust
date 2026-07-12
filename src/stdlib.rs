pub fn is_known_jdk_shadow_sensitive_owner(name: &str) -> bool {
    matches!(name, "Objects" | "Collections" | "Integer" | "Paths")
}
