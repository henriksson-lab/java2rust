//! Differential tests: run the Java and run the translation, compare output.
//!
//! Compile errors only catch idioms that fail loudly. The defects that cost the
//! most — arithmetic that wraps the wrong way, a `StringBuilder` that silently
//! returns `""`, a `static` block that never runs, `Integer.MAX_VALUE` emitted
//! as `0` — all produce Rust that compiles cleanly and computes the wrong
//! answer. Nothing but running both sides finds those.
//!
//! Each fixture in `tests/differential/` is a self-contained class exposing
//! `public static String run()` and a `main` that prints it. Java's stdout is
//! the oracle; the translation must produce the same bytes.
//!
//! Requires `javac`, `java` and `rustc` on PATH. Missing prerequisites are
//! failures, because a skipped behavioural oracle is indistinguishable from a
//! passing one in the gate.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

use new_java2rust::translate_source;

const EXPECTED_DIFFERENTIAL_FIXTURES: usize = 202;
const EXPECTED_KNOWN_GAP_FIXTURES: usize = 6;

#[test]
fn translated_output_matches_java_output() {
    let translator = translator_binary().unwrap_or_else(|| {
        panic!("translator binary not built; run `cargo build --release` before differential tests")
    });
    for tool in ["javac", "java", "rustc"] {
        assert!(
            have_tool(tool),
            "required differential test tool `{tool}` is not available"
        );
    }

    let fixtures = fixture_paths();
    assert_eq!(
        fixtures.len(),
        EXPECTED_DIFFERENTIAL_FIXTURES,
        "differential fixture count changed; update EXPECTED_DIFFERENTIAL_FIXTURES intentionally"
    );

    let mut failures = Vec::new();
    for fixture in &fixtures {
        let class = fixture
            .file_stem()
            .and_then(|stem| stem.to_str())
            .expect("fixture has a class name");
        match compare_fixture(&translator, fixture, class) {
            Ok(()) => {}
            Err(report) => failures.push(format!("{class}: {report}")),
        }
    }

    assert!(
        failures.is_empty(),
        "translated output differs from Java for {} of {} fixtures:\n\n{}",
        failures.len(),
        fixtures.len(),
        failures.join("\n\n")
    );
}

