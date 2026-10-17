use new_java2rust::translate_source;

#[path = "common/mod.rs"]
mod common;

#[test]
fn anonymous_to_string_overrides_and_inheritance_format_through_the_carrier() {
    let translation = translate_source(
        r#"
class Named { public String toString() { return "base"; } }
class Plain { }
class Use {
    static Named inherited() { return new Named() {}; }
    static Named overridden() {
        return new Named() { public String toString() { return "child"; } };
    }
    static Plain objectOverride() {
        return new Plain() { public String toString() { return "object-child"; } };
    }
    static String run() {
        return "" + inherited() + ":" + overridden() + ":" + objectOverride();
    }
}
"#,
    );
    common::cargo_test_generated_modules(
        "anonymous_object_display",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "#[test] fn formatting_uses_the_virtual_java_override() { assert_eq!(Use::run(), \"base:child:object-child\"); }",
        }],
    );
}
