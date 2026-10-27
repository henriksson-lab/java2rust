#[path = "common/mod.rs"]
mod common;
const CELL: &str = include_str!("fixtures/scoped_storage/Cell.java");
const UNRELATED: &str = include_str!("fixtures/scoped_storage/Unrelated.java");
const PROBE: &str = include_str!("fixtures/scoped_storage/ScopedStorageProbe.java");
#[test]
fn unrelated_generic_binder_cannot_disable_nominal_alias_storage() {
    common::verify_java_sources_zero_contract(
        "ScopedStorageProbe",
        &[
            ("tests/fixtures/scoped_storage/Cell.java", CELL),
            ("tests/fixtures/scoped_storage/Unrelated.java", UNRELATED),
            (
                "tests/fixtures/scoped_storage/ScopedStorageProbe.java",
                PROBE,
            ),
        ],
    );
    let translated = new_java2rust::translate_source(&format!("{CELL}\n{UNRELATED}\n{PROBE}"));
    assert!(
        !translated.diagnostics.has_errors(),
        "{:?}",
        translated.diagnostics
    );
    assert!(translated.rust.contains("__JavaTypeParam_"));
    common::cargo_test_generated_modules(
        "scoped_storage_identity",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translated.rust,
            tests: r#"
trait Exit { fn value(self) -> i32; }
impl Exit for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Exit for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn alias_and_generic_transport() { assert_eq!(ScopedStorageProbe::run().value(), 0); }
"#,
        }],
    );
}
#[test]
fn scoped_binders_agree_across_primary_units_and_production_support_assembly() {
    common::cargo_run_translated_batch_sources(
        "scoped_storage_identity_assembly",
        &[
            new_java2rust::BatchSource::new("Cell.java", CELL),
            new_java2rust::BatchSource::new("Unrelated.java", UNRELATED),
            new_java2rust::BatchSource::new("ScopedStorageProbe.java", PROBE),
        ],
        r#"
trait Exit { fn value(self) -> i32; }
impl Exit for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Exit for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
fn main() { assert_eq!(translated::ScopedStorageProbe::run().value(), 0); }
"#,
    );
}
