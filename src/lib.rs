mod access_lowering;
mod aliasing;
mod array_default_obligations;
mod array_lowering;
mod array_plan;
pub mod binding_id;
pub mod call_lowering;
pub mod call_plan;
mod callable_analysis;
pub mod coercion;
pub mod constructor_plan;
mod control_flow_lowering;
mod conversion_lowering;
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
mod fact_saturation;
pub mod facts;
mod fallibility_invariant;
mod graph;
mod inherited_members;
mod inner_class_capture;
mod item_assembly;
pub mod java_ir;
mod java_naming;
mod java_signature;
mod javalibs;
mod lattice;
mod literal_lowering;
mod liveness;
pub mod loop_lowering;
mod loop_plan;
pub mod lower;
pub mod lowering_diagnostics;
pub mod member_access_plan;
mod method_reference_lowering;
mod method_reference_plan;
mod method_reference_shape;
mod nullability_plan;
pub mod object_model;
pub mod operator_lowering;
pub mod output_markers;
pub mod parser;
pub mod representation;
mod representation_lowering;
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
mod source_constant_values;
mod statement_render;
pub mod stdlib;
pub mod storage_strategy;
pub mod stream;
pub mod stream_lowering;
pub mod stream_shape;
mod string_concat;
mod string_concat_plan;
mod string_concat_render;
pub mod switch_lowering;
pub mod switch_plan;
pub mod types;
pub mod unsupported;
pub mod usage;

use diagnostics::DiagnosticBag;
use facts::FactSet;
use java_ir::{
    Block, Expr, JavaType, LambdaBody, Member, Program, Stmt, SwitchLabel, TypeDecl, WildcardBound,
};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::Arc;
use std::time::Instant;

pub struct Translation {
    pub rust: String,
    pub diagnostics: DiagnosticBag,
}

#[derive(Debug, Clone)]
pub struct BatchSource {
    path: String,
    source: String,
}

impl BatchSource {
    pub fn new(path: impl Into<String>, source: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            source: source.into(),
        }
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn source(&self) -> &str {
        &self.source
    }
}

#[derive(Debug, Clone)]
pub struct BatchTranslation {
    path: String,
    rust: String,
    diagnostics: DiagnosticBag,
}

impl BatchTranslation {
    pub fn new(path: impl Into<String>, rust: String, diagnostics: DiagnosticBag) -> Self {
        Self {
            path: path.into(),
            rust,
            diagnostics,
        }
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn rust(&self) -> &str {
        &self.rust
    }

    pub fn diagnostics(&self) -> &DiagnosticBag {
        &self.diagnostics
    }
}

pub type BatchTranslationEventSink = dyn Fn(BatchTranslationEvent) + Send + Sync;

#[derive(Clone)]
pub struct BatchTranslationOptions {
    emission_jobs: usize,
    event_sink: Option<Arc<BatchTranslationEventSink>>,
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

impl BatchTranslationOptions {
    pub fn new(emission_jobs: usize, event_sink: Option<Arc<BatchTranslationEventSink>>) -> Self {
        Self {
            emission_jobs,
            event_sink,
        }
    }

    pub fn emission_jobs(&self) -> usize {
        self.emission_jobs
    }

    pub fn event_sink(&self) -> Option<&BatchTranslationEventSink> {
        self.event_sink.as_deref()
    }
}

#[derive(Debug, Clone, Default)]
pub struct BatchTranslationTimings {
    primary_parse_ms: u128,
    context_parse_ms: u128,
    summary_parse_ms: u128,
    global_analysis_ms: u128,
    global_resolve_ms: u128,
    global_semantic_ms: u128,
    global_representation_ms: u128,
    global_signature_ms: u128,
    emission_ms: u128,
}

impl BatchTranslationTimings {
    pub fn primary_parse_ms(&self) -> u128 {
        self.primary_parse_ms
    }

    pub fn context_parse_ms(&self) -> u128 {
        self.context_parse_ms
    }

    pub fn summary_parse_ms(&self) -> u128 {
        self.summary_parse_ms
    }

    pub fn global_analysis_ms(&self) -> u128 {
        self.global_analysis_ms
    }

    pub fn global_resolve_ms(&self) -> u128 {
        self.global_resolve_ms
    }

    pub fn global_semantic_ms(&self) -> u128 {
        self.global_semantic_ms
    }

    pub fn global_representation_ms(&self) -> u128 {
        self.global_representation_ms
    }

    pub fn global_signature_ms(&self) -> u128 {
        self.global_signature_ms
    }

    pub fn emission_ms(&self) -> u128 {
        self.emission_ms
    }
}

#[derive(Debug, Clone, Default)]
pub struct BatchAnalysisProfile {
    primary_files: usize,
    context_files: usize,
    summary_files: usize,
    program_types: usize,
    program_methods: usize,
    program_constructors: usize,
    program_fields: usize,
    external_fact_count: usize,
    pub signature: signature::SignatureSolverProfile,
}

impl BatchAnalysisProfile {
    pub fn primary_files(&self) -> usize {
        self.primary_files
    }

    pub fn context_files(&self) -> usize {
        self.context_files
    }

    pub fn summary_files(&self) -> usize {
        self.summary_files
    }

    pub fn program_types(&self) -> usize {
        self.program_types
    }

    pub fn program_methods(&self) -> usize {
        self.program_methods
    }

    pub fn program_constructors(&self) -> usize {
        self.program_constructors
    }

    pub fn program_fields(&self) -> usize {
        self.program_fields
    }

    pub fn external_fact_count(&self) -> usize {
        self.external_fact_count
    }
}

#[derive(Debug, Clone)]
pub struct BatchTranslationEvent {
    phase: &'static str,
    index: usize,
    path: String,
    elapsed_ms: u128,
    status: &'static str,
}

impl BatchTranslationEvent {
    pub fn new(
        phase: &'static str,
        index: usize,
        path: impl Into<String>,
        elapsed_ms: u128,
        status: &'static str,
    ) -> Self {
        Self {
            phase,
            index,
            path: path.into(),
            elapsed_ms,
            status,
        }
    }

    pub fn phase(&self) -> &'static str {
        self.phase
    }

    pub fn index(&self) -> usize {
        self.index
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn elapsed_ms(&self) -> u128 {
        self.elapsed_ms
    }

    pub fn status(&self) -> &'static str {
        self.status
    }
}

#[derive(Debug, Clone)]
pub struct BatchTranslationReport {
    translations: Vec<BatchTranslation>,
    timings: BatchTranslationTimings,
    analysis_profile: BatchAnalysisProfile,
    events: Vec<BatchTranslationEvent>,
}

impl BatchTranslationReport {
    pub fn translations(&self) -> &[BatchTranslation] {
        &self.translations
    }

    pub fn timings(&self) -> &BatchTranslationTimings {
        &self.timings
    }

    pub fn analysis_profile(&self) -> &BatchAnalysisProfile {
        &self.analysis_profile
    }

