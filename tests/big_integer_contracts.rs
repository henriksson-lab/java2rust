#[path = "common/mod.rs"]
mod common;

#[test]
fn mapped_big_integer_constants_use_the_manifest_for_qualified_and_static_imports() {
    let parsed = new_java2rust::external_packs::parse_external_pack_manifest(include_str!(
        "../runtime/java-math/manifests/big-integer.facts"
    ));
    let translation = new_java2rust::translate_source_with_facts(
        r#"
import java.math.BigInteger;
import static java.math.BigInteger.TWO;
import static java.math.BigInteger.TEN;
class Probe {
    static int run() {
        BigInteger first = BigInteger.TWO;
        BigInteger second = java.math.BigInteger.TEN;
        int sum = first.intValue() + second.intValue();
        sum += TWO.intValue() + TEN.intValue();
        return sum + BigInteger.ZERO.intValue() + BigInteger.ONE.intValue();
    }
}
"#,
        parsed.facts(),
    );
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    let runtime = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("runtime/java-math");
    common::cargo_verify_generated_modules_with_dependencies(
        "mapped_big_integer_constants",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "#[test] fn constants() { assert_eq!(Probe::run(), 25); }",
        }],
        "test",
        "2021",
        &[("java-math-runtime", runtime.as_path())],
    );
}

#[test]
fn mapped_scalar_and_format_receivers_preserve_later_reads() {
    let parsed = new_java2rust::external_packs::parse_external_pack_manifest(include_str!(
        "../runtime/java-math/manifests/big-integer.facts"
    ));
    let translation = new_java2rust::translate_source_with_facts(
        r#"
import java.math.BigInteger;
class Probe {
    static String run() {
        BigInteger value = BigInteger.valueOf(3);
        int count = 0;
        for (int i = 0; i < 3; i++) {
            count += value.testBit(i) ? 1 : 0;
            count += value.compareTo(BigInteger.ZERO);
        }
        BigInteger square = value.multiply(value);
        String first = value.toString();
        return first + ":" + square.toString() + ":" + count + ":" + value.toString() + ":" + square.testBit(0);
    }
}
"#,
        parsed.facts(),
    );
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    let runtime = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("runtime/java-math");
    common::cargo_verify_generated_modules_with_dependencies(
        "mapped_receiver_liveness_paths",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
trait Value { fn value(self) -> String; }
impl Value for JavaString { fn value(self) -> String { self.scalar_string().expect("scalar numeric output") } }
impl<E: std::fmt::Debug> Value for Result<JavaString, E> { fn value(self) -> String { self.unwrap().value() } }
#[test] fn repeated_receivers() { assert_eq!(Probe::run().value(), "3:9:5:3:true"); }
"#,
        }],
        "test",
        "2021",
        &[("java-math-runtime", runtime.as_path())],
    );
}

#[test]
fn mapped_instance_calls_do_not_resolve_to_same_named_source_static_helpers() {
    let parsed = new_java2rust::external_packs::parse_external_pack_manifest(include_str!(
        "../runtime/java-math/manifests/big-integer.facts"
    ));
    let translation = new_java2rust::translate_source_with_facts(
        r#"
import java.math.BigInteger;
class Probe {
    static BigInteger pow(BigInteger k, int e) { return k.pow(e); }
    static BigInteger multiply(BigInteger k) { return k.multiply(k); }
    static int rejectsNegativeExponent() {
        try { pow(BigInteger.valueOf(3), -1); return 0; }
        catch (ArithmeticException error) { return 1; }
    }
    static int run() {
        BigInteger value = BigInteger.valueOf(3);
        return pow(value, 4).intValue() + multiply(value).intValue();
    }
}
"#,
        parsed.facts(),
    );
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    let runtime = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("runtime/java-math");
    common::cargo_verify_generated_modules_with_dependencies(
        "mapped_instance_same_named_static",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn mapped_receivers() {
    assert_eq!(Probe::run().value(), 90);
    assert_eq!(Probe::rejects_negative_exponent().value(), 1);
}
"#,
        }],
        "test",
        "2021",
        &[("java-math-runtime", runtime.as_path())],
    );
}

