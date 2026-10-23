#[path = "common/mod.rs"]
mod common;

#[test]
fn decimal_runtime_matches_independent_jdk_requests() {
    use std::process::Command;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let scratch = std::env::temp_dir().join(format!(
        "java2rust-decimal-oracle-{}",
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
            .arg(root.join("tests/corpus/numbers/BigDecimalOracle.java")),
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
            .arg("BigDecimalOracle"),
        Duration::from_secs(30),
    );
    assert!(
        oracle.status.is_some_and(|status| status.success()),
        "{}",
        oracle.stderr
    );
    assert_eq!(oracle.stdout.lines().count(), 2908);
    let tests = format!(
        r#"
fn view(value: BigDecimal) -> String {{ format!("{{}};{{}};{{:x}};{{:x}}", value, value.scale(), value.double_value().to_bits(), value.float_value().to_bits()) }}
fn construct<T: BigDecimalInput>(value: T) -> Result<BigDecimal, String> {{ BigDecimal::new(value).map_err(|error| format!("ERR:NumberFormatException:{{}}", number_format_message(&error).unwrap_or_else(|| "null".into()))) }}
#[test] fn exact_jdk_matrix() {{
    let modes = [RoundingMode::UP, RoundingMode::DOWN, RoundingMode::CEILING, RoundingMode::FLOOR, RoundingMode::HALF_UP, RoundingMode::HALF_DOWN, RoundingMode::HALF_EVEN, RoundingMode::UNNECESSARY];
    for line in {:?}.lines() {{
        let parts = line.split('|').collect::<Vec<_>>();
        let result = match parts[0] {{
            "S" => construct(parts[1]),
            "D" => construct(f64::from_bits(u64::from_str_radix(parts[1], 16).unwrap())),
            "R" => construct(parts[1]).and_then(|value| value.set_scale(parts[2].parse().unwrap(), modes[parts[3].parse::<usize>().unwrap()]).map_err(|error| format!("ERR:ArithmeticException:{{error}}"))),
            "A" => construct(parts[1]).and_then(|left| construct(parts[2]).and_then(|right| left.add(right).map_err(|error| format!("ERR:ArithmeticException:{{error}}")))),
            "B" => construct(parts[1]).and_then(|left| construct(parts[2]).and_then(|right| left.subtract(right).map_err(|error| format!("ERR:ArithmeticException:{{error}}")))),
            "Q" => construct(f64::from_bits(u64::from_str_radix(parts[1], 16).unwrap())).and_then(|left| construct(f64::from_bits(u64::from_str_radix(parts[2], 16).unwrap())).and_then(|right| left.add(right).map_err(|error| format!("ERR:ArithmeticException:{{error}}")))),
            other => panic!("unknown request {{other}}"),
        }};
        let observed = result.map(view).unwrap_or_else(|error| error);
        assert_eq!(observed, *parts.last().unwrap(), "{{line}}");
    }}
}}
"#,
        oracle.stdout
    );
    let runtime = root.join("runtime/java-math");
    common::cargo_verify_generated_modules_with_dependencies(
        "decimal_runtime_jdk_matrix",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: "pub use java_math_runtime::*;".into(),
            tests: Box::leak(tests.into_boxed_str()),
        }],
        "test",
        "2021",
        &[("java-math-runtime", runtime.as_path())],
    );
    std::fs::remove_dir_all(scratch).unwrap();
}