#[test]
fn known_gap_fixtures_are_valid_java_and_match_expected_failure_mode() {
    for tool in ["javac", "java", "rustc"] {
        assert!(
            have_tool(tool),
            "required known-gap differential test tool `{tool}` is not available"
        );
    }

    let fixtures = known_gap_fixtures();
    let expected_paths = fixtures
        .iter()
        .map(|fixture| fixture.display.to_string())
        .collect::<BTreeSet<_>>();
    let actual_paths = known_gap_fixture_paths()
        .into_iter()
        .map(|path| {
            path.strip_prefix(Path::new(env!("CARGO_MANIFEST_DIR")))
                .unwrap_or(&path)
                .to_string_lossy()
                .into_owned()
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(
        fixtures.len(),
        EXPECTED_KNOWN_GAP_FIXTURES,
        "known-gap fixture count changed; update EXPECTED_KNOWN_GAP_FIXTURES intentionally"
    );
    assert_eq!(
        actual_paths, expected_paths,
        "known-gap fixture list does not match tests/differential/known-gaps/*.java"
    );

    let mut failures = Vec::new();
    for fixture in fixtures {
        if let Err(report) = check_known_gap(&fixture) {
            failures.push(format!("{}: {report}", fixture.display));
        }
    }

    assert!(
        failures.is_empty(),
        "known-gap fixture checks failed:\n\n{}",
        failures.join("\n\n")
    );
}

fn compare_fixture(translator: &Path, fixture: &Path, class: &str) -> Result<(), String> {
    let work = std::env::temp_dir().join(format!("new_java2rust_differential_{class}"));
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).map_err(|err| format!("temp dir: {err}"))?;

    let java_source = work.join(format!("{class}.java"));
    std::fs::copy(fixture, &java_source).map_err(|err| format!("copy fixture: {err}"))?;

    let expected = run_java(&work, &java_source, class)?;
    let actual = run_translated(translator, &work, &java_source, class)?;

    if expected == actual {
        Ok(())
    } else {
        Err(format!(
            "java produced {expected:?}, translation produced {actual:?}"
        ))
    }
}

struct KnownGapFixture {
    display: &'static str,
    class: &'static str,
    expected_java_output: &'static str,
    expectation: KnownGapExpectation,
}

enum KnownGapExpectation {
    LoudCompileError {
        message: &'static str,
        diagnostic_code: Option<&'static str>,
    },
    DivergentOutput {
        expected_translated_output: &'static str,
        diagnostic_code: &'static str,
    },
    RuntimePanic {
        panic_message: &'static str,
        diagnostic_code: &'static str,
    },
}

fn known_gap_fixtures() -> Vec<KnownGapFixture> {
    vec![
        KnownGapFixture {
            display: "tests/differential/known-gaps/Aliasing.java",
            class: "Aliasing",
            expected_java_output: "5|7|3|9",
            expectation: KnownGapExpectation::DivergentOutput {
                expected_translated_output: "0|0|0|0",
                diagnostic_code: "observable_alias",
            },
        },
        KnownGapFixture {
            display: "tests/differential/known-gaps/InheritanceUpcast.java",
            class: "InheritanceUpcast",
            expected_java_output: "savings,105,savings:105",
            expectation: KnownGapExpectation::LoudCompileError {
                message: "unsupported Java class upcast: Savings to Account requires class hierarchy lowering",
                diagnostic_code: Some("unsupported_inheritance"),
            },
        },
        KnownGapFixture {
            display: "tests/differential/known-gaps/NestedInheritanceUpcast.java",
            class: "NestedInheritanceUpcast",
            expected_java_output: "woof,woofwoof",
            expectation: KnownGapExpectation::LoudCompileError {
                message: "unsupported Java class upcast: NestedInheritanceUpcast.Dog to NestedInheritanceUpcast.Animal requires class hierarchy lowering",
                diagnostic_code: Some("unsupported_inheritance"),
            },
        },
        KnownGapFixture {
            display: "tests/differential/known-gaps/InnerCapture.java",
            class: "InnerCapture",
            expected_java_output: "21",
            expectation: KnownGapExpectation::RuntimePanic {
                panic_message: "unsupported Java inner-class outer capture construction for InnerCapture.Adder; pass an outer InnerCapture handle",
                diagnostic_code: "inner_class_construction_panics",
            },
        },
        KnownGapFixture {
            display: "tests/differential/known-gaps/InstanceofSupertype.java",
            class: "InstanceofSupertype",
            expected_java_output: "true,true,false",
            expectation: KnownGapExpectation::LoudCompileError {
                message: "unsupported Java instanceof check: target type A has source subtypes and requires class hierarchy lowering",
                diagnostic_code: Some("unsupported_instanceof_source_hierarchy_runtime_carrier"),
            },
        },
        KnownGapFixture {
            display: "tests/differential/known-gaps/StringEqualsVsEq.java",
            class: "StringEqualsVsEq",
            expected_java_output: "true,false,true,true,false",
            expectation: KnownGapExpectation::LoudCompileError {
                message: "unsupported Java String reference identity comparison requires string identity representation",
                diagnostic_code: Some("java_string_identity"),
            },
        },
    ]
}

fn check_known_gap(fixture: &KnownGapFixture) -> Result<(), String> {
    let source_path = Path::new(env!("CARGO_MANIFEST_DIR")).join(fixture.display);
    let source = std::fs::read_to_string(&source_path).map_err(|err| err.to_string())?;

    let work = std::env::temp_dir().join(format!("new_java2rust_known_gap_{}", fixture.class));
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).map_err(|err| format!("temp dir: {err}"))?;
    let java_output = run_java(&work, &source_path, fixture.class);
    let java_output = match java_output {
        Ok(output) => output,
        Err(err) => {
            let _ = std::fs::remove_dir_all(&work);
            return Err(err);
        }
    };

    if java_output != fixture.expected_java_output {
        let _ = std::fs::remove_dir_all(&work);
        return Err(format!(
            "Java oracle changed: expected {:?}, got {:?}",
            fixture.expected_java_output, java_output
        ));
    }

    let translation = translate_source(&source);
    match fixture.expectation {
        KnownGapExpectation::LoudCompileError {
            message,
            diagnostic_code,
        } => {
            let expected = format!("compile_error!(\"{message}\")");
            if !translation.rust.contains(&expected) {
                let _ = std::fs::remove_dir_all(&work);
                return Err(format!(
                    "translation did not fail loudly with {expected:?}\n{}",
                    translation.rust
                ));
            }
            if let Some(diagnostic_code) = diagnostic_code
                && !translation.diagnostics.has_code(diagnostic_code)
            {
                let _ = std::fs::remove_dir_all(&work);
                return Err(format!(
                    "translation did not report diagnostic code {diagnostic_code:?}: {:?}",
                    translation.diagnostics.iter().collect::<Vec<_>>()
                ));
            }
        }
        KnownGapExpectation::DivergentOutput {
            expected_translated_output,
            diagnostic_code,
        } => {
            if !translation.diagnostics.has_code(diagnostic_code) {
                let _ = std::fs::remove_dir_all(&work);
                return Err(format!(
                    "translation did not report diagnostic code {diagnostic_code:?}: {:?}",
                    translation.diagnostics.iter().collect::<Vec<_>>()
                ));
            }
            let translated_output =
                match run_translated_rust_source(&work, fixture.class, &translation.rust) {
                    Ok(output) => output,
                    Err(err) => {
                        let _ = std::fs::remove_dir_all(&work);
                        return Err(err);
                    }
                };
            if translated_output != expected_translated_output {
                let _ = std::fs::remove_dir_all(&work);
                return Err(format!(
                    "known divergent translation changed: expected {:?}, got {:?}",
                    expected_translated_output, translated_output
                ));
            }
            if translated_output == java_output {
                let _ = std::fs::remove_dir_all(&work);
                return Err("known divergent translation unexpectedly matched Java".to_string());
            }
        }
        KnownGapExpectation::RuntimePanic {
            panic_message,
            diagnostic_code,
        } => {
            if !translation.diagnostics.has_code(diagnostic_code) {
                let _ = std::fs::remove_dir_all(&work);
                return Err(format!(
                    "translation did not report diagnostic code {diagnostic_code:?}: {:?}",
                    translation.diagnostics.iter().collect::<Vec<_>>()
                ));
            }
            let panic_stderr = match run_translated_rust_source_expect_failure(
                &work,
                fixture.class,
                &translation.rust,
            ) {
                Ok(stderr) => stderr,
                Err(err) => {
                    let _ = std::fs::remove_dir_all(&work);
                    return Err(err);
                }
            };
            if !panic_stderr.contains(panic_message) {
                let _ = std::fs::remove_dir_all(&work);
                return Err(format!(
                    "translated program did not panic with {panic_message:?}: {panic_stderr}"
                ));
            }
        }
    }
    let _ = std::fs::remove_dir_all(&work);
    Ok(())
}

