#[path = "common/mod.rs"]
mod common;

#[test]
fn enum_constructor_arguments_keep_required_and_nullable_slots() {
    common::verify_java_fixture_zero_contract(
        "enum_constructor_argument_slots",
        "EnumConstructorArgumentProbe",
        "tests/corpus/numbers/EnumConstructorArgumentProbe.java",
        include_str!("corpus/numbers/EnumConstructorArgumentProbe.java"),
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn slots() { assert_eq!(EnumConstructorArgumentProbe::run().value(), 0); }
"#,
    );
}

#[test]
fn required_foreign_values_use_mapped_java_text_without_consuming_receivers() {
    let source = include_str!("corpus/numbers/RequiredForeignTextProbe.java");
    common::verify_java_sources_zero_contract(
        "RequiredForeignTextProbe",
        &[("tests/corpus/numbers/RequiredForeignTextProbe.java", source)],
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
        "required_foreign_text",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn text() { assert_eq!(RequiredForeignTextProbe::run().value(), 0); }
"#,
        }],
        "test",
        "2021",
        &[("java-math-runtime", runtime.as_path())],
    );
}

#[test]
fn required_foreign_text_uses_declared_method_and_utf16_abi_not_rust_display() {
    let pack = new_java2rust::external_packs::parse_external_pack_manifest(
        "external-pack fixture.print fixture.Printable rust-crate-mapping mapped target=crate::native_runtime string-abi=utf16\n\
         external-pack-constructor fixture.print fixture.Printable 0 mapped rust-type=Printable rust=new\n\
         external-pack-api fixture.print fixture.Printable toString mapped getter read-only definitely-non-null args=0 return=String rust=render_units\n",
    );
    assert!(!pack.diagnostics().has_errors(), "{:?}", pack.diagnostics());
    let translation = new_java2rust::translate_source_with_facts(
        r#"
import fixture.Printable;
class Probe {
    static int run() {
        Printable value = new Printable();
        String result = "p=" + value;
        if (result.length() != 3 || result.charAt(2) != '\ud800') return 1;
        if (!new StringBuilder().append(value).toString().equals("\ud800")) return 2;
        return 0;
    }
}
"#,
        pack.facts(),
    );
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "required_foreign_declared_text",
        vec![
            common::GeneratedCrateModule {
                name: "native_runtime",
                rust: r#"
#[derive(Clone)] pub struct Printable;
impl Printable {
    pub fn new() -> Self { Self }
}
pub fn render_units(_: Printable) -> Vec<u16> { vec![0xd800] }
"#
                .into(),
                tests: "",
            },
            common::GeneratedCrateModule {
                name: "fixture",
                rust: translation.rust,
                tests: r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn text() { assert_eq!(Probe::run().value(), 0); }
"#,
            },
        ],
    );
}

#[test]
fn static_lambda_calls_select_source_overloads_from_native_sam_results() {
    let operations = include_str!("corpus/numbers/samlookup/Overloads.java");
    let probe = include_str!("corpus/numbers/samlookup/StaticLambdaOverloadProbe.java");
    common::verify_java_sources_zero_contract(
        "samlookup.StaticLambdaOverloadProbe",
        &[
            ("tests/corpus/numbers/samlookup/Overloads.java", operations),
            (
                "tests/corpus/numbers/samlookup/StaticLambdaOverloadProbe.java",
                probe,
            ),
        ],
    );
    common::cargo_run_translated_batch_sources(
        "static_lambda_overload_contract",
        &[
            new_java2rust::BatchSource::new("samlookup/Overloads.java", operations),
            new_java2rust::BatchSource::new("samlookup/StaticLambdaOverloadProbe.java", probe),
        ],
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
fn main() { assert_eq!(translated::StaticLambdaOverloadProbe::run().value(), 0); }
"#,
    );
}
