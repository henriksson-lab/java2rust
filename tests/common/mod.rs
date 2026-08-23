//! Shared test support for verifying that generated Rust actually compiles.
//!
//! Lived in `tests/idiom_gaps.rs` only, which is why `slot_shape_design` -- 596
//! tests, many asserting on rendered types -- had **no** generated-crate check
//! at all and could not tell a stale expectation from a type error. That is the
//! blind spot claude_audits/0018 describes, inside the suite itself.

#![allow(dead_code)]

use std::fs;
use std::io::Read;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

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
            "[package]\nname = \"new-java2rust-{crate_name}-{unique}\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\n[lib]\npath = \"src/lib.rs\"\n"
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

    let Some(status) = output.status else {
        keep_sources_discard_build_artifacts(&crate_dir);
        panic!(
            "generated crate {cargo_command} for {crate_name} did not finish within {}s and was killed; \
             crate sources kept at {}",
            timeout.as_secs(),
            crate_dir.display()
        );
    };

    if !status.success() {
        keep_sources_discard_build_artifacts(&crate_dir);
        panic!(
            "generated crate {cargo_command} failed for {crate_name}\nstdout:\n{}\nstderr:\n{}",
            output.stdout, output.stderr
        );
    }

    let _ = fs::remove_dir_all(&crate_dir);
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
