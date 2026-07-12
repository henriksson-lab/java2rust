use std::env;
use std::fs;
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut args = env::args().skip(1);
    let Some(path) = args.next() else {
        eprintln!("usage: new-java2rust <file.java>");
        return ExitCode::from(2);
    };

    let source = match fs::read_to_string(&path) {
        Ok(source) => source,
        Err(err) => {
            eprintln!("failed to read {path}: {err}");
            return ExitCode::from(1);
        }
    };

    let translation = new_java2rust::translate_source(&source);
    for diagnostic in translation.diagnostics.iter() {
        eprintln!(
            "{:?} {}: {}",
            diagnostic.severity, diagnostic.code, diagnostic.message
        );
    }
    println!("{}", translation.rust);
    ExitCode::SUCCESS
}
