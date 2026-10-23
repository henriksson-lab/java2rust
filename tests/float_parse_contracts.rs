#[path = "common/mod.rs"]
mod common;

#[test]
fn java_float_grammar_nulls_references_and_hex_literals_execute() {
    common::verify_java_fixture_zero_contract(
        "java_float_parse_contract",
        "FloatParseProbe",
        "tests/corpus/numbers/FloatParseProbe.java",
        include_str!("corpus/numbers/FloatParseProbe.java"),
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn parsing() { assert_eq!(FloatParseProbe::run().value(), 0); }
"#,
    );
}

#[test]
fn java_float_parsing_matches_jdk_bits_and_validation_messages() {
    use std::process::Command;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let scratch = std::env::temp_dir().join(format!(
        "java2rust-float-parse-oracle-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&scratch).unwrap();
    let compiled = common::run_with_watchdog(
        Command::new("javac")
            .arg("-encoding")
            .arg("UTF-8")
            .arg("-d")
            .arg(&scratch)
            .arg(root.join("tests/corpus/numbers/FloatParseProbe.java"))
            .arg(root.join("tests/corpus/numbers/FloatParseOracle.java"))
            .arg(root.join("tests/corpus/numbers/FloatTextOracle.java")),
        Duration::from_secs(30),
    );
    assert!(
        compiled.status.is_some_and(|status| status.success()),
        "{}",
        compiled.stderr
    );
    let oracle = common::run_with_watchdog(
        Command::new("java")
            .arg("-Xmx128m")
            .arg("-cp")
            .arg(&scratch)
            .arg("FloatParseOracle"),
        Duration::from_secs(30),
    );
    assert!(
        oracle.status.is_some_and(|status| status.success()),
        "{}",
        oracle.stderr
    );
    assert_eq!(oracle.stdout.lines().count(), 972);
    let text_oracle = common::run_with_watchdog(
        Command::new("java")
            .arg("-cp")
            .arg(&scratch)
            .arg("FloatTextOracle"),
        Duration::from_secs(30),
    );
    assert!(
        text_oracle.status.is_some_and(|status| status.success()),
        "{}",
        text_oracle.stderr
    );
    assert_eq!(text_oracle.stdout.lines().count(), 36);
    let tests = format!(
        r#"
#[test] fn exact_jdk_parse_requests() {{
    for line in {:?}.lines() {{
        let parts: Vec<_> = line.split('|').collect();
        let bytes = parts[1].as_bytes().chunks_exact(2).map(|digits| u8::from_str_radix(std::str::from_utf8(digits).unwrap(),16).unwrap()).collect();
        let value = JavaString::fresh(String::from_utf8(bytes).unwrap());
        let observed = if parts[0] == "D" {{
            FloatParseProbe::parse_double(Some(value)).map(|number| format!("{{:x}}", number.to_bits()))
        }} else {{
            FloatParseProbe::parse_float(Some(&value)).map(|number| format!("{{:x}}", number.to_bits()))
        }}.unwrap_or_else(|e| format!("ERR:{{}}:{{}}", e.class_name().rsplit('.').next().unwrap(), e.message().scalar_string().expect("the UTF-8 oracle has scalar messages")));
        assert_eq!(observed, parts[2], "{{line}}");
    }}
}}

#[test] fn exact_jdk_utf16_error_requests() {{
    fn units(text: &JavaString) -> String {{
        text.units().iter().map(|unit| format!("{{unit:x}}")).collect::<Vec<_>>().join(",")
    }}
    for line in {:?}.lines() {{
        let parts: Vec<_> = line.split('|').collect();
        let value = JavaString::from_units(parts[1].split(',').filter(|part| !part.is_empty())
            .map(|part| u16::from_str_radix(part,16).unwrap()).collect());
        let observed = if parts[0] == "D" {{
            FloatParseProbe::parse_double(Some(value)).map(|number| format!("{{:x}}", number.to_bits()))
        }} else {{
            FloatParseProbe::parse_float(Some(&value)).map(|number| format!("{{:x}}", number.to_bits()))
        }}.unwrap_or_else(|error| format!("ERR:{{}}:{{}}", error.class_name().rsplit('.').next().unwrap(), units(error.message())));
        assert_eq!(observed, parts[2], "{{line}}");
    }}
}}
"#,
        oracle.stdout, text_oracle.stdout
    );
    let translated =
        new_java2rust::translate_source(include_str!("corpus/numbers/FloatParseProbe.java"));
    assert!(
        !translated.diagnostics.has_errors(),
        "{:?}",
        translated.diagnostics
    );
    common::cargo_verify_generated_modules_with_edition(
        "java_float_parse_jdk_matrix",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translated.rust,
            tests: Box::leak(tests.into_boxed_str()),
        }],
        "test",
        "2021",
    );
    std::fs::remove_dir_all(scratch).unwrap();
}