#[test]
fn mapped_decimal_constructors_rounding_addition_and_nullable_errors_execute() {
    let parsed = new_java2rust::external_packs::parse_external_pack_manifest(include_str!(
        "../runtime/java-math/manifests/big-integer.facts"
    ));
    assert!(
        !parsed.diagnostics().has_errors(),
        "{:?}",
        parsed.diagnostics()
    );
    let source = r#"
import java.math.BigDecimal;
import java.math.RoundingMode;
class Probe {
    static int modeCalls;
    static RoundingMode nextMode() { modeCalls++; return RoundingMode.HALF_UP; }
    static int run() {
        double binary = 0.1;
        if (!new BigDecimal(binary).toString().equals("0.1000000000000000055511151231257827021181583404541015625")) return 23;
        if (!new BigDecimal("1.00").subtract(new BigDecimal("2.0")).toString().equals("-1.00")) return 24;
        if (!new BigDecimal(0.1).toString().equals("0.1000000000000000055511151231257827021181583404541015625")) return 1;
        if (!new BigDecimal("1.00").add(new BigDecimal("2.0")).toString().equals("3.00")) return 2;
        if (new BigDecimal("2.5").setScale(0, RoundingMode.HALF_EVEN).doubleValue() != 2.0) return 3;
        try { new BigDecimal(""); return 4; }
        catch (NumberFormatException error) { if (error.getMessage() != null) return 5; }
        try { new BigDecimal(Double.NaN); return 6; }
        catch (NumberFormatException error) { if (!"Infinite or NaN".equals(error.getMessage())) return 7; }
        if (new BigDecimal("1.25").floatValue() != 1.25f) return 8;
        if (new BigDecimal("1.00").scale() != 2) return 9;
        if (new BigDecimal("-2.5").setScale(0, RoundingMode.UP).doubleValue() != -3.0) return 10;
        if (new BigDecimal("-2.5").setScale(0, RoundingMode.DOWN).doubleValue() != -2.0) return 11;
        if (new BigDecimal("-2.5").setScale(0, RoundingMode.CEILING).doubleValue() != -2.0) return 12;
        if (new BigDecimal("-2.5").setScale(0, RoundingMode.FLOOR).doubleValue() != -3.0) return 13;
        if (new BigDecimal("-2.5").setScale(0, RoundingMode.HALF_UP).doubleValue() != -3.0) return 14;
        if (new BigDecimal("-2.5").setScale(0, RoundingMode.HALF_DOWN).doubleValue() != -2.0) return 15;
        try { new BigDecimal("1.1").setScale(0, RoundingMode.UNNECESSARY); return 16; }
        catch (ArithmeticException error) { if (!"Rounding necessary".equals(error.getMessage())) return 17; }
        if (!RoundingMode.HALF_EVEN.name().equals("HALF_EVEN")) return 18;
        if (!RoundingMode.HALF_DOWN.toString().equals("HALF_DOWN")) return 19;
        if (RoundingMode.UNNECESSARY.ordinal() != 7) return 20;
        try { new BigDecimal("invalid").setScale(0, nextMode()); return 21; }
        catch (NumberFormatException error) { if (modeCalls != 0) return 22; }
        return 0;
    }
}
"#;
    verify_java_probe(source);
    let translation = new_java2rust::translate_source_with_facts(source, parsed.facts());
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    let runtime = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("runtime/java-math");
    common::cargo_verify_generated_modules_with_dependencies(
        "decimal_mapped_boundaries",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn decimal_contracts() { assert_eq!(Probe::run().value(), 0); }
"#,
        }],
        "test",
        "2021",
        &[("java-math-runtime", runtime.as_path())],
    );
}

#[test]
fn mapped_decimal_text_input_and_exception_messages_keep_original_units() {
    let parsed = new_java2rust::external_packs::parse_external_pack_manifest(include_str!(
        "../runtime/java-math/manifests/big-integer.facts"
    ));
    assert!(!parsed.diagnostics().has_errors());
    let source = r#"
import java.math.BigDecimal;
class Probe {
    static BigDecimal parse(String value) { return new BigDecimal(value); }
    static int run() {
        String value = "١٢.٣";
        if (!parse(value).toString().equals("12.3") || !value.equals("١٢.٣")) return 1;
        if (!parse(Double.toString(1.25)).toString().equals("1.25")) return 2;
        try { parse("\ud800"); return 3; }
        catch (NumberFormatException error) {
            if (!error.getMessage().equals("Character \ud800 is neither a decimal digit number, decimal point, nor \"e\" notation exponential mark.")) return 4;
        }
        try { parse("𝟙"); return 5; }
        catch (NumberFormatException error) {
            if (!error.getMessage().equals("Character \ud835 is neither a decimal digit number, decimal point, nor \"e\" notation exponential mark.")) return 6;
        }
        try { parse(null); return 7; }
        catch (NullPointerException expected) { }
        return 0;
    }
}
"#;
    verify_java_probe(source);
    let translation = new_java2rust::translate_source_with_facts(source, parsed.facts());
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    let runtime = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("runtime/java-math");
    common::cargo_verify_generated_modules_with_dependencies(
        "decimal_lossless_text_bridge",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn text() { assert_eq!(Probe::run().value(), 0); }
"#,
        }],
        "test",
        "2021",
        &[("java-math-runtime", runtime.as_path())],
    );
}