fn known_gap_fixture_paths() -> Vec<PathBuf> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/differential/known-gaps");
    let mut paths = std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .filter(|path| path.extension().is_some_and(|ext| ext == "java"))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    paths.sort();
    paths
}

fn run_translated_rust_source(work: &Path, class: &str, rust: &str) -> Result<String, String> {
    let binary = compile_translated_rust_source(work, class, rust)?;
    let run = Command::new(&binary)
        .output()
        .map_err(|err| format!("run: {err}"))?;
    if !run.status.success() {
        return Err(format!(
            "translated program failed: {}",
            String::from_utf8_lossy(&run.stderr).trim_end()
        ));
    }
    Ok(String::from_utf8_lossy(&run.stdout).trim_end().to_string())
}

fn run_translated_rust_source_expect_failure(
    work: &Path,
    class: &str,
    rust: &str,
) -> Result<String, String> {
    let binary = compile_translated_rust_source(work, class, rust)?;
    let run = Command::new(&binary)
        .output()
        .map_err(|err| format!("run: {err}"))?;
    if run.status.success() {
        return Err(format!(
            "translated program unexpectedly succeeded: {}",
            String::from_utf8_lossy(&run.stdout).trim_end()
        ));
    }
    Ok(String::from_utf8_lossy(&run.stderr).trim_end().to_string())
}

