#[path = "common/mod.rs"]
mod common;

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
