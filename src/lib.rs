pub mod access_lowering;
pub mod aliasing;
pub mod array_default_obligations;
pub mod array_lowering;
pub mod array_plan;
pub mod binding_id;
pub mod call_lowering;
pub mod call_plan;
pub mod callable_analysis;
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
pub mod fallibility_invariant;
pub mod fixtures;
pub mod graph;
pub mod inherited_members;
pub mod item_assembly;
pub mod java_ir;
pub mod java_naming;
pub mod java_signature;
pub mod javalibs;
pub mod lattice;
pub mod literal_lowering;
pub mod liveness;
pub mod loop_lowering;
pub mod loop_plan;
pub mod lower;
pub mod lowering_diagnostics;
pub mod member_access_plan;
pub mod method_reference_lowering;
pub mod method_reference_plan;
pub mod method_reference_shape;
pub mod nullability_plan;
pub mod object_model;
pub mod operator_lowering;
pub mod output_markers;
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
pub mod storage_strategy;
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
use std::fmt;
use std::sync::Arc;
use std::time::Instant;

pub struct Translation {
    pub rust: String,
    pub diagnostics: DiagnosticBag,
}

#[derive(Debug, Clone)]
pub struct BatchSource {
    pub path: String,
    pub source: String,
}

#[derive(Debug, Clone)]
pub struct BatchTranslation {
    pub path: String,
    pub rust: String,
    pub diagnostics: DiagnosticBag,
}

#[derive(Clone)]
pub struct BatchTranslationOptions {
    pub emission_jobs: usize,
    pub event_sink: Option<Arc<dyn Fn(BatchTranslationEvent) + Send + Sync>>,
}

impl fmt::Debug for BatchTranslationOptions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BatchTranslationOptions")
            .field("emission_jobs", &self.emission_jobs)
            .field(
                "event_sink",
                &self.event_sink.as_ref().map(|_| "<event-sink>"),
            )
            .finish()
    }
}

