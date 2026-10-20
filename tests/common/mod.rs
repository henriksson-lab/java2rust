//! Shared test support for verifying that generated Rust actually compiles.
//!
//! Lived in `tests/idiom_gaps.rs` only, which is why `slot_shape_design` -- 596
//! tests, many asserting on rendered types -- had **no** generated-crate check
//! at all and could not tell a stale expectation from a type error. That is the
//! blind spot claude_audits/0018 describes, inside the suite itself.

#![allow(dead_code)]

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Fixture {
    pub(crate) relative_path: PathBuf,
    pub(crate) source: String,
}

pub(crate) fn load_java_fixtures(root: impl AsRef<Path>) -> std::io::Result<Vec<Fixture>> {
    let root = root.as_ref();
    let mut files = Vec::new();
    collect_java_files(root, root, &mut files)?;
    files.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
    Ok(files)
}

fn collect_java_files(root: &Path, dir: &Path, out: &mut Vec<Fixture>) -> std::io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_java_files(root, &path, out)?;
        } else if path.extension().and_then(|e| e.to_str()) == Some("java") {
            out.push(Fixture {
                relative_path: path.strip_prefix(root).unwrap_or(&path).to_path_buf(),
                source: fs::read_to_string(&path)?,
            });
        }
    }
    Ok(())
}

pub struct GeneratedCrateModule {
    pub name: &'static str,
    pub rust: String,
    pub tests: &'static str,
}

pub(crate) fn cargo_check_generated_modules(crate_name: &str, modules: Vec<GeneratedCrateModule>) {
    cargo_verify_generated_modules(crate_name, modules, "check");
}

pub(crate) fn cargo_test_generated_modules(crate_name: &str, modules: Vec<GeneratedCrateModule>) {
    cargo_verify_generated_modules(crate_name, modules, "test");
}

/// Verify an actual multi-file translation through production crate assembly.
/// A peer emitted by the batch is not an opaque stand-in in each isolated file.
pub(crate) fn cargo_check_translated_batch_sources(
    name: &str,
    sources: &[new_java2rust::BatchSource],
) {
    verify_translated_batch_sources(name, sources, None, None);
}

pub(crate) fn cargo_run_translated_batch_sources(
    name: &str,
    sources: &[new_java2rust::BatchSource],
    probe_main: &str,
) {
    verify_translated_batch_sources(name, sources, Some(probe_main), None);
}

pub(crate) fn cargo_run_translated_batch_sources_with_facts(
    name: &str,
    sources: &[new_java2rust::BatchSource],
    probe_main: &str,
    facts: &str,
) {
    verify_translated_batch_sources(name, sources, Some(probe_main), Some(facts));
}

fn verify_translated_batch_sources(
    name: &str,
    sources: &[new_java2rust::BatchSource],
    probe_main: Option<&str>,
    facts: Option<&str>,
) {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("new_java2rust_batch_{name}_{unique}"));
    let primary = dir.join("java");
    for source in sources {
        let relative = Path::new(source.path());
        assert!(
            relative.is_relative()
                && relative
                    .components()
                    .all(|part| matches!(part, std::path::Component::Normal(_)))
        );
        let path = primary.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, source.source()).unwrap();
    }
    let out = dir.join("out");
    let mut command = Command::new(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/translate-focused-package.sh"),
    );
    if let Some(facts) = facts {
        let facts_path = dir.join("external.facts");
        fs::write(&facts_path, facts).unwrap();
        command.arg("--facts").arg(facts_path);
    }
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
    let output = run_with_watchdog(&mut command, generated_crate_verify_timeout());
    assert!(
        output.status.is_some_and(|status| status.success()),
        "{}\n{}\nfixture: {}",
        output.stdout,
        output.stderr,
        dir.display()
    );
    if let Some(main) = probe_main {
        // A separate consumer checks behavior without changing generated Rust.
        let probe = dir.join("probe");
        fs::create_dir_all(probe.join("src")).unwrap();
        fs::write(probe.join("Cargo.toml"), format!(
            "[package]\nname=\"batch-behavior-probe\"\nversion=\"0.0.0\"\nedition=\"2024\"\n[dependencies]\ntranslated={{package=\"new-java2rust-focused-output\",path={:?}}}\n",
            out.join("generated-crate").to_str().unwrap(),
        )).unwrap();
        fs::write(probe.join("src/main.rs"), main).unwrap();
        let mut command = Command::new(env!("CARGO"));
        command
            .args(["run", "--quiet", "--offline"])
            .current_dir(&probe)
            .env("CARGO_TARGET_DIR", probe.join("target"))
            .env_remove("CARGO_MAKEFLAGS")
            .env_remove("MAKEFLAGS");
        let output = run_with_watchdog(&mut command, generated_crate_verify_timeout());
        assert!(
            output.status.is_some_and(|status| status.success()),
            "{}\n{}\nfixture: {}",
            output.stdout,
            output.stderr,
            dir.display()
        );
    }
    fs::remove_dir_all(dir).unwrap();
}

