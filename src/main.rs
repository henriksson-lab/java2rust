//! CLI entry point — port of `JavaConverter.main` / `convert2Rust(File, ...)`.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use java2rust_rs::convert_full;
use java2rust_rs::naming::camel_to_snake_case;
use java2rust_rs::stubs::{collect_defined_types, StubCollector};
use java2rust_rs::symbol_map::LinkIndex;

struct Options {
    input: Option<String>,
    output: String,
    ignore_existing: bool,
    verbosity: i32,
    copy_other_files: bool,
    link: Vec<String>,
    stubs: bool,
}

fn parse_args(args: &[String]) -> Result<Options, String> {
    let mut opts = Options {
        input: None,
        output: "output".to_string(),
        ignore_existing: false,
        verbosity: 2,
        copy_other_files: false,
        link: Vec::new(),
        stubs: false,
    };
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-d" | "--input" => {
                i += 1;
                opts.input = Some(args.get(i).ok_or("missing value for -d")?.clone());
            }
            "-o" | "--output" => {
                i += 1;
                opts.output = args.get(i).ok_or("missing value for -o")?.clone();
            }
            "-i" | "--ignore-existing" => opts.ignore_existing = true,
            "-v" | "--verbosity" => {
                i += 1;
                opts.verbosity = args
                    .get(i)
                    .ok_or("missing value for -v")?
                    .parse()
                    .map_err(|_| "invalid verbosity".to_string())?;
            }
            "-cp" | "--copy-other-files" => opts.copy_other_files = true,
            "-l" | "--link" => {
                i += 1;
                opts.link.push(args.get(i).ok_or("missing value for -l")?.clone());
            }
            "-s" | "--stubs" => opts.stubs = true,
            other => return Err(format!("unknown option: {other}")),
        }
        i += 1;
    }
    Ok(opts)
}

const EXTENSION: &str = ".rs";

/// Shared, mostly read-only state for a conversion run; `stubs` accumulates
/// unresolved external symbols across the whole tree (interior-mutable so the
/// recursion doesn't thread a `&mut`).
struct Ctx<'a> {
    link: &'a LinkIndex,
    known: &'a HashSet<String>,
    emit_stubs: bool,
    ignore_existing: bool,
    verbosity: i32,
    copy_other_files: bool,
    stubs: std::cell::RefCell<StubCollector>,
}

/// Port of `JavaConverter.convert2Rust(File, outputDir, ...)`.
fn convert_to_rust(file: &Path, output_dir: &str, ctx: &Ctx) -> std::io::Result<()> {
    let file_dir = Path::new(output_dir);
    if !file_dir.exists() {
        fs::create_dir_all(file_dir)?;
    }

    let mut output = format!("{output_dir}/");

    if !file.exists() {
        eprintln!("\nThe file does not exist!");
        return Ok(());
    }

    if file.is_dir() {
        let name = file.file_name().unwrap_or_default().to_string_lossy();
        output.push_str(&name);
        let mut entries: Vec<PathBuf> = fs::read_dir(file)?.filter_map(|e| e.ok().map(|e| e.path())).collect();
        entries.sort();
        for entry in entries {
            convert_to_rust(&entry, &output, ctx)?;
        }
        return Ok(());
    }

    let file_name = file.file_name().unwrap().to_string_lossy().into_owned();
    let parts: Vec<&str> = file_name.split('.').collect();
    if parts[parts.len() - 1] == "java" {
        output.push_str(&camel_to_snake_case(parts[0]));
        output.push_str(EXTENSION);
        let out_path = Path::new(&output);
        if !ctx.ignore_existing || !out_path.exists() {
            let text = fs::read_to_string(file)?;
            if ctx.verbosity > 0 {
                println!("- {output}");
            }
            let (result, file_stubs) =
                convert_full(&text, ctx.link, ctx.known, ctx.emit_stubs);
            if ctx.emit_stubs {
                ctx.stubs.borrow_mut().merge(file_stubs);
            }
            fs::write(out_path, result)?;
        } else if ctx.verbosity > 1 {
            println!("- {output} (ignored) because it already exists");
        }
    } else if ctx.copy_other_files {
        output.push_str(&file_name);
        let out_path = Path::new(&output);
        if !ctx.ignore_existing || !out_path.exists() {
            fs::copy(file, out_path)?;
            if ctx.verbosity > 0 {
                println!("- {output}");
            }
        } else if ctx.verbosity > 1 {
            println!("- {output} (ignored) because it already exists");
        }
    }
    Ok(())
}

/// Recursively gather the FQNs of every type defined under `path` (for stub
/// cross-file dedup).
fn gather_known_types(path: &Path, into: &mut HashSet<String>) {
    if path.is_dir() {
        if let Ok(rd) = fs::read_dir(path) {
            let mut entries: Vec<PathBuf> = rd.filter_map(|e| e.ok().map(|e| e.path())).collect();
            entries.sort();
            for e in entries {
                gather_known_types(&e, into);
            }
        }
    } else if path.extension().map(|e| e == "java").unwrap_or(false) {
        if let Ok(text) = fs::read_to_string(path) {
            collect_defined_types(&text, into);
        }
    }
}

fn print_help() {
    eprintln!("usage: java2rust-rs -d <input file|dir> [options]");
    eprintln!("  -d,  --input <path>       input file or directory");
    eprintln!("  -o,  --output <dir>       output directory (default: output)");
    eprintln!("  -i,  --ignore-existing    skip files already present in output");
    eprintln!("  -v,  --verbosity <n>      verbosity level (default: 2)");
    eprintln!("  -cp, --copy-other-files   copy non-java files to output");
    eprintln!("  -l,  --link <map.json>    link against a dependency symbol map (repeatable)");
    eprintln!("  -s,  --stubs              emit <output>/stubs.rs for unresolved external symbols");
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let opts = match parse_args(&args) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("{e}");
            print_help();
            std::process::exit(1);
        }
    };

    let mut link = LinkIndex::default();
    for path in &opts.link {
        if let Err(e) = link.load(Path::new(path)) {
            eprintln!("error loading link map: {e}");
            std::process::exit(1);
        }
    }

    match opts.input {
        Some(ref input) => {
            // Pre-pass: which types are defined in this tree (so cross-file refs
            // aren't recorded as missing externals).
            let mut known = HashSet::new();
            if opts.stubs {
                gather_known_types(Path::new(input), &mut known);
            }
            let ctx = Ctx {
                link: &link,
                known: &known,
                emit_stubs: opts.stubs,
                ignore_existing: opts.ignore_existing,
                verbosity: opts.verbosity,
                copy_other_files: opts.copy_other_files,
                stubs: std::cell::RefCell::new(StubCollector::default()),
            };
            if let Err(e) = convert_to_rust(Path::new(input), &opts.output, &ctx) {
                eprintln!("error: {e}");
                std::process::exit(1);
            }
            if opts.stubs {
                let collected = ctx.stubs.into_inner();
                if !collected.is_empty() {
                    let path = format!("{}/stubs.rs", opts.output);
                    if let Err(e) = fs::write(&path, collected.render()) {
                        eprintln!("error writing stubs: {e}");
                        std::process::exit(1);
                    }
                    if opts.verbosity > 0 {
                        println!("- {path}");
                    }
                }
            }
        }
        None => print_help(),
    }
}
