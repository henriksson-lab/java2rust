#[path = "common/mod.rs"]
mod common;

#[test]
fn literal_split_preserves_utf16_limits_empty_input_and_no_match_identity() {
    common::verify_java_fixture_zero_contract(
        "literal_split_contracts",
        "LiteralSplitProbe",
        "tests/corpus/numbers/LiteralSplitProbe.java",
        include_str!("corpus/numbers/LiteralSplitProbe.java"),
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn splits() { assert_eq!(LiteralSplitProbe::run().value(), 0); }
"#,
    );
}

#[test]
fn literal_split_admission_respects_regex_syntax_shadowing_and_source_ownership() {
    for source in [
        r#"class Probe { static String[] f(String text) { return text.split("."); } }"#,
        r#"class Probe { static String[] f(String text) { return text.split(""); } }"#,
        r#"class Probe { static String[] f(String text) { return text.split("\\."); } }"#,
        r#"class Probe { static String[] f(String text) { return text.split("\ud800"); } }"#,
        r#"class Probe { static final String SEP = " "; static String[] f(String text, String SEP) { return text.split(SEP); } }"#,
        r#"class Other { String[] split(String text) { return new String[]{text}; } } class Probe { static String[] f(Other other) { return other.split(" "); } }"#,
    ] {
        let translation = new_java2rust::translate_source(source);
        assert!(
            !translation.diagnostics.has_errors(),
            "{:?}",
            translation.diagnostics
        );
        assert!(
            !translation
                .rust
                .contains("java_string_split_literal_identity("),
            "{source}"
        );
    }
    for source in [
        r#"class Probe { static String[] f(String text) { return text.split("::"); } }"#,
        r#"class Probe { static final String SEP = " "; static String[] f(String text) { return text.split(SEP); } }"#,
        r#"class Probe { static final String SEP = " "; static String[] f(String text) { return text.split(Probe.SEP); } }"#,
    ] {
        let translation = new_java2rust::translate_source(source);
        assert!(
            !translation.diagnostics.has_errors(),
            "{:?}",
            translation.diagnostics
        );
        assert!(
            translation
                .rust
                .contains("java_string_split_literal_identity("),
            "{source}"
        );
    }
}
