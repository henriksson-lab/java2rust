use new_java2rust::translate_source;

#[path = "common/mod.rs"]
mod common;

#[test]
fn class_parameter_subtyping_is_available_in_constructor_field_assignments() {
    let translation = translate_source(
        r#"
class Owner<A, B extends A> {
    A value;
    Owner(B value) { this.value = value; }
    A get() { return value; }
}
"#,
    );
    common::cargo_check_generated_modules(
        "class_type_parameter_subtyping",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn dependent_widening_preserves_both_null_and_checked_exception_carriers() {
    let translation = translate_source(
        r#"
class Use {
    static <T> T checked(T value, boolean fail) throws Exception {
        if (fail) throw new Exception();
        boolean missing = value == null;
        return value;
    }
    static <A, B extends A> A copy(B value, boolean fail) throws Exception {
        boolean missing = value == null;
        return checked(value, fail);
    }
}
"#,
    );
    common::cargo_test_generated_modules(
        "fallible_nullable_type_parameter_subtyping",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
#[test]
fn null_and_exception_states_are_preserved() {
    assert_eq!(Use::copy::<String, String>(None, false).unwrap(), None);
    assert_eq!(Use::copy::<String, String>(Some("ok".to_string()), false).unwrap(), Some("ok".to_string()));
    assert!(Use::copy::<String, String>(Some("ok".to_string()), true).is_err());
}
"#,
        }],
    );
}

#[test]
fn dependent_parameter_widening_preserves_nullable_values() {
    let translation = translate_source(
        r#"
class Use {
    static <A, B extends A> A widen(B value) {
        boolean missing = value == null;
        return value;
    }
}
"#,
    );
    common::cargo_test_generated_modules(
        "nullable_type_parameter_subtyping",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "#[test] fn nullable_widening_executes() { assert_eq!(Use::widen::<String, String>(None), None); assert_eq!(Use::widen::<String, String>(Some(\"ok\".to_string())), Some(\"ok\".to_string())); }",
        }],
    );
}

#[test]
fn method_type_parameter_can_extend_another_method_type_parameter() {
    let translation = translate_source(
        r#"
class Use {
    static <A, B extends A> A widen(B value) { return value; }
}
"#,
    );
    common::cargo_test_generated_modules(
        "method_type_parameter_subtyping",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "#[test] fn method_widening_executes() { assert_eq!(Use::widen::<String, String>(\"ok\".to_string()), \"ok\"); }",
        }],
    );
}

#[test]
fn method_type_parameter_can_extend_an_enclosing_class_type_parameter() {
    let translation = translate_source(
        r#"
class Owner<A> {
    <B extends A> A widen(B value) { return value; }
}
"#,
    );
    common::cargo_test_generated_modules(
        "owner_type_parameter_subtyping",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "#[test] fn owner_widening_executes() { assert_eq!(Owner::<String>::new().widen::<String>(\"ok\".to_string()), \"ok\"); }",
        }],
    );
}

#[test]
fn transitive_method_parameter_bounds_preserve_widening_capabilities() {
    let translation = translate_source(
        "class Use { static <A, B extends A, C extends B> A widen(C value) { return value; } }",
    );
    common::cargo_test_generated_modules(
        "transitive_type_parameter_subtyping",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "#[test] fn transitive_widening_executes() { assert_eq!(Use::widen::<String, String, String>(\"ok\".to_string()), \"ok\"); }",
        }],
    );
}

#[test]
fn type_parameter_bound_shadows_a_source_class_with_the_same_name() {
    let translation = translate_source(
        "class A {} class Use { static <A, B extends A> A widen(B value) { return value; } }",
    );
    assert!(
        !translation.rust.contains("pub trait ABound"),
        "{}",
        translation.rust
    );
    common::cargo_test_generated_modules(
        "shadowed_type_parameter_bound",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "#[test] fn lexical_scope_wins() { assert_eq!(Use::widen::<String, String>(\"ok\".to_string()), \"ok\"); }",
        }],
    );
}

#[test]
fn dependent_bounds_preserve_the_upper_parameters_interface_capabilities() {
    let translation = translate_source(
        r#"
interface Label { int code(); }
class Value implements Label { public int code() { return 21; } }
class Owner<A extends Label> {
    <B extends A> int inspect(B value) { return value.code(); }
}
class Use {
    static int run() { return new Owner<Value>().inspect(new Value()); }
}
"#,
    );
    common::cargo_test_generated_modules(
        "dependent_interface_capability",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "#[test] fn inherited_capability_executes() { assert_eq!(Use::run(), 21); }",
        }],
    );
}
