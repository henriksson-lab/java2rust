//! Corpus checker: prints PASS/FAIL per case, and a diff for `FOCUS=<name>`.
use std::fs;
use std::path::Path;

fn main() {
    let focus = std::env::var("FOCUS").ok();
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/corpus");
    let mut names: Vec<String> = fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            e.file_name()
                .to_string_lossy()
                .strip_suffix(".rs.expected")
                .map(|s| s.to_string())
        })
        .collect();
    names.sort();
    let mut pass = 0;
    for name in &names {
        if let Some(f) = &focus {
            if name != f {
                continue;
            }
        }
        let input = fs::read_to_string(dir.join(format!("{name}.java"))).unwrap();
        let expected = fs::read_to_string(dir.join(format!("{name}.rs.expected"))).unwrap();
        let got = std::panic::catch_unwind(|| java2rust_rs::convert(&input))
            .unwrap_or_else(|_| "<PANIC>".to_string());
        if got == expected {
            pass += 1;
            println!("PASS {name}");
        } else {
            println!("FAIL {name}");
            if focus.is_some() {
                println!("--- expected ---\n{expected}\n--- got ---\n{got}\n--- end ---");
            }
        }
    }
    eprintln!("{pass}/{} passing", names.len());
}
