#[path = "common/mod.rs"]
mod common;

use new_java2rust::java_ir::{Expr, Literal};

#[test]
fn parsed_literal_code_units_match_javac() {
    use std::process::Command;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    let mut tokens: Vec<String> = (0xd800..=0xdfff)
        .map(|unit| format!("'\\u{unit:04x}'"))
        .collect();
    tokens.extend((0..=255).map(|unit| format!("'\\{unit:03o}'")));
    tokens.extend(
        [
            r#""""#,
            r#""é😀""#,
            r#""A\u00e9\uD83D\uDE00Z""#,
            r#""\uD800x\uDFFF""#,
            r#""\uDE00\uD83D""#,
            r#""\u0000\uFFFF""#,
            r#""\b\t\n\f\r\s\"\'\\""#,
            r#""\377\400\777\378""#,
            r#""\\u0041""#,
            r#""\\\u0041""#,
            r#""\uuuu0041""#,
            r#""\u005cn""#,
            r#""\u005c\u005cn""#,
            r"'\b'",
            r"'\f'",
            r"'\s'",
            r"'\''",
            r"'\u005cn'",
            r"'\uFFFF'",
            "\"\"\" \t\r\n  A\\s  \r\n  \\uD83D\\\r\n  Z\r\n  \"\"\"",
            "\"\"\"\n  a\n    b  \n  \"\"\"",
            "\"\"\"\n  \\uD800\n  \"\"\"",
        ]
        .map(str::to_owned),
    );

    let scratch = std::env::temp_dir().join(format!(
        "java2rust-literal-oracle-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&scratch).unwrap();
    // Only the input tokens are shared. javac evaluates them; the translator's
    // parser independently decodes them. No Rust expectation supplies values
    // to the Java oracle, and this is not a hand-repaired translated program.
    let calls = tokens
        .iter()
        .map(|token| format!("dump({token});\n"))
        .collect::<String>();
    let source = format!(
        r#"
class LiteralOracle {{
    static void dump(char value) {{ dump(String.valueOf(value)); }}
    static void dump(String value) {{
        StringBuilder out = new StringBuilder();
        for (int i = 0; i < value.length(); ++i) {{
            if (i != 0) out.append(',');
            out.append(Integer.toHexString(value.charAt(i)));
        }}
        System.out.println(out);
    }}
    public static void main(String[] args) {{ {calls} }}
}}
"#
    );
    std::fs::write(scratch.join("LiteralOracle.java"), source).unwrap();
    let compiled = common::run_with_watchdog(
        Command::new("javac")
            .arg("-encoding")
            .arg("UTF-8")
            .arg("-d")
            .arg(&scratch)
            .arg(scratch.join("LiteralOracle.java")),
        Duration::from_secs(30),
    );
    assert!(
        compiled.status.is_some_and(|s| s.success()),
        "{}",
        compiled.stderr
    );
    let oracle = common::run_with_watchdog(
        Command::new("java")
            .arg("-Xmx128m")
            .arg("-cp")
            .arg(&scratch)
            .arg("LiteralOracle"),
        Duration::from_secs(30),
    );
    assert!(
        oracle.status.is_some_and(|s| s.success()),
        "{}",
        oracle.stderr
    );
    let expected: Vec<_> = oracle.stdout.lines().collect();
    assert_eq!(expected.len(), tokens.len());
    for (token, expected) in tokens.iter().zip(expected) {
        let units = match new_java2rust::parser::parse_expr(token) {
            Expr::Literal(Literal::String(value)) => value.encode_utf16().collect::<Vec<_>>(),
            Expr::Literal(Literal::StringUtf16(units)) => units,
            Expr::Literal(literal) => vec![literal.java_char_unit().expect("Java char")],
            other => panic!("{token}: {other:?}"),
        };
        let actual = units
            .iter()
            .map(|unit| format!("{unit:x}"))
            .collect::<Vec<_>>()
            .join(",");
        assert_eq!(actual, expected, "{token}");
    }
    std::fs::remove_file(scratch.join("LiteralOracle.java")).unwrap();
    std::fs::remove_file(scratch.join("LiteralOracle.class")).unwrap();
    std::fs::remove_dir(&scratch).unwrap();
}

#[test]
fn scalar_literal_escape_contracts_execute_in_generated_rust() {
    common::verify_java_fixture_zero_contract(
        "literal_escape_contract",
        "LiteralEscapesProbe",
        "tests/corpus/numbers/LiteralEscapesProbe.java",
        include_str!("corpus/numbers/LiteralEscapesProbe.java"),
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn escapes() { assert_eq!(LiteralEscapesProbe::run().value(), 0); }
"#,
    );
}