fn compile_translated_rust_source(work: &Path, class: &str, rust: &str) -> Result<PathBuf, String> {
    let mut rust = rust.to_string();
    rust.push_str(&harness_main(&rust, class));
    let rust_path = work.join(format!("{class}.rs"));
    std::fs::write(&rust_path, rust).map_err(|err| format!("write rust: {err}"))?;

    let binary = work.join(class);
    let compile = Command::new("rustc")
        .args(["--edition", "2021", "--crate-name"])
        .arg(class.to_lowercase())
        .arg(&rust_path)
        .arg("-o")
        .arg(&binary)
        .output()
        .map_err(|err| format!("rustc: {err}"))?;
    if !compile.status.success() {
        return Err(format!(
            "generated Rust did not compile: {}",
            String::from_utf8_lossy(&compile.stderr)
        ));
    }
    Ok(binary)
}

fn run_java(work: &Path, source: &Path, class: &str) -> Result<String, String> {
    let compile = Command::new("javac")
        .arg("-d")
        .arg(work)
        .arg(source)
        .output()
        .map_err(|err| format!("javac: {err}"))?;
    if !compile.status.success() {
        return Err(format!(
            "javac failed: {}",
            String::from_utf8_lossy(&compile.stderr)
        ));
    }
    let run = Command::new("java")
        .arg("-cp")
        .arg(work)
        .arg(class)
        .output()
        .map_err(|err| format!("java: {err}"))?;
    if !run.status.success() {
        return Err(format!(
            "java failed: {}",
            String::from_utf8_lossy(&run.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&run.stdout).trim_end().to_string())
}

fn run_translated(
    translator: &Path,
    work: &Path,
    source: &Path,
    class: &str,
) -> Result<String, String> {
    let translated = Command::new(translator)
        .arg(source)
        .output()
        .map_err(|err| format!("translator: {err}"))?;
    if !translated.status.success() {
        return Err(format!(
            "translation failed: {}",
            String::from_utf8_lossy(&translated.stderr)
        ));
    }
    let mut rust = String::from_utf8_lossy(&translated.stdout).into_owned();
    rust.push_str(&harness_main(&rust, class));

    let rust_path = work.join(format!("{class}.rs"));
    std::fs::write(&rust_path, &rust).map_err(|err| format!("write rust: {err}"))?;

    let binary = work.join(class);
    let compile = Command::new("rustc")
        .args(["--edition", "2021", "--crate-name"])
        .arg(class.to_lowercase())
        .arg(&rust_path)
        .arg("-o")
        .arg(&binary)
        .output()
        .map_err(|err| format!("rustc: {err}"))?;
    if !compile.status.success() {
        let stderr = String::from_utf8_lossy(&compile.stderr);
        let errors = stderr
            .lines()
            .filter(|line| line.starts_with("error"))
            .take(5)
            .collect::<Vec<_>>()
            .join("; ");
        return Err(format!("generated Rust did not compile: {errors}"));
    }
    let run = Command::new(&binary)
        .output()
        .map_err(|err| format!("run: {err}"))?;
    if !run.status.success() {
        return Err(format!(
            "translated program failed: {}",
            String::from_utf8_lossy(&run.stderr).trim_end()
        ));
    }
    Ok(String::from_utf8_lossy(&run.stdout).trim_end().to_string())
}

/// `run()` is lowered as fallible whenever the analysis says it can throw, so
/// the harness has to accept both shapes.
fn harness_main(rust: &str, class: &str) -> String {
    let fallible = rust.contains(&format!("pub fn run() -> Result<"));
    if fallible {
        format!(
            "\nfn main() {{\n    match {class}::run() {{\n        Ok(value) => println!(\"{{value}}\"),\n        Err(err) => println!(\"EXCEPTION:{{}}\", err.class_name),\n    }}\n}}\n"
        )
    } else {
        format!("\nfn main() {{\n    println!(\"{{}}\", {class}::run());\n}}\n")
    }
}

fn fixture_paths() -> Vec<PathBuf> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/differential");
    let mut paths = std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .filter(|path| path.extension().is_some_and(|ext| ext == "java"))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    paths.sort();
    paths
}

fn translator_binary() -> Option<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    ["release", "debug"]
        .iter()
        .map(|profile| root.join("target").join(profile).join("new-java2rust"))
        .find(|path| path.exists())
}

fn have_tool(tool: &str) -> bool {
    Command::new(tool)
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success())
}
