//! CLI entry point — port of `JavaConverter.main` / `convert2Rust(File, ...)`.

use std::fs;
use std::path::{Path, PathBuf};

use java2rust_rs::convert;
use java2rust_rs::naming::camel_to_snake_case;

struct Options {
    input: Option<String>,
    output: String,
    ignore_existing: bool,
    verbosity: i32,
    copy_other_files: bool,
}

fn parse_args(args: &[String]) -> Result<Options, String> {
    let mut opts = Options {
        input: None,
        output: "output".to_string(),
        ignore_existing: false,
        verbosity: 2,
        copy_other_files: false,
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
            other => return Err(format!("unknown option: {other}")),
        }
        i += 1;
    }
    Ok(opts)
}

const EXTENSION: &str = ".rs";

/// Port of `JavaConverter.convert2Rust(File, outputDir, ...)`.
fn convert_to_rust(
    file: &Path,
    output_dir: &str,
    ignore_existing: bool,
    verbosity: i32,
    copy_other_files: bool,
) -> std::io::Result<()> {
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
            convert_to_rust(&entry, &output, ignore_existing, verbosity, copy_other_files)?;
        }
        return Ok(());
    }

    let file_name = file.file_name().unwrap().to_string_lossy().into_owned();
    let parts: Vec<&str> = file_name.split('.').collect();
    if parts[parts.len() - 1] == "java" {
        output.push_str(&camel_to_snake_case(parts[0]));
        output.push_str(EXTENSION);
        let out_path = Path::new(&output);
        if !ignore_existing || !out_path.exists() {
            let text = fs::read_to_string(file)?;
            if verbosity > 0 {
                println!("- {output}");
            }
            let result = convert(&text);
            fs::write(out_path, result)?;
        } else if verbosity > 1 {
            println!("- {output} (ignored) because it already exists");
        }
    } else if copy_other_files {
        output.push_str(&file_name);
        let out_path = Path::new(&output);
        if !ignore_existing || !out_path.exists() {
            fs::copy(file, out_path)?;
            if verbosity > 0 {
                println!("- {output}");
            }
        } else if verbosity > 1 {
            println!("- {output} (ignored) because it already exists");
        }
    }
    Ok(())
}

fn print_help() {
    eprintln!("usage: java2rust-rs -d <input file|dir> [options]");
    eprintln!("  -d,  --input <path>       input file or directory");
    eprintln!("  -o,  --output <dir>       output directory (default: output)");
    eprintln!("  -i,  --ignore-existing    skip files already present in output");
    eprintln!("  -v,  --verbosity <n>      verbosity level (default: 2)");
    eprintln!("  -cp, --copy-other-files   copy non-java files to output");
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

    match opts.input {
        Some(ref input) => {
            if let Err(e) = convert_to_rust(
                Path::new(input),
                &opts.output,
                opts.ignore_existing,
                opts.verbosity,
                opts.copy_other_files,
            ) {
                eprintln!("error: {e}");
                std::process::exit(1);
            }
        }
        None => print_help(),
    }
}
