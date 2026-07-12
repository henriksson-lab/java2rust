pub mod coercion;
pub mod diagnostics;
pub mod emit;
pub mod facts;
pub mod fixtures;
pub mod java_ir;
pub mod lower;
pub mod parser;
pub mod representation;
pub mod resolve;
pub mod rewrite;
pub mod rust_ir;
pub mod signature;
pub mod slot_shape;
pub mod stdlib;
pub mod types;
pub mod usage;

use diagnostics::DiagnosticBag;

pub struct Translation {
    pub rust: String,
    pub diagnostics: DiagnosticBag,
}

pub fn translate_source(source: &str) -> Translation {
    let parsed = parser::parse_java_source(source);
    let resolved = resolve::resolve_program(&parsed.program, parsed.diagnostics);
    let usage = usage::analyze_usage(&resolved.program);
    let representations = representation::plan_representations(&resolved.program, &resolved.facts);
    let signatures = signature::solve_signatures(&resolved.program, &representations, &usage);
    let lowered = lower::lower_program_with_plans(
        &resolved.program,
        resolved.diagnostics,
        &resolved.facts,
        &usage,
        &representations,
        &signatures,
    );
    let rust = emit::emit_file(&lowered.file);
    Translation {
        rust,
        diagnostics: lowered.diagnostics,
    }
}
