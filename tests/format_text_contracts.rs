#[path = "common/mod.rs"]
mod common;

#[test]
fn formatting_retains_units_and_defers_display_until_requested() {
    common::verify_java_fixture_zero_contract(
        "lossless_formatting",
        "FormatTextProbe",
        "tests/corpus/numbers/FormatTextProbe.java",
        include_str!("corpus/numbers/FormatTextProbe.java"),
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn format_text() { assert_eq!(FormatTextProbe::run().value(), 0); }
"#,
    );
}
