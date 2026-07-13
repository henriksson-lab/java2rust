pub mod access_lowering;
pub mod call_lowering;
pub mod call_plan;
pub mod coercion;
pub mod constructor_plan;
pub mod conversion_lowering;
pub mod conversion_plan;
pub mod diagnostics;
pub mod emit;
pub mod exception_lowering;
pub mod exception_plan;
pub mod expr_render;
pub mod expr_shape;
pub mod external_facts;
pub mod fact_saturation;
pub mod facts;
pub mod fixtures;
pub mod item_assembly;
pub mod java_ir;
pub mod literal_lowering;
pub mod loop_lowering;
pub mod lower;
pub mod member_access_plan;
pub mod operator_lowering;
pub mod parser;
pub mod representation;
pub mod representation_lowering;
pub mod resolve;
pub mod rewrite;
pub mod rust_ir;
pub mod semantic_analysis;
pub mod shape_projection;
pub mod signature;
pub mod slot_policy;
pub mod slot_render;
pub mod slot_shape;
pub mod statement_render;
pub mod stdlib;
pub mod stream;
pub mod stream_lowering;
pub mod stream_shape;
pub mod string_concat;
pub mod string_concat_plan;
pub mod string_concat_render;
pub mod switch_lowering;
pub mod switch_plan;
pub mod types;
pub mod unsupported;
pub mod usage;

use diagnostics::DiagnosticBag;
use facts::FactSet;

pub struct Translation {
    pub rust: String,
    pub diagnostics: DiagnosticBag,
}

pub fn translate_source(source: &str) -> Translation {
    translate_source_with_facts(source, &FactSet::default())
}

pub fn translate_source_with_facts(source: &str, external_facts: &FactSet) -> Translation {
    let parsed = parser::parse_java_source(source);
    let resolved = resolve::resolve_program(&parsed.program, parsed.diagnostics);
    let semantic =
        semantic_analysis::analyze_program(&resolved.program, &resolved.facts, external_facts);
    let representations = representation::plan_representations(&resolved.program, &semantic.facts);
    let signatures = signature::solve_signatures_with_facts(
        &resolved.program,
        &representations,
        &semantic.usage,
        &semantic.facts,
    );
    let mut lowered = lower::lower_program_with_semantic_plans(
        &resolved.program,
        resolved.diagnostics,
        &semantic.facts,
        &semantic.usage,
        &representations,
        &signatures,
        &semantic.control_flow_plans,
    );
    for diagnostic in semantic.diagnostics {
        lowered
            .diagnostics
            .warning(diagnostic.code, diagnostic.message);
    }
    let rust = emit::emit_file(&lowered.file);
    Translation {
        rust,
        diagnostics: lowered.diagnostics,
    }
}
