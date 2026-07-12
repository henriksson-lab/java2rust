pub mod coercion;
pub mod diagnostics;
pub mod emit;
pub mod facts;
pub mod fixtures;
pub mod java_ir;
pub mod lower;
pub mod parser;
pub mod resolve;
pub mod rewrite;
pub mod rust_ir;
pub mod stdlib;
pub mod types;

use diagnostics::DiagnosticBag;

pub struct Translation {
    pub rust: String,
    pub diagnostics: DiagnosticBag,
}

pub fn translate_source(source: &str) -> Translation {
    let parsed = parser::parse_java_source(source);
    let resolved = resolve::resolve_program(&parsed.program, parsed.diagnostics);
    let lowered = lower::lower_program(&resolved.program, resolved.diagnostics);
    let rust = emit::emit_file(&lowered.file);
    Translation {
        rust,
        diagnostics: lowered.diagnostics,
    }
}