#[test]
fn nullable_mapped_rounding_mode_is_checked_after_decimal_construction() {
    let parsed = new_java2rust::external_packs::parse_external_pack_manifest(include_str!(
        "../runtime/java-math/manifests/big-integer.facts"
    ));
    assert!(!parsed.diagnostics().has_errors());
    let source = r#"
import java.math.BigDecimal;
import java.math.RoundingMode;
class Probe {
    static double rounded(double value, RoundingMode mode) {
        try {
            return new BigDecimal(Double.toString(value)).setScale(0, mode).doubleValue();
        } catch (NumberFormatException error) {
            return value;
        }
    }
    static double exact(double value) {
        try {
            return new BigDecimal(Double.toString(value)).setScale(0, RoundingMode.UNNECESSARY).doubleValue();
        } catch (NumberFormatException error) {
            return value;
        }
    }
    static BigDecimal construct(double value) { return new BigDecimal(value); }
    static int run() {
        if (!Double.isNaN(rounded(Double.NaN, null))) return 1;
        if (rounded(Double.POSITIVE_INFINITY, null) != Double.POSITIVE_INFINITY) return 2;
        try { rounded(1.5, null); return 3; }
        catch (NullPointerException error) { }
        if (rounded(1.5, RoundingMode.HALF_UP) != 2.0) return 4;
        try { exact(1.5); return 5; }
        catch (ArithmeticException error) { }
        try { construct(Double.NaN); return 6; }
        catch (NumberFormatException error) { }
        return 0;
    }
}
"#;
    verify_java_probe(source);
    let translation = new_java2rust::translate_source_with_facts(source, parsed.facts());
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    let runtime = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("runtime/java-math");
    common::cargo_verify_generated_modules_with_dependencies(
        "decimal_nullable_mode",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn decimal_contracts() { assert_eq!(Probe::run().value(), 0); }
"#,
        }],
        "test",
        "2021",
        &[("java-math-runtime", runtime.as_path())],
    );
}

fn verify_java_probe(source: &str) {
    use std::process::Command;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};
    let scratch = std::env::temp_dir().join(format!(
        "java2rust-decimal-probe-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&scratch).unwrap();
    std::fs::write(scratch.join("Probe.java"), source).unwrap();
    std::fs::write(scratch.join("Oracle.java"),
        "class Oracle { public static void main(String[] args) { System.out.println(Probe.run()); } }").unwrap();
    let compiled = common::run_with_watchdog(
        Command::new("javac")
            .arg("-d")
            .arg(&scratch)
            .arg(scratch.join("Probe.java"))
            .arg(scratch.join("Oracle.java")),
        Duration::from_secs(30),
    );
    assert!(
        compiled.status.is_some_and(|status| status.success()),
        "{}",
        compiled.stderr
    );
    let observed = common::run_with_watchdog(
        Command::new("java")
            .arg("-Xmx128m")
            .arg("-cp")
            .arg(&scratch)
            .arg("Oracle"),
        Duration::from_secs(30),
    );
    assert!(
        observed.status.is_some_and(|status| status.success()),
        "{}",
        observed.stderr
    );
    assert_eq!(observed.stdout.trim(), "0");
    std::fs::remove_dir_all(scratch).unwrap();
}
