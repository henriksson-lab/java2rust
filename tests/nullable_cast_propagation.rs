use new_java2rust::translate_source;

#[path = "common/mod.rs"]
mod common;

#[test]
fn checked_cast_caught_in_its_own_method_does_not_escape() {
    let translated = translate_source(
        r#"
class Use {
    static int run() {
        Object input = 17;
        try {
            String value = (String) input;
            return 0;
        } catch (ClassCastException error) { return 7; }
    }
}
"#,
    );
    common::cargo_test_generated_modules(
        "checked_cast_local_handler",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translated.rust,
            tests: "#[test] fn local_handler_keeps_plain_abi() { assert_eq!(Use::run(), 7); }",
        }],
    );
}

#[test]
fn checked_cast_failures_are_caught_after_null_and_present_values() {
    let translated = translate_source(
        r#"
class Use {
    static String cast(Object value) {
        boolean missing = value == null;
        return (String) value;
    }
    static int run() {
        int stage = 0;
        try {
            if (cast(null) == null) stage++;
            if (cast("ok") != null) stage++;
            cast(17);
        } catch (ClassCastException error) { return stage + 1; }
        return -1;
    }
}
"#,
    );
    common::cargo_test_generated_modules(
        "checked_cast_failure_propagation",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translated.rust,
            tests: "#[test] fn checked_cast_is_a_java_exception() { assert_eq!(Use::run().unwrap(), 3); }",
        }],
    );
}

#[test]
fn nullable_reference_casts_preserve_null_and_present_values() {
    let translated = translate_source(
        r#"
class Use {
    static String cast(Object value) {
        boolean missing = value == null;
        return (String) value;
    }
    static int run() {
        int stage = 0;
        if (cast(null) == null) stage++;
        if (cast("ok") != null) stage++;
        String literal = (String) null;
        if (literal == null) stage++;
        return stage;
    }
}
"#,
    );
    common::cargo_test_generated_modules(
        "nullable_cast_propagation",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translated.rust,
            tests: r#"
#[test]
fn reference_cast_copies_the_nullable_value_domain() {
    assert_eq!(Use::run().unwrap(), 3);
    assert_eq!(Use::cast(None).unwrap(), None);
    assert_eq!(Use::cast(Some(Box::new("ok".to_string()))).unwrap(), Some("ok".to_string()));
}
"#,
        }],
    );
}
