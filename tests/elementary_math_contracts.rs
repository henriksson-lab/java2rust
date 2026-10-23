#[path = "common/mod.rs"]
mod common;

#[test]
fn ordinary_complex_math_calls_meet_reference_accuracy_and_keep_branch_cut_bits() {
    use std::process::Command;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let scratch = std::env::temp_dir().join(format!(
        "java2rust-elementary-math-{}",
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
            .arg(root.join("tests/corpus/numbers/ElementaryMathProbe.java"))
            .arg(root.join("tests/corpus/numbers/ElementaryMathOracle.java")),
        Duration::from_secs(30),
    );
    assert!(
        compiled.status.is_some_and(|status| status.success()),
        "{}",
        compiled.stderr
    );
    let oracle = common::run_with_watchdog(
        Command::new("java")
            .arg("-cp")
            .arg(&scratch)
            .arg("ElementaryMathOracle"),
        Duration::from_secs(30),
    );
    assert!(
        oracle.status.is_some_and(|status| status.success()),
        "{}",
        oracle.stderr
    );
    assert_eq!(oracle.stdout.lines().count(), 11);
    let tests = format!(
        r#"
// Rounded references independently evaluated at 100 and 200 decimal digits.
// These bounds concern the primitive Math calls only, not composed Complex
// functions. No blanket tolerance is installed in the corpus comparator.
fn reference_distance(actual: u64, reference: u64) -> u64 {{
    assert_eq!(actual >> 63, reference >> 63, "sign/class must agree");
    assert!(f64::from_bits(actual).is_finite());
    actual.abs_diff(reference)
}}
#[test] fn primitive_accuracy_and_exact_branch_sides() {{
    for line in {:?}.lines() {{
        let (key, value) = line.split_once('=').unwrap();
        let java = u64::from_str_radix(value, 16).unwrap();
        let (actual, reference, max_steps) = match key {{
            "cosh.1" => (ElementaryMathProbe::cosh(1.0).to_bits(), 0x3ff8b07551d9f550, 2),
            "cosh.-1" => (ElementaryMathProbe::cosh(-1.0).to_bits(), 0x3ff8b07551d9f550, 2),
            "atan2.-3.-2" => (ElementaryMathProbe::atan2(-3.0, -2.0).to_bits(), 0xc00145385fa3af71, 1),
            "strict.cosh.1" => {{ assert!(reference_distance(java, 0x3ff8b07551d9f550) <= 2); continue; }},
            "strict.atan2.-3.-2" => {{ assert!(reference_distance(java, 0xc00145385fa3af71) <= 1); continue; }},
            "cosh.+0" => (ElementaryMathProbe::cosh(0.0).to_bits(), 0x3ff0000000000000, 0),
            "cosh.-0" => (ElementaryMathProbe::cosh(-0.0).to_bits(), 0x3ff0000000000000, 0),
            "atan2.+0.-1" => (ElementaryMathProbe::atan2(0.0, -1.0).to_bits(), std::f64::consts::PI.to_bits(), 0),
            "atan2.-0.-1" => (ElementaryMathProbe::atan2(-0.0, -1.0).to_bits(), (-std::f64::consts::PI).to_bits(), 0),
            "atan2.+0.+1" => (ElementaryMathProbe::atan2(0.0, 1.0).to_bits(), 0, 0),
            "atan2.-0.+1" => (ElementaryMathProbe::atan2(-0.0, 1.0).to_bits(), 1 << 63, 0),
            _ => panic!("unrecognized JDK observation {{line}}"),
        }};
        assert!(reference_distance(java, reference) <= max_steps, "JDK {{line}}");
        assert!(reference_distance(actual, reference) <= max_steps, "generated {{line}}: {{actual:x}}");
        eprintln!("{{key}}: java={{java:016x}}, generated={{actual:016x}}, reference={{reference:016x}}");
    }}
}}
"#,
        oracle.stdout
    );
    let translated =
        new_java2rust::translate_source(include_str!("corpus/numbers/ElementaryMathProbe.java"));
    assert!(
        !translated.diagnostics.has_errors(),
        "{:?}",
        translated.diagnostics
    );
    common::cargo_test_generated_modules(
        "complex_elementary_boundaries",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translated.rust,
            tests: Box::leak(tests.into_boxed_str()),
        }],
    );
    std::fs::remove_dir_all(scratch).unwrap();
}
