mod common;

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use common::run_with_watchdog;
use new_java2rust::fixtures::load_java_fixtures;
use new_java2rust::translate_source;

const EXPECTED_FAILURES: &str = include_str!("baselines/corpus-compile-failures.txt");

#[test]
fn checked_in_example_corpora_have_named_compile_failures() {
    assert_tool_on_path("rustc");

    let baseline = parse_baseline(EXPECTED_FAILURES);
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!("new_java2rust_corpus_compile_{unique}"));
    fs::create_dir_all(&temp_dir).unwrap();

    let fixtures = Arc::new(load_corpus_fixtures());
    let jobs = corpus_compile_jobs().min(fixtures.len().max(1));
    let next = Arc::new(AtomicUsize::new(0));
    let (tx, rx) = mpsc::channel();

    std::thread::scope(|scope| {
        for _ in 0..jobs {
            let fixtures = Arc::clone(&fixtures);
            let next = Arc::clone(&next);
            let temp_dir = temp_dir.clone();
            let tx = tx.clone();
            scope.spawn(move || {
                loop {
                    let index = next.fetch_add(1, Ordering::Relaxed);
                    if index >= fixtures.len() {
                        break;
                    }
                    let fixture = &fixtures[index];
                    tx.send(compile_corpus_fixture(fixture, &temp_dir)).unwrap();
                }
            });
        }
    });
    drop(tx);

    let mut actual = BTreeSet::new();
    let mut details = BTreeMap::new();
    for (display_path, maybe_error) in rx {
        if let Some(error) = maybe_error {
            actual.insert(display_path.clone());
            details.insert(display_path, error);
        }
    }

    if actual != baseline {
        keep_rust_sources_drop_metadata(&temp_dir);
        panic!(
            "corpus compile failure baseline changed; generated Rust kept at {}\n{}",
            temp_dir.display(),
            format_baseline_delta(&baseline, &actual, &details)
        );
    }

    let _ = fs::remove_dir_all(&temp_dir);
}

struct CorpusFixture {
    display_path: String,
    source: String,
}

fn load_corpus_fixtures() -> Vec<CorpusFixture> {
    let mut fixtures = Vec::new();
    for corpus_root in ["examples/java", "examples/idiom-gaps"] {
        for fixture in load_java_fixtures(corpus_root).unwrap() {
            let display_path = format!(
                "{}/{}",
                corpus_root.strip_prefix("examples/").unwrap(),
                fixture.relative_path.display()
            );
            fixtures.push(CorpusFixture {
                display_path,
                source: fixture.source,
            });
        }
    }
    fixtures.sort_by(|left, right| left.display_path.cmp(&right.display_path));
    fixtures
}

fn compile_corpus_fixture(fixture: &CorpusFixture, temp_dir: &Path) -> (String, Option<String>) {
    let translation = translate_source(&fixture.source);
    let rust_path = temp_dir.join(format!("{}.rs", sanitize_path(&fixture.display_path)));
    let meta_path = rust_path.with_extension("rmeta");
    fs::write(&rust_path, translation.rust).unwrap();

    let mut command = Command::new("rustc");
    command
        .arg("--edition")
        .arg("2021")
        .arg("--crate-type")
        .arg("lib")
        .arg("--emit=metadata")
        .arg("--crate-name")
        .arg("corpus_fixture")
        .arg(&rust_path)
        .arg("-o")
        .arg(&meta_path);
    let output = run_with_watchdog(&mut command, Duration::from_secs(30));
    let error = output
        .status
        .map_or(true, |status| !status.success())
        .then(|| {
            format!(
                "{}{}",
                if output.status.is_none() {
                    "rustc timed out after 30s\n"
                } else {
                    ""
                },
                first_diagnostic(&output.stderr)
            )
        });
    (fixture.display_path.clone(), error)
}

fn corpus_compile_jobs() -> usize {
    std::env::var("NEW_JAVA2RUST_CORPUS_COMPILE_JOBS")
        .ok()
        .and_then(|raw| raw.parse::<usize>().ok())
        .filter(|jobs| *jobs > 0)
        .unwrap_or_else(|| std::thread::available_parallelism().map_or(4, |jobs| jobs.get().min(4)))
}

fn parse_baseline(raw: &str) -> BTreeSet<String> {
    raw.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(str::to_owned)
        .collect()
}

fn sanitize_path(path: &str) -> String {
    path.chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' })
        .collect()
}

fn first_diagnostic(stderr: &str) -> String {
    stderr.lines().take(12).collect::<Vec<_>>().join("\n")
}

fn format_baseline_delta(
    baseline: &BTreeSet<String>,
    actual: &BTreeSet<String>,
    details: &BTreeMap<String, String>,
) -> String {
    let mut message = String::new();
    let added: Vec<_> = actual.difference(baseline).collect();
    let removed: Vec<_> = baseline.difference(actual).collect();

    if !added.is_empty() {
        message.push_str("new compile failures:\n");
        for path in added {
            message.push_str(&format!("- {path}\n{}\n", details[path]));
        }
    }
    if !removed.is_empty() {
        message.push_str("compile failures fixed; remove these baseline entries:\n");
        for path in removed {
            message.push_str(&format!("- {path}\n"));
        }
    }
    message
}

fn keep_rust_sources_drop_metadata(temp_dir: &Path) {
    if let Ok(entries) = fs::read_dir(temp_dir) {
        for entry in entries.flatten() {
            let path: PathBuf = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) == Some("rmeta") {
                let _ = fs::remove_file(path);
            }
        }
    }
}

fn assert_tool_on_path(tool: &str) {
    let output = Command::new(tool)
        .arg("--version")
        .output()
        .unwrap_or_else(|err| {
            panic!("{tool} is required for corpus compile checks but could not be run: {err}")
        });
    assert!(
        output.status.success(),
        "{tool} --version failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
