#[path = "common/mod.rs"]
mod common;

#[test]
fn interface_append_reuses_checked_virtual_display_and_releases_owner_guards() {
    common::verify_java_fixture_zero_contract(
        "interface_append_text",
        "InterfaceAppendProbe",
        "tests/fixtures/InterfaceAppendProbe.java",
        include_str!("fixtures/InterfaceAppendProbe.java"),
        r#"
trait Exit { fn value(self) -> i32; }
impl Exit for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Exit for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn text() { assert_eq!(InterfaceAppendProbe::run().value(), 0); }
"#,
    );
}

#[test]
fn throwable_append_uses_runtime_carrier_and_lossless_logical_text() {
    common::verify_java_fixture_zero_contract(
        "throwable_append_text",
        "ThrowableAppendProbe",
        "tests/fixtures/ThrowableAppendProbe.java",
        include_str!("fixtures/ThrowableAppendProbe.java"),
        r#"
trait Exit { fn value(self) -> i32; }
impl Exit for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Exit for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn text() { assert_eq!(ThrowableAppendProbe::run().value(), 0); }
"#,
    );
}

#[test]
fn nullable_sam_messages_preserve_source_invocation_laziness_and_user_errors() {
    common::verify_java_fixture_zero_contract(
        "nullable_sam_null_messages",
        "NullableSamNullMessageProbe",
        "tests/corpus/numbers/NullableSamNullMessageProbe.java",
        include_str!("corpus/numbers/NullableSamNullMessageProbe.java"),
        r#"
trait Exit { fn value(self) -> i32; }
impl Exit for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Exit for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn messages() { assert_eq!(NullableSamNullMessageProbe::run().value(), 0); }
"#,
    );
}

#[test]
fn local_null_messages_follow_binding_scopes_wide_parameters_and_slot_reuse() {
    common::verify_java_fixture_zero_contract(
        "local_null_messages",
        "LocalNullMessageProbe",
        "tests/corpus/numbers/LocalNullMessageProbe.java",
        include_str!("corpus/numbers/LocalNullMessageProbe.java"),
        r#"
trait Exit { fn value(self) -> i32; }
impl Exit for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Exit for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn messages() { assert_eq!(LocalNullMessageProbe::run().value(), 0); }
"#,
    );
}

#[test]
fn native_null_messages_keep_explicit_text_order_and_java_descriptors() {
    let source = include_str!("corpus/numbers/NativeNullMessageProbe.java");
    common::verify_java_sources_zero_contract(
        "NativeNullMessageProbe",
        &[("tests/corpus/numbers/NativeNullMessageProbe.java", source)],
    );
    let pack = new_java2rust::external_packs::parse_external_pack_manifest(include_str!(
        "../runtime/java-math/manifests/big-integer.facts"
    ));
    let translation = new_java2rust::translate_source_with_facts(source, pack.facts());
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    let runtime = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("runtime/java-math");
    common::cargo_verify_generated_modules_with_dependencies(
        "native_null_messages",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
trait ExitCode { fn value(self) -> i32; }
impl ExitCode for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> ExitCode for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn messages() { assert_eq!(NativeNullMessageProbe::run().value(), 0); }
"#,
        }],
        "test",
        "2021",
        &[("java-math-runtime", runtime.as_path())],
    );
}

#[test]
fn exception_binary_identity_survives_inheritance_causes_and_suppression() {
    common::verify_java_fixture_zero_contract(
        "exception_binary_identity",
        "identity.ExceptionBinaryIdentityProbe",
        "tests/corpus/numbers/ExceptionBinaryIdentityProbe.java",
        include_str!("corpus/numbers/ExceptionBinaryIdentityProbe.java"),
        r#"
trait ExitCode { fn value(self) -> i32; }
impl ExitCode for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> ExitCode for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn identity() {
    assert_eq!(ExceptionBinaryIdentityProbe::run().value(), 0);
    // Native aliases and fully qualified names must reach the Object root too.
    let native = JavaException::new("IOException", "native");
    assert!(native.is_instance(&["Object"]));
    assert!(native.is_instance(&["java.lang.Object"]));
    assert!(!native.is_instance(&["other.IOException"]));
}
"#,
    );
}

#[test]
fn source_exception_constructor_initializes_cause_before_publication() {
    common::verify_java_fixture_zero_contract(
        "prepublication_exception_cause",
        "ExceptionCauseConstructorProbe",
        "tests/corpus/numbers/ExceptionCauseConstructorProbe.java",
        include_str!("corpus/numbers/ExceptionCauseConstructorProbe.java"),
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn causes() { assert_eq!(ExceptionCauseConstructorProbe::run().value(), 0); }
"#,
    );
}

#[test]
fn source_exception_bases_use_the_physical_runtime_carrier_with_and_without_foreign_packs() {
    let source = include_str!("corpus/numbers/SourceExceptionTextProbe.java");
    common::verify_java_sources_zero_contract(
        "SourceExceptionTextProbe",
        &[("tests/corpus/numbers/SourceExceptionTextProbe.java", source)],
    );
    let pack = new_java2rust::external_packs::parse_external_pack_manifest(include_str!(
        "../runtime/java-math/manifests/big-integer.facts"
    ));
    let empty = new_java2rust::facts::FactSet::default();
    let runtime = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("runtime/java-math");
    for (tag, facts) in [("native", &empty), ("mapped", pack.facts())] {
        let translation = new_java2rust::translate_source_with_facts(source, facts);
        assert!(
            !translation.diagnostics.has_errors(),
            "{:?}",
            translation.diagnostics
        );
        assert!(
            !translation.rust.contains("impl NumberFormatException {"),
            "{tag} carrier"
        );
        common::cargo_verify_generated_modules_with_dependencies(
            &format!("source_exception_carrier_{tag}"),
            vec![common::GeneratedCrateModule {
                name: "fixture",
                rust: translation.rust,
                tests: r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn messages() { assert_eq!(SourceExceptionTextProbe::run().value(), 0); }
"#,
            }],
            "test",
            "2021",
            &[("java-math-runtime", runtime.as_path())],
        );
    }
}

#[test]
fn exception_messages_remain_nullable_lossless_and_reference_preserving() {
    common::verify_java_fixture_zero_contract(
        "lossless_exception_messages",
        "ExceptionTextProbe",
        "tests/corpus/numbers/ExceptionTextProbe.java",
        include_str!("corpus/numbers/ExceptionTextProbe.java"),
        r#"
trait ExitCode { fn value(self) -> i32; }
impl ExitCode for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> ExitCode for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn messages() { assert_eq!(ExceptionTextProbe::run().value(), 0); }
"#,
    );
}
