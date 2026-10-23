#[path = "common/mod.rs"]
mod common;

#[test]
fn live_text_views_string_writers_and_incremental_utf8_match_java() {
    use std::process::Command;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};
    let scratch = std::env::temp_dir().join(format!(
        "java2rust-text-transport-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&scratch).unwrap();
    let compiled = common::run_with_watchdog(
        Command::new("javac").arg("-d").arg(&scratch).arg(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/corpus/numbers/TextTransportOracle.java"),
        ),
        Duration::from_secs(30),
    );
    assert!(
        compiled.status.is_some_and(|s| s.success()),
        "{}",
        compiled.stderr
    );
    let java = common::run_with_watchdog(
        Command::new("java")
            .arg("-cp")
            .arg(&scratch)
            .arg("TextTransportOracle"),
        Duration::from_secs(30),
    );
    assert!(java.status.is_some_and(|s| s.success()), "{}", java.stderr);
    assert_eq!(java.stdout.lines().count(), 154);
    let translated = new_java2rust::translate_source(
        "class RuntimeTextTransport { static int run() { StringBuilder builder = new StringBuilder(); return builder.length(); } }",
    );
    assert!(
        !translated.diagnostics.has_errors(),
        "{:?}",
        translated.diagnostics
    );
    let tests = format!(
        "{}\n#[test] fn transport() {{ let actual = transport_observations(); let expected: Vec<_> = {:?}.lines().collect(); assert_eq!(actual, expected); }}",
        include_str!("corpus/numbers/TextTransportOracle.rs"),
        java.stdout
    );
    common::cargo_test_generated_modules(
        "native_lossless_text_transport",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translated.rust,
            tests: Box::leak(tests.into_boxed_str()),
        }],
    );
    std::fs::remove_dir_all(scratch).unwrap();
}