impl Default for BatchTranslationOptions {
    fn default() -> Self {
        Self {
            emission_jobs: 1,
            event_sink: None,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct BatchTranslationTimings {
    pub primary_parse_ms: u128,
    pub context_parse_ms: u128,
    pub summary_parse_ms: u128,
    pub global_analysis_ms: u128,
    pub global_resolve_ms: u128,
    pub global_semantic_ms: u128,
    pub global_representation_ms: u128,
    pub global_signature_ms: u128,
    pub emission_ms: u128,
}

#[derive(Debug, Clone, Default)]
pub struct BatchAnalysisProfile {
    pub primary_files: usize,
    pub context_files: usize,
    pub summary_files: usize,
    pub program_types: usize,
    pub program_methods: usize,
    pub program_constructors: usize,
    pub program_fields: usize,
    pub external_fact_count: usize,
    pub signature: signature::SignatureSolverProfile,
}

#[derive(Debug, Clone)]
pub struct BatchTranslationEvent {
    pub phase: &'static str,
    pub index: usize,
    pub path: String,
    pub elapsed_ms: u128,
    pub status: &'static str,
}

#[derive(Debug, Clone)]
pub struct BatchTranslationReport {
    pub translations: Vec<BatchTranslation>,
    pub timings: BatchTranslationTimings,
    pub analysis_profile: BatchAnalysisProfile,
    pub events: Vec<BatchTranslationEvent>,
}

pub fn translate_source(source: &str) -> Translation {
    translate_source_with_facts(source, &FactSet::default())
}

pub fn translate_source_with_facts(source: &str, external_facts: &FactSet) -> Translation {
    let parsed = parser::parse_java_source(source);
    let effective_external_facts = effective_external_facts_with_stdlib(external_facts, None);
    translate_parsed_source_with_context(
        parsed,
        None,
        None,
        None,
        None,
        false,
        &effective_external_facts,
    )
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
        false,
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
        false,
        external_facts,
    )
}

pub fn translate_source_with_context_sources_and_summary_sources(
    source: &str,
    context_sources: &[String],
    summary_sources: &[String],
    context_sources_are_emitted: bool,
    external_facts: &FactSet,
) -> Translation {
    let parsed = parser::parse_java_source(source);
    let mut source_program = parsed.program.clone();
    let mut summary_program = Program::default();
    let mut summary_facts = FactSet::default();
    let mut context_diagnostics = DiagnosticBag::new();
    for context_source in context_sources {
        let context_parsed = parser::parse_java_source(context_source);
        context_diagnostics.extend(context_parsed.diagnostics);
        merge_program_context(&mut source_program, context_parsed.program.clone());
    }
    for summary_source in summary_sources {
        let summary_parsed = parser::parse_java_source(summary_source);
        context_diagnostics.extend(summary_parsed.diagnostics);
        summary_facts.extend_from(resolve::declaration_facts_from_program(
            &summary_parsed.program,
        ));
        let summary_declarations = declaration_only_program(summary_parsed.program);
        merge_program_context(&mut summary_program, summary_declarations.clone());
        merge_program_context(&mut source_program, summary_declarations);
    }
    let source_program_context =
        (!context_sources.is_empty() || !summary_sources.is_empty()).then_some(source_program);
    let support_program_context = source_program_context.clone();
    let summary_program_context = (!summary_sources.is_empty()).then_some(summary_program);
    let effective_external_facts =
        effective_external_facts_with_stdlib(external_facts, Some(summary_facts));
    let mut translation = translate_parsed_source_with_context(
        parsed,
        source_program_context.clone(),
        source_program_context,
        support_program_context,
        summary_program_context,
        context_sources_are_emitted,
        &effective_external_facts,
    );
    translation.diagnostics.extend(context_diagnostics);
    translation
}

pub fn translate_batch_sources_with_context_sources_and_summary_sources(
    primary_sources: &[BatchSource],
    context_sources: &[String],
    summary_sources: &[String],
    context_sources_are_emitted: bool,
    external_facts: &FactSet,
) -> Vec<BatchTranslation> {
    translate_batch_sources_with_context_sources_summary_sources_and_options(
        primary_sources,
        context_sources,
        summary_sources,
        context_sources_are_emitted,
        external_facts,
        &BatchTranslationOptions::default(),
    )
    .translations
}

pub fn translate_batch_sources_with_context_sources_summary_sources_and_options(
    primary_sources: &[BatchSource],
    context_sources: &[String],
    summary_sources: &[String],
    context_sources_are_emitted: bool,
    external_facts: &FactSet,
    options: &BatchTranslationOptions,
) -> BatchTranslationReport {
    let batch_start = Instant::now();
    let event_sink = options.event_sink.as_deref();
    let mut timings = BatchTranslationTimings::default();
    let mut parsed_primary = Vec::with_capacity(primary_sources.len());
    let mut source_program = Program::default();
    let mut context_diagnostics = DiagnosticBag::new();

    emit_batch_phase_progress(
        event_sink,
        "batch_primary_parse",
        "running",
        batch_start.elapsed().as_millis(),
        &format!("{} primary files", primary_sources.len()),
    );
    let primary_parse_start = Instant::now();
    for source in primary_sources {
        let parsed = parser::parse_java_source(&source.source);
        context_diagnostics.extend(parsed.diagnostics.clone());
        merge_program_context(&mut source_program, parsed.program.clone());
        parsed_primary.push((source.path.clone(), parsed));
    }
    timings.primary_parse_ms = primary_parse_start.elapsed().as_millis();
    emit_batch_phase_progress(
        event_sink,
        "batch_primary_parse",
        "ok",
        timings.primary_parse_ms,
        &format!("{} primary files", primary_sources.len()),
    );

    let mut summary_program = Program::default();
    let mut summary_facts = FactSet::default();
    emit_batch_phase_progress(
        event_sink,
        "batch_context_parse",
        "running",
        batch_start.elapsed().as_millis(),
        &format!("{} context files", context_sources.len()),
    );
    let context_parse_start = Instant::now();
    for context_source in context_sources {
        let context_parsed = parser::parse_java_source(context_source);
        context_diagnostics.extend(context_parsed.diagnostics);
        merge_program_context(&mut source_program, context_parsed.program.clone());
    }
    timings.context_parse_ms = context_parse_start.elapsed().as_millis();
    emit_batch_phase_progress(
        event_sink,
        "batch_context_parse",
        "ok",
        timings.context_parse_ms,
        &format!("{} context files", context_sources.len()),
    );
    emit_batch_phase_progress(
        event_sink,
        "batch_summary_parse",
        "running",
        batch_start.elapsed().as_millis(),
        &format!("{} summary files", summary_sources.len()),
    );
    let summary_parse_start = Instant::now();
    for summary_source in summary_sources {
        let summary_parsed = parser::parse_java_source(summary_source);
        context_diagnostics.extend(summary_parsed.diagnostics);
        summary_facts.extend_from(resolve::declaration_facts_from_program(
            &summary_parsed.program,
        ));
        let summary_declarations = declaration_only_program(summary_parsed.program);
        merge_program_context(&mut summary_program, summary_declarations.clone());
        merge_program_context(&mut source_program, summary_declarations);
    }
    timings.summary_parse_ms = summary_parse_start.elapsed().as_millis();
    emit_batch_phase_progress(
        event_sink,
        "batch_summary_parse",
        "ok",
        timings.summary_parse_ms,
        &format!("{} summary files", summary_sources.len()),
    );

    let effective_external_facts =
        effective_external_facts_with_stdlib(external_facts, Some(summary_facts));
    emit_batch_phase_progress(
        event_sink,
        "batch_global_analysis",
        "running",
        batch_start.elapsed().as_millis(),
        "resolver/semantic/representation/signature",
    );
    let global_analysis_start = Instant::now();
    let context_build = build_batch_translation_context(
        source_program,
        summary_program,
        context_sources_are_emitted,
        &effective_external_facts,
        event_sink,
        batch_start,
    );
    timings.global_analysis_ms = global_analysis_start.elapsed().as_millis();
    timings.global_resolve_ms = context_build.timings.global_resolve_ms;
    timings.global_semantic_ms = context_build.timings.global_semantic_ms;
    timings.global_representation_ms = context_build.timings.global_representation_ms;
    timings.global_signature_ms = context_build.timings.global_signature_ms;
    emit_batch_phase_progress(
        event_sink,
        "batch_global_analysis",
        "ok",
        timings.global_analysis_ms,
        "resolver/semantic/representation/signature",
    );

    emit_batch_phase_progress(
        event_sink,
        "batch_emission",
        "running",
        batch_start.elapsed().as_millis(),
        &format!("jobs={}", options.emission_jobs.max(1)),
    );
    let emission_start = Instant::now();
    let (translations, events) = emit_batch_translations(
        parsed_primary,
        &context_build.context,
        &context_diagnostics,
        options.emission_jobs.max(1),
        options.event_sink.as_deref(),
    );
    timings.emission_ms = emission_start.elapsed().as_millis();
    emit_batch_phase_progress(
        event_sink,
        "batch_emission",
        "ok",
        timings.emission_ms,
        &format!("jobs={}", options.emission_jobs.max(1)),
    );
    let mut analysis_profile = context_build.analysis_profile;
    analysis_profile.primary_files = primary_sources.len();
    analysis_profile.context_files = context_sources.len();
    analysis_profile.summary_files = summary_sources.len();
    BatchTranslationReport {
        translations,
        timings,
        analysis_profile,
        events,
    }
}

fn effective_external_facts_with_stdlib(
    external_facts: &FactSet,
    summary_facts: Option<FactSet>,
) -> FactSet {
    let mut effective = stdlib::rust_stdlib_signature_facts();
    effective.extend_from(external_facts.clone());
    if let Some(summary_facts) = summary_facts {
        effective.extend_from(summary_facts);
    }
    effective
}

struct BatchTranslationContext {
    source_symbols: resolve::SourceSymbols,
    support_program: Program,
    summary_program: Program,
    context_sources_are_emitted: bool,
    external_facts: FactSet,
    semantic: semantic_analysis::SemanticFacts,
    representations: representation::RepresentationPlan,
    signatures: signature::SignaturePlan,
}

struct BatchTranslationContextBuild {
    context: BatchTranslationContext,
    analysis_profile: BatchAnalysisProfile,
    timings: BatchTranslationTimings,
}

fn build_batch_translation_context(
    source_program: Program,
    summary_program: Program,
    context_sources_are_emitted: bool,
    external_facts: &FactSet,
    event_sink: Option<&(dyn Fn(BatchTranslationEvent) + Send + Sync)>,
    batch_start: Instant,
) -> BatchTranslationContextBuild {
    let mut timings = BatchTranslationTimings::default();
    let mut analysis_profile = BatchAnalysisProfile {
        program_types: count_program_types(&source_program),
        program_methods: count_program_methods(&source_program),
        program_constructors: count_program_constructors(&source_program),
        program_fields: count_program_fields(&source_program),
        external_fact_count: count_facts(external_facts),
        ..BatchAnalysisProfile::default()
    };
    emit_batch_phase_progress(
        event_sink,
        "batch_global_resolve_context",
        "running",
        batch_start.elapsed().as_millis(),
        "resolve_program_with_source_program_and_facts",
    );
    let resolve_start = Instant::now();
    let source_symbols = resolve::source_symbols_from_program(&source_program);
    let resolved_context = resolve::resolve_program_with_source_symbols_and_facts(
        &source_program,
        DiagnosticBag::new(),
        &source_symbols,
        external_facts,
    );
    timings.global_resolve_ms = resolve_start.elapsed().as_millis();
    emit_batch_phase_progress(
        event_sink,
        "batch_global_resolve_context",
        "ok",
        timings.global_resolve_ms,
        "resolve_program_with_source_program_and_facts",
    );

    emit_batch_phase_progress(
        event_sink,
        "batch_global_semantic",
        "running",
        batch_start.elapsed().as_millis(),
        "semantic_analysis::analyze_program",
    );
    let semantic_start = Instant::now();
    let semantic = semantic_analysis::analyze_program(
        &resolved_context.program,
        &resolved_context.facts,
        external_facts,
    );
    timings.global_semantic_ms = semantic_start.elapsed().as_millis();
    emit_batch_phase_progress(
        event_sink,
        "batch_global_semantic",
        "ok",
        timings.global_semantic_ms,
        "semantic_analysis::analyze_program",
    );

    emit_batch_phase_progress(
        event_sink,
        "batch_global_representation",
        "running",
        batch_start.elapsed().as_millis(),
        "representation::plan_representations",
    );
    let representation_start = Instant::now();
    let representations =
        representation::plan_representations(&resolved_context.program, &semantic.facts);
    timings.global_representation_ms = representation_start.elapsed().as_millis();
    emit_batch_phase_progress(
        event_sink,
        "batch_global_representation",
        "ok",
        timings.global_representation_ms,
        "representation::plan_representations",
    );

    emit_batch_phase_progress(
        event_sink,
        "batch_global_signature",
        "running",
        batch_start.elapsed().as_millis(),
        "signature::solve_signatures",
    );
    let signature_start = Instant::now();
    let signature_progress = |event: signature::SignatureSolverProgressEvent| {
        emit_batch_phase_progress(
            event_sink,
            signature_solver_batch_phase(event.phase),
            event.status,
            event.elapsed_ms,
            &event.detail,
        );
    };
    let profiled_signatures =
        signature::solve_signatures_with_facts_and_support_context_profiled_with_progress(
            &resolved_context.program,
            &representations,
            &semantic.usage,
            &semantic.facts,
            &resolved_context.program,
            event_sink.map(|_| &signature_progress as signature::SignatureSolverProgressSink<'_>),
        );
    timings.global_signature_ms = signature_start.elapsed().as_millis();
    analysis_profile.signature = profiled_signatures.profile;
    emit_batch_phase_progress(
        event_sink,
        "batch_global_signature",
        "ok",
        timings.global_signature_ms,
        "signature::solve_signatures",
    );

    BatchTranslationContextBuild {
        context: BatchTranslationContext {
            source_symbols,
            support_program: resolved_context.program,
            summary_program,
            context_sources_are_emitted,
            external_facts: external_facts.clone(),
            semantic,
            representations,
            signatures: profiled_signatures.plan,
        },
        analysis_profile,
        timings,
    }
}

fn emit_batch_phase_progress(
    event_sink: Option<&(dyn Fn(BatchTranslationEvent) + Send + Sync)>,
    phase: &'static str,
    status: &'static str,
    elapsed_ms: u128,
    detail: &str,
) {
    if let Some(event_sink) = event_sink {
        event_sink(BatchTranslationEvent {
            phase,
            index: 0,
            path: detail.to_string(),
            elapsed_ms,
            status,
        });
    }
}

fn signature_solver_batch_phase(phase: &'static str) -> &'static str {
    match phase {
        "collect_constraints" => "batch_global_signature_collect_constraints_live",
        "dependency_graph" => "batch_global_signature_dependency_graph_live",
        "scc_analysis" => "batch_global_signature_scc_live",
        "pass_plan" => "batch_global_signature_pass_plan_live",
        "fixpoint" => "batch_global_signature_fixpoint_live",
        "fixpoint_iteration" => "batch_global_signature_fixpoint_iteration_live",
        "fixpoint_component" => "batch_global_signature_fixpoint_component_live",
        "fixpoint_component_sweep" => "batch_global_signature_fixpoint_component_sweep_live",
        "plan_build" => "batch_global_signature_plan_build_live",
        _ => "batch_global_signature_progress",
    }
}

fn count_program_types(program: &Program) -> usize {
    program.types.iter().map(count_type_and_nested_types).sum()
}

fn count_type_and_nested_types(ty: &java_ir::TypeDecl) -> usize {
    1 + ty
        .members
        .iter()
        .filter_map(|member| match member {
            java_ir::Member::Type(nested) => Some(count_type_and_nested_types(nested)),
            _ => None,
        })
        .sum::<usize>()
}

fn count_program_methods(program: &Program) -> usize {
    program.types.iter().map(count_type_methods).sum()
}

fn count_type_methods(ty: &java_ir::TypeDecl) -> usize {
    ty.members
        .iter()
        .map(|member| match member {
            java_ir::Member::Method(_) => 1,
            java_ir::Member::Type(nested) => count_type_methods(nested),
            _ => 0,
        })
        .sum()
}

fn count_program_constructors(program: &Program) -> usize {
    program.types.iter().map(count_type_constructors).sum()
}

fn count_type_constructors(ty: &java_ir::TypeDecl) -> usize {
    ty.members
        .iter()
        .map(|member| match member {
            java_ir::Member::Constructor(_) => 1,
            java_ir::Member::Type(nested) => count_type_constructors(nested),
            _ => 0,
        })
        .sum()
}

fn count_program_fields(program: &Program) -> usize {
    program.types.iter().map(count_type_fields).sum()
}

fn count_type_fields(ty: &java_ir::TypeDecl) -> usize {
    ty.members
        .iter()
        .map(|member| match member {
            java_ir::Member::Field(_) => 1,
            java_ir::Member::Type(nested) => count_type_fields(nested),
            _ => 0,
        })
        .sum()
}

fn count_facts(facts: &FactSet) -> usize {
    facts.notes.len()
        + facts.captures.owners.len()
        + facts.hierarchy_overrides.len()
        + facts.closed_hierarchy_facts.len()
        + facts.sealed_permits_facts.len()
        + facts.object_safety_facts.len()
        + facts.type_capability_overrides.len()
        + facts.owned_recovery_capability_facts.len()
        + facts.field_nullability_facts.len()
        + facts.trivial_accessor_facts.len()
        + facts.external_api_effect_facts.len()
        + facts.external_api_argument_effect_facts.len()
        + facts.external_ffi_handover_facts.len()
        + facts.external_constructor_argument_effect_facts.len()
        + facts.external_conversion_adapter_facts.len()
        + facts.external_crate_api_mapping_facts.len()
        + facts.external_crate_type_mapping_facts.len()
        + facts.external_crate_constructor_mapping_facts.len()
        + facts.facade_abi_facts.len()
        + facts.external_rust_result_facts.len()
        + facts.external_rust_return_type_facts.len()
        + facts.external_constructor_rust_result_facts.len()
        + facts.external_constructor_facts.len()
        + facts.external_field_facts.len()
        + facts.method_effect_facts.len()
        + facts.recursion_facts.len()
        + facts.constructor_recursion_facts.len()
        + facts.control_flow_facts.len()
        + facts.unresolved_target_facts.len()
        + facts.dispatch_requirement_facts.len()
        + facts.exception_facts.len()
        + facts.catch_facts.len()
        + facts.external_library_packs.len()
        + facts.unsupported_external_api_facts.len()
        + facts.unsupported_external_constructor_facts.len()
        + facts.unsupported_external_field_facts.len()
}

fn translate_parsed_source_with_batch_context(
    parsed: parser::ParsedProgram,
    context: &BatchTranslationContext,
) -> Translation {
    let resolved = resolve::resolve_program_with_source_symbols_and_facts(
        &parsed.program,
        parsed.diagnostics,
        &context.source_symbols,
        &context.external_facts,
    );
    let mut lowered = lower::lower_program_with_semantic_plans_and_support_context(
        &resolved.program,
        resolved.diagnostics,
        &context.semantic.facts,
        &context.semantic.usage,
        &context.representations,
        &context.signatures,
        &context.semantic.control_flow_plans,
        &context.support_program,
        &context.summary_program,
        context.context_sources_are_emitted,
    );
    for diagnostic in &context.semantic.diagnostics {
        lowered
            .diagnostics
            .warning(diagnostic.code, diagnostic.message.clone());
    }
    let rust = emit::emit_file(&lowered.file);
    Translation {
        rust,
        diagnostics: lowered.diagnostics,
    }
}

fn emit_batch_translations(
    parsed_primary: Vec<(String, parser::ParsedProgram)>,
    context: &BatchTranslationContext,
    context_diagnostics: &DiagnosticBag,
    emission_jobs: usize,
    event_sink: Option<&(dyn Fn(BatchTranslationEvent) + Send + Sync)>,
) -> (Vec<BatchTranslation>, Vec<BatchTranslationEvent>) {
    if emission_jobs <= 1 || parsed_primary.len() <= 1 {
        let mut events = Vec::with_capacity(parsed_primary.len());
        let mut translations = Vec::with_capacity(parsed_primary.len());
        for (index, (path, parsed)) in parsed_primary.into_iter().enumerate() {
            let start_event = BatchTranslationEvent {
                phase: "emit_start",
                index: index + 1,
                path: path.clone(),
                elapsed_ms: 0,
                status: "running",
            };
            if let Some(event_sink) = event_sink {
                event_sink(start_event.clone());
            }
            events.push(start_event);
            let file_start = Instant::now();
            let mut translation = translate_parsed_source_with_batch_context(parsed, context);
            translation.diagnostics.extend(context_diagnostics.clone());
            let elapsed_ms = file_start.elapsed().as_millis();
            let end_event = BatchTranslationEvent {
                phase: "emit_end",
                index: index + 1,
                path: path.clone(),
                elapsed_ms,
                status: "ok",
            };
            if let Some(event_sink) = event_sink {
                event_sink(end_event.clone());
            }
            events.push(end_event);
            translations.push(BatchTranslation {
                path,
                rust: translation.rust,
                diagnostics: translation.diagnostics,
            });
        }
        return (translations, events);
    }

    let worker_count = emission_jobs.min(parsed_primary.len());
    let mut per_worker = Vec::with_capacity(worker_count);
    for worker_index in 0..worker_count {
        let mut worker_items = Vec::new();
        for (index, item) in parsed_primary.iter().enumerate() {
            if index % worker_count == worker_index {
                worker_items.push((index, item.0.clone(), item.1.clone()));
            }
        }
        per_worker.push(worker_items);
    }

    let mut joined = Vec::with_capacity(parsed_primary.len());
    std::thread::scope(|scope| {
        let handles = per_worker
            .into_iter()
            .map(|items| {
                scope.spawn(move || {
                    let mut worker_results = Vec::with_capacity(items.len());
                    for (index, path, parsed) in items {
                        let file_start = Instant::now();
                        let start_event = BatchTranslationEvent {
                            phase: "emit_start",
                            index: index + 1,
                            path: path.clone(),
                            elapsed_ms: 0,
                            status: "running",
                        };
                        if let Some(event_sink) = event_sink {
                            event_sink(start_event.clone());
                        }
                        let mut translation =
                            translate_parsed_source_with_batch_context(parsed, context);
                        translation.diagnostics.extend(context_diagnostics.clone());
                        let elapsed_ms = file_start.elapsed().as_millis();
                        let end_event = BatchTranslationEvent {
                            phase: "emit_end",
                            index: index + 1,
                            path: path.clone(),
                            elapsed_ms,
                            status: "ok",
                        };
                        if let Some(event_sink) = event_sink {
                            event_sink(end_event.clone());
                        }
                        worker_results.push((
                            index,
                            BatchTranslation {
                                path,
                                rust: translation.rust,
                                diagnostics: translation.diagnostics,
                            },
                            vec![start_event, end_event],
                        ));
                    }
                    worker_results
                })
            })
            .collect::<Vec<_>>();
        for handle in handles {
            joined.extend(handle.join().expect("batch emission worker panicked"));
        }
    });

    joined.sort_by_key(|(index, _, _)| *index);
    let mut translations = Vec::with_capacity(joined.len());
    let mut events = Vec::with_capacity(joined.len() * 2);
    for (_, translation, item_events) in joined {
        translations.push(translation);
        events.extend(item_events);
    }
    (translations, events)
}

fn translate_parsed_source_with_context(
    parsed: parser::ParsedProgram,
    source_program_context: Option<Program>,
    semantic_program_context: Option<Program>,
    support_program_context: Option<Program>,
    summary_program_context: Option<Program>,
    context_sources_are_emitted: bool,
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
    let empty_summary_program = Program::default();
    let summary_program = summary_program_context
        .as_ref()
        .unwrap_or(&empty_summary_program);
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
        summary_program,
        context_sources_are_emitted,
    );
    for diagnostic in semantic.diagnostics {
        lowered
            .diagnostics
            .warning(diagnostic.code, diagnostic.message);
    }
    // Lowering copies where Java aliased, because there is no shared
    // representation to lower an alias into. Where that copy is observable the
    // output compiles and computes a different answer, so it is reported here
    // rather than left to be discovered downstream.
    aliasing::report_observable_aliases(
        &resolved.program,
        &semantic.usage,
        &mut lowered.diagnostics,
    );
    // Step 2 of the 0002b design: the decision rule runs, lowering does not
    // change, and every type the plan says should be shared while lowering
    // holds it by value is reported. That list is the work queue for the
    // representation change and the size estimate it needs first.
    storage_strategy::report_storage_disagreements(
        &resolved.program,
        &semantic.usage,
        &mut lowered.diagnostics,
    );
    let rust = emit::emit_file(&lowered.file);
    Translation {
        rust,
        diagnostics: lowered.diagnostics,
    }
}

fn merge_program_context(target: &mut Program, source: Program) {
    if target.package.is_none() {
        target.package = source.package.clone();
    }
    for import in source.imports {
        if !target.imports.contains(&import) {
            target.imports.push(import);
        }
    }
    for mut ty in source.types {
        if target.types.iter().any(|existing| existing.name == ty.name) {
            if let Some(qualified) = source
                .package
                .as_deref()
                .and_then(|package| qualified_merge_type_name(package, &ty.name))
                && !target
                    .types
                    .iter()
                    .any(|existing| existing.name == qualified)
            {
                ty.name = qualified;
                target.types.push(ty);
            }
            continue;
        }
        target.types.push(ty);
    }
}

fn qualified_merge_type_name(package: &str, type_name: &str) -> Option<String> {
    if package.is_empty() || type_name.contains('.') {
        return None;
    }
    Some(format!("{package}.{type_name}"))
}

fn declaration_only_program(mut program: Program) -> Program {
    for ty in &mut program.types {
        strip_summary_bodies_from_type(ty);
    }
    program
}

fn strip_summary_bodies_from_type(ty: &mut java_ir::TypeDecl) {
    for member in &mut ty.members {
        match member {
            java_ir::Member::Field(_) => {}
            java_ir::Member::Method(method) => {
                method.body = None;
            }
            java_ir::Member::Constructor(constructor) => {
                constructor.body = None;
            }
            java_ir::Member::Type(nested) => strip_summary_bodies_from_type(nested),
            java_ir::Member::Unsupported { .. } => {}
        }
    }
}
