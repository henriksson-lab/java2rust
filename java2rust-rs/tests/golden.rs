//! Golden conformance tests.
//!
//! For every `tests/corpus/<name>.rs.expected` we read `<name>.java`, run it
//! through `java2rust_rs::convert`, and assert byte-identical output to what the
//! original `java2rust.jar` produced.
//!
//! Regenerate fixtures with `tools/gen_golden.sh`.

use std::fs;
use std::path::Path;

fn corpus_dir() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/corpus")
}

/// Returns (name, input, expected) for every fixture pair, sorted by name.
fn cases() -> Vec<(String, String, String)> {
    let dir = corpus_dir();
    let mut names: Vec<String> = fs::read_dir(&dir)
        .expect("read corpus dir")
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let n = e.file_name().to_string_lossy().into_owned();
            n.strip_suffix(".rs.expected").map(|s| s.to_string())
        })
        .collect();
    names.sort();
    names
        .into_iter()
        .map(|name| {
            let input = fs::read_to_string(dir.join(format!("{name}.java"))).unwrap();
            let expected = fs::read_to_string(dir.join(format!("{name}.rs.expected"))).unwrap();
            (name, input, expected)
        })
        .collect()
}

#[test]
fn golden_corpus_matches() {
    // Optionally focus on a single case: CASE=decl_field cargo test --test golden
    let filter = std::env::var("CASE").ok();
    // Silence panic backtraces from intentionally-unfinished adapter paths.
    std::panic::set_hook(Box::new(|_| {}));

    let mut failures = Vec::new();
    let mut passed = 0;
    let mut total = 0;
    for (name, input, expected) in cases() {
        if let Some(f) = &filter {
            if &name != f {
                continue;
            }
        }
        total += 1;
        let got = std::panic::catch_unwind(|| java2rust_rs::convert(&input))
            .unwrap_or_else(|e| {
                let msg = e
                    .downcast_ref::<String>()
                    .cloned()
                    .or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string()))
                    .unwrap_or_else(|| "<panic>".to_string());
                format!("<<PANIC: {msg}>>")
            });
        if got == expected {
            passed += 1;
        } else {
            failures.push((name, expected, got));
        }
    }

    eprintln!("golden: {passed}/{total} passing");
    if !failures.is_empty() {
        let n = failures.len();
        let mut msg = format!("\n{n}/{total} golden case(s) failed:\n");
        for (name, expected, got) in &failures {
            msg.push_str(&format!(
                "\n===== {name} =====\n--- expected ---\n{expected}\n--- got ---\n{got}\n"
            ));
        }
        panic!("{msg}");
    }
}
