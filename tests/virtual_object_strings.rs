#[path = "common/mod.rs"]
mod common;

#[test]
fn virtual_object_string_results_preserve_java_string_identity() {
    let translation =
        new_java2rust::translate_source(include_str!("fixtures/object_string_identity.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "object_string_identity",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
trait Probe { fn value(self) -> bool; }
impl Probe for bool { fn value(self) -> bool { self } }
impl<E: std::fmt::Debug> Probe for Result<bool,E> { fn value(self) -> bool { self.unwrap() } }
trait IntProbe { fn number(self) -> i32; }
impl IntProbe for i32 { fn number(self) -> i32 { self } }
impl<E: std::fmt::Debug> IntProbe for Result<i32,E> { fn number(self) -> i32 { self.unwrap() } }
#[test] fn preserves_source_result_reference() { assert!(ObjectStringIdentity::source_result().value()); }
#[test] fn string_to_string_returns_the_original_reference() { assert!(ObjectStringIdentity::native_result().value()); }
#[test] fn nullable_comparisons_evaluate_both_operands_in_order() {
    assert_eq!(ObjectStringIdentity::nullable_operand_order().number(), 12);
    assert_eq!(ObjectStringIdentity::reverse_nullable_operand_order().number(), 21);
}
#[test] fn nullable_reference_equality_is_not_structural_equality() {
    assert!(ObjectStringIdentity::two_nullable_strings().value());
    assert!(ObjectStringIdentity::snapshot_nullable_operands().value());
}
"#,
        }],
    );
}

#[test]
fn virtual_object_string_calls_preserve_nullable_results_and_exceptions() {
    let translation =
        new_java2rust::translate_source(include_str!("fixtures/virtual_object_strings.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "virtual_object_strings",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
trait ProbeBool { fn value(self) -> bool; }
impl ProbeBool for bool { fn value(self) -> bool { self } }
impl<E: std::fmt::Debug> ProbeBool for Result<bool, E> { fn value(self) -> bool { self.unwrap() } }
trait ProbeInt { fn value(self) -> i32; }
impl ProbeInt for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> ProbeInt for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
trait ProbeString { fn text(self) -> String; }
impl ProbeString for String { fn text(self) -> String { self } }
impl ProbeString for Option<String> { fn text(self) -> String { self.unwrap() } }
impl<T: ProbeString, E: std::fmt::Debug> ProbeString for Result<T, E> { fn text(self) -> String { self.unwrap().text() } }
#[test] fn nullable_result_is_not_the_string_null() { assert!(VirtualObjectStrings::nullable_result().value()); }
#[test] fn object_display_normalizes_null_result() { assert_eq!(VirtualObjectStrings::display_null_result(), "value=null"); }
#[test] fn virtual_exception_is_catchable() { assert_eq!(VirtualObjectStrings::checked_dispatch().value(), 7); }
#[test] fn object_parameter_keeps_nullable_result() { assert!(VirtualObjectStrings::object_parameter_null_result().value()); }
#[test] fn mutable_virtual_calls_preserve_object_identity() { assert_eq!(VirtualObjectStrings::mutable_dispatch().text(), "n1:n2:2"); }
#[test] fn typed_mutating_display_uses_the_same_reference_protocol() {
    assert_eq!(VirtualObjectStrings::typed_display().text(), "n1:n2:2");
    assert_eq!(VirtualObjectStrings::typed_parameter().text(), "typed=n1:1");
}
#[test] fn nullable_checked_result_preserves_both_domains() {
    assert!(VirtualObjectStrings::nullable_checked_result().value());
    assert_eq!(VirtualObjectStrings::checked_nullable_dispatch().value(), 11);
}
#[test] fn runtime_result_protocol_defers_null_display_normalization() {
    assert_eq!(None::<String>.into_java_erased_object_string_result().unwrap(), None);
    assert_eq!(None::<String>.into_java_erased_display_result().unwrap(), "null");
    assert_eq!(Ok::<_, JavaException>(None::<String>).into_java_erased_object_string_result().unwrap(), None);
    let error = Err::<Option<String>, _>(JavaException::new("IllegalStateException", "probe".to_string())).into_java_erased_display_result().unwrap_err();
    assert!(error.is_instance(&["IllegalStateException"]));
    struct NoEqualityKey;
    let map = JavaLinkedHashMap::<NoEqualityKey, i32>::new();
    assert_eq!(map.iter().count(), 0);
    assert!(map.is_empty());
}
#[test] fn generic_dispatch_preserves_results_effects_and_entry_loans() {
    assert_eq!(VirtualObjectStrings::generic_text().text(), "ok");
    assert_eq!(VirtualObjectStrings::static_text().text(), "generic");
    assert_eq!(VirtualObjectStrings::map_key().text(), "key");
    assert!(VirtualObjectStrings::generic_null_result().value());
    assert_eq!(VirtualObjectStrings::generic_caught().value(), 9);
}

"#,
        }],
    );
}

#[test]
fn generic_string_protocol_emits_support_without_explicit_object_types() {
    let translation = new_java2rust::translate_source(
        r#"
class OnlyGeneric<T> { String show(T value) { return value.toString(); } }
class OnlyGenericUse {
    static String run() {
        OnlyGeneric<String> renderer = new OnlyGeneric<String>();
        return renderer.show("only");
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
        "generic_string_support_closure",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
trait Probe { fn text(self) -> String; }
impl Probe for String { fn text(self) -> String { self } }
impl Probe for Option<String> { fn text(self) -> String { self.unwrap() } }
impl<T: Probe, E: std::fmt::Debug> Probe for Result<T, E> { fn text(self) -> String { self.unwrap().text() } }
#[test] fn executes() { assert_eq!(OnlyGenericUse::run().text(), "only"); }
"#,
        }],
    );
}
