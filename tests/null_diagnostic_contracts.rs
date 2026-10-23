#[path = "common/mod.rs"]
mod common;

#[test]
fn null_diagnostic_cross_unit_source_names_survive_batch_summary_copies() {
    let sources = [
        (
            "producer/Datum.java",
            include_str!("corpus/numbers/null-diagnostics/producer/Datum.java"),
        ),
        (
            "consumer/CrossUnitNpeProbe.java",
            include_str!("corpus/numbers/null-diagnostics/consumer/CrossUnitNpeProbe.java"),
        ),
    ];
    let paths = sources
        .iter()
        .map(|(path, source)| {
            (
                format!("tests/corpus/numbers/null-diagnostics/{path}"),
                *source,
            )
        })
        .collect::<Vec<_>>();
    let oracle = paths
        .iter()
        .map(|(path, source)| (path.as_str(), *source))
        .collect::<Vec<_>>();
    common::verify_java_sources_zero_contract("consumer.CrossUnitNpeProbe", &oracle);
    let batch = sources
        .iter()
        .map(|(path, source)| new_java2rust::BatchSource::new(*path, *source))
        .collect::<Vec<_>>();
    common::cargo_run_translated_batch_sources(
        "parameter_npe_batch",
        &batch,
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
fn main() { assert_eq!(translated::CrossUnitNpeProbe::run().value(), 0); }
"#,
    );
}

#[test]
fn null_diagnostic_parameter_dereferences_preserve_source_messages() {
    common::verify_java_fixture_zero_contract(
        "parameter_npe_diagnostics",
        "npeprobe.ParameterNpeProbe",
        "tests/corpus/numbers/ParameterNpeProbe.java",
        include_str!("corpus/numbers/ParameterNpeProbe.java"),
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn checked_parameter_operations() { assert_eq!(ParameterNpeProbe::run().value(), 0); }
"#,
    );
}