    pub fn events(&self) -> &[BatchTranslationEvent] {
        &self.events
    }
}

pub fn translate_source(source: &str) -> Translation {
    translate_source_with_facts(source, &FactSet::default())
}

pub fn translate_source_with_facts(source: &str, external_facts: &FactSet) -> Translation {
    let mut parsed = parser::parse_java_source(source);
    qualify_same_package_class_literals(&mut parsed.program);
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
    let mut parsed = parser::parse_java_source(source);
    qualify_same_package_class_literals(&mut parsed.program);
    let mut summary_program = Program::default();
    let mut summary_facts = FactSet::default();
    let mut context_diagnostics = DiagnosticBag::new();
    let mut parsed_context = Vec::with_capacity(context_sources.len());
    for context_source in context_sources {
        let mut context_parsed = parser::parse_java_source(context_source);
        qualify_same_package_class_literals(&mut context_parsed.program);
        context_diagnostics.extend(context_parsed.diagnostics.clone());
        parsed_context.push(context_parsed);
    }
    {
        let mut programs = Vec::with_capacity(1 + parsed_context.len());
        programs.push(&mut parsed.program);
        for context_parsed in &mut parsed_context {
            programs.push(&mut context_parsed.program);
        }
        qualify_duplicate_program_type_names(&mut programs);
    }
    let mut source_program = parsed.program.clone();
    for context_parsed in &parsed_context {
        merge_program_context(&mut source_program, context_parsed.program.clone());
    }
    // Pooled across files, because a constant may be defined in terms of one
    // owned by another class in another file. Type names are qualified with
    // each file's own package here; the merged program no longer knows them.
    let mut summary_constant_declarations = Vec::new();
    for summary_source in summary_sources {
        let summary_parsed = parser::parse_java_source(summary_source);
        context_diagnostics.extend(summary_parsed.diagnostics);
        summary_facts.extend_from(resolve::declaration_facts_from_program(
            &summary_parsed.program,
        ));
        summary_facts.extend_from(summary_field_nullability_facts_from_program(
            &summary_parsed.program,
            external_facts,
        ));
        summary_constant_declarations.extend(resolve::static_final_constant_declarations(
            &summary_parsed.program,
        ));
        let mut summary_declarations = declaration_only_program(summary_parsed.program);
        resolve::assign_rust_names_to_context_program(&mut summary_declarations);
        merge_program_context(&mut summary_program, summary_declarations.clone());
        merge_program_context(&mut source_program, summary_declarations);
    }
    summary_facts.extend_from(resolve::constant_value_facts_from_declarations(
        &summary_constant_declarations,
    ));
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
    let event_sink = options.event_sink();
    let mut timings = BatchTranslationTimings::default();
    let mut parsed_primary = Vec::with_capacity(primary_sources.len());
    let mut parsed_context = Vec::with_capacity(context_sources.len());
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
        let mut parsed = parser::parse_java_source(source.source());
        qualify_same_package_class_literals(&mut parsed.program);
        // A *primary* file's parse diagnostics belong to that file, and already
        // reach it through `parsed.diagnostics` in
        // `translate_parsed_source_with_batch_context`. Adding them to the
        // batch-wide bag as well gave every file in the batch every other file's
        // parse errors -- so one unparseable file marked all its clean siblings
        // `errors`, and was itself counted twice. The bag is for *context* and
        // summary sources, which really are shared; that is the invariant the
        // single-file path above keeps. See claude_audits/0055.
        parsed_primary.push((source.path().to_string(), parsed));
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
        let mut context_parsed = parser::parse_java_source(context_source);
        qualify_same_package_class_literals(&mut context_parsed.program);
        context_diagnostics.extend(context_parsed.diagnostics.clone());
        parsed_context.push(context_parsed);
    }
    {
        let mut programs = Vec::with_capacity(parsed_primary.len() + parsed_context.len());
        for (_, parsed) in &mut parsed_primary {
            programs.push(&mut parsed.program);
        }
        for parsed in &mut parsed_context {
            programs.push(&mut parsed.program);
        }
        qualify_duplicate_program_type_names(&mut programs);
    }
    for (_, parsed) in &parsed_primary {
        merge_program_context(&mut source_program, parsed.program.clone());
    }
    for parsed in &parsed_context {
        merge_program_context(&mut source_program, parsed.program.clone());
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
    // Pooled across files, because a constant may be defined in terms of one
    // owned by another class in another file. Type names are qualified with
    // each file's own package here; the merged program no longer knows them.
    let mut summary_constant_declarations = Vec::new();
    for summary_source in summary_sources {
        let summary_parsed = parser::parse_java_source(summary_source);
        context_diagnostics.extend(summary_parsed.diagnostics);
        summary_facts.extend_from(resolve::declaration_facts_from_program(
            &summary_parsed.program,
        ));
        summary_facts.extend_from(summary_field_nullability_facts_from_program(
            &summary_parsed.program,
            external_facts,
        ));
        summary_constant_declarations.extend(resolve::static_final_constant_declarations(
            &summary_parsed.program,
        ));
        let mut summary_declarations = declaration_only_program(summary_parsed.program);
        resolve::assign_rust_names_to_context_program(&mut summary_declarations);
        merge_program_context(&mut summary_program, summary_declarations.clone());
        merge_program_context(&mut source_program, summary_declarations);
    }
    summary_facts.extend_from(resolve::constant_value_facts_from_declarations(
        &summary_constant_declarations,
    ));
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
        &format!("jobs={}", options.emission_jobs().max(1)),
    );
    let emission_start = Instant::now();
    let (translations, events) = emit_batch_translations(
        parsed_primary,
        &context_build.context,
        &context_diagnostics,
        options.emission_jobs().max(1),
        options.event_sink(),
    );
    timings.emission_ms = emission_start.elapsed().as_millis();
    emit_batch_phase_progress(
        event_sink,
        "batch_emission",
        "ok",
        timings.emission_ms,
        &format!("jobs={}", options.emission_jobs().max(1)),
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

fn summary_field_nullability_facts_from_program(
    program: &Program,
    external_facts: &FactSet,
) -> FactSet {
    let resolved = resolve::resolve_program(program, DiagnosticBag::new());
    let semantic = semantic_analysis::analyze_program_with_config(
        &resolved.program,
        &resolved.facts,
        &FactSet::default(),
        semantic_analysis_config_from_facts(external_facts),
    );
    let mut facts = FactSet::default();
    facts.field_nullability_facts = semantic.facts.field_nullability_facts;
    facts
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
    let support_program = program_with_synthetic_local_capture_types(
        resolved_context.program.clone(),
        &resolved_context.facts,
    );
    let mut semantic = semantic_analysis::analyze_program_with_config(
        &support_program,
        &resolved_context.facts,
        external_facts,
        semantic_analysis_config_from_facts(external_facts),
    );
    semantic.facts.storage_strategies = storage_strategy::plan_storage_strategies_from_facts(
        &support_program,
        &semantic.usage,
        &resolved_context.facts,
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
    let representations = representation::plan_representations(&support_program, &semantic.facts);
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
            signature_solver_batch_phase(event.phase()),
            event.status(),
            event.elapsed_ms(),
            event.detail(),
        );
    };
    let profiled_signatures =
        signature::solve_signatures_with_facts_and_support_context_profiled_with_progress(
            &support_program,
            &representations,
            &semantic.usage,
            &semantic.facts,
            &support_program,
            event_sink.map(|_| &signature_progress as signature::SignatureSolverProgressSink<'_>),
        );
    timings.global_signature_ms = signature_start.elapsed().as_millis();
    let (signature_plan, signature_profile) = profiled_signatures.into_parts();
    analysis_profile.signature = signature_profile;
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
            support_program,
            summary_program,
            context_sources_are_emitted,
            external_facts: external_facts.clone(),
            semantic,
            representations,
            signatures: signature_plan,
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
        event_sink(BatchTranslationEvent::new(
            phase, 0, detail, elapsed_ms, status,
        ));
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
        &resolved.program,
        &context.summary_program,
        context.context_sources_are_emitted,
    );
    for diagnostic in &context.semantic.diagnostics {
        lowered
            .diagnostics_mut()
            .warning(diagnostic.code(), diagnostic.message().to_string());
    }
    report_post_lowering_translation_diagnostics(
        &resolved.program,
        &context.semantic.usage,
        &context.semantic.facts,
        lowered.diagnostics_mut(),
    );
    let (file, diagnostics) = lowered.into_parts();
    let rust = emit::emit_file(&file);
    Translation { rust, diagnostics }
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
            let start_event =
                BatchTranslationEvent::new("emit_start", index + 1, path.clone(), 0, "running");
            if let Some(event_sink) = event_sink {
                event_sink(start_event.clone());
            }
            events.push(start_event);
            let file_start = Instant::now();
            let mut translation = translate_parsed_source_with_batch_context(parsed, context);
            translation.diagnostics.extend(context_diagnostics.clone());
            let elapsed_ms = file_start.elapsed().as_millis();
            let end_event =
                BatchTranslationEvent::new("emit_end", index + 1, path.clone(), elapsed_ms, "ok");
            if let Some(event_sink) = event_sink {
                event_sink(end_event.clone());
            }
            events.push(end_event);
            translations.push(BatchTranslation::new(
                path,
                translation.rust,
                translation.diagnostics,
            ));
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
                        let start_event = BatchTranslationEvent::new(
                            "emit_start",
                            index + 1,
                            path.clone(),
                            0,
                            "running",
                        );
                        if let Some(event_sink) = event_sink {
                            event_sink(start_event.clone());
                        }
                        let mut translation =
                            translate_parsed_source_with_batch_context(parsed, context);
                        translation.diagnostics.extend(context_diagnostics.clone());
                        let elapsed_ms = file_start.elapsed().as_millis();
                        let end_event = BatchTranslationEvent::new(
                            "emit_end",
                            index + 1,
                            path.clone(),
                            elapsed_ms,
                            "ok",
                        );
                        if let Some(event_sink) = event_sink {
                            event_sink(end_event.clone());
                        }
                        worker_results.push((
                            index,
                            BatchTranslation::new(path, translation.rust, translation.diagnostics),
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
    let semantic_program =
        program_with_synthetic_local_capture_types(semantic_program, &resolved.facts);
    let mut semantic = semantic_analysis::analyze_program_with_config(
        &semantic_program,
        &resolved.facts,
        external_facts,
        semantic_analysis_config_from_facts(external_facts),
    );
    // Step 3 of the 0002b design: the storage decision has to be visible to
    // *every* place that renders a type, or a struct field and a parameter of
    // the same Java type get different Rust spellings and the output does not
    // compile. The fact set is the only thing all of them already have.
    semantic.facts.storage_strategies = storage_strategy::plan_storage_strategies_from_facts(
        &semantic_program,
        &semantic.usage,
        &resolved.facts,
    );
    let representations = representation::plan_representations(&semantic_program, &semantic.facts);
    let resolved_support_program =
        support_program_context
            .as_ref()
            .map(|support_program_context| {
                resolve::resolve_program_with_source_program_and_facts(
                    support_program_context,
                    DiagnosticBag::new(),
                    source_program,
                    external_facts,
                )
                .program
            });
    let support_program = resolved_support_program
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
        &resolved.program,
        summary_program,
        context_sources_are_emitted,
    );
    for diagnostic in semantic.diagnostics {
        let (code, message) = diagnostic.into_parts();
        lowered.diagnostics_mut().warning(code, message);
    }
    report_post_lowering_translation_diagnostics(
        &resolved.program,
        &semantic.usage,
        &semantic.facts,
        lowered.diagnostics_mut(),
    );
    let (file, diagnostics) = lowered.into_parts();
    let rust = emit::emit_file(&file);
    Translation { rust, diagnostics }
}

fn program_with_synthetic_local_capture_types(mut program: Program, facts: &FactSet) -> Program {
    let mut emitted = BTreeSet::new();
    let mut local_types = Vec::new();
    for ty in &program.types {
        collect_synthetic_local_capture_types_from_type(ty, facts, &mut emitted, &mut local_types);
    }
    program.types.extend(local_types);
    program
}

fn collect_synthetic_local_capture_types_from_type(
    ty: &TypeDecl,
    facts: &FactSet,
    emitted: &mut BTreeSet<String>,
    out: &mut Vec<TypeDecl>,
) {
    for member in &ty.members {
        match member {
            Member::Method(method) => {
                if let Some(body) = &method.body {
                    collect_synthetic_local_capture_types_from_block(body, facts, emitted, out);
                }
            }
            Member::Constructor(constructor) => {
                if let Some(body) = &constructor.body {
                    collect_synthetic_local_capture_types_from_block(body, facts, emitted, out);
                }
            }
            Member::Type(nested) => {
                collect_synthetic_local_capture_types_from_type(nested, facts, emitted, out);
            }
            _ => {}
        }
    }
}

fn collect_synthetic_local_capture_types_from_block(
    block: &Block,
    facts: &FactSet,
    emitted: &mut BTreeSet<String>,
    out: &mut Vec<TypeDecl>,
) {
    for stmt in &block.statements {
        collect_synthetic_local_capture_types_from_stmt(stmt, facts, emitted, out);
    }
}

fn collect_synthetic_local_capture_types_from_stmt(
    stmt: &Stmt,
    facts: &FactSet,
    emitted: &mut BTreeSet<String>,
    out: &mut Vec<TypeDecl>,
) {
    match stmt {
        Stmt::LocalType(local) => {
            collect_synthetic_local_capture_types_from_type(local, facts, emitted, out);
            if emitted.insert(local.name.clone())
                && let Some(owner) = facts.captures.owners.iter().find(|owner| {
                    owner.kind == facts::CaptureOwnerKind::LocalClass
                        && owner.name.as_deref() == Some(local.name.as_str())
                })
                && let Some(local) = local_type_with_synthetic_capture_constructor(local, owner)
            {
                out.push(local);
            }
        }
        Stmt::If {
            condition: _,
            then_block,
            else_block,
        } => {
            collect_synthetic_local_capture_types_from_block(then_block, facts, emitted, out);
            if let Some(else_block) = else_block {
                collect_synthetic_local_capture_types_from_block(else_block, facts, emitted, out);
            }
        }
        Stmt::While { body, .. }
        | Stmt::DoWhile { body, .. }
        | Stmt::EnhancedFor { body, .. }
        | Stmt::Block(body)
        | Stmt::LabeledBlock { body, .. } => {
            collect_synthetic_local_capture_types_from_block(body, facts, emitted, out);
        }
        Stmt::For { init, body, .. } => {
            for stmt in init {
                collect_synthetic_local_capture_types_from_stmt(stmt, facts, emitted, out);
            }
            collect_synthetic_local_capture_types_from_block(body, facts, emitted, out);
        }
        Stmt::Switch { cases, .. } => {
            for case in cases {
                for stmt in &case.statements {
                    collect_synthetic_local_capture_types_from_stmt(stmt, facts, emitted, out);
                }
            }
        }
        Stmt::Try {
            resources,
            body,
            catches,
            finally_block,
        } => {
            for resource in resources {
                collect_synthetic_local_capture_types_from_stmt(resource, facts, emitted, out);
            }
            collect_synthetic_local_capture_types_from_block(body, facts, emitted, out);
            for catch in catches {
                collect_synthetic_local_capture_types_from_block(&catch.body, facts, emitted, out);
            }
            if let Some(finally_block) = finally_block {
                collect_synthetic_local_capture_types_from_block(
                    finally_block,
                    facts,
                    emitted,
                    out,
                );
            }
        }
        Stmt::VarDecl { .. }
        | Stmt::Assign { .. }
        | Stmt::Expr(_)
        | Stmt::Return(_)
        | Stmt::Throw(_)
        | Stmt::ConstructorInvocation { .. }
        | Stmt::Break(_)
        | Stmt::Continue(_)
        | Stmt::Unsupported(_) => {}
    }
}

fn local_type_with_synthetic_capture_constructor(
    local: &TypeDecl,
    owner: &facts::CaptureOwner,
) -> Option<TypeDecl> {
    if local
        .members
        .iter()
        .any(|member| matches!(member, Member::Constructor(_)))
        || owner.captures.iter().any(|capture| {
            !matches!(
                capture.source,
                facts::CaptureSource::Local | facts::CaptureSource::Parameter
            ) || !capture.effectively_final
                || capture.mutable_after_capture
        })
    {
        return None;
    }
    let mut local = local.clone();
    let mut members = owner
        .captures
        .iter()
        .map(|capture| {
            Member::Field(java_ir::FieldDecl {
                name: capture.name.clone(),
                ty: capture.ty.clone(),
                modifiers: vec![java_ir::Modifier::Private, java_ir::Modifier::Final],
                initializer: None,
                span: Default::default(),
            })
        })
        .collect::<Vec<_>>();
    members.push(Member::Constructor(java_ir::ConstructorDecl {
        name: local.name.clone(),
        rust_name: Some("new".to_string()),
        type_params: Vec::new(),
        type_param_bounds: Vec::new(),
        params: owner
            .captures
            .iter()
            .map(|capture| java_ir::Param {
                name: capture.name.clone(),
                source_name: capture.name.clone(),
                ty: capture.ty.clone(),
                is_varargs: false,
                binding: None,
            })
            .collect(),
        throws: Vec::new(),
        modifiers: Vec::new(),
        body: Some(Block {
            statements: owner
                .captures
                .iter()
                .map(|capture| Stmt::Assign {
                    target: Expr::FieldAccess(Box::new(Expr::This), capture.name.clone()),
                    value: Expr::Var(capture.name.clone()),
                })
                .collect(),
        }),
        span: Default::default(),
    }));
    members.extend(local.members);
    local.members = members;
    Some(local)
}

fn report_post_lowering_translation_diagnostics(
    program: &Program,
    usage: &usage::UsageFacts,
    facts: &FactSet,
    diagnostics: &mut DiagnosticBag,
) {
    // Lowering copies where Java aliased, because there is no shared
    // representation to lower an alias into. Where that copy is observable the
    // output compiles and computes a different answer, so it is reported here
    // rather than left to be discovered downstream.
    aliasing::report_observable_aliases(program, usage, facts, diagnostics);
    // Step 2 of the 0002b design: the decision rule runs, lowering does not
    // change, and every type the plan says should be shared while lowering
    // holds it by value is reported. That list is the work queue for the
    // representation change and the size estimate it needs first.
    storage_strategy::report_storage_disagreements(program, usage, facts, diagnostics);
    // A non-static inner class compiles and then panics on construction. The
    // translator has to say so; a compile-error count cannot see it.
    inner_class_capture::report_unconstructable_inner_classes(program, facts, diagnostics);
}

fn semantic_analysis_config_from_facts(
    external_facts: &FactSet,
) -> semantic_analysis::SemanticAnalysisConfig {
    semantic_analysis::SemanticAnalysisConfig::default()
        .with_builtin_external_packs_enabled(!external_facts.disable_builtin_external_packs)
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

fn qualify_duplicate_program_type_names(programs: &mut [&mut Program]) {
    let mut packages_by_simple_name: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for program in programs.iter() {
        let package = program.package.clone().unwrap_or_default();
        for ty in &program.types {
            if !ty.name.contains('.') {
                packages_by_simple_name
                    .entry(ty.name.clone())
                    .or_default()
                    .insert(package.clone());
            }
        }
    }
    let duplicate_names = packages_by_simple_name
        .into_iter()
        .filter_map(|(name, packages)| (packages.len() > 1).then_some(name))
        .collect::<BTreeSet<_>>();

    #[derive(Debug, Clone)]
    struct TypeNamePlan {
        program_index: usize,
        type_index: usize,
        package: String,
        old_name: String,
        source_name: String,
        planned_name: String,
    }

    let mut plans = Vec::new();
    for (program_index, program) in programs.iter().enumerate() {
        let package = program.package.clone().unwrap_or_default();
        for (type_index, ty) in program.types.iter().enumerate() {
            let source_name = source_type_qualified_name(program.package.as_deref(), &ty.name);
            let planned_name = if !package.is_empty()
                && duplicate_names.contains(&ty.name)
                && let Some(qualified) = qualified_merge_type_name(&package, &ty.name)
            {
                qualified
            } else {
                ty.name.clone()
            };
            plans.push(TypeNamePlan {
                program_index,
                type_index,
                package: package.clone(),
                old_name: ty.name.clone(),
                source_name,
                planned_name,
            });
        }
    }

    let mut order = (0..plans.len()).collect::<Vec<_>>();
    order.sort_by_key(|index| {
        !plans[*index].package.is_empty() && !plans[*index].old_name.contains('.')
    });
    let mut occupied_rust_names: BTreeMap<String, String> = BTreeMap::new();
    for index in order {
        let mut attempt = 0;
        loop {
            let rust_name =
                crate::java_naming::rust_name_for_java_named_type(&plans[index].planned_name);
            if occupied_rust_names
                .get(&rust_name)
                .is_none_or(|source_name| source_name == &plans[index].source_name)
            {
                occupied_rust_names.insert(rust_name, plans[index].source_name.clone());
                break;
            }
            if let Some(disambiguated) = disambiguated_merge_type_name(
                &plans[index].package,
                &plans[index].old_name,
                attempt,
            ) {
                plans[index].planned_name = disambiguated;
                attempt += 1;
            } else {
                break;
            }
        }
    }

    let changes = plans
        .into_iter()
        .filter(|plan| plan.old_name != plan.planned_name)
        .collect::<Vec<_>>();
    if changes.is_empty() {
        return;
    }

    for change in &changes {
        let ty = &mut programs[change.program_index].types[change.type_index];
        ty.name = change.planned_name.clone();
    }
    for change in changes {
        let old_source_name = source_type_qualified_name(Some(&change.package), &change.old_name);
        for program in programs.iter_mut() {
            rewrite_type_references_in_program(program, &old_source_name, &change.planned_name);
        }
        if !change.package.is_empty() && !change.old_name.contains('.') {
            rewrite_type_references_in_program(
                programs[change.program_index],
                &change.old_name,
                &change.planned_name,
            );
        }
    }
}

fn source_type_qualified_name(package: Option<&str>, name: &str) -> String {
    if name.contains('.') {
        return name.to_string();
    }
    package
        .filter(|package| !package.is_empty())
        .map(|package| format!("{package}.{name}"))
        .unwrap_or_else(|| name.to_string())
}

fn disambiguated_merge_type_name(package: &str, type_name: &str, attempt: usize) -> Option<String> {
    if package.is_empty() || type_name.contains('.') {
        return None;
    }
    let marker = if attempt == 0 {
        "JavaType".to_string()
    } else {
        format!("JavaType{attempt}")
    };
    Some(format!("{package}.{marker}.{type_name}"))
}

fn rewrite_type_references_in_program(program: &mut Program, old_name: &str, new_name: &str) {
    for ty in &mut program.types {
        qualify_type_references_in_type_with_name(ty, old_name, new_name);
    }
}

fn qualify_type_references_in_type_with_name(ty: &mut TypeDecl, old_name: &str, new_name: &str) {
    rewrite_type_name_in_string_option(&mut ty.extends, old_name, new_name);
    for implemented in &mut ty.implements {
        rewrite_type_name_in_string(implemented, old_name, new_name);
    }
    for bound in &mut ty.type_param_bounds {
        rewrite_type_name_in_java_type_option(bound, old_name, new_name);
    }
    for block in &mut ty.static_init_blocks {
        rewrite_type_name_in_block(&mut block.block, old_name, new_name);
    }
    for member in &mut ty.members {
        rewrite_type_name_in_member(member, old_name, new_name);
    }
}

fn qualify_type_references_in_type(ty: &mut TypeDecl, old_name: &str) {
    let new_name = ty.name.clone();
    qualify_type_references_in_type_with_name(ty, old_name, &new_name);
}

fn rewrite_type_name_in_member(member: &mut Member, old_name: &str, new_name: &str) {
    match member {
        Member::Field(field) => {
            rewrite_type_name_in_java_type(&mut field.ty, old_name, new_name);
            if let Some(initializer) = &mut field.initializer {
                rewrite_type_name_in_expr(initializer, old_name, new_name);
            }
        }
        Member::Method(method) => {
            rewrite_type_name_in_java_type(&mut method.return_type, old_name, new_name);
            for bound in &mut method.type_param_bounds {
                rewrite_type_name_in_java_type_option(bound, old_name, new_name);
            }
            for param in &mut method.params {
                rewrite_type_name_in_java_type(&mut param.ty, old_name, new_name);
            }
            if let Some(body) = &mut method.body {
                rewrite_type_name_in_block(body, old_name, new_name);
            }
        }
        Member::Constructor(constructor) => {
            for bound in &mut constructor.type_param_bounds {
                rewrite_type_name_in_java_type_option(bound, old_name, new_name);
            }
            for param in &mut constructor.params {
                rewrite_type_name_in_java_type(&mut param.ty, old_name, new_name);
            }
            if let Some(body) = &mut constructor.body {
                rewrite_type_name_in_block(body, old_name, new_name);
            }
        }
        Member::Type(nested) => qualify_type_references_in_type(nested, old_name),
        Member::Unsupported { .. } => {}
    }
}

fn rewrite_type_name_in_block(block: &mut Block, old_name: &str, new_name: &str) {
    for stmt in &mut block.statements {
        rewrite_type_name_in_stmt(stmt, old_name, new_name);
    }
}

fn rewrite_type_name_in_stmt(stmt: &mut Stmt, old_name: &str, new_name: &str) {
    match stmt {
        Stmt::VarDecl {
            ty, initializer, ..
        } => {
            rewrite_type_name_in_java_type(ty, old_name, new_name);
            if let Some(initializer) = initializer {
                rewrite_type_name_in_expr(initializer, old_name, new_name);
            }
        }
        Stmt::Assign { target, value } => {
            rewrite_type_name_in_expr(target, old_name, new_name);
            rewrite_type_name_in_expr(value, old_name, new_name);
        }
        Stmt::Expr(expr) | Stmt::Return(Some(expr)) | Stmt::Throw(expr) => {
            rewrite_type_name_in_expr(expr, old_name, new_name);
        }
        Stmt::Return(None) | Stmt::Break(_) | Stmt::Continue(_) | Stmt::Unsupported(_) => {}
        Stmt::If {
            condition,
            then_block,
            else_block,
        } => {
            rewrite_type_name_in_expr(condition, old_name, new_name);
            rewrite_type_name_in_block(then_block, old_name, new_name);
            if let Some(else_block) = else_block {
                rewrite_type_name_in_block(else_block, old_name, new_name);
            }
        }
        Stmt::While {
            condition, body, ..
        }
        | Stmt::DoWhile {
            condition, body, ..
        } => {
            rewrite_type_name_in_expr(condition, old_name, new_name);
            rewrite_type_name_in_block(body, old_name, new_name);
        }
        Stmt::For {
            init,
            condition,
            update,
            body,
            ..
        } => {
            for stmt in init {
                rewrite_type_name_in_stmt(stmt, old_name, new_name);
            }
            if let Some(condition) = condition {
                rewrite_type_name_in_expr(condition, old_name, new_name);
            }
            for expr in update {
                rewrite_type_name_in_expr(expr, old_name, new_name);
            }
            rewrite_type_name_in_block(body, old_name, new_name);
        }
        Stmt::EnhancedFor {
            var_type,
            iterable,
            body,
            ..
        } => {
            rewrite_type_name_in_java_type(var_type, old_name, new_name);
            rewrite_type_name_in_expr(iterable, old_name, new_name);
            rewrite_type_name_in_block(body, old_name, new_name);
        }
        Stmt::Switch { selector, cases } => {
            rewrite_type_name_in_expr(selector, old_name, new_name);
            for case in cases {
                for label in &mut case.labels {
                    rewrite_type_name_in_switch_label(label, old_name, new_name);
                }
                for stmt in &mut case.statements {
                    rewrite_type_name_in_stmt(stmt, old_name, new_name);
                }
            }
        }
        Stmt::Try {
            resources,
            body,
            catches,
            finally_block,
        } => {
            for resource in resources {
                rewrite_type_name_in_stmt(resource, old_name, new_name);
            }
            rewrite_type_name_in_block(body, old_name, new_name);
            for catch in catches {
                for ty in &mut catch.types {
                    rewrite_type_name_in_java_type(ty, old_name, new_name);
                }
                rewrite_type_name_in_block(&mut catch.body, old_name, new_name);
            }
            if let Some(finally_block) = finally_block {
                rewrite_type_name_in_block(finally_block, old_name, new_name);
            }
        }
        Stmt::LocalType(ty) => qualify_type_references_in_type(ty, old_name),
        Stmt::ConstructorInvocation { args, resolved, .. } => {
            for arg in args {
                rewrite_type_name_in_expr(arg, old_name, new_name);
            }
            if let Some(resolved) = resolved {
                rewrite_type_name_in_resolved_constructor(resolved, old_name, new_name);
            }
        }
        Stmt::Block(block) | Stmt::LabeledBlock { body: block, .. } => {
            rewrite_type_name_in_block(block, old_name, new_name);
        }
    }
}

fn rewrite_type_name_in_switch_label(label: &mut SwitchLabel, old_name: &str, new_name: &str) {
    if let SwitchLabel::Expr(expr) = label {
        rewrite_type_name_in_expr(expr, old_name, new_name);
    }
}

fn rewrite_type_name_in_expr(expr: &mut Expr, old_name: &str, new_name: &str) {
    match expr {
        Expr::Literal(_) | Expr::This | Expr::Super | Expr::Raw(_) | Expr::Unsupported { .. } => {}
        Expr::Var(name) => {
            if name == old_name {
                *name = new_name.to_string();
            }
        }
        Expr::FieldAccess(receiver, _) => rewrite_type_name_in_expr(receiver, old_name, new_name),
        Expr::ArrayAccess(array, index) => {
            rewrite_type_name_in_expr(array, old_name, new_name);
            rewrite_type_name_in_expr(index, old_name, new_name);
        }
        Expr::ArrayCreation {
            element_type,
            dimensions,
            initializer,
        } => {
            rewrite_type_name_in_java_type(element_type, old_name, new_name);
            for dimension in dimensions {
                rewrite_type_name_in_expr(dimension, old_name, new_name);
            }
            if let Some(initializer) = initializer {
                for expr in initializer {
                    rewrite_type_name_in_expr(expr, old_name, new_name);
                }
            }
        }
        Expr::ArrayInitializer(items) => {
            for item in items {
                rewrite_type_name_in_expr(item, old_name, new_name);
            }
        }
        Expr::Binary { left, right, .. } => {
            rewrite_type_name_in_expr(left, old_name, new_name);
            rewrite_type_name_in_expr(right, old_name, new_name);
        }
        Expr::Unary { expr, .. } => rewrite_type_name_in_expr(expr, old_name, new_name),
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => {
            rewrite_type_name_in_expr(condition, old_name, new_name);
            rewrite_type_name_in_expr(then_expr, old_name, new_name);
            rewrite_type_name_in_expr(else_expr, old_name, new_name);
        }
        Expr::Switch { selector, cases } => {
            rewrite_type_name_in_expr(selector, old_name, new_name);
            for case in cases {
                for label in &mut case.labels {
                    rewrite_type_name_in_switch_label(label, old_name, new_name);
                }
                rewrite_type_name_in_expr(&mut case.value, old_name, new_name);
            }
        }
        Expr::Call {
            receiver,
            args,
            resolved,
            ..
        } => {
            if let Some(receiver) = receiver {
                rewrite_type_name_in_expr(receiver, old_name, new_name);
            }
            for arg in args {
                rewrite_type_name_in_expr(arg, old_name, new_name);
            }
            if let Some(resolved) = resolved {
                rewrite_type_name_in_resolved_call(resolved, old_name, new_name);
            }
        }
        Expr::New {
            ty,
            qualifier,
            args,
            body,
            resolved,
        } => {
            rewrite_type_name_in_java_type(ty, old_name, new_name);
            if let Some(qualifier) = qualifier {
                rewrite_type_name_in_expr(qualifier, old_name, new_name);
            }
            for arg in args {
                rewrite_type_name_in_expr(arg, old_name, new_name);
            }
            if let Some(body) = body {
                for member in body {
                    rewrite_type_name_in_member(member, old_name, new_name);
                }
            }
            if let Some(resolved) = resolved {
                rewrite_type_name_in_resolved_constructor(resolved, old_name, new_name);
            }
        }
        Expr::Cast { ty, expr } | Expr::InstanceOf { ty, expr, .. } => {
            rewrite_type_name_in_java_type(ty, old_name, new_name);
            rewrite_type_name_in_expr(expr, old_name, new_name);
        }
        Expr::Lambda { params, body } => {
            for param in params {
                if let Some(ty) = &mut param.ty {
                    rewrite_type_name_in_java_type(ty, old_name, new_name);
                }
            }
            match body {
                LambdaBody::Expr(expr) => rewrite_type_name_in_expr(expr, old_name, new_name),
                LambdaBody::Block(block) => rewrite_type_name_in_block(block, old_name, new_name),
            }
        }
        Expr::MethodReference { target, .. } => {
            if target == old_name {
                *target = new_name.to_string();
            }
        }
        Expr::ClassLiteral(ty) => rewrite_type_name_in_java_type(ty, old_name, new_name),
    }
}

fn rewrite_type_name_in_resolved_call(
    resolved: &mut java_ir::ResolvedCall,
    old_name: &str,
    new_name: &str,
) {
    if resolved.declaring_type == old_name {
        resolved.declaring_type = new_name.to_string();
    }
    rewrite_type_name_in_java_type(&mut resolved.return_type, old_name, new_name);
    for ty in &mut resolved.arg_target_types {
        rewrite_type_name_in_java_type(ty, old_name, new_name);
    }
    for coercion in &mut resolved.arg_coercions {
        rewrite_type_name_in_java_type(&mut coercion.actual, old_name, new_name);
        rewrite_type_name_in_java_type(&mut coercion.target, old_name, new_name);
    }
}

fn rewrite_type_name_in_resolved_constructor(
    resolved: &mut java_ir::ResolvedConstructor,
    old_name: &str,
    new_name: &str,
) {
    if resolved.declaring_type == old_name {
        resolved.declaring_type = new_name.to_string();
    }
    rewrite_type_name_in_java_type(&mut resolved.constructed_type, old_name, new_name);
    for param in &mut resolved.params {
        rewrite_type_name_in_java_type(&mut param.ty, old_name, new_name);
    }
    for coercion in &mut resolved.arg_coercions {
        rewrite_type_name_in_java_type(&mut coercion.actual, old_name, new_name);
        rewrite_type_name_in_java_type(&mut coercion.target, old_name, new_name);
    }
}

fn rewrite_type_name_in_java_type_option(
    ty: &mut Option<JavaType>,
    old_name: &str,
    new_name: &str,
) {
    if let Some(ty) = ty {
        rewrite_type_name_in_java_type(ty, old_name, new_name);
    }
}

fn rewrite_type_name_in_string_option(ty: &mut Option<String>, old_name: &str, new_name: &str) {
    if let Some(ty) = ty {
        rewrite_type_name_in_string(ty, old_name, new_name);
    }
}

fn rewrite_type_name_in_string(ty: &mut String, old_name: &str, new_name: &str) {
    if ty == old_name {
        *ty = new_name.to_string();
    }
}

fn rewrite_type_name_in_java_type(ty: &mut JavaType, old_name: &str, new_name: &str) {
    match ty {
        JavaType::Named { name, args } => {
            if name == old_name {
                *name = new_name.to_string();
            }
            for arg in args {
                rewrite_type_name_in_java_type(arg, old_name, new_name);
            }
        }
        JavaType::Array(inner) => rewrite_type_name_in_java_type(inner, old_name, new_name),
        JavaType::Wildcard(Some(WildcardBound::Extends(bound)))
        | JavaType::Wildcard(Some(WildcardBound::Super(bound))) => {
            rewrite_type_name_in_java_type(bound, old_name, new_name);
        }
        JavaType::Void
        | JavaType::Primitive(_)
        | JavaType::String
        | JavaType::Wildcard(None)
        | JavaType::Unknown(_) => {}
    }
}

fn qualified_merge_type_name(package: &str, type_name: &str) -> Option<String> {
    if package.is_empty() || type_name.contains('.') {
        return None;
    }
    Some(format!("{package}.{type_name}"))
}

fn qualify_same_package_class_literals(program: &mut Program) {
    let Some(package) = program.package.as_deref() else {
        return;
    };
    if package.is_empty() {
        return;
    }
    let top_level_type_names = program
        .types
        .iter()
        .map(|ty| ty.name.rsplit('.').next().unwrap_or(&ty.name).to_string())
        .collect::<BTreeSet<_>>();
    if top_level_type_names.is_empty() {
        return;
    }
    for ty in &mut program.types {
        qualify_same_package_class_literals_in_type(ty, package, &top_level_type_names);
    }
}

fn qualify_same_package_class_literals_in_type(
    ty: &mut TypeDecl,
    package: &str,
    top_level_type_names: &BTreeSet<String>,
) {
    for member in &mut ty.members {
        qualify_same_package_class_literals_in_member(member, package, top_level_type_names);
    }
    for static_init in &mut ty.static_init_blocks {
        qualify_same_package_class_literals_in_block(
            &mut static_init.block,
            package,
            top_level_type_names,
        );
    }
}

fn qualify_same_package_class_literals_in_member(
    member: &mut Member,
    package: &str,
    top_level_type_names: &BTreeSet<String>,
) {
    match member {
        Member::Field(field) => {
            if let Some(initializer) = &mut field.initializer {
                qualify_same_package_class_literals_in_expr(
                    initializer,
                    package,
                    top_level_type_names,
                );
            }
        }
        Member::Method(method) => {
            if let Some(body) = &mut method.body {
                qualify_same_package_class_literals_in_block(body, package, top_level_type_names);
            }
        }
        Member::Constructor(ctor) => {
            if let Some(body) = &mut ctor.body {
                qualify_same_package_class_literals_in_block(body, package, top_level_type_names);
            }
        }
        Member::Type(nested) => {
            qualify_same_package_class_literals_in_type(nested, package, top_level_type_names);
        }
        Member::Unsupported { .. } => {}
    }
}

fn qualify_same_package_class_literals_in_block(
    block: &mut Block,
    package: &str,
    top_level_type_names: &BTreeSet<String>,
) {
    for stmt in &mut block.statements {
        qualify_same_package_class_literals_in_stmt(stmt, package, top_level_type_names);
    }
}

fn qualify_same_package_class_literals_in_stmt(
    stmt: &mut Stmt,
    package: &str,
    top_level_type_names: &BTreeSet<String>,
) {
    match stmt {
        Stmt::VarDecl { initializer, .. } => {
            if let Some(initializer) = initializer {
                qualify_same_package_class_literals_in_expr(
                    initializer,
                    package,
                    top_level_type_names,
                );
            }
        }
        Stmt::Assign { target, value } => {
            qualify_same_package_class_literals_in_expr(target, package, top_level_type_names);
            qualify_same_package_class_literals_in_expr(value, package, top_level_type_names);
        }
        Stmt::Expr(expr) | Stmt::Throw(expr) => {
            qualify_same_package_class_literals_in_expr(expr, package, top_level_type_names);
        }
        Stmt::Return(expr) => {
            if let Some(expr) = expr {
                qualify_same_package_class_literals_in_expr(expr, package, top_level_type_names);
            }
        }
        Stmt::If {
            condition,
            then_block,
            else_block,
        } => {
            qualify_same_package_class_literals_in_expr(condition, package, top_level_type_names);
            qualify_same_package_class_literals_in_block(then_block, package, top_level_type_names);
            if let Some(else_block) = else_block {
                qualify_same_package_class_literals_in_block(
                    else_block,
                    package,
                    top_level_type_names,
                );
            }
        }
        Stmt::While {
            condition, body, ..
        }
        | Stmt::DoWhile {
            condition, body, ..
        } => {
            qualify_same_package_class_literals_in_expr(condition, package, top_level_type_names);
            qualify_same_package_class_literals_in_block(body, package, top_level_type_names);
        }
        Stmt::For {
            init,
            condition,
            update,
            body,
            ..
        } => {
            for init in init {
                qualify_same_package_class_literals_in_stmt(init, package, top_level_type_names);
            }
            if let Some(condition) = condition {
                qualify_same_package_class_literals_in_expr(
                    condition,
                    package,
                    top_level_type_names,
                );
            }
            for update in update {
                qualify_same_package_class_literals_in_expr(update, package, top_level_type_names);
            }
            qualify_same_package_class_literals_in_block(body, package, top_level_type_names);
        }
        Stmt::EnhancedFor { iterable, body, .. } => {
            qualify_same_package_class_literals_in_expr(iterable, package, top_level_type_names);
            qualify_same_package_class_literals_in_block(body, package, top_level_type_names);
        }
        Stmt::Switch { selector, cases } => {
            qualify_same_package_class_literals_in_expr(selector, package, top_level_type_names);
            for case in cases {
                for label in &mut case.labels {
                    if let SwitchLabel::Expr(expr) = label {
                        qualify_same_package_class_literals_in_expr(
                            expr,
                            package,
                            top_level_type_names,
                        );
                    }
                }
                for stmt in &mut case.statements {
                    qualify_same_package_class_literals_in_stmt(
                        stmt,
                        package,
                        top_level_type_names,
                    );
                }
            }
        }
        Stmt::Try {
            resources,
            body,
            catches,
            finally_block,
        } => {
            for resource in resources {
                qualify_same_package_class_literals_in_stmt(
                    resource,
                    package,
                    top_level_type_names,
                );
            }
            qualify_same_package_class_literals_in_block(body, package, top_level_type_names);
            for catch in catches {
                qualify_same_package_class_literals_in_block(
                    &mut catch.body,
                    package,
                    top_level_type_names,
                );
            }
            if let Some(finally_block) = finally_block {
                qualify_same_package_class_literals_in_block(
                    finally_block,
                    package,
                    top_level_type_names,
                );
            }
        }
        Stmt::ConstructorInvocation { args, .. } => {
            for arg in args {
                qualify_same_package_class_literals_in_expr(arg, package, top_level_type_names);
            }
        }
        Stmt::Block(block) | Stmt::LabeledBlock { body: block, .. } => {
            qualify_same_package_class_literals_in_block(block, package, top_level_type_names);
        }
        Stmt::LocalType(nested) => {
            qualify_same_package_class_literals_in_type(nested, package, top_level_type_names);
        }
        Stmt::Break(_) | Stmt::Continue(_) | Stmt::Unsupported(_) => {}
    }
}

fn qualify_same_package_class_literals_in_expr(
    expr: &mut Expr,
    package: &str,
    top_level_type_names: &BTreeSet<String>,
) {
    match expr {
        Expr::FieldAccess(receiver, _) => {
            qualify_same_package_class_literals_in_expr(receiver, package, top_level_type_names);
        }
        Expr::ArrayAccess(array, index) => {
            qualify_same_package_class_literals_in_expr(array, package, top_level_type_names);
            qualify_same_package_class_literals_in_expr(index, package, top_level_type_names);
        }
        Expr::ArrayCreation {
            dimensions,
            initializer,
            ..
        } => {
            for dimension in dimensions {
                qualify_same_package_class_literals_in_expr(
                    dimension,
                    package,
                    top_level_type_names,
                );
            }
            if let Some(initializer) = initializer {
                for element in initializer {
                    qualify_same_package_class_literals_in_expr(
                        element,
                        package,
                        top_level_type_names,
                    );
                }
            }
        }
        Expr::ArrayInitializer(elements) => {
            for element in elements {
                qualify_same_package_class_literals_in_expr(element, package, top_level_type_names);
            }
        }
        Expr::Binary { left, right, .. } => {
            qualify_same_package_class_literals_in_expr(left, package, top_level_type_names);
            qualify_same_package_class_literals_in_expr(right, package, top_level_type_names);
        }
        Expr::Unary { expr, .. } => {
            qualify_same_package_class_literals_in_expr(expr, package, top_level_type_names);
        }
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => {
            qualify_same_package_class_literals_in_expr(condition, package, top_level_type_names);
            qualify_same_package_class_literals_in_expr(then_expr, package, top_level_type_names);
            qualify_same_package_class_literals_in_expr(else_expr, package, top_level_type_names);
        }
        Expr::Switch { selector, cases } => {
            qualify_same_package_class_literals_in_expr(selector, package, top_level_type_names);
            for case in cases {
                for label in &mut case.labels {
                    if let SwitchLabel::Expr(expr) = label {
                        qualify_same_package_class_literals_in_expr(
                            expr,
                            package,
                            top_level_type_names,
                        );
                    }
                }
                qualify_same_package_class_literals_in_expr(
                    &mut case.value,
                    package,
                    top_level_type_names,
                );
            }
        }
        Expr::Call { receiver, args, .. } => {
            if let Some(receiver) = receiver {
                qualify_same_package_class_literals_in_expr(
                    receiver,
                    package,
                    top_level_type_names,
                );
            }
            for arg in args {
                qualify_same_package_class_literals_in_expr(arg, package, top_level_type_names);
            }
        }
        Expr::New { args, body, .. } => {
            for arg in args {
                qualify_same_package_class_literals_in_expr(arg, package, top_level_type_names);
            }
            if let Some(body) = body {
                for member in body {
                    qualify_same_package_class_literals_in_member(
                        member,
                        package,
                        top_level_type_names,
                    );
                }
            }
        }
        Expr::Cast { expr, .. } | Expr::InstanceOf { expr, .. } => {
            qualify_same_package_class_literals_in_expr(expr, package, top_level_type_names);
        }
        Expr::Lambda { body, .. } => match body {
            LambdaBody::Expr(expr) => {
                qualify_same_package_class_literals_in_expr(expr, package, top_level_type_names);
            }
            LambdaBody::Block(block) => {
                qualify_same_package_class_literals_in_block(block, package, top_level_type_names);
            }
        },
        Expr::ClassLiteral(ty) => {
            qualify_same_package_class_literal_type(ty, package, top_level_type_names);
        }
        Expr::Literal(_)
        | Expr::Var(_)
        | Expr::This
        | Expr::Super
        | Expr::MethodReference { .. }
        | Expr::Unsupported { .. }
        | Expr::Raw(_) => {}
    }
}

fn qualify_same_package_class_literal_type(
    ty: &mut JavaType,
    package: &str,
    top_level_type_names: &BTreeSet<String>,
) {
    match ty {
        JavaType::Named { name, args } => {
            if !name.contains('.') && top_level_type_names.contains(name) {
                *name = format!("{package}.{name}");
            }
            for arg in args {
                qualify_same_package_class_literal_type(arg, package, top_level_type_names);
            }
        }
        JavaType::Array(inner) => {
            qualify_same_package_class_literal_type(inner, package, top_level_type_names);
        }
        JavaType::Wildcard(Some(java_ir::WildcardBound::Extends(bound)))
        | JavaType::Wildcard(Some(java_ir::WildcardBound::Super(bound))) => {
            qualify_same_package_class_literal_type(bound, package, top_level_type_names);
        }
        JavaType::Void
        | JavaType::String
        | JavaType::Primitive(_)
        | JavaType::Wildcard(None)
        | JavaType::Unknown(_) => {}
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::java_ir::{JavaType, Member, MethodDecl, TypeDecl};
    use crate::java_signature::JavaCallableKey;

    fn source_who_methods(
        context: &BatchTranslationContext,
    ) -> Vec<(&TypeDecl, &MethodDecl, JavaCallableKey)> {
        let mut methods = context
            .support_program
            .types
            .iter()
            .filter_map(|ty| {
                let method = ty.members.iter().find_map(|member| match member {
                    Member::Method(method) if method.name == "who" => Some(method),
                    _ => None,
                })?;
                Some((
                    ty,
                    method,
                    JavaCallableKey::source_method_for_type(ty, method),
                ))
            })
            .collect::<Vec<_>>();
        methods.sort_by(|left, right| left.2.owner().cmp(right.2.owner()));
        methods
    }

    fn assert_package_peer_who_keys_stay_in_java_owner_space(
        context: &BatchTranslationContext,
        methods: &[(&TypeDecl, &MethodDecl, JavaCallableKey)],
    ) {
        assert_eq!(
            methods
                .iter()
                .map(|(_, _, key)| key.owner())
                .collect::<Vec<_>>(),
            vec!["a.Item", "b.Item"]
        );
        assert!(
            !context
                .semantic
                .usage
                .source_method_usage_has_collapsed_owner_alias(&["Item"], "who"),
            "same-simple-name package peers must not collapse onto Item::who"
        );
        assert!(
            !context
                .semantic
                .usage
                .source_method_usage_has_collapsed_owner_alias(&["AItem", "BItem"], "who"),
            "analysis identity should stay in Java owner space, not emitted Rust spelling"
        );
    }

    #[test]
    fn batch_analysis_keys_keep_same_simple_name_package_peers_distinct() {
        let mut a = parser::parse_java_source(
            r#"
package a;

public class Item {
    public int who() { return 1; }
}
"#,
        );
        let mut b = parser::parse_java_source(
            r#"
package b;

public class Item {
    public int who() { return 2; }
}
"#,
        );
        qualify_duplicate_program_type_names(&mut [&mut a.program, &mut b.program]);

        let mut source_program = Program::default();
        merge_program_context(&mut source_program, a.program);
        merge_program_context(&mut source_program, b.program);
        let external_facts = effective_external_facts_with_stdlib(&FactSet::default(), None);
        let context = build_batch_translation_context(
            source_program,
            Program::default(),
            true,
            &external_facts,
            None,
            Instant::now(),
        )
        .context;

        let methods = source_who_methods(&context);
        for (ty, method, key) in &methods {
            assert!(
                context
                    .semantic
                    .usage
                    .source_method_usage(ty, method)
                    .is_some(),
                "usage facts missing {key:?}"
            );
            assert!(
                context
                    .signatures
                    .method_signature_by_java_key(key)
                    .is_some(),
                "signature plan missing {key:?}"
            );
        }
        assert!(
            context
                .semantic
                .usage
                .current_rust_method_usage("a.Item", "who")
                .is_some(),
            "exact Java owner should remain available through the legacy Rust-name map"
        );
        assert!(
            context
                .semantic
                .usage
                .current_rust_method_usage("b.Item", "who")
                .is_some(),
            "exact Java owner should remain available through the legacy Rust-name map"
        );
        assert!(
            context
                .semantic
                .usage
                .current_rust_method_usage("Item", "who")
                .is_none(),
            "bare same-simple owner must not borrow either package peer's usage facts"
        );
        assert!(
            context
                .signatures
                .method_signature_by_base_name_and_arity("a.Item", "who", 0)
                .is_some(),
            "exact Java owner should keep its signature"
        );
        assert!(
            context
                .signatures
                .method_signature_by_base_name_and_arity("b.Item", "who", 0)
                .is_some(),
            "exact Java owner should keep its signature"
        );
        assert!(
            context
                .signatures
                .method_signature_by_base_name_and_arity("Item", "who", 0)
                .is_none(),
            "bare same-simple owner must fail closed instead of selecting a package peer"
        );
        assert_package_peer_who_keys_stay_in_java_owner_space(&context, &methods);
    }

    #[test]
    fn duplicate_program_prepass_avoids_projected_rust_type_name_collisions() {
        let mut root = parser::parse_java_source(
            r#"
public class ABC {
    public int id() { return 0; }
}
"#,
        );
        let mut ab = parser::parse_java_source(
            r#"
package a.b;

public class C {
    public C self() { return new C(); }
}
"#,
        );
        let mut x = parser::parse_java_source(
            r#"
package x;

public class C {
    public int id() { return 1; }
}
"#,
        );
        let mut use_ab = parser::parse_java_source(
            r#"
package usecase;

public class UseAB {
    public a.b.C make() { return new a.b.C(); }
}
"#,
        );

        qualify_duplicate_program_type_names(&mut [
            &mut root.program,
            &mut ab.program,
            &mut x.program,
            &mut use_ab.program,
        ]);

        let java_names = [
            root.program.types[0].name.as_str(),
            ab.program.types[0].name.as_str(),
            x.program.types[0].name.as_str(),
        ];
        let rust_names = java_names
            .iter()
            .map(|name| crate::java_naming::rust_name_for_java_named_type(name))
            .collect::<BTreeSet<_>>();

        assert_eq!(
            rust_names.len(),
            java_names.len(),
            "projected Rust declaration names should remain injective: {java_names:?}"
        );
        assert_eq!(root.program.types[0].name, "ABC");
        assert_eq!(ab.program.types[0].name, "a.b.JavaType.C");
        assert_eq!(x.program.types[0].name, "x.C");
        assert_eq!(
            rust_names,
            BTreeSet::from([
                "ABC".to_string(),
                "ABJavaType_C".to_string(),
                "XC".to_string()
            ])
        );

        let self_method = ab.program.types[0]
            .members
            .iter()
            .find_map(|member| match member {
                Member::Method(method) if method.name == "self" => Some(method),
                _ => None,
            })
            .expect("self method should be parsed");
        assert_eq!(
            self_method.return_type,
            JavaType::Named {
                name: "a.b.JavaType.C".to_string(),
                args: Vec::new(),
            }
        );

        let make_method = use_ab.program.types[0]
            .members
            .iter()
            .find_map(|member| match member {
                Member::Method(method) if method.name == "make" => Some(method),
                _ => None,
            })
            .expect("make method should be parsed");
        assert_eq!(
            make_method.return_type,
            JavaType::Named {
                name: "a.b.JavaType.C".to_string(),
                args: Vec::new(),
            }
        );
    }
}
