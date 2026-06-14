//! Walk a directory of .java files; for each, write convert(content) to
//! <outDir>/<relpath>.rs. Panics are caught and recorded per file.
//! Usage: cargo run --example batch -- <inDir> <outDir>
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    let in_dir = PathBuf::from(std::env::args().nth(1).expect("inDir"));
    let out_dir = PathBuf::from(std::env::args().nth(2).expect("outDir"));
    std::panic::set_hook(Box::new(|_| {}));
    let mut files = Vec::new();
    collect(&in_dir, &mut files);
    files.sort();
    let mut n = 0;
    for p in &files {
        let text = fs::read_to_string(p).unwrap_or_default();
        let result = std::panic::catch_unwind(|| java2rust_rs::convert(&text)).unwrap_or_else(|e| {
            let msg = e
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string()))
                .unwrap_or_else(|| "panic".to_string());
            format!("<<RUST_PANIC: {msg}>>")
        });
        let rel = p.strip_prefix(&in_dir).unwrap();
        let out = out_dir.join(format!("{}.rs", rel.display()));
        fs::create_dir_all(out.parent().unwrap()).unwrap();
        fs::write(&out, result).unwrap();
        n += 1;
    }
    eprintln!("converted {n} files");
}

fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
    if let Ok(rd) = fs::read_dir(dir) {
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                collect(&p, out);
            } else if p.extension().map(|x| x == "java").unwrap_or(false) {
                out.push(p);
            }
        }
    }
}
