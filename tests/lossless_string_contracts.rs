#[path = "common/mod.rs"]
mod common;

#[test]
fn reflective_class_names_share_the_logical_string_result_boundary() {
    common::verify_java_fixture_zero_contract(
        "reflective_class_name_contracts",
        "ClassNameProbe",
        "tests/corpus/numbers/ClassNameProbe.java",
        include_str!("corpus/numbers/ClassNameProbe.java"),
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn names() { assert_eq!(ClassNameProbe::run().value(), 0); }
"#,
    );
}

#[test]
fn source_constant_reads_materialize_interned_references_across_files() {
    let sources = [
        (
            "textconstants/Constants.java",
            include_str!("corpus/numbers/stringconstants/Constants.java"),
        ),
        (
            "textconstants/ConstantProbe.java",
            include_str!("corpus/numbers/stringconstants/ConstantProbe.java"),
        ),
    ];
    let oracle_sources = [
        (
            "tests/corpus/numbers/stringconstants/Constants.java",
            sources[0].1,
        ),
        (
            "tests/corpus/numbers/stringconstants/ConstantProbe.java",
            sources[1].1,
        ),
    ];
    common::verify_java_sources_zero_contract("textconstants.ConstantProbe", &oracle_sources);
    let batch = sources
        .into_iter()
        .map(|(path, source)| new_java2rust::BatchSource::new(path, source))
        .collect::<Vec<_>>();
    common::cargo_run_translated_batch_sources(
        "source_string_constants",
        &batch,
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
fn main() { assert_eq!(translated::ConstantProbe::run().value(), 0); }
"#,
    );
}

#[test]
fn source_only_concat_uses_shared_lossless_append_values() {
    let source = include_str!("corpus/numbers/SourceOnlyConcatProbe.java");
    common::verify_java_sources_zero_contract(
        "SourceOnlyConcatOracle",
        &[
            ("tests/corpus/numbers/SourceOnlyConcatProbe.java", source),
            (
                "tests/corpus/numbers/SourceOnlyConcatOracle.java",
                include_str!("corpus/numbers/SourceOnlyConcatOracle.java"),
            ),
        ],
    );
    let translated = new_java2rust::translate_source(source);
    assert!(
        !translated.diagnostics.has_errors(),
        "{:?}",
        translated.diagnostics
    );
    assert!(
        !translated.rust.contains("pub struct StringBuilder"),
        "must exercise source-only support"
    );
    common::cargo_test_generated_modules(
        "source_only_lossless_concat",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translated.rust,
            tests: r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn concat() { assert_eq!(SourceOnlyConcatProbe::run().value(), 0); }
#[test] fn append_borrow_forwarding_accepts_unsized_scalar_and_unit_views() {
    let scalar: &str = "é😀";
    assert_eq!(JavaAppendValue::into_java_append_string(&scalar).units(), &[233, 0xd83d, 0xde00]);
    let units: &[JavaCodeUnit] = &[JavaCodeUnit::narrow_i32(0xd800)];
    assert_eq!(JavaAppendValue::into_java_append_string(&units).units(), &[0xd800]);
}
"#,
        }],
    );
}

#[test]
fn source_string_payloads_remain_lossless_across_views_substrings_and_append() {
    common::verify_java_fixture_zero_contract(
        "lossless_source_string_contracts",
        "LosslessStringProbe",
        "tests/corpus/numbers/LosslessStringProbe.java",
        include_str!("corpus/numbers/LosslessStringProbe.java"),
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn text() { assert_eq!(LosslessStringProbe::run().value(), 0); }
#[test] fn checked_hash_consumers_share_the_unit_kernel() {
    let raw = JavaString::from_units(vec![0xd800, 65, 0xdc00]);
    assert_eq!(java_string_hash_code(&raw), 53197791);
    assert_eq!(JavaHashCode::java_hash_code_result(&raw).unwrap(), 53197791);
    assert_eq!(JavaHashCode::java_hash_code_result(&Some(raw)).unwrap(), 53197791);
    assert_eq!(JavaHashCode::java_hash_code_result(&None::<JavaString>).unwrap(), 0);
}
"#,
    );
}
