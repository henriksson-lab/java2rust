pub mod access_lowering;
pub mod array_lowering;
pub mod array_plan;
pub mod call_lowering;
pub mod call_plan;
pub mod coercion;
pub mod constructor_plan;
pub mod control_flow_lowering;
pub mod conversion_lowering;
pub mod conversion_plan;
pub mod dependency_mode;
pub mod diagnostics;
pub mod effective_facts;
pub mod emit;
pub mod exception_lowering;
pub mod exception_plan;
pub mod expr_render;
pub mod expr_shape;
pub mod external_facts;
pub mod external_packs;
pub mod fact_saturation;
pub mod facts;
pub mod fixtures;
pub mod graph;
pub mod inherited_members;
pub mod item_assembly;
pub mod java_ir;
pub mod lattice;
pub mod literal_lowering;
pub mod loop_lowering;
pub mod loop_plan;
pub mod lower;
pub mod lowering_diagnostics;
pub mod member_access_plan;
pub mod method_reference_lowering;
pub mod method_reference_plan;
pub mod nullability_plan;
pub mod object_model;
pub mod operator_lowering;
pub mod parser;
pub mod representation;
pub mod representation_lowering;
pub mod resolve;
pub mod resolver_benchmark;
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
use java_ir::Program;

pub struct Translation {
    pub rust: String,
    pub diagnostics: DiagnosticBag,
}

pub fn translate_source(source: &str) -> Translation {
    translate_source_with_facts(source, &FactSet::default())
}

pub fn translate_source_with_facts(source: &str, external_facts: &FactSet) -> Translation {
    let parsed = parser::parse_java_source(source);
    translate_parsed_source_with_context(parsed, None, None, None, external_facts)
}

pub fn translate_source_with_context_sources(
    source: &str,
    context_sources: &[String],
    external_facts: &FactSet,
) -> Translation {
    translate_source_with_context_sources_and_summary_sources(
        source,
        context_sources,
        &[],
        external_facts,
    )
}

pub fn translate_source_with_context_summary_sources(
    source: &str,
    summary_sources: &[String],
    external_facts: &FactSet,
) -> Translation {
    translate_source_with_context_sources_and_summary_sources(
        source,
        &[],
        summary_sources,
        external_facts,
    )
}

pub fn translate_source_with_context_sources_and_summary_sources(
    source: &str,
    context_sources: &[String],
    summary_sources: &[String],
    external_facts: &FactSet,
) -> Translation {
    let parsed = parser::parse_java_source(source);
    let mut source_program = parsed.program.clone();
    let mut semantic_program = parsed.program.clone();
    let mut summary_facts = FactSet::default();
    let mut context_diagnostics = DiagnosticBag::new();
    for context_source in context_sources {
        let context_parsed = parser::parse_java_source(context_source);
        context_diagnostics.extend(context_parsed.diagnostics);
        merge_program_context(&mut source_program, context_parsed.program.clone());
        merge_program_context(&mut semantic_program, context_parsed.program);
    }
    for summary_source in summary_sources {
        let summary_parsed = parser::parse_java_source(summary_source);
        context_diagnostics.extend(summary_parsed.diagnostics);
        summary_facts.extend_from(resolve::declaration_facts_from_program(
            &summary_parsed.program,
        ));
        merge_program_context(&mut source_program, summary_parsed.program);
    }
    let source_program_context =
        (!context_sources.is_empty() || !summary_sources.is_empty()).then_some(source_program);
    let semantic_program_context = (!context_sources.is_empty()).then_some(semantic_program);
    let support_program_context = source_program_context.clone();
    let mut effective_external_facts = external_facts.clone();
    effective_external_facts.extend_from(summary_facts);
    let mut translation = translate_parsed_source_with_context(
        parsed,
        source_program_context,
        semantic_program_context,
        support_program_context,
        &effective_external_facts,
    );
    translation.diagnostics.extend(context_diagnostics);
    translation
}

fn translate_parsed_source_with_context(
    parsed: parser::ParsedProgram,
    source_program_context: Option<Program>,
    semantic_program_context: Option<Program>,
    support_program_context: Option<Program>,
    external_facts: &FactSet,
) -> Translation {
    let source_program = source_program_context.as_ref().unwrap_or(&parsed.program);
    let resolved = resolve::resolve_program_with_source_program_and_facts(
        &parsed.program,
        parsed.diagnostics,
        source_program,
        external_facts,
    );
    let semantic_program = if let Some(semantic_program_context) = semantic_program_context {
        resolve::resolve_program_with_source_program_and_facts(
            &semantic_program_context,
            DiagnosticBag::new(),
            source_program,
            external_facts,
        )
        .program
    } else {
        resolved.program.clone()
    };
    let semantic =
        semantic_analysis::analyze_program(&semantic_program, &resolved.facts, external_facts);
    let representations = representation::plan_representations(&semantic_program, &semantic.facts);
    let support_program = support_program_context
        .as_ref()
        .unwrap_or(&semantic_program);
    let signatures = signature::solve_signatures_with_facts_and_support_context(
        &semantic_program,
        &representations,
        &semantic.usage,
        &semantic.facts,
        support_program,
    );
    let mut lowered = lower::lower_program_with_semantic_plans_and_support_context(
        &resolved.program,
        resolved.diagnostics,
        &semantic.facts,
        &semantic.usage,
        &representations,
        &signatures,
        &semantic.control_flow_plans,
        support_program,
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

fn merge_program_context(target: &mut Program, source: Program) {
    if target.package.is_none() {
        target.package = source.package;
    }
    for import in source.imports {
        if !target.imports.contains(&import) {
            target.imports.push(import);
        }
    }
    for ty in source.types {
        if !target.types.iter().any(|existing| existing.name == ty.name) {
            target.types.push(ty);
        }
    }
}
