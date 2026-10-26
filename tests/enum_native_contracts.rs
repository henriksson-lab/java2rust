#[path = "common/mod.rs"]
mod common;

#[test]
fn source_enum_methods_arrays_and_native_sets_keep_their_declared_contracts() {
    common::verify_java_fixture_zero_contract(
        "enum_native_contracts",
        "EnumNativeContractsProbe",
        "tests/fixtures/EnumNativeContractsProbe.java",
        include_str!("fixtures/EnumNativeContractsProbe.java"),
        r#"
trait Exit { fn value(self) -> i32; }
impl Exit for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Exit for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn enum_contracts() { assert_eq!(EnumNativeContractsProbe::run().value(), 0); }
#[test] fn nullable_enum_storage_keeps_the_same_complement_universe() {
    let source = std::collections::BTreeSet::from([Some(EnumNativeContractsProbe_Choice::FIRST)]);
    let complement = java_enum_set_complement(&source);
    assert_eq!(complement, std::collections::BTreeSet::from([Some(EnumNativeContractsProbe_Choice::SECOND)]));
}
"#,
    );
}

#[test]
fn separately_emitted_class_tokens_retain_reflective_constructor_targets() {
    let sources = [
        (
            "tests/fixtures/reflection_factory/Work.java",
            include_str!("fixtures/reflection_factory/Work.java"),
        ),
        (
            "tests/fixtures/reflection_factory/FirstWork.java",
            include_str!("fixtures/reflection_factory/FirstWork.java"),
        ),
        (
            "tests/fixtures/reflection_factory/SecondWork.java",
            include_str!("fixtures/reflection_factory/SecondWork.java"),
        ),
        (
            "tests/fixtures/reflection_factory/ReflectionFactoryProbe.java",
            include_str!("fixtures/reflection_factory/ReflectionFactoryProbe.java"),
        ),
    ];
    common::verify_java_sources_zero_contract(
        "contracts.reflection.ReflectionFactoryProbe",
        &sources,
    );
    let sources = sources
        .iter()
        .map(|(path, source)| {
            new_java2rust::BatchSource::new(
                std::path::Path::new(path)
                    .file_name()
                    .unwrap()
                    .to_str()
                    .unwrap(),
                *source,
            )
        })
        .collect::<Vec<_>>();
    common::cargo_run_translated_batch_sources(
        "reflection_factory_contracts",
        &sources,
        r#"
trait Exit { fn value(self) -> i32; }
impl Exit for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Exit for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
fn main() { assert_eq!(translated::ReflectionFactoryProbe::run().value(), 0); }
"#,
    );
}
