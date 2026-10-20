#[path = "common/mod.rs"]
mod common;

#[test]
fn nullable_list_probe_checks_signed_indices_and_preserves_exceptions() {
    let translation =
        new_java2rust::translate_source(include_str!("fixtures/nullable_list_probe.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "nullable_list_probe",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn java_null_probe_contract() { assert_eq!(NullableListProbe::run().value(), 215); }
"#,
        }],
    );
}

#[test]
fn source_list_get_is_not_a_native_null_probe() {
    let translation = new_java2rust::translate_source(
        r#"
class List<T> {
    T get(int index) {
        if (index < 0) throw new IllegalArgumentException("source method");
        return null;
    }
}
class SourceListProbe {
    static int run() {
        List<String> values = new List<>();
        int result = values.get(0) == null ? 1 : -100;
        try {
            if (values.get(-1) == null) result = -100;
        } catch (IllegalArgumentException expected) {
            result += 2;
        }
        return result;
    }
}
"#,
    );
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "source_list_null_probe",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn selected_source_method_contract() { assert_eq!(SourceListProbe::run().value(), 3); }
"#,
        }],
    );
}
