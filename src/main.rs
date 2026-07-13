use std::env;
use std::fs;
use std::process::ExitCode;

fn main() -> ExitCode {
    let Some(args) = parse_args(env::args().skip(1)) else {
        eprintln!("usage: new-java2rust [--facts facts.txt] <file.java>");
        return ExitCode::from(2);
    };

    let source = match fs::read_to_string(&args.source_path) {
        Ok(source) => source,
        Err(err) => {
            eprintln!("failed to read {}: {err}", args.source_path);
            return ExitCode::from(1);
        }
    };

    let mut fact_diagnostics = new_java2rust::diagnostics::DiagnosticBag::default();
    let external_facts = if let Some(facts_path) = &args.facts_path {
        let fact_source = match fs::read_to_string(facts_path) {
            Ok(source) => source,
            Err(err) => {
                eprintln!("failed to read {facts_path}: {err}");
                return ExitCode::from(1);
            }
        };
        let parsed_facts = new_java2rust::external_facts::parse_external_facts(&fact_source);
        fact_diagnostics = parsed_facts.diagnostics;
        parsed_facts.facts
    } else {
        new_java2rust::facts::FactSet::default()
    };

    let translation = new_java2rust::translate_source_with_facts(&source, &external_facts);
    for diagnostic in fact_diagnostics.iter() {
        eprintln!(
            "{:?} {}: {}",
            diagnostic.severity, diagnostic.code, diagnostic.message
        );
    }
    for diagnostic in translation.diagnostics.iter() {
        eprintln!(
            "{:?} {}: {}",
            diagnostic.severity, diagnostic.code, diagnostic.message
        );
    }
    println!("{}", translation.rust);
    ExitCode::SUCCESS
}

struct Args {
    source_path: String,
    facts_path: Option<String>,
}

fn parse_args(args: impl Iterator<Item = String>) -> Option<Args> {
    let mut source_path = None;
    let mut facts_path = None;
    let mut args = args.peekable();
    while let Some(arg) = args.next() {
        if arg == "--facts" {
            facts_path = args.next();
        } else if source_path.is_none() {
            source_path = Some(arg);
        } else {
            return None;
        }
    }
    Some(Args {
        source_path: source_path?,
        facts_path,
    })
}
