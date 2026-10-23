#[path = "common/mod.rs"]
mod common;

#[test]
fn code_units_preserve_storage_casts_promotion_arrays_boxing_and_indexed_reads() {
    common::verify_java_fixture_zero_contract(
        "code_unit_storage_contracts",
        "CodeUnitStorageProbe",
        "tests/corpus/numbers/CodeUnitStorageProbe.java",
        include_str!("corpus/numbers/CodeUnitStorageProbe.java"),
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn units() { assert_eq!(CodeUnitStorageProbe::run().value(), 0); }
"#,
    );
}

#[test]
fn source_only_char_slots_emit_their_own_nominal_runtime_dependency() {
    let translated = new_java2rust::translate_source(
        "class UnitsOnly { char echo(char value) { return value; } static char[] zeros() { return new char[2]; } }",
    );
    assert!(
        !translated.diagnostics.has_errors(),
        "{:?}",
        translated.diagnostics
    );
    assert!(translated.rust.contains("JavaCodeUnit"));
    common::cargo_test_generated_modules(
        "code_unit_source_only",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translated.rust,
            tests: r#"
#[test] fn units() {
    assert_eq!(UnitsOnly::new().echo(JavaCodeUnit(0xdfff)).as_u16(), 0xdfff);
    assert_eq!(UnitsOnly::zeros(), vec![JavaCodeUnit(0), JavaCodeUnit(0)]);
}
"#,
        }],
    );
}
