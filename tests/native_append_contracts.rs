#[path = "common/mod.rs"]
mod common;

#[test]
fn object_append_preserves_checked_display_evaluation_order_and_aliases() {
    common::verify_java_fixture_zero_contract(
        "native_object_append",
        "appendcontracts.ObjectAppendProbe",
        "tests/corpus/numbers/ObjectAppendProbe.java",
        include_str!("corpus/numbers/ObjectAppendProbe.java"),
        r#"
trait OracleExitCode { fn value(self) -> i32; }
impl OracleExitCode for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> OracleExitCode for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn append() { assert_eq!(ObjectAppendProbe::run().value(), 0); }
"#,
    );
}

#[test]
fn source_builder_lookalikes_keep_their_own_append_contract() {
    common::verify_java_fixture_zero_contract(
        "source_builder_append",
        "appendshadow.SourceAppendProbe",
        "tests/corpus/numbers/SourceAppendProbe.java",
        include_str!("corpus/numbers/SourceAppendProbe.java"),
        r#"
trait OracleExitCode { fn value(self) -> i32; }
impl OracleExitCode for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> OracleExitCode for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn append() { assert_eq!(SourceAppendProbe::run().value(), 0); }
"#,
    );
}