pub(crate) fn cargo_check_generated_modules_fails_with(
    crate_name: &str,
    modules: Vec<GeneratedCrateModule>,
    expected_stderr: &str,
) {
    let run = cargo_run_generated_modules(crate_name, modules, "check");
    let Some(status) = run.output.status else {
        panic!("generated crate check for {crate_name} timed out before producing diagnostics");
    };
    assert!(
        !status.success(),
        "generated crate check for {crate_name} unexpectedly succeeded"
    );
    assert!(
        run.output.stderr.contains(expected_stderr),
        "generated crate check for {crate_name} did not include expected diagnostic {expected_stderr:?}\nstderr:\n{}",
        run.output.stderr
    );
    let _ = fs::remove_dir_all(&run.crate_dir);
}

/// How long a nested `cargo` invocation may run before its test fails.
///
/// A nested cargo that never returns is indistinguishable, to anything waiting
/// on this suite, from one that passed — see
/// `claude_audits/0035-idiom-gaps-suite-never-terminates.md`. The watchdog turns
/// a hang into a named failure so a baseline can be established at all.
pub(crate) const GENERATED_CRATE_VERIFY_TIMEOUT: Duration = Duration::from_secs(300);

pub(crate) fn generated_crate_verify_timeout() -> Duration {
    std::env::var("NEW_JAVA2RUST_GENERATED_CRATE_TIMEOUT_SECS")
        .ok()
        .and_then(|raw| raw.parse::<u64>().ok())
        .map(Duration::from_secs)
        .unwrap_or(GENERATED_CRATE_VERIFY_TIMEOUT)
}

/// Outcome of a watched child process. `status` is `None` when the watchdog
/// fired and the child was killed.
pub(crate) struct WatchedOutput {
    pub(crate) status: Option<std::process::ExitStatus>,
    pub(crate) stdout: String,
    pub(crate) stderr: String,
}

pub(crate) fn run_with_watchdog(command: &mut Command, timeout: Duration) -> WatchedOutput {
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    // Drain both pipes on their own threads: a child that fills a pipe buffer
    // while we are waiting on the other one would deadlock regardless of the
    // watchdog.
    let mut stdout_pipe = child.stdout.take().unwrap();
    let mut stderr_pipe = child.stderr.take().unwrap();
    let stdout_reader = std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = stdout_pipe.read_to_end(&mut buf);
        buf
    });
    let stderr_reader = std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = stderr_pipe.read_to_end(&mut buf);
        buf
    });

    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait().unwrap() {
            Some(status) => break Some(status),
            None if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                break None;
            }
            None => std::thread::sleep(Duration::from_millis(25)),
        }
    };

    WatchedOutput {
        status,
        stdout: String::from_utf8_lossy(&stdout_reader.join().unwrap_or_default()).into_owned(),
        stderr: String::from_utf8_lossy(&stderr_reader.join().unwrap_or_default()).into_owned(),
    }
}

pub(crate) fn cargo_verify_generated_modules(
    crate_name: &str,
    modules: Vec<GeneratedCrateModule>,
    cargo_command: &str,
) {
    cargo_verify_generated_modules_with_edition(crate_name, modules, cargo_command, "2024");
}