#[test]
fn mapped_big_integer_extended_api_matches_jdk_observations() {
    use std::{
        process::Command,
        time::{Duration, SystemTime, UNIX_EPOCH},
    };
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let oracle = root.join("tests/corpus/numbers/BigIntegerOracle.java");
    let scratch = std::env::temp_dir().join(format!(
        "java2rust-big-integer-oracle-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&scratch).unwrap();
    let compilation = common::run_with_watchdog(
        Command::new("javac").arg("-d").arg(&scratch).arg(&oracle),
        Duration::from_secs(30),
    );
    assert!(
        compilation.status.is_some_and(|status| status.success()),
        "{}",
        compilation.stderr
    );
    let java = common::run_with_watchdog(
        Command::new("java")
            .arg("-cp")
            .arg(&scratch)
            .arg("BigIntegerOracle"),
        Duration::from_secs(30),
    );
    assert!(
        java.status.is_some_and(|status| status.success()),
        "{}",
        java.stderr
    );
    assert_eq!(java.stdout.lines().count(), 542);
    let parsed = new_java2rust::external_packs::parse_external_pack_manifest(include_str!(
        "../runtime/java-math/manifests/big-integer.facts"
    ));
    assert!(
        !parsed.diagnostics().has_errors(),
        "{:?}",
        parsed.diagnostics()
    );
    let translation = new_java2rust::translate_source_with_facts(
        include_str!("corpus/numbers/BigIntegerOracle.java"),
        parsed.facts(),
    );
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    let tests = format!(
        r#"
trait OracleValue {{ fn value(self) -> String; }}
impl OracleValue for JavaString {{ fn value(self) -> String {{ self.scalar_string().expect("scalar numeric output") }} }}
impl<E: std::fmt::Debug> OracleValue for Result<JavaString, E> {{ fn value(self) -> String {{ self.unwrap().value() }} }}
#[test] fn exact_jdk_observations() {{ assert_eq!(BigIntegerOracle::run().value(), {:?}); }}
"#,
        java.stdout
    );
    let runtime = root.join("runtime/java-math");
    common::cargo_verify_generated_modules_with_dependencies(
        "mapped_big_integer_extended",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: Box::leak(tests.into_boxed_str()),
        }],
        "test",
        "2021",
        &[("java-math-runtime", runtime.as_path())],
    );
    std::fs::remove_dir_all(scratch).unwrap();
}

#[test]
fn mapped_constant_values_do_not_replace_source_or_local_bindings() {
    let parsed = new_java2rust::external_packs::parse_external_pack_manifest(include_str!(
        "../runtime/java-math/manifests/big-integer.facts"
    ));
    let translation = new_java2rust::translate_source_with_facts(
        r#"
import static java.math.BigInteger.ONE;
class BigInteger { static final int ONE = 7; }
class Probe { static int run() { int ONE = 3; return BigInteger.ONE + ONE; } }
class StaticImportShadow { static final int ONE = 4; static int run() { return ONE; } }
"#,
        parsed.facts(),
    );
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    let runtime = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("runtime/java-math");
    common::cargo_verify_generated_modules_with_dependencies(
        "mapped_constant_source_shadow",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "#[test] fn source_value() { assert_eq!(Probe::run(), 10); assert_eq!(StaticImportShadow::run(), 4); }",
        }],
        "test",
        "2021",
        &[("java-math-runtime", runtime.as_path())],
    );
}

