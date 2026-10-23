#[path = "common/mod.rs"]
mod common;

#[test]
fn enum_constants_are_not_confused_with_self_typed_fields_or_static_aliases() {
    common::verify_java_fixture_zero_contract(
        "enum_declaration_roles",
        "EnumDeclarationProbe",
        "tests/corpus/numbers/EnumDeclarationProbe.java",
        include_str!("corpus/numbers/EnumDeclarationProbe.java"),
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn roles() { assert_eq!(EnumDeclarationProbe::run().value(), 0); }
"#,
    );
}

#[test]
fn enum_payloads_preserve_construction_effects_alias_identity_and_mutable_state() {
    common::verify_java_fixture_zero_contract(
        "enum_state",
        "EnumStateProbe",
        "tests/corpus/numbers/EnumStateProbe.java",
        include_str!("corpus/numbers/EnumStateProbe.java"),
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn state() { assert_eq!(EnumStateProbe::run().value(), 0); }
"#,
    );
}

#[test]
fn enum_constructors_initialize_once_delegate_and_publish_in_order() {
    common::verify_java_fixture_zero_contract(
        "enum_construction",
        "EnumConstructionProbe",
        "tests/corpus/numbers/EnumConstructionProbe.java",
        include_str!("corpus/numbers/EnumConstructionProbe.java"),
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn construction() { assert_eq!(EnumConstructionProbe::run().value(), 0); }
"#,
    );
}

#[test]
fn source_sam_references_use_typed_overloads_and_retained_array_contracts() {
    common::verify_java_fixture_zero_contract(
        "source_sam_references",
        "SourceSamReferenceProbe",
        "tests/corpus/numbers/SourceSamReferenceProbe.java",
        include_str!("corpus/numbers/SourceSamReferenceProbe.java"),
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn references() { assert_eq!(SourceSamReferenceProbe::run().value(), 0); }
"#,
    );
}