/// Exercise production-edition temporary lifetimes without changing the
/// existing suite's default edition or repairing any generated Rust.
pub(crate) fn cargo_verify_generated_modules_with_edition(
    crate_name: &str,
    modules: Vec<GeneratedCrateModule>,
    cargo_command: &str,
    edition: &str,
) {
    let run = cargo_run_generated_modules_with_edition(crate_name, modules, cargo_command, edition);
    let timeout = generated_crate_verify_timeout();
    let Some(status) = run.output.status else {
        panic!(
            "generated crate {cargo_command} for {crate_name} did not finish within {}s and was killed",
            timeout.as_secs()
        );
    };

    if !status.success() {
        keep_sources_discard_build_artifacts(&run.crate_dir);
        panic!(
            "generated crate {cargo_command} failed for {crate_name}\nstdout:\n{}\nstderr:\n{}",
            run.output.stdout, run.output.stderr
        );
    }
    let _ = fs::remove_dir_all(&run.crate_dir);
}

struct GeneratedCrateRun {
    crate_dir: PathBuf,
    output: WatchedOutput,
}

fn cargo_run_generated_modules(
    crate_name: &str,
    modules: Vec<GeneratedCrateModule>,
    cargo_command: &str,
) -> GeneratedCrateRun {
    cargo_run_generated_modules_with_edition(crate_name, modules, cargo_command, "2024")
}

fn cargo_run_generated_modules_with_edition(
    crate_name: &str,
    modules: Vec<GeneratedCrateModule>,
    cargo_command: &str,
    edition: &str,
) -> GeneratedCrateRun {
    assert!(matches!(edition, "2021" | "2024"));
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let crate_dir = std::env::temp_dir().join(format!("new_java2rust_{crate_name}_{unique}"));
    let src_dir = crate_dir.join("src");
    fs::create_dir_all(&src_dir).unwrap();
    fs::write(
        crate_dir.join("Cargo.toml"),
        format!(
            "[package]\nname = \"new-java2rust-{crate_name}-{unique}\"\nversion = \"0.0.0\"\nedition = \"{edition}\"\n\n[lib]\npath = \"src/lib.rs\"\n"
        ),
    )
    .unwrap();
    let mut lib_rs = String::new();
    for module in modules {
        lib_rs.push_str(&format!(
            "pub mod {} {{\n{}\n{}\n}}\n",
            module.name, module.rust, module.tests
        ));
    }
    fs::write(src_dir.join("lib.rs"), lib_rs).unwrap();

    let mut command = Command::new(env!("CARGO"));
    command
        .arg(cargo_command)
        .arg("--tests")
        .arg("--quiet")
        .current_dir(&crate_dir)
        // A nested cargo that inherits the outer run's jobserver blocks
        // indefinitely waiting for a token the parent will never hand out, and
        // an inherited CARGO_TARGET_DIR would make every generated crate
        // contend on one build lock. Both are hangs, not failures.
        .env_remove("CARGO_MAKEFLAGS")
        .env_remove("MAKEFLAGS")
        .env("CARGO_TARGET_DIR", crate_dir.join("target"));

    let timeout = generated_crate_verify_timeout();
    let output = run_with_watchdog(&mut command, timeout);

    if output.status.is_none() {
        keep_sources_discard_build_artifacts(&crate_dir);
        panic!(
            "generated crate {cargo_command} for {crate_name} did not finish within {}s and was killed; \
             crate sources kept at {}",
            timeout.as_secs(),
            crate_dir.display()
        );
    };

    GeneratedCrateRun { crate_dir, output }
}

/// Keep a failed crate's `Cargo.toml` and `src/`, drop its `target/`.
///
/// A failing crate is kept so it can be inspected, which is right — but it was
/// kept *whole*, and `target/` is ~99% of its bytes and entirely regenerable by
/// re-running `cargo` in the directory. With ~146 failures a run and no upper
/// bound on runs, the leftovers reached **10,525 directories / 31 GB** and
/// filled the root filesystem. The suite then failed 404 of 684 tests for no
/// reason but disk, which is exactly the "a metric can silently report the wrong
/// thing" failure
/// [0018](../claude_audits/0018-the-compile-error-metric-cannot-see-silent-failures.md)
/// is about — and it had already perturbed one gate run by a single test before
/// it became obvious.
pub(crate) fn keep_sources_discard_build_artifacts(crate_dir: &std::path::Path) {
    let _ = fs::remove_dir_all(crate_dir.join("target"));
}