#[test]
fn mapped_big_integer_calls_preserve_numeric_abi_and_checked_errors() {
    let parsed = new_java2rust::external_packs::parse_external_pack_manifest(include_str!(
        "../runtime/java-math/manifests/big-integer.facts"
    ));
    assert!(
        !parsed.diagnostics().has_errors(),
        "{:?}",
        parsed.diagnostics()
    );
    let translation = new_java2rust::translate_source_with_facts(
        r#"
import java.math.BigInteger;
class Probe {
    static final int[] INPUTS = {2, 10, 1000};
    static int power(long value, long exponent, long modulus) {
        return BigInteger.valueOf(value).modPow(BigInteger.valueOf(exponent), BigInteger.valueOf(modulus)).intValue();
    }
    static boolean divisible(int value, int divisor) { return value % divisor == 0; }
    static long wide(long value) { return BigInteger.valueOf(value).longValue(); }
    static int named(int value, int exponent, int modulus) {
        BigInteger base = BigInteger.valueOf(value);
        BigInteger exp = BigInteger.valueOf(exponent);
        BigInteger mod = BigInteger.valueOf(modulus);
        BigInteger result = base.modPow(exp, mod);
        return result.intValue();
    }
    static int fromArray(int value) {
        final int exponent = value >>> 1;
        final BigInteger exp = BigInteger.valueOf(exponent);
        final BigInteger mod = BigInteger.valueOf(value);
        final BigInteger base = BigInteger.valueOf(INPUTS[0]);
        return base.modPow(exp, mod).intValue();
    }
    static int repeated(int count) {
        final BigInteger exponent = BigInteger.valueOf(10);
        final BigInteger modulus = BigInteger.valueOf(1000);
        int total = 0;
        for (int i = 0; i < count; i++) {
            final BigInteger base = BigInteger.valueOf(2);
            total += base.modPow(exponent, modulus).intValue();
        }
        return total;
    }
}
"#,
        parsed.facts(),
    );
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    assert!(
        !translation.rust.contains("JavaMathBigInteger"),
        "{}",
        translation.rust
    );
    assert!(
        !translation
            .rust
            .contains("BigInteger.modPow is not modelled")
    );
    let runtime = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("runtime/java-math");
    common::cargo_verify_generated_modules_with_dependencies(
        "mapped_big_integer",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
#[test] fn values_and_failures() {
    assert_eq!(Probe::power(2, 10, 1000).unwrap(), 24);
    assert_eq!(Probe::named(2, 10, 1000).unwrap(), 24);
    assert_eq!(Probe::named(-2, -1, 5).unwrap(), 2);
    assert_eq!(Probe::from_array(10).unwrap(), 2);
    assert_eq!(Probe::repeated(3).unwrap(), 72);
    assert_eq!(Probe::power(2, -1, 5).unwrap(), 3);
    assert_eq!(Probe::power(-2, -1, 5).unwrap(), 2);
    assert_eq!(Probe::power(0, -1, 1).unwrap(), 0);
    assert!(Probe::power(2, -1, 4).unwrap_err().is_instance(&["ArithmeticException"]));
    assert!(Probe::power(2, 3, 0).unwrap_err().is_instance(&["ArithmeticException"]));
    let error = Probe::power(2, 3, 0).unwrap_err();
    assert!(error.is_instance(&["java.lang.ArithmeticException"]));
    assert!(error.is_instance(&["RuntimeException"]));
    assert!(error.is_instance(&["java.lang.Exception"]));
    assert!(!JavaException::new("vendor.ArithmeticException", "other")
        .is_instance(&["ArithmeticException"]));
    assert_eq!(Probe::wide(i64::MIN), i64::MIN);
    assert_eq!(Probe::wide(i64::MAX), i64::MAX);
    assert!(Probe::divisible(10, 2).unwrap());
    assert!(!Probe::divisible(10, 3).unwrap());
    assert!(Probe::divisible(10, 0).unwrap_err().is_instance(&["ArithmeticException"]));
}
"#,
        }],
        "test",
        "2021",
        &[("java-math-runtime", runtime.as_path())],
    );
}

#[test]
fn numeric_source_value_of_factory_keeps_its_selected_signature() {
    let translation = new_java2rust::translate_source(
        r#"
class Codes { static int valueOf(int code) { return code + 1; } }
class Probe { static int run() { return Codes.valueOf(7); } }
"#,
    );
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "numeric_value_of",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "#[test] fn value() { assert_eq!(Probe::run(), 8); }",
        }],
    );
}
