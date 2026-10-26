#[path = "common/mod.rs"]
mod common;

#[test]
fn tokenizer_text_preserves_nulls_identity_aliases_and_pushback() {
    common::verify_java_fixture_zero_contract(
        "stream_tokenizer_text",
        "StreamTokenizerTextProbe",
        "tests/fixtures/StreamTokenizerTextProbe.java",
        include_str!("fixtures/StreamTokenizerTextProbe.java"),
        r#"
trait Exit { fn value(self) -> i32; }
impl Exit for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Exit for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn token_text() { assert_eq!(StreamTokenizerTextProbe::run().value(), 0); }
"#,
    );
}

#[test]
fn java_char_syntax_ranges_accept_the_utf16_carrier() {
    let translation = new_java2rust::translate_source(
        r#"
import java.io.StreamTokenizer;
class TokenizerSyntax {
    static void configure(StreamTokenizer tokenizer) {
        tokenizer.resetSyntax();
        tokenizer.wordChars('0', '9');
        tokenizer.whitespaceChars(' ', ' ');
    }
}
"#,
    );
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "stream_tokenizer_char_syntax",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
#[test]
fn tokenizer_keeps_utf16_units_and_token_reference_identity() {
    let text = JavaString::from_units(vec![0xd800, 32, 0xd83d, 0xde00]);
    let reader = StringReader::new(text.clone());
    let reader_handle: Reader = reader.clone().into();
    assert_eq!(reader_handle.read_0().unwrap(), 0xd800);
    assert_eq!(reader_handle.read_0().unwrap(), 32);
    assert_eq!(reader_handle.read_0().unwrap(), 0xd83d);
    assert_eq!(reader_handle.read_0().unwrap(), 0xde00);
    let tokenizer = StreamTokenizer::new(StringReader::new(text));
    assert!(tokenizer.sval().is_none());
    assert_eq!(tokenizer.next_token(), StreamTokenizer::TT_WORD);
    let first = tokenizer.sval().unwrap();
    assert_eq!(first.units(), &[0xd800]);
    for _ in 0..1000 { assert!(first.java_identity_eq(&tokenizer.sval().unwrap())); }
    tokenizer.push_back();
    assert_eq!(tokenizer.next_token(), StreamTokenizer::TT_WORD);
    assert!(first.java_identity_eq(&tokenizer.sval().unwrap()));
    assert_eq!(tokenizer.next_token(), StreamTokenizer::TT_WORD);
    assert_eq!(tokenizer.sval().unwrap().units(), &[0xd83d, 0xde00]);
}
#[test]
fn configured_digit_range_classifies_a_word() {
    let tokenizer = StreamTokenizer::new(StringReader::new("123 456"));
    tokenizer.reset_syntax();
    tokenizer.word_chars(JavaCodeUnit(48), JavaCodeUnit(57));
    tokenizer.whitespace_chars(JavaCodeUnit(32), JavaCodeUnit(32));
    assert_eq!(tokenizer.next_token(), StreamTokenizer::TT_WORD);
    assert_eq!(tokenizer.sval().unwrap().units(), &[49, 50, 51]);
    assert_eq!(tokenizer.next_token(), StreamTokenizer::TT_WORD);
    assert_eq!(tokenizer.sval().unwrap().units(), &[52, 53, 54]);
}
"#,
        }],
    );
}
