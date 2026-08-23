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

use std::path::{Path, PathBuf};
use std::process::Command;

const EXPECTED_DIFFERENTIAL_FIXTURES: usize = 23;

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
