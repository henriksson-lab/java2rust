#[path = "common/mod.rs"]
mod common;

#[test]
fn fallible_payloads_survive_ownership_and_boolean_transforms() {
    let translation = new_java2rust::translate_source(
        r#"
class PayloadTransforms {
    static boolean checked(boolean fail) {
        if (fail) throw new IllegalArgumentException("failed");
        return true;
    }
    static boolean eager(boolean fail) { return false | checked(fail); }
    static boolean negated(boolean fail) { return !checked(fail); }
    static boolean shortCircuit(boolean left, boolean fail) { return left || checked(fail); }
    static <Value> Value checkedValue(Value value, boolean fail) {
        if (fail) throw new IllegalArgumentException("failed");
        return value;
    }
    static <Value> Value retained(Value value, boolean fail) {
        Value copied = checkedValue(value, fail);
        return copied;
    }
    static String text() { return retained("hello", false); }
}
"#,
    );
    common::cargo_test_generated_modules(
        "result_payload_transforms",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
#[test]
fn payload_transforms_preserve_success_errors_and_short_circuiting() {
    assert!(PayloadTransforms::eager(false).unwrap());
    assert!(PayloadTransforms::eager(true).is_err());
    assert!(!PayloadTransforms::negated(false).unwrap());
    assert!(PayloadTransforms::negated(true).is_err());
    assert!(PayloadTransforms::short_circuit(true, true).unwrap());
    assert!(PayloadTransforms::short_circuit(false, true).is_err());
    assert_eq!(PayloadTransforms::text().unwrap(), "hello");
}
"#,
        }],
    );
}
