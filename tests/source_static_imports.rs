use new_java2rust::{BatchSource, BatchTranslationOptions};

#[path = "common/mod.rs"]
mod common;

const UTILITY: &str = r#"
package base;
public class Utility {
    public static int increment(int value) { return value + 1; }
    public static boolean isZero(int value) { return value == 0; }
    public static <T> T identity(T value) { return value; }
    public static int choose(int value) { return 1; }
    public static int choose(String value) { return 2; }
    public static int requirePositive(int value) {
        if (value < 0) throw new IllegalArgumentException("negative");
        return value;
    }
}
"#;
const USE: &str = r#"
package client;
import static base.Utility.increment;
import static base.Utility.isZero;
import static base.Utility.identity;
import static base.Utility.choose;
import static base.Utility.requirePositive;
public class Use {
    public static int run() { if (isZero(0)) return increment(4); return 0; }
    public static String text() { return identity("ok"); }
    public static int overloaded() { return choose(2) + choose("x"); }
    public static int caught() {
        try { return requirePositive(-1); }
        catch (IllegalArgumentException ex) { return 9; }
    }
}
"#;

#[test]
fn static_imports_use_emitted_source_owner_without_package_facades() {
    for context_emitted in [false, true] {
        let report =
            new_java2rust::translate_batch_sources_with_context_sources_summary_sources_and_options(
                &[
                    BatchSource::new("base/Utility.java", UTILITY),
                    BatchSource::new("client/Use.java", USE),
                ],
                &[],
                &[],
                context_emitted,
                &Default::default(),
                &BatchTranslationOptions::new(1, None),
            );
        let client = report
            .translations()
            .iter()
            .find(|item| item.path() == "client/Use.java")
            .unwrap();
        let rust = client.rust();
        assert!(
            !client.diagnostics().has_errors(),
            "{:?}",
            client.diagnostics()
        );
        assert!(
            rust.contains("Utility::increment(") && rust.contains("Utility::is_zero("),
            "{rust}"
        );
        assert!(
            !rust.contains("BaseUtility::") && !rust.contains("pub struct BaseUtility;"),
            "{rust}"
        );
        assert!(
            !rust.contains("source-summary static method base.Utility"),
            "{rust}"
        );
    }
}

#[test]
fn emission_filter_does_not_claim_unemitted_primary_definitions() {
    let options =
        BatchTranslationOptions::new(1, None).with_emission_paths(["client/Use.java".to_string()]);
    let report =
        new_java2rust::translate_batch_sources_with_context_sources_summary_sources_and_options(
            &[
                BatchSource::new("base/Utility.java", UTILITY),
                BatchSource::new("client/Use.java", USE),
            ],
            &[],
            &[],
            false,
            &Default::default(),
            &options,
        );
    assert_eq!(report.translations().len(), 1);
    // A primary used only for analysis is a real unavailable boundary in this
    // selected output. Do not suppress its declaration by assuming all analyzed
    // primary files will be present in the surrounding crate.
    assert!(
        report.translations()[0]
            .rust()
            .contains("pub struct BaseUtility;"),
        "{}",
        report.translations()[0].rust()
    );
}

#[test]
fn assembled_static_imports_execute_source_bodies_instead_of_facades() {
    use std::{
        fs,
        process::Command,
        time::{Duration, SystemTime, UNIX_EPOCH},
    };
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("new_java2rust_static_import_assembly_{unique}"));
    let primary = dir.join("java");
    fs::create_dir_all(primary.join("base")).unwrap();
    fs::create_dir_all(primary.join("client")).unwrap();
    fs::write(primary.join("base/Utility.java"), UTILITY).unwrap();
    fs::write(primary.join("client/Use.java"), USE).unwrap();
    let out = dir.join("out");
    let mut command = Command::new(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("scripts/translate-focused-package.sh"),
    );
    command
        .args([
            "--skip-resolver-bench",
            "--offline-cargo",
            "--jobs",
            "2",
            "--primary",
        ])
        .arg(&primary)
        .arg("--out")
        .arg(&out)
        .env("NEW_JAVA2RUST_BIN", env!("CARGO_BIN_EXE_new-java2rust"))
        .env_remove("CARGO_MAKEFLAGS")
        .env_remove("MAKEFLAGS");
    let output = common::run_with_watchdog(&mut command, Duration::from_secs(120));
    assert!(
        output.status.is_some_and(|status| status.success()),
        "{}\n{}\nfixture: {}",
        output.stdout,
        output.stderr,
        dir.display()
    );
    let support = fs::read_to_string(out.join("generated-crate/src/lib.rs")).unwrap();
    assert!(
        !support.contains("pub struct BaseUtility;"),
        "facade for emitted source survived assembly: {}",
        dir.display()
    );

    // This probe is a separate consumer crate, not a repair or modification of
    // the generated code. Compilation alone cannot detect a callable stub.
    let probe = dir.join("probe");
    fs::create_dir_all(probe.join("src")).unwrap();
    fs::write(probe.join("Cargo.toml"), format!(
        "[package]\nname=\"static-import-probe\"\nversion=\"0.0.0\"\nedition=\"2024\"\n[dependencies]\ntranslated={{package=\"new-java2rust-focused-output\",path={:?}}}\n",
        out.join("generated-crate").to_str().unwrap(),
    )).unwrap();
    fs::write(probe.join("src/main.rs"), "fn main() { assert_eq!(translated::Use::run(), 5); assert_eq!(translated::Use::text(), \"ok\"); assert_eq!(translated::Use::overloaded(), 3); assert_eq!(translated::Use::caught(), 9); }").unwrap();
    let mut command = Command::new(env!("CARGO"));
    command
        .args(["run", "--quiet", "--offline"])
        .current_dir(&probe)
        .env("CARGO_TARGET_DIR", probe.join("target"))
        .env_remove("CARGO_MAKEFLAGS")
        .env_remove("MAKEFLAGS");
    let output = common::run_with_watchdog(&mut command, Duration::from_secs(120));
    assert!(
        output.status.is_some_and(|status| status.success()),
        "{}\n{}\nfixture: {}",
        output.stdout,
        output.stderr,
        dir.display()
    );
    fs::remove_dir_all(dir).unwrap();
}
