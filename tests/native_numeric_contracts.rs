#[path = "common/mod.rs"]
mod common;

#[test]
fn integral_radix_formatting_uses_native_widths_widening_and_checked_unboxing() {
    common::verify_java_fixture_zero_contract(
        "integral_format_contract",
        "IntegralFormatProbe",
        "tests/corpus/numbers/IntegralFormatProbe.java",
        include_str!("corpus/numbers/IntegralFormatProbe.java"),
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn formatting() { assert_eq!(IntegralFormatProbe::run().value(),0); }
"#,
    );
}

#[test]
fn scalb_overloads_keep_java_widths_imports_unboxing_and_evaluation_order() {
    common::verify_java_fixture_zero_contract(
        "native_scalb_overloads",
        "ScalbProbe",
        "tests/corpus/numbers/ScalbProbe.java",
        include_str!("corpus/numbers/ScalbProbe.java"),
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn overloads() { assert_eq!(ScalbProbe::run().value(), 0); }
"#,
    );
}

#[test]
fn scalb_raw_bits_match_jdk_across_scaling_and_rounding_boundaries() {
    use std::process::Command;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let scratch = std::env::temp_dir().join(format!(
        "java2rust-scalb-oracle-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&scratch).unwrap();
    let compiled = common::run_with_watchdog(
        Command::new("javac")
            .arg("-d")
            .arg(&scratch)
            .arg(root.join("tests/corpus/numbers/ScalbProbe.java"))
            .arg(root.join("tests/corpus/numbers/ScalbOracle.java")),
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
            .arg("ScalbOracle"),
        Duration::from_secs(30),
    );
    assert!(
        oracle.status.is_some_and(|status| status.success()),
        "{}",
        oracle.stderr
    );
    assert_eq!(oracle.stdout.lines().count(), 15856);
    let tests = format!(
        r#"
#[test] fn raw_scaling_matrix() {{
    for line in {:?}.lines() {{
        let parts: Vec<_> = line.split('|').collect();
        let exponent: i32 = parts[2].parse().unwrap();
        if parts[0] == "D" {{
            let value = f64::from_bits(u64::from_str_radix(parts[1], 16).unwrap());
            assert_eq!(format!("{{:x}}", ScalbProbe::scale_double(value, exponent).to_bits()), parts[3], "{{line}}");
            assert_eq!(format!("{{:x}}", ScalbProbe::strict_double(value, exponent).to_bits()), parts[4], "{{line}}");
        }} else {{
            let value = f32::from_bits(u32::from_str_radix(parts[1], 16).unwrap());
            assert_eq!(format!("{{:x}}", ScalbProbe::scale_float(value, exponent).to_bits()), parts[3], "{{line}}");
            assert_eq!(format!("{{:x}}", ScalbProbe::strict_float(value, exponent).to_bits()), parts[4], "{{line}}");
        }}
    }}
}}
"#,
        oracle.stdout
    );
    let translated =
        new_java2rust::translate_source(include_str!("corpus/numbers/ScalbProbe.java"));
    assert!(
        !translated.diagnostics.has_errors(),
        "{:?}",
        translated.diagnostics
    );
    common::cargo_verify_generated_modules_with_edition(
        "native_scalb_jdk_matrix",
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

#[test]
fn native_numeric_widths_sign_bits_checked_errors_and_imports() {
    common::verify_java_fixture_zero_contract(
        "native_numeric",
        "NativeNumericProbe",
        "tests/corpus/numbers/NativeNumericProbe.java",
        include_str!("corpus/numbers/NativeNumericProbe.java"),
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn numeric() { assert_eq!(NativeNumericProbe::run().value(), 0); }
"#,
    );
}

#[test]
fn source_numeric_owner_lookalikes_keep_their_declared_contracts() {
    common::verify_java_fixture_zero_contract(
        "native_numeric_shadow",
        "NativeNumericShadowProbe",
        "tests/corpus/numbers/NativeNumericShadowProbe.java",
        include_str!("corpus/numbers/NativeNumericShadowProbe.java"),
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn shadow() { assert_eq!(NativeNumericShadowProbe::run().value(), 0); }
"#,
    );
}
