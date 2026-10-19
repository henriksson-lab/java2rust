#[path = "common/mod.rs"]
mod common;

#[test]
fn facade_string_protocol_closes_dependencies_without_erased_object_runtime() {
    let (facts, diagnostics) = new_java2rust::external_facts::parse_external_facts(
        "external-api toString RemoteLabel getter read-only non-null args=0 return=String",
    )
    .into_parts();
    assert!(!diagnostics.has_errors(), "{diagnostics:?}");
    let sources = [
        (
            "class_scope",
            "class ProtocolOwner { public String toString() { return \"own\"; } static void accept(RemoteLabel value) {} }",
        ),
        (
            "interface_scope",
            "interface ProtocolOwner { default void accept(RemoteLabel value) {} }",
        ),
    ];
    let mut modules = Vec::new();
    for (name, source) in sources {
        let translation = new_java2rust::translate_source_with_facts(source, &facts);
        assert!(
            !translation.diagnostics.has_errors(),
            "{:?}",
            translation.diagnostics
        );
        assert!(
            translation
                .rust
                .contains("pub trait JavaErasedDisplayResult"),
            "{}",
            translation.rust
        );
        assert!(
            translation
                .rust
                .contains("java_erased_display_unwrap(self.to_string())"),
            "{}",
            translation.rust
        );
        assert!(
            !translation
                .rust
                .contains("pub fn java_erased_any_to_string("),
            "the scalar result protocol must not pull the complete Any runtime"
        );
        modules.push(common::GeneratedCrateModule {
            name,
            rust: translation.rust,
            tests: "",
        });
    }
    common::cargo_check_generated_modules("facade_string_protocol_dependencies", modules);
}

#[test]
fn native_and_virtual_string_contracts_keep_their_result_carriers() {
    let mut modules = Vec::new();
    for identity in [false, true] {
        let mut facts = new_java2rust::facts::FactSet::default();
        facts.java_string_identity_representation = identity;
        let translation = new_java2rust::translate_source_with_facts(
            include_str!("fixtures/object_string_contracts.java"),
            &facts,
        );
        assert!(
            !translation.diagnostics.has_errors(),
            "{:?}",
            translation.diagnostics
        );
        let identity_probe = if identity {
            r#"
impl Text for JavaString { fn text(self) -> String { self.as_str().to_string() } }
impl Text for Option<JavaString> { fn text(self) -> String { self.unwrap().as_str().to_string() } }
trait IdentityValue { fn reference(self) -> JavaString; }
impl IdentityValue for JavaString { fn reference(self) -> JavaString { self } }
impl IdentityValue for Option<JavaString> { fn reference(self) -> JavaString { self.unwrap() } }
impl<T: IdentityValue, E: std::fmt::Debug> IdentityValue for Result<T,E> { fn reference(self) -> JavaString { self.unwrap().reference() } }
#[test] fn nullable_source_field_keeps_the_original_string_reference() {
    let original = JavaString::fresh("cached");
    let value = NullableDescription::new(Some(original.clone()));
    assert!(value.to_string_boolean(false).reference().java_identity_eq(&original));
}
"#
        } else {
            ""
        };
        let tests = format!(
            r#"
trait Text {{ fn text(self) -> String; }}
impl Text for String {{ fn text(self) -> String {{ self }} }}
impl Text for Option<String> {{ fn text(self) -> String {{ self.unwrap() }} }}
impl<T: Text, E: std::fmt::Debug> Text for Result<T, E> {{ fn text(self) -> String {{ self.unwrap().text() }} }}
{identity_probe}
trait Boolean {{ fn value(self) -> bool; }}
impl Boolean for bool {{ fn value(self) -> bool {{ self }} }}
impl<E: std::fmt::Debug> Boolean for Result<bool, E> {{ fn value(self) -> bool {{ self.unwrap() }} }}
#[test] fn native_contracts_are_non_null() {{
    assert_eq!(ObjectStringContracts::native_builder(true).text(), "value");
    assert_eq!(ObjectStringContracts::native_builder(false).text(), "absent");
    assert_eq!(ObjectStringContracts::native_buffer().text(), "BUFFER");
    assert!(ObjectStringContracts::native_random().value());
}}
#[test] fn virtual_branch_keeps_one_nullable_carrier() {{
    assert!(ObjectStringContracts::nullable_branch().value());
    assert_eq!(ObjectStringContracts::present_branch().text(), "present");
    assert_eq!(ObjectStringContracts::absent_branch().text(), "missing");
}}
#[test] fn nullable_generic_result_can_cross_object_storage() {{
    assert!(ObjectStringContracts::boxed_null_result().value());
    assert!(ObjectStringContracts::boxed_present_result().value());
}}
"#
        );
        modules.push(common::GeneratedCrateModule {
            name: if identity { "identity" } else { "ordinary" },
            rust: translation.rust,
            tests: Box::leak(tests.into_boxed_str()),
        });
    }
    common::cargo_test_generated_modules("object_string_contracts", modules);
}
