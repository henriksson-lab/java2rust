use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use crate::dependency_mode::{DependencyFactProvider, DependencyMode, DependencyModeConfig};
use crate::diagnostics::Severity;
use crate::facts::{
    ExternalApiRole, ExternalLibraryMode, FactProvenance, FactSet, HierarchyOverride,
    HierarchyStrategy, UnresolvedTargetKind, external_type_name_matches,
};
use crate::java_ir::{
    Block, Expr, ImportDecl, JavaType, LambdaBody, Member, Param, Program, Stmt, SwitchLabel,
    TypeDecl,
};
use crate::parser::ParsedProgram;
use crate::types::TypeEnv;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResolverBenchmarkReport {
    pub root: PathBuf,
    pub source_roots: Vec<ResolverBenchmarkSourceRoot>,
    pub fact_providers: Vec<ResolverBenchmarkFactProvider>,
    pub configured_provider_facts: usize,
    pub configured_provider_external_api_effect_facts: usize,
    pub configured_provider_external_api_argument_effect_facts: usize,
    pub configured_provider_capture_facts: usize,
    pub configured_provider_trivial_accessor_facts: usize,
    pub configured_provider_method_effect_facts: usize,
    pub configured_provider_recursion_facts: usize,
    pub configured_provider_control_flow_facts: usize,
    pub configured_provider_dispatch_requirement_facts: usize,
    pub configured_provider_catch_facts: usize,
    pub configured_provider_external_crate_api_mapping_facts: usize,
    pub configured_provider_external_crate_constructor_mapping_facts: usize,
    pub configured_provider_external_constructor_facts: usize,
    pub configured_provider_external_field_facts: usize,
    pub configured_provider_hierarchy_override_facts: usize,
    pub configured_provider_closed_hierarchy_facts: usize,
    pub configured_provider_object_safety_facts: usize,
    pub configured_provider_type_capability_facts: usize,
    pub configured_provider_field_nullability_facts: usize,
    pub configured_provider_exception_facts: usize,
    pub configured_provider_external_library_pack_facts: usize,
    pub configured_provider_unsupported_external_api_facts: usize,
    pub configured_provider_unsupported_external_constructor_facts: usize,
    pub configured_provider_unsupported_external_field_facts: usize,
    pub source_summary_facts: usize,
    pub source_summary_root_type_capability_facts: usize,
    pub source_summary_root_object_safety_facts: usize,
    pub source_summary_root_closed_hierarchy_facts: usize,
    pub source_summary_provider_facts: usize,
    pub source_summary_provider_external_api_effect_facts: usize,
    pub source_summary_provider_external_api_argument_effect_facts: usize,
    pub source_summary_provider_capture_facts: usize,
    pub source_summary_provider_trivial_accessor_facts: usize,
    pub source_summary_provider_method_effect_facts: usize,
    pub source_summary_provider_recursion_facts: usize,
    pub source_summary_provider_control_flow_facts: usize,
    pub source_summary_provider_dispatch_requirement_facts: usize,
    pub source_summary_provider_catch_facts: usize,
    pub source_summary_provider_external_crate_api_mapping_facts: usize,
    pub source_summary_provider_external_crate_constructor_mapping_facts: usize,
    pub source_summary_provider_external_constructor_facts: usize,
    pub source_summary_provider_external_field_facts: usize,
    pub source_summary_provider_hierarchy_override_facts: usize,
    pub source_summary_provider_closed_hierarchy_facts: usize,
    pub source_summary_provider_object_safety_facts: usize,
    pub source_summary_provider_type_capability_facts: usize,
    pub source_summary_provider_field_nullability_facts: usize,
    pub source_summary_provider_exception_facts: usize,
    pub source_summary_provider_external_library_pack_facts: usize,
    pub source_summary_provider_unsupported_external_api_facts: usize,
    pub source_summary_provider_unsupported_external_constructor_facts: usize,
    pub source_summary_provider_unsupported_external_field_facts: usize,
    pub files_seen: usize,
    pub files_parsed_without_errors: usize,
    pub files_resolved_without_errors: usize,
    pub parser_diagnostics: usize,
    pub resolver_diagnostics: usize,
    pub semantic_diagnostics: usize,
    pub external_pack_manifest_coverage_diagnostics: usize,
    pub top_resolver_diagnostics: Vec<DiagnosticSummary>,
    pub top_semantic_diagnostics: Vec<DiagnosticSummary>,
    pub top_external_pack_manifest_coverage_diagnostics: Vec<DiagnosticSummary>,
    pub source_dependency_resolved_method_calls: usize,
    pub source_dependency_resolved_constructors: usize,
    pub source_dependency_resolved_field_accesses: usize,
    pub top_source_dependency_method_calls: Vec<SourceDependencyMethodSummary>,
    pub top_source_dependency_constructor_types: Vec<SourceDependencyConstructorSummary>,
    pub top_source_dependency_field_accesses: Vec<SourceDependencyFieldSummary>,
    pub top_source_dependency_method_contexts: Vec<SourceDependencyMethodContextSummary>,
    pub top_source_dependency_constructor_contexts: Vec<SourceDependencyConstructorContextSummary>,
    pub top_source_dependency_field_contexts: Vec<SourceDependencyFieldContextSummary>,
    pub unresolved_method_calls: usize,
    pub external_api_explained_method_calls: usize,
    pub external_api_unsupported_method_calls: usize,
    pub external_api_missing_method_calls: usize,
    pub unexplained_method_calls: usize,
    pub unresolved_constructors: usize,
    pub external_api_explained_constructors: usize,
    pub external_api_unsupported_constructor_calls: usize,
    pub external_api_missing_constructor_calls: usize,
    pub anonymous_class_constructors: usize,
    pub unexplained_constructors: usize,
    pub unexplained_source_constructors: usize,
    pub unexplained_non_source_constructors: usize,
    pub unresolved_constructor_delegations: usize,
    pub unresolved_field_accesses: usize,
    pub external_api_explained_field_accesses: usize,
    pub external_api_unsupported_field_accesses: usize,
    pub external_api_missing_field_accesses: usize,
    pub unexplained_field_accesses: usize,
    pub unexplained_source_field_accesses: usize,
    pub unexplained_non_source_field_accesses: usize,
    pub external_api_guesses: usize,
    pub external_pack_api_facts: usize,
    pub external_pack_constructor_facts: usize,
    pub external_fact_model_pack_api_facts: usize,
    pub external_fact_model_pack_constructor_facts: usize,
    pub rust_crate_mapping_pack_api_facts: usize,
    pub rust_crate_mapping_pack_constructor_facts: usize,
    pub ffi_boundary_pack_api_facts: usize,
    pub ffi_boundary_pack_constructor_facts: usize,
    pub conservative_boundary_pack_api_facts: usize,
    pub conservative_boundary_pack_constructor_facts: usize,
    pub external_pack_explained_method_calls: usize,
    pub external_pack_explained_constructors: usize,
    pub external_fact_model_explained_method_calls: usize,
    pub external_fact_model_explained_constructors: usize,
    pub rust_crate_mapping_explained_method_calls: usize,
    pub rust_crate_mapping_explained_constructors: usize,
    pub ffi_boundary_explained_method_calls: usize,
    pub ffi_boundary_explained_constructors: usize,
    pub conservative_boundary_explained_method_calls: usize,
    pub conservative_boundary_explained_constructors: usize,
    pub external_constructor_guesses: usize,
    pub project_constructor_guesses: usize,
    pub source_constructor_guesses: usize,
    pub external_field_guesses: usize,
    pub project_field_guesses: usize,
    pub source_field_guesses: usize,
    pub external_api_return_nullability_facts: usize,
    pub external_api_fluent_builder_facts: usize,
    pub external_api_argument_effect_facts: usize,
    pub total_facts: usize,
    pub elapsed_ms: u128,
    pub object_concrete_struct_representations: usize,
    pub object_dyn_trait_representations: usize,
    pub object_closed_enum_representations: usize,
    pub object_erased_object_representations: usize,
    pub object_representation_diagnostics: usize,
    pub top_object_representation_diagnostics: Vec<DiagnosticSummary>,
    pub top_external_api_guess_methods: Vec<ExternalApiGuessSummary>,
    pub top_external_pack_api_methods: Vec<ExternalApiPackSummary>,
    pub top_external_pack_api_method_contexts: Vec<ExternalApiMethodContextSummary>,
    pub top_external_api_unsupported_methods: Vec<ExternalApiPackSummary>,
    pub top_external_api_missing_methods: Vec<ExternalApiPackSummary>,
    pub top_external_api_unsupported_method_contexts: Vec<ExternalApiMethodContextSummary>,
    pub top_external_api_missing_method_contexts: Vec<ExternalApiMethodContextSummary>,
    pub top_external_pack_constructor_types: Vec<ExternalPackConstructorSummary>,
    pub top_external_pack_constructor_contexts: Vec<ExternalApiConstructorContextSummary>,
    pub top_external_api_unsupported_constructors: Vec<ExternalPackConstructorSummary>,
    pub top_external_api_missing_constructors: Vec<ExternalPackConstructorSummary>,
    pub top_external_api_unsupported_constructor_contexts:
        Vec<ExternalApiConstructorContextSummary>,
    pub top_external_api_missing_constructor_contexts: Vec<ExternalApiConstructorContextSummary>,
    pub top_external_constructor_guess_types: Vec<ExternalConstructorGuessSummary>,
    pub top_project_constructor_guess_types: Vec<ExternalConstructorGuessSummary>,
    pub top_source_constructor_guess_types: Vec<ExternalConstructorGuessSummary>,
    pub top_external_constructor_guess_contexts: Vec<UnresolvedConstructorContextSummary>,
    pub top_project_constructor_guess_contexts: Vec<UnresolvedConstructorContextSummary>,
    pub top_source_constructor_guess_contexts: Vec<UnresolvedConstructorContextSummary>,
    pub top_external_field_guess_targets: Vec<ExternalFieldGuessSummary>,
    pub top_project_field_guess_targets: Vec<ExternalFieldGuessSummary>,
    pub top_source_field_guess_targets: Vec<ExternalFieldGuessSummary>,
    pub top_external_pack_field_accesses: Vec<ExternalFieldGuessSummary>,
    pub top_external_pack_field_contexts: Vec<ExternalApiFieldContextSummary>,
    pub top_external_api_unsupported_fields: Vec<ExternalFieldGuessSummary>,
    pub top_external_api_missing_fields: Vec<ExternalFieldGuessSummary>,
    pub top_external_api_unsupported_field_contexts: Vec<ExternalApiFieldContextSummary>,
    pub top_external_api_missing_field_contexts: Vec<ExternalApiFieldContextSummary>,
    pub top_external_field_guess_contexts: Vec<UnresolvedFieldContextSummary>,
    pub top_project_field_guess_contexts: Vec<UnresolvedFieldContextSummary>,
    pub top_source_field_guess_contexts: Vec<UnresolvedFieldContextSummary>,
    pub top_unresolved_targets: Vec<UnresolvedTargetSummary>,
    pub top_unexplained_method_targets: Vec<UnresolvedTargetSummary>,
    pub top_unexplained_method_receivers: Vec<UnresolvedMethodReceiverSummary>,
    pub top_unresolved_constructor_targets: Vec<UnresolvedTargetSummary>,
    pub top_anonymous_constructor_targets: Vec<ExternalConstructorGuessSummary>,
    pub top_unexplained_constructor_targets: Vec<UnresolvedTargetSummary>,
    pub top_unexplained_constructor_contexts: Vec<UnresolvedConstructorContextSummary>,
    pub top_unresolved_field_targets: Vec<UnresolvedTargetSummary>,
    pub top_unexplained_field_targets: Vec<UnresolvedTargetSummary>,
    pub top_unresolved_field_receivers: Vec<UnresolvedFieldReceiverSummary>,
    pub top_unexplained_field_receivers: Vec<UnresolvedFieldReceiverSummary>,
    pub top_unexplained_field_receiver_files: Vec<UnresolvedFieldReceiverFileSummary>,
    pub top_unexplained_field_contexts: Vec<UnresolvedFieldContextSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolverBenchmarkSourceRoot {
    pub path: PathBuf,
    pub mode: DependencyMode,
    pub files_seen: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolverBenchmarkFactProvider {
    pub name: String,
    pub mode: DependencyMode,
    pub facts: usize,
    pub external_api_effect_facts: usize,
    pub external_api_argument_effect_facts: usize,
    pub external_crate_api_mapping_facts: usize,
    pub external_crate_constructor_mapping_facts: usize,
    pub external_constructor_facts: usize,
    pub external_field_facts: usize,
    pub hierarchy_override_facts: usize,
    pub closed_hierarchy_facts: usize,
    pub object_safety_facts: usize,
    pub type_capability_facts: usize,
    pub field_nullability_facts: usize,
    pub exception_facts: usize,
    pub external_library_pack_facts: usize,
    pub unsupported_external_api_facts: usize,
    pub unsupported_external_constructor_facts: usize,
    pub unsupported_external_field_facts: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnresolvedTargetSummary {
    pub kind: UnresolvedTargetKind,
    pub target_name: String,
    pub count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnresolvedMethodReceiverSummary {
    pub target_name: String,
    pub receiver_type: String,
    pub count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnresolvedFieldReceiverSummary {
    pub field_name: String,
    pub receiver_type: String,
    pub count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnresolvedFieldReceiverFileSummary {
    pub field_name: String,
    pub receiver_type: String,
    pub count: usize,
    pub sample_files: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnresolvedFieldContextSummary {
    pub field_name: String,
    pub receiver_type: String,
    pub enclosing_type: String,
    pub enclosing_member: String,
    pub count: usize,
    pub sample_files: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnresolvedConstructorContextSummary {
    pub target_name: String,
    pub receiver_type: String,
    pub arg_types: Vec<String>,
    pub enclosing_type: String,
    pub enclosing_member: String,
    pub count: usize,
    pub sample_files: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceDependencyMethodContextSummary {
    pub declaring_type: String,
    pub method_name: String,
    pub source_mode: DependencyMode,
    pub arg_count: usize,
    pub enclosing_type: String,
    pub enclosing_member: String,
    pub count: usize,
    pub sample_files: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceDependencyConstructorContextSummary {
    pub declaring_type: String,
    pub source_mode: DependencyMode,
    pub arg_count: usize,
    pub enclosing_type: String,
    pub enclosing_member: String,
    pub count: usize,
    pub sample_files: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceDependencyFieldContextSummary {
    pub declaring_type: String,
    pub field_name: String,
    pub receiver_type: String,
    pub source_mode: DependencyMode,
    pub enclosing_type: String,
    pub enclosing_member: String,
    pub count: usize,
    pub sample_files: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceDependencyMethodSummary {
    pub receiver_type: String,
    pub method_name: String,
    pub source_mode: DependencyMode,
    pub count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceDependencyConstructorSummary {
    pub type_name: String,
    pub arity: usize,
    pub source_mode: DependencyMode,
    pub count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceDependencyFieldSummary {
    pub type_name: String,
    pub field_name: String,
    pub source_mode: DependencyMode,
    pub count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalApiGuessSummary {
    pub receiver_type: String,
    pub method_name: String,
    pub count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternalReportMode {
    Known(ExternalLibraryMode),
    Unknown,
}

impl From<Option<ExternalLibraryMode>> for ExternalReportMode {
    fn from(mode: Option<ExternalLibraryMode>) -> Self {
        match mode {
            Some(mode) => ExternalReportMode::Known(mode),
            None => ExternalReportMode::Unknown,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalApiPackSummary {
    pub receiver_type: String,
    pub method_name: String,
    pub mode: ExternalReportMode,
    pub count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalApiMethodContextSummary {
    pub receiver_type: String,
    pub method_name: String,
    pub mode: ExternalReportMode,
    pub arg_count: usize,
    pub enclosing_type: String,
    pub enclosing_member: String,
    pub count: usize,
    pub sample_files: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalApiConstructorContextSummary {
    pub target_name: String,
    pub receiver_type: String,
    pub mode: ExternalReportMode,
    pub arg_types: Vec<String>,
    pub enclosing_type: String,
    pub enclosing_member: String,
    pub count: usize,
    pub sample_files: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalPackConstructorSummary {
    pub type_name: String,
    pub arity: usize,
    pub mode: ExternalReportMode,
    pub count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalConstructorGuessSummary {
    pub type_name: String,
    pub arity: usize,
    pub count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalFieldGuessSummary {
    pub type_name: String,
    pub field_name: String,
    pub mode: ExternalReportMode,
    pub count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalApiFieldContextSummary {
    pub field_name: String,
    pub receiver_type: String,
    pub mode: ExternalReportMode,
    pub enclosing_type: String,
    pub enclosing_member: String,
    pub count: usize,
    pub sample_files: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticSummary {
    pub code: &'static str,
    pub message: String,
    pub mode: Option<DependencyMode>,
    pub count: usize,
    pub sample_files: Vec<String>,
}

pub fn benchmark_java_resolver_dir(root: &Path) -> std::io::Result<ResolverBenchmarkReport> {
    benchmark_java_resolver_config(&DependencyModeConfig::for_primary_source_root(root))
}

pub fn benchmark_java_resolver_config(
    config: &DependencyModeConfig,
) -> std::io::Result<ResolverBenchmarkReport> {
    let started_at = Instant::now();
    let root = config
        .primary_source_roots()
        .next()
        .or_else(|| config.source_roots_for_resolution().next())
        .map(|root| root.path.clone())
        .unwrap_or_default();
    let mut report = ResolverBenchmarkReport {
        root,
        source_roots: config
            .source_roots
            .iter()
            .map(|root| ResolverBenchmarkSourceRoot {
                path: root.path.clone(),
                mode: root.mode,
                files_seen: 0,
            })
            .collect(),
        fact_providers: config
            .fact_providers
            .iter()
            .map(|provider| ResolverBenchmarkFactProvider {
                name: provider.name.clone(),
                mode: provider.mode,
                facts: fact_count(&provider.facts),
                external_api_effect_facts: provider.facts.external_api_effect_facts.len(),
                external_api_argument_effect_facts: provider
                    .facts
                    .external_api_argument_effect_facts
                    .len(),
                external_crate_api_mapping_facts: provider
                    .facts
                    .external_crate_api_mapping_facts
                    .len(),
                external_crate_constructor_mapping_facts: provider
                    .facts
                    .external_crate_constructor_mapping_facts
                    .len(),
                external_constructor_facts: provider.facts.external_constructor_facts.len(),
                external_field_facts: provider.facts.external_field_facts.len(),
                hierarchy_override_facts: provider.facts.hierarchy_overrides.len(),
                closed_hierarchy_facts: provider.facts.closed_hierarchy_facts.len(),
                object_safety_facts: provider.facts.object_safety_facts.len(),
                type_capability_facts: provider.facts.type_capability_overrides.len(),
                field_nullability_facts: provider.facts.field_nullability_facts.len(),
                exception_facts: provider.facts.exception_facts.len(),
                external_library_pack_facts: provider.facts.external_library_packs.len(),
                unsupported_external_api_facts: provider.facts.unsupported_external_api_facts.len(),
                unsupported_external_constructor_facts: provider
                    .facts
                    .unsupported_external_constructor_facts
                    .len(),
                unsupported_external_field_facts: provider
                    .facts
                    .unsupported_external_field_facts
                    .len(),
            })
            .collect(),
        ..ResolverBenchmarkReport::default()
    };
    collect_configured_provider_fact_counts(&config.fact_providers, &mut report);
    collect_source_summary_provider_fact_counts(&config.fact_providers, &mut report);
    collect_external_pack_provider_boundary_audits(config, &mut report);
    if config.enable_builtin_external_packs {
        collect_builtin_external_pack_manifest_coverage_audit(&mut report);
    }
    let mut external_facts = FactSet::default();
    for provider_facts in config.fact_provider_facts() {
        external_facts.extend_from(provider_facts.clone());
    }
    let mut parsed_files = Vec::new();
    let mut seen_source_paths = HashSet::new();
    for source_root in config.source_roots_for_source_index() {
        let source_root_index = config
            .source_roots
            .iter()
            .position(|root| root.path == source_root.path && root.mode == source_root.mode);
        for path in java_files_under(&source_root.path)? {
            if config.mode_for_source_path(&path) != Some(source_root.mode) {
                continue;
            }
            if !seen_source_paths.insert(path.clone()) {
                continue;
            }
            let source = fs::read_to_string(&path)?;
            report.files_seen += 1;
            if let Some(index) = source_root_index {
                if let Some(root) = report.source_roots.get_mut(index) {
                    root.files_seen += 1;
                }
            }
            let parsed = crate::parser::parse_java_source(&source);
            report.parser_diagnostics += parsed.diagnostics.len();
            if !parsed.diagnostics.has_errors() {
                report.files_parsed_without_errors += 1;
            }
            parsed_files.push(ParsedJavaFile {
                parsed,
                mode: source_root.mode,
                file_label: path
                    .strip_prefix(&source_root.path)
                    .unwrap_or(&path)
                    .display()
                    .to_string(),
            });
        }
    }
    for parsed_file in parsed_files
        .iter()
        .filter(|file| file.mode == DependencyMode::SourceSummary)
    {
        let summary_facts =
            crate::resolve::declaration_facts_from_program(&parsed_file.parsed.program);
        report.source_summary_facts += fact_count(&summary_facts);
        report.source_summary_root_type_capability_facts +=
            summary_facts.type_capability_overrides.len();
        report.source_summary_root_object_safety_facts += summary_facts.object_safety_facts.len();
        report.source_summary_root_closed_hierarchy_facts +=
            summary_facts.closed_hierarchy_facts.len();
        external_facts.extend_from(summary_facts);
    }
    let source_programs = combined_source_programs_by_package(&parsed_files);
    let source_index = SourceTypeIndex::from_programs(&source_programs);
    let source_type_modes = source_type_modes_by_name(&parsed_files);
    let source_type_fields = source_type_fields_by_name(&parsed_files);
    let mut representation_facts = external_facts.clone();
    extend_representation_facts_for_source_summary_superclass_boundaries(
        &parsed_files,
        &source_type_modes,
        &mut representation_facts,
    );
    if config.enable_builtin_external_packs {
        extend_representation_facts_for_builtin_anonymous_targets(
            parsed_files
                .iter()
                .filter(|file| file.mode != DependencyMode::SourceSummary)
                .map(|file| &file.parsed.program),
            &mut representation_facts,
        );
        extend_representation_facts_for_builtin_hierarchy_targets(
            parsed_files
                .iter()
                .filter(|file| file.mode != DependencyMode::SourceSummary)
                .map(|file| &file.parsed.program),
            &mut representation_facts,
        );
    }
    collect_object_representation_counts_for_programs(
        source_programs
            .iter()
            .map(|(package, program)| (package.as_str(), program)),
        &representation_facts,
        &mut report,
        ObjectRepresentationBenchmarkMode {
            count_representations: true,
            collect_diagnostics: false,
        },
    );
    collect_object_representation_counts_for_programs_with_context(
        parsed_files
            .iter()
            .filter(|file| file.mode != DependencyMode::SourceSummary)
            .map(|file| {
                (
                    file.file_label.as_str(),
                    file.mode,
                    source_index.source_program_for_file(&file.parsed.program, &source_programs),
                    &file.parsed.program,
                )
            }),
        &representation_facts,
        &mut report,
        ObjectRepresentationBenchmarkMode {
            count_representations: false,
            collect_diagnostics: true,
        },
    );
    for parsed_file in parsed_files {
        if !parsed_file.mode.participates_in_resolution() {
            continue;
        }
        let source_program =
            source_index.source_program_for_file(&parsed_file.parsed.program, &source_programs);
        let source_symbols = crate::resolve::source_symbols_from_program(&source_program);
        let source_type_names = source_type_names(&source_program);
        benchmark_parsed_java_file(
            parsed_file.parsed,
            &source_symbols,
            &source_type_names,
            Some(&source_index),
            &external_facts,
            config.enable_builtin_external_packs,
            &mut report,
            &parsed_file.file_label,
            parsed_file.mode,
            &source_type_modes,
            &source_type_fields,
        );
    }
    report.elapsed_ms = started_at.elapsed().as_millis();
    Ok(report)
}

fn extend_representation_facts_for_builtin_anonymous_targets<'a>(
    programs: impl IntoIterator<Item = &'a Program>,
    facts: &mut FactSet,
) {
    let mut target_names = HashSet::new();
    for program in programs {
        collect_anonymous_constructor_target_names_for_program(program, &mut target_names);
    }
    for target_name in target_names {
        facts.extend_from(crate::external_packs::representation_facts_for_type(
            &target_name,
        ));
    }
}

fn extend_representation_facts_for_builtin_hierarchy_targets<'a>(
    programs: impl IntoIterator<Item = &'a Program>,
    facts: &mut FactSet,
) {
    let mut target_names = HashSet::new();
    for program in programs {
        collect_declared_hierarchy_target_names_for_program(program, &mut target_names);
    }
    for target_name in target_names {
        facts.extend_from(crate::external_packs::representation_facts_for_type(
            &target_name,
        ));
    }
}

fn collect_anonymous_constructor_target_names_for_program(
    program: &Program,
    target_names: &mut HashSet<String>,
) {
    for ty in &program.types {
        collect_anonymous_constructor_target_names_for_type(ty, target_names);
    }
}

fn collect_declared_hierarchy_target_names_for_program(
    program: &Program,
    target_names: &mut HashSet<String>,
) {
    for ty in &program.types {
        collect_declared_hierarchy_target_names_for_type(ty, target_names);
    }
}

fn collect_declared_hierarchy_target_names_for_type(
    ty: &TypeDecl,
    target_names: &mut HashSet<String>,
) {
    if let Some(super_type) = &ty.extends {
        insert_type_name_dependencies(
            &crate::parser::parse_hierarchy_type(super_type),
            target_names,
        );
    }
    for interface in &ty.implements {
        insert_type_name_dependencies(
            &crate::parser::parse_hierarchy_type(interface),
            target_names,
        );
    }
    for member in &ty.members {
        if let Member::Type(nested) = member {
            collect_declared_hierarchy_target_names_for_type(nested, target_names);
        }
    }
}

fn insert_type_name_dependencies(ty: &JavaType, target_names: &mut HashSet<String>) {
    let mut names = Vec::new();
    collect_type_name_dependencies(ty, &mut names);
    target_names.extend(names);
}

fn collect_anonymous_constructor_target_names_for_type(
    ty: &TypeDecl,
    target_names: &mut HashSet<String>,
) {
    for member in &ty.members {
        match member {
            Member::Field(field) => {
                if let Some(initializer) = &field.initializer {
                    collect_anonymous_constructor_target_names_for_expr(initializer, target_names);
                }
            }
            Member::Method(method) => {
                if let Some(body) = &method.body {
                    collect_anonymous_constructor_target_names_for_block(body, target_names);
                }
            }
            Member::Constructor(ctor) => {
                if let Some(body) = &ctor.body {
                    collect_anonymous_constructor_target_names_for_block(body, target_names);
                }
            }
            Member::Type(nested) => {
                collect_anonymous_constructor_target_names_for_type(nested, target_names);
            }
            Member::Unsupported { .. } => {}
        }
    }
}

fn collect_anonymous_constructor_target_names_for_block(
    block: &Block,
    target_names: &mut HashSet<String>,
) {
    for stmt in &block.statements {
        collect_anonymous_constructor_target_names_for_stmt(stmt, target_names);
    }
}

fn collect_anonymous_constructor_target_names_for_stmt(
    stmt: &Stmt,
    target_names: &mut HashSet<String>,
) {
    match stmt {
        Stmt::VarDecl { initializer, .. } => {
            if let Some(initializer) = initializer {
                collect_anonymous_constructor_target_names_for_expr(initializer, target_names);
            }
        }
        Stmt::Assign { target, value } => {
            collect_anonymous_constructor_target_names_for_expr(target, target_names);
            collect_anonymous_constructor_target_names_for_expr(value, target_names);
        }
        Stmt::Expr(expr) | Stmt::Throw(expr) => {
            collect_anonymous_constructor_target_names_for_expr(expr, target_names);
        }
        Stmt::Return(expr) => {
            if let Some(expr) = expr {
                collect_anonymous_constructor_target_names_for_expr(expr, target_names);
            }
        }
        Stmt::If {
            condition,
            then_block,
            else_block,
        } => {
            collect_anonymous_constructor_target_names_for_expr(condition, target_names);
            collect_anonymous_constructor_target_names_for_block(then_block, target_names);
            if let Some(else_block) = else_block {
                collect_anonymous_constructor_target_names_for_block(else_block, target_names);
            }
        }
        Stmt::While { condition, body } => {
            collect_anonymous_constructor_target_names_for_expr(condition, target_names);
            collect_anonymous_constructor_target_names_for_block(body, target_names);
        }
        Stmt::For {
            init,
            condition,
            update,
            body,
            ..
        } => {
            for stmt in init {
                collect_anonymous_constructor_target_names_for_stmt(stmt, target_names);
            }
            if let Some(condition) = condition {
                collect_anonymous_constructor_target_names_for_expr(condition, target_names);
            }
            for expr in update {
                collect_anonymous_constructor_target_names_for_expr(expr, target_names);
            }
            collect_anonymous_constructor_target_names_for_block(body, target_names);
        }
        Stmt::EnhancedFor { iterable, body, .. } => {
            collect_anonymous_constructor_target_names_for_expr(iterable, target_names);
            collect_anonymous_constructor_target_names_for_block(body, target_names);
        }
        Stmt::Switch { selector, cases } => {
            collect_anonymous_constructor_target_names_for_expr(selector, target_names);
            for case in cases {
                for label in &case.labels {
                    if let crate::java_ir::SwitchLabel::Expr(expr) = label {
                        collect_anonymous_constructor_target_names_for_expr(expr, target_names);
                    }
                }
                for stmt in &case.statements {
                    collect_anonymous_constructor_target_names_for_stmt(stmt, target_names);
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
                collect_anonymous_constructor_target_names_for_stmt(resource, target_names);
            }
            collect_anonymous_constructor_target_names_for_block(body, target_names);
            for catch in catches {
                collect_anonymous_constructor_target_names_for_block(&catch.body, target_names);
            }
            if let Some(finally_block) = finally_block {
                collect_anonymous_constructor_target_names_for_block(finally_block, target_names);
            }
        }
        Stmt::LocalType(ty) => {
            collect_anonymous_constructor_target_names_for_type(ty, target_names)
        }
        Stmt::ConstructorInvocation { args, .. } => {
            for arg in args {
                collect_anonymous_constructor_target_names_for_expr(arg, target_names);
            }
        }
        Stmt::Block(block) => {
            collect_anonymous_constructor_target_names_for_block(block, target_names)
        }
        Stmt::Break(_) | Stmt::Continue(_) | Stmt::Unsupported(_) => {}
    }
}

fn collect_anonymous_constructor_target_names_for_expr(
    expr: &Expr,
    target_names: &mut HashSet<String>,
) {
    match expr {
        Expr::New { ty, args, body, .. } => {
            for arg in args {
                collect_anonymous_constructor_target_names_for_expr(arg, target_names);
            }
            if body.is_some() {
                if let JavaType::Named { name, .. } = ty {
                    target_names.insert(name.clone());
                }
            }
        }
        Expr::FieldAccess(receiver, _) => {
            collect_anonymous_constructor_target_names_for_expr(receiver, target_names);
        }
        Expr::ArrayAccess(array, index) => {
            collect_anonymous_constructor_target_names_for_expr(array, target_names);
            collect_anonymous_constructor_target_names_for_expr(index, target_names);
        }
        Expr::ArrayCreation { dimensions, .. } | Expr::ArrayInitializer(dimensions) => {
            for expr in dimensions {
                collect_anonymous_constructor_target_names_for_expr(expr, target_names);
            }
        }
        Expr::Binary { left, right, .. } => {
            collect_anonymous_constructor_target_names_for_expr(left, target_names);
            collect_anonymous_constructor_target_names_for_expr(right, target_names);
        }
        Expr::Unary { expr, .. } | Expr::Cast { expr, .. } => {
            collect_anonymous_constructor_target_names_for_expr(expr, target_names);
        }
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => {
            collect_anonymous_constructor_target_names_for_expr(condition, target_names);
            collect_anonymous_constructor_target_names_for_expr(then_expr, target_names);
            collect_anonymous_constructor_target_names_for_expr(else_expr, target_names);
        }
        Expr::Call { receiver, args, .. } => {
            if let Some(receiver) = receiver {
                collect_anonymous_constructor_target_names_for_expr(receiver, target_names);
            }
            for arg in args {
                collect_anonymous_constructor_target_names_for_expr(arg, target_names);
            }
        }
        Expr::InstanceOf { expr, .. } => {
            collect_anonymous_constructor_target_names_for_expr(expr, target_names);
        }
        Expr::Lambda { body, .. } => match body {
            LambdaBody::Expr(expr) => {
                collect_anonymous_constructor_target_names_for_expr(expr, target_names);
            }
            LambdaBody::Block(block) => {
                collect_anonymous_constructor_target_names_for_block(block, target_names);
            }
        },
        Expr::Literal(_)
        | Expr::Var(_)
        | Expr::This
        | Expr::Super
        | Expr::MethodReference { .. }
        | Expr::ClassLiteral(_)
        | Expr::Raw(_) => {}
    }
}

pub fn benchmark_java_file(
    path: &Path,
    report: &mut ResolverBenchmarkReport,
) -> std::io::Result<()> {
    let source = fs::read_to_string(path)?;
    report.files_seen += 1;

    let parsed = crate::parser::parse_java_source(&source);
    report.parser_diagnostics += parsed.diagnostics.len();
    if !parsed.diagnostics.has_errors() {
        report.files_parsed_without_errors += 1;
    }

    let source_type_names = source_type_names(&parsed.program);
    let source_symbols = crate::resolve::source_symbols_from_program(&parsed.program);
    let file_label = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string());
    collect_object_representation_counts_for_programs(
        std::iter::once((file_label.as_str(), &parsed.program)),
        &FactSet::default(),
        report,
        ObjectRepresentationBenchmarkMode {
            count_representations: true,
            collect_diagnostics: true,
        },
    );
    benchmark_parsed_java_file(
        parsed,
        &source_symbols,
        &source_type_names,
        None,
        &FactSet::default(),
        true,
        report,
        &file_label,
        DependencyMode::PrimarySource,
        &HashMap::new(),
        &HashMap::new(),
    );
    Ok(())
}

#[derive(Debug, Clone)]
struct ParsedJavaFile {
    parsed: ParsedProgram,
    mode: DependencyMode,
    file_label: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SourceFieldInfo {
    declaring_type: String,
    ty: JavaType,
}

fn combined_source_programs_by_package(
    parsed_files: &[ParsedJavaFile],
) -> HashMap<String, Program> {
    let mut programs = HashMap::<String, Program>::new();
    for parsed_file in parsed_files {
        let package = package_key(&parsed_file.parsed.program);
        let program = programs.entry(package).or_insert_with(|| Program {
            package: parsed_file.parsed.program.package.clone(),
            imports: Vec::new(),
            types: Vec::new(),
        });
        for import in &parsed_file.parsed.program.imports {
            if !program.imports.iter().any(|existing| existing == import) {
                program.imports.push(import.clone());
            }
        }
        program
            .types
            .extend(parsed_file.parsed.program.types.iter().cloned());
    }
    programs
}

fn package_key(program: &Program) -> String {
    program.package.clone().unwrap_or_default()
}

fn source_type_modes_by_name(parsed_files: &[ParsedJavaFile]) -> HashMap<String, DependencyMode> {
    let mut modes = HashMap::new();
    for parsed_file in parsed_files {
        let package = package_key(&parsed_file.parsed.program);
        for ty in &parsed_file.parsed.program.types {
            collect_source_type_mode(&mut modes, &package, ty, parsed_file.mode);
        }
    }
    modes
}

fn source_type_fields_by_name(
    parsed_files: &[ParsedJavaFile],
) -> HashMap<String, HashMap<String, SourceFieldInfo>> {
    let mut fields = HashMap::new();
    let mut extends_by_type = HashMap::new();
    for parsed_file in parsed_files {
        let package = package_key(&parsed_file.parsed.program);
        for ty in &parsed_file.parsed.program.types {
            collect_source_type_fields(&mut fields, &mut extends_by_type, &package, ty);
        }
    }
    inherit_source_type_fields(&mut fields, &extends_by_type);
    fields
}

fn extend_representation_facts_for_source_summary_superclass_boundaries(
    parsed_files: &[ParsedJavaFile],
    source_type_modes: &HashMap<String, DependencyMode>,
    facts: &mut FactSet,
) {
    let mut boundary_types = HashSet::new();
    for parsed_file in parsed_files {
        if !parsed_file.mode.emits_rust() {
            continue;
        }
        for ty in &parsed_file.parsed.program.types {
            collect_source_summary_superclasses_for_type(
                ty,
                source_type_modes,
                &mut boundary_types,
            );
        }
    }
    for type_name in boundary_types {
        if facts
            .hierarchy_overrides
            .iter()
            .any(|existing| existing.type_name == type_name)
        {
            continue;
        }
        facts.hierarchy_overrides.push(HierarchyOverride {
            type_name,
            strategy: HierarchyStrategy::ErasedObject,
            provenance: FactProvenance::CorpusAnalysis,
        });
    }
}

fn collect_source_summary_superclasses_for_type(
    ty: &TypeDecl,
    source_type_modes: &HashMap<String, DependencyMode>,
    boundary_types: &mut HashSet<String>,
) {
    if let Some(super_name) = ty.extends.as_deref() {
        if source_type_modes.get(super_name) == Some(&DependencyMode::SourceSummary) {
            boundary_types.insert(super_name.to_string());
        }
    }
    for member in &ty.members {
        if let Member::Type(nested) = member {
            collect_source_summary_superclasses_for_type(nested, source_type_modes, boundary_types);
        }
    }
}

fn collect_source_type_fields(
    fields: &mut HashMap<String, HashMap<String, SourceFieldInfo>>,
    extends_by_type: &mut HashMap<String, String>,
    package: &str,
    ty: &TypeDecl,
) {
    let simple_name = ty.name.clone();
    let qualified_name = qualified_type_name(package, &ty.name);
    let simple_alias = crate::parser::to_snake_case(&simple_name);
    let qualified_alias = crate::parser::to_snake_case(&qualified_name);
    let direct_simple_fields = direct_source_field_infos(ty, &simple_name);
    fields
        .entry(simple_name.clone())
        .or_insert_with(|| direct_simple_fields.clone());
    fields
        .entry(qualified_name.clone())
        .or_insert_with(|| direct_source_field_infos(ty, &qualified_name));
    fields
        .entry(simple_alias.clone())
        .or_insert_with(|| direct_simple_fields.clone());
    fields
        .entry(qualified_alias.clone())
        .or_insert_with(|| direct_simple_fields.clone());
    if let Some(super_type) = &ty.extends {
        let super_name = source_hierarchy_type_name(super_type);
        extends_by_type.insert(simple_name, super_name.clone());
        extends_by_type.insert(qualified_name, super_name.clone());
        extends_by_type.insert(simple_alias, super_name.clone());
        extends_by_type.insert(qualified_alias, super_name);
    }
    for member in &ty.members {
        if let Member::Type(nested) = member {
            collect_source_type_fields(fields, extends_by_type, package, nested);
        }
    }
}

fn inherit_source_type_fields(
    fields: &mut HashMap<String, HashMap<String, SourceFieldInfo>>,
    extends_by_type: &HashMap<String, String>,
) {
    let type_names = fields.keys().cloned().collect::<Vec<_>>();
    for type_name in type_names {
        let mut seen = HashSet::new();
        let inherited =
            inherited_source_fields_for_type(&type_name, fields, extends_by_type, &mut seen);
        if let Some(type_fields) = fields.get_mut(&type_name) {
            for (field_name, field_info) in inherited {
                type_fields.entry(field_name).or_insert(field_info);
            }
        }
    }
}

fn inherited_source_fields_for_type(
    type_name: &str,
    fields: &HashMap<String, HashMap<String, SourceFieldInfo>>,
    extends_by_type: &HashMap<String, String>,
    seen: &mut HashSet<String>,
) -> HashMap<String, SourceFieldInfo> {
    if !seen.insert(type_name.to_string()) {
        return HashMap::new();
    }
    let Some(super_name) = extends_by_type.get(type_name) else {
        return HashMap::new();
    };
    let mut inherited = inherited_source_fields_for_type(super_name, fields, extends_by_type, seen);
    if let Some(super_fields) = fields.get(super_name) {
        for (field_name, field_info) in super_fields {
            inherited
                .entry(field_name.clone())
                .or_insert_with(|| field_info.clone());
        }
    }
    inherited
}

fn source_hierarchy_type_name(raw_type: &str) -> String {
    match crate::parser::parse_hierarchy_type(raw_type) {
        JavaType::Named { name, .. } | JavaType::Unknown(name) => name,
        other => java_type_summary(&other),
    }
}

fn direct_source_field_infos(
    ty: &TypeDecl,
    declaring_type: &str,
) -> HashMap<String, SourceFieldInfo> {
    direct_field_types(ty)
        .into_iter()
        .map(|(name, ty)| {
            (
                name,
                SourceFieldInfo {
                    declaring_type: declaring_type.to_string(),
                    ty,
                },
            )
        })
        .collect()
}

fn collect_source_type_mode(
    modes: &mut HashMap<String, DependencyMode>,
    package: &str,
    ty: &TypeDecl,
    mode: DependencyMode,
) {
    modes.entry(ty.name.clone()).or_insert(mode);
    modes
        .entry(qualified_type_name(package, &ty.name))
        .or_insert(mode);
    modes
        .entry(crate::parser::to_snake_case(&ty.name))
        .or_insert(mode);
    modes
        .entry(crate::parser::to_snake_case(&qualified_type_name(
            package, &ty.name,
        )))
        .or_insert(mode);
    for member in &ty.members {
        if let Member::Type(nested) = member {
            collect_source_type_mode(modes, package, nested, mode);
        }
    }
}

#[derive(Debug, Clone, Default)]
struct SourceTypeIndex {
    by_qualified_name: HashMap<String, SourceTypeEntry>,
    by_package: HashMap<String, Vec<String>>,
    project_package_roots: HashSet<String>,
}

#[derive(Debug, Clone)]
struct SourceTypeEntry {
    qualified_name: String,
    package: String,
    imports: Vec<ImportDecl>,
    ty: TypeDecl,
}

impl SourceTypeIndex {
    fn from_programs(programs: &HashMap<String, Program>) -> Self {
        let mut index = Self::default();
        for (package, program) in programs {
            if let Some(root) = project_package_root(package) {
                index.project_package_roots.insert(root);
            }
            for ty in &program.types {
                index.collect_type(package, &program.imports, ty);
            }
        }
        index
    }

    fn collect_type(&mut self, package: &str, imports: &[ImportDecl], ty: &TypeDecl) {
        let qualified_name = qualified_type_name(package, &ty.name);
        self.collect_type_with_qualified_name(package, imports, ty, qualified_name);
    }

    fn collect_type_with_qualified_name(
        &mut self,
        package: &str,
        imports: &[ImportDecl],
        ty: &TypeDecl,
        qualified_name: String,
    ) {
        self.by_package
            .entry(package.to_string())
            .or_default()
            .push(qualified_name.clone());
        self.by_qualified_name.insert(
            qualified_name.clone(),
            SourceTypeEntry {
                qualified_name: qualified_name.clone(),
                package: package.to_string(),
                imports: imports.to_vec(),
                ty: ty.clone(),
            },
        );
        for member in &ty.members {
            if let Member::Type(nested) = member {
                self.collect_type_with_qualified_name(
                    package,
                    imports,
                    nested,
                    format!("{qualified_name}.{}", nested.name),
                );
            }
        }
    }

    fn source_program_for_file(
        &self,
        file_program: &Program,
        package_programs: &HashMap<String, Program>,
    ) -> Program {
        let current_package = package_key(file_program);
        let mut types = Vec::new();
        let mut seen = HashSet::new();
        let mut seen_simple_names = HashSet::new();
        let current_file_type_names = file_program
            .types
            .iter()
            .map(|ty| ty.name.clone())
            .collect::<HashSet<_>>();

        if let Some(package_program) = package_programs.get(&current_package) {
            for ty in &package_program.types {
                if current_file_type_names.contains(&ty.name) {
                    continue;
                }
                let qualified_name = qualified_type_name(&current_package, &ty.name);
                push_source_type_by_qualified_name(
                    self,
                    &mut types,
                    &mut seen,
                    &mut seen_simple_names,
                    &qualified_name,
                );
            }
        }

        for import in &file_program.imports {
            if import.is_static || import.is_wildcard {
                continue;
            }
            push_source_type_by_qualified_name(
                self,
                &mut types,
                &mut seen,
                &mut seen_simple_names,
                &import.path,
            );
        }

        for import in &file_program.imports {
            if import.is_static {
                continue;
            }
            if import.is_wildcard {
                let package = import.path.trim_end_matches(".*");
                if let Some(qualified_names) = self.by_package.get(package) {
                    for qualified_name in qualified_names {
                        if !is_top_level_qualified_type(package, qualified_name) {
                            continue;
                        }
                        push_source_type_by_qualified_name(
                            self,
                            &mut types,
                            &mut seen,
                            &mut seen_simple_names,
                            qualified_name,
                        );
                    }
                }
            } else if let Some(package) = package_name_for_qualified_type(&import.path) {
                if let Some(qualified_names) = self.by_package.get(package) {
                    for qualified_name in qualified_names {
                        if !is_top_level_qualified_type(package, qualified_name) {
                            continue;
                        }
                        push_source_type_by_qualified_name(
                            self,
                            &mut types,
                            &mut seen,
                            &mut seen_simple_names,
                            qualified_name,
                        );
                    }
                }
            }
        }

        self.add_transitive_source_dependencies(&mut types, &mut seen, &mut seen_simple_names);

        for ty in &file_program.types {
            types.push(ty.clone());
        }

        Program {
            package: file_program.package.clone(),
            imports: file_program.imports.clone(),
            types,
        }
    }

    fn add_transitive_source_dependencies(
        &self,
        types: &mut Vec<TypeDecl>,
        seen: &mut HashSet<String>,
        seen_simple_names: &mut HashSet<String>,
    ) {
        let mut cursor = 0;
        while cursor < types.len() {
            let ty_name = types[cursor].name.clone();
            let Some(qualified_name) = seen
                .iter()
                .find(|qualified_name| {
                    self.by_qualified_name
                        .get(*qualified_name)
                        .is_some_and(|entry| entry.ty.name == ty_name)
                })
                .cloned()
            else {
                cursor += 1;
                continue;
            };
            cursor += 1;

            let Some(entry) = self.by_qualified_name.get(&qualified_name) else {
                continue;
            };
            for dependency in source_type_dependencies(&entry.ty) {
                for qualified_dependency in self.resolve_source_type_dependency(entry, &dependency)
                {
                    push_source_type_by_qualified_name(
                        self,
                        types,
                        seen,
                        seen_simple_names,
                        &qualified_dependency,
                    );
                }
            }
        }
    }

    fn resolve_source_type_dependency(
        &self,
        entry: &SourceTypeEntry,
        type_name: &str,
    ) -> Vec<String> {
        if type_name.is_empty() || is_primitive_or_jdk_builtin_name(type_name) {
            return Vec::new();
        }
        if type_name.contains('.') {
            return self
                .by_qualified_name
                .contains_key(type_name)
                .then(|| type_name.to_string())
                .into_iter()
                .collect();
        }

        let mut out = Vec::new();
        if let Some(nested_sibling_name) = nested_sibling_qualified_type_name(entry, type_name) {
            if self.by_qualified_name.contains_key(&nested_sibling_name) {
                out.push(nested_sibling_name);
            }
        }
        let same_package_name = qualified_type_name(&entry.package, type_name);
        if self.by_qualified_name.contains_key(&same_package_name) {
            out.push(same_package_name);
        }
        let mut exact_import_for_type = false;
        for import in &entry.imports {
            if import.is_static {
                continue;
            }
            if import.is_wildcard {
                let package = import.path.trim_end_matches(".*");
                let qualified_name = qualified_type_name(package, type_name);
                if self.by_qualified_name.contains_key(&qualified_name) {
                    out.push(qualified_name);
                }
            } else if import.path.rsplit('.').next() == Some(type_name) {
                exact_import_for_type = true;
                if self.by_qualified_name.contains_key(&import.path) {
                    out.push(import.path.clone());
                }
            }
        }
        if exact_import_for_type {
            out.sort();
            out.dedup();
            return out;
        }
        out.sort();
        out.dedup();
        out
    }

    fn explicit_import_source_ownership(&self, program: &Program, type_name: &str) -> Option<bool> {
        let simple_name = type_name.rsplit('.').next().unwrap_or(type_name);
        let canonical_simple_name = crate::parser::to_snake_case(simple_name);
        program.imports.iter().find_map(|import| {
            if import.is_static || import.is_wildcard {
                return None;
            }
            let imported_simple_name = import.path.rsplit('.').next().unwrap_or(&import.path);
            (imported_simple_name == simple_name
                || crate::parser::to_snake_case(imported_simple_name) == canonical_simple_name)
                .then(|| self.by_qualified_name.contains_key(&import.path))
        })
    }

    fn explicit_import_project_ownership(&self, program: &Program, type_name: &str) -> bool {
        let simple_name = type_name.rsplit('.').next().unwrap_or(type_name);
        let canonical_simple_name = crate::parser::to_snake_case(simple_name);
        program.imports.iter().any(|import| {
            let imported_simple_name = import.path.rsplit('.').next().unwrap_or(&import.path);
            !import.is_static
                && !import.is_wildcard
                && (imported_simple_name == simple_name
                    || crate::parser::to_snake_case(imported_simple_name) == canonical_simple_name)
                && !self.by_qualified_name.contains_key(&import.path)
                && self.project_package_roots.iter().any(|root| {
                    import.path == *root || import.path.starts_with(&format!("{root}."))
                })
        })
    }
}

fn project_package_root(package: &str) -> Option<String> {
    let mut segments = package.split('.').filter(|segment| !segment.is_empty());
    let first = segments.next()?;
    let second = segments.next();
    if matches!(first, "com" | "org" | "net" | "io") {
        second.map(|second| format!("{first}.{second}"))
    } else {
        Some(first.to_string())
    }
}

fn push_source_type_by_qualified_name(
    index: &SourceTypeIndex,
    types: &mut Vec<TypeDecl>,
    seen: &mut HashSet<String>,
    seen_simple_names: &mut HashSet<String>,
    qualified_name: &str,
) {
    let Some(entry) = index.by_qualified_name.get(qualified_name) else {
        return;
    };
    let ty = &entry.ty;
    if seen.insert(qualified_name.to_string()) && seen_simple_names.insert(ty.name.clone()) {
        types.push(ty.clone());
    }
}

fn source_type_dependencies(ty: &TypeDecl) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(super_type) = &ty.extends {
        collect_type_name_dependencies(&crate::parser::parse_hierarchy_type(super_type), &mut out);
    }
    for interface in &ty.implements {
        collect_type_name_dependencies(&crate::parser::parse_hierarchy_type(interface), &mut out);
    }
    for bound in &ty.type_param_bounds {
        if let Some(bound) = bound {
            collect_type_name_dependencies(bound, &mut out);
        }
    }
    for member in &ty.members {
        match member {
            Member::Field(field) => collect_type_name_dependencies(&field.ty, &mut out),
            Member::Method(method) => {
                collect_type_name_dependencies(&method.return_type, &mut out);
                collect_param_type_dependencies(&method.params, &mut out);
            }
            Member::Constructor(constructor) => {
                collect_param_type_dependencies(&constructor.params, &mut out);
            }
            Member::Type(nested) => out.extend(source_type_dependencies(nested)),
            _ => {}
        }
    }
    out.sort();
    out.dedup();
    out
}

fn collect_param_type_dependencies(params: &[Param], out: &mut Vec<String>) {
    for param in params {
        collect_type_name_dependencies(&param.ty, out);
    }
}

fn collect_type_name_dependencies(ty: &JavaType, out: &mut Vec<String>) {
    match ty {
        JavaType::Named { name, args } => {
            out.push(name.clone());
            for arg in args {
                collect_type_name_dependencies(arg, out);
            }
        }
        JavaType::Array(element) => collect_type_name_dependencies(element, out),
        JavaType::Wildcard(bound) => match bound.as_ref() {
            Some(
                crate::java_ir::WildcardBound::Extends(bound)
                | crate::java_ir::WildcardBound::Super(bound),
            ) => {
                collect_type_name_dependencies(bound, out);
            }
            None => {}
        },
        JavaType::Unknown(name) => out.push(name.clone()),
        JavaType::Primitive(_) | JavaType::String | JavaType::Void => {}
    }
}

fn is_primitive_or_jdk_builtin_name(type_name: &str) -> bool {
    matches!(
        type_name,
        "boolean"
            | "byte"
            | "char"
            | "short"
            | "int"
            | "long"
            | "float"
            | "double"
            | "void"
            | "String"
            | "Object"
            | "Class"
    )
}

fn qualified_type_name(package: &str, type_name: &str) -> String {
    if package.is_empty() {
        type_name.to_string()
    } else {
        format!("{package}.{type_name}")
    }
}

fn is_top_level_qualified_type(package: &str, qualified_name: &str) -> bool {
    if package.is_empty() {
        !qualified_name.contains('.')
    } else {
        qualified_name
            .strip_prefix(package)
            .and_then(|rest| rest.strip_prefix('.'))
            .is_some_and(|type_path| !type_path.contains('.'))
    }
}

fn nested_sibling_qualified_type_name(entry: &SourceTypeEntry, type_name: &str) -> Option<String> {
    let type_path = if entry.package.is_empty() {
        entry.qualified_name.as_str()
    } else {
        entry
            .qualified_name
            .strip_prefix(&entry.package)
            .and_then(|rest| rest.strip_prefix('.'))?
    };
    type_path.rsplit_once('.')?;
    let parent_qualified_name = entry.qualified_name.rsplit_once('.')?.0;
    Some(format!("{parent_qualified_name}.{type_name}"))
}

fn package_name_for_qualified_type(qualified_name: &str) -> Option<&str> {
    qualified_name.rsplit_once('.').map(|(package, _)| package)
}

fn external_pack_mode_for_type(facts: &FactSet, type_name: &str) -> Option<ExternalLibraryMode> {
    facts
        .external_library_pack_for_type(type_name)
        .map(|pack| pack.mode)
        .or_else(|| configured_pack_mode_for_member_type(facts, type_name))
        .or_else(|| {
            crate::external_packs::external_library_pack_for_type(type_name).map(|pack| pack.mode)
        })
}

fn configured_pack_mode_for_member_type(
    facts: &FactSet,
    type_name: &str,
) -> Option<ExternalLibraryMode> {
    let mentioned_by_pack_member = facts.external_api_effect_facts.iter().any(|fact| {
        fact.provenance == FactProvenance::ExternalLibraryPack
            && fact
                .receiver_type
                .as_deref()
                .is_some_and(|receiver_type| external_type_name_matches(receiver_type, type_name))
    }) || facts.external_constructor_facts.iter().any(|fact| {
        fact.provenance == FactProvenance::ExternalLibraryPack
            && external_type_name_matches(&fact.type_name, type_name)
    }) || facts.external_field_facts.iter().any(|fact| {
        fact.provenance == FactProvenance::ExternalLibraryPack
            && external_type_name_matches(&fact.field.type_name, type_name)
    }) || facts.unsupported_external_api_facts.iter().any(|fact| {
        fact.provenance == FactProvenance::ExternalLibraryPack
            && external_type_name_matches(&fact.receiver_type, type_name)
    }) || facts.unsupported_external_constructor_facts.iter().any(
        |fact| {
            fact.provenance == FactProvenance::ExternalLibraryPack
                && external_type_name_matches(&fact.type_name, type_name)
        },
    ) || facts.unsupported_external_field_facts.iter().any(|fact| {
        fact.provenance == FactProvenance::ExternalLibraryPack
            && external_type_name_matches(&fact.field.type_name, type_name)
    });
    if !mentioned_by_pack_member {
        return None;
    }

    let modes = facts
        .external_library_packs
        .iter()
        .map(|pack| pack.mode)
        .collect::<Vec<_>>();
    let first = modes.first().copied()?;
    if modes.iter().all(|mode| *mode == first) {
        Some(first)
    } else {
        None
    }
}

fn missing_external_pack_mode_for_type(
    facts: &FactSet,
    enable_builtin_external_packs: bool,
    type_name: &str,
) -> Option<ExternalLibraryMode> {
    facts
        .external_library_pack_for_type(type_name)
        .map(|pack| pack.mode)
        .or_else(|| {
            enable_builtin_external_packs
                .then(|| {
                    crate::external_packs::external_library_pack_for_type(type_name)
                        .map(|pack| pack.mode)
                })
                .flatten()
        })
}

fn unsupported_external_api_provenance_for_receiver(
    facts: &FactSet,
    enable_builtin_external_packs: bool,
    receiver_type: &str,
    method_name: &str,
) -> Option<FactProvenance> {
    facts
        .unsupported_external_api_for_receiver(receiver_type, method_name)
        .map(|fact| fact.provenance)
        .or_else(|| {
            enable_builtin_external_packs
                .then(|| {
                    crate::external_packs::unsupported_external_api_for_receiver(
                        Some(receiver_type),
                        method_name,
                    )
                    .map(|fact| fact.provenance)
                })
                .flatten()
        })
}

fn unsupported_external_constructor_provenance_for(
    facts: &FactSet,
    enable_builtin_external_packs: bool,
    type_name: &str,
    arg_count: usize,
) -> Option<FactProvenance> {
    facts
        .unsupported_external_constructor_for(type_name, arg_count)
        .map(|fact| fact.provenance)
        .or_else(|| {
            enable_builtin_external_packs
                .then(|| {
                    crate::external_packs::unsupported_external_constructor_for(
                        type_name, arg_count,
                    )
                    .map(|fact| fact.provenance)
                })
                .flatten()
        })
}

fn unsupported_external_field_provenance_for(
    facts: &FactSet,
    enable_builtin_external_packs: bool,
    type_name: &str,
    field_name: &str,
) -> Option<FactProvenance> {
    facts
        .unsupported_external_field_for(type_name, field_name)
        .map(|fact| fact.provenance)
        .or_else(|| {
            enable_builtin_external_packs
                .then(|| {
                    crate::external_packs::unsupported_external_field_for(type_name, field_name)
                        .map(|fact| fact.provenance)
                })
                .flatten()
        })
}

fn increment_external_pack_api_mode_count(
    report: &mut ResolverBenchmarkReport,
    mode: ExternalLibraryMode,
) {
    match mode {
        ExternalLibraryMode::ExternalFactModel => report.external_fact_model_pack_api_facts += 1,
        ExternalLibraryMode::RustCrateMapping => report.rust_crate_mapping_pack_api_facts += 1,
        ExternalLibraryMode::FfiBoundary => report.ffi_boundary_pack_api_facts += 1,
        ExternalLibraryMode::ConservativeBoundary => {
            report.conservative_boundary_pack_api_facts += 1;
        }
    }
}

fn increment_external_pack_constructor_mode_count(
    report: &mut ResolverBenchmarkReport,
    mode: ExternalLibraryMode,
) {
    match mode {
        ExternalLibraryMode::ExternalFactModel => {
            report.external_fact_model_pack_constructor_facts += 1;
        }
        ExternalLibraryMode::RustCrateMapping => {
            report.rust_crate_mapping_pack_constructor_facts += 1;
        }
        ExternalLibraryMode::FfiBoundary => report.ffi_boundary_pack_constructor_facts += 1,
        ExternalLibraryMode::ConservativeBoundary => {
            report.conservative_boundary_pack_constructor_facts += 1;
        }
    }
}

fn increment_external_pack_explained_method_count(
    report: &mut ResolverBenchmarkReport,
    mode: ExternalLibraryMode,
) {
    match mode {
        ExternalLibraryMode::ExternalFactModel => {
            report.external_fact_model_explained_method_calls += 1;
        }
        ExternalLibraryMode::RustCrateMapping => {
            report.rust_crate_mapping_explained_method_calls += 1;
        }
        ExternalLibraryMode::FfiBoundary => report.ffi_boundary_explained_method_calls += 1,
        ExternalLibraryMode::ConservativeBoundary => {
            report.conservative_boundary_explained_method_calls += 1;
        }
    }
}

fn increment_external_pack_explained_constructor_count(
    report: &mut ResolverBenchmarkReport,
    mode: ExternalLibraryMode,
) {
    match mode {
        ExternalLibraryMode::ExternalFactModel => {
            report.external_fact_model_explained_constructors += 1;
        }
        ExternalLibraryMode::RustCrateMapping => {
            report.rust_crate_mapping_explained_constructors += 1;
        }
        ExternalLibraryMode::FfiBoundary => report.ffi_boundary_explained_constructors += 1,
        ExternalLibraryMode::ConservativeBoundary => {
            report.conservative_boundary_explained_constructors += 1;
        }
    }
}

fn benchmark_parsed_java_file(
    parsed: ParsedProgram,
    source_symbols: &crate::resolve::SourceSymbols,
    source_type_names: &std::collections::HashSet<String>,
    source_index: Option<&SourceTypeIndex>,
    external_facts: &FactSet,
    enable_builtin_external_packs: bool,
    report: &mut ResolverBenchmarkReport,
    file_label: &str,
    current_mode: DependencyMode,
    source_type_modes: &HashMap<String, DependencyMode>,
    source_type_fields: &HashMap<String, HashMap<String, SourceFieldInfo>>,
) {
    let resolved = crate::resolve::resolve_program_with_source_symbols_and_facts(
        &parsed.program,
        parsed.diagnostics,
        source_symbols,
        external_facts,
    );
    report.resolver_diagnostics += resolved
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.severity == Severity::Error)
        .count();
    for diagnostic in resolved
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.severity == Severity::Error)
    {
        push_diagnostic_summary(
            &mut report.top_resolver_diagnostics,
            diagnostic.code,
            &diagnostic.message,
            file_label,
            None,
        );
    }
    if !resolved.diagnostics.has_errors() {
        report.files_resolved_without_errors += 1;
    }
    collect_source_dependency_resolution_counts(
        &resolved.program,
        current_mode,
        source_type_modes,
        source_type_fields,
        file_label,
        report,
    );

    let semantic = crate::semantic_analysis::analyze_program_with_config(
        &resolved.program,
        &resolved.facts,
        external_facts,
        crate::semantic_analysis::SemanticAnalysisConfig {
            enable_builtin_external_packs,
            ..crate::semantic_analysis::SemanticAnalysisConfig::default()
        },
    );
    report.semantic_diagnostics += semantic.diagnostics.len();
    for diagnostic in &semantic.diagnostics {
        push_diagnostic_summary(
            &mut report.top_semantic_diagnostics,
            diagnostic.code,
            &diagnostic.message,
            file_label,
            None,
        );
    }
    collect_resolved_external_pack_field_access_counts(
        &resolved.program,
        &resolved.facts,
        &semantic.facts,
        file_label,
        report,
    );
    report.external_api_guesses += semantic
        .facts
        .external_api_effect_facts
        .iter()
        .filter(|fact| fact.provenance == FactProvenance::ConservativeFallback)
        .count();
    report.external_pack_api_facts += semantic
        .facts
        .external_api_effect_facts
        .iter()
        .filter(|fact| fact.provenance == FactProvenance::ExternalLibraryPack)
        .filter(|fact| !external_facts.external_api_effect_facts.contains(fact))
        .count();
    report.external_pack_constructor_facts += semantic
        .facts
        .external_constructor_facts
        .iter()
        .filter(|fact| fact.provenance == FactProvenance::ExternalLibraryPack)
        .filter(|fact| !external_facts.external_constructor_facts.contains(fact))
        .count();
    for fact in semantic
        .facts
        .external_api_effect_facts
        .iter()
        .filter(|fact| fact.provenance == FactProvenance::ExternalLibraryPack)
        .filter(|fact| !external_facts.external_api_effect_facts.contains(fact))
    {
        if let Some(mode) = fact
            .receiver_type
            .as_deref()
            .and_then(|receiver_type| external_pack_mode_for_type(&semantic.facts, receiver_type))
        {
            increment_external_pack_api_mode_count(report, mode);
        }
    }
    for fact in semantic
        .facts
        .external_constructor_facts
        .iter()
        .filter(|fact| fact.provenance == FactProvenance::ExternalLibraryPack)
        .filter(|fact| !external_facts.external_constructor_facts.contains(fact))
    {
        if let Some(mode) = external_pack_mode_for_type(&semantic.facts, &fact.type_name) {
            increment_external_pack_constructor_mode_count(report, mode);
        }
    }
    let constructor_guess_ownership =
        ConstructorGuessOwnership::from_conservative_constructor_facts(
            &semantic.facts,
            source_type_names,
            source_index,
            &resolved.program,
        );
    for fact in semantic
        .facts
        .external_constructor_facts
        .iter()
        .filter(|fact| fact.provenance == FactProvenance::ConservativeFallback)
    {
        match constructor_guess_ownership.ownership_for(&fact.type_name) {
            DependencyGuessOwner::Source => {
                report.source_constructor_guesses += 1;
                push_external_constructor_guess_summary(
                    &mut report.top_source_constructor_guess_types,
                    &fact.type_name,
                    fact.arg_types.len(),
                );
            }
            DependencyGuessOwner::Project => {
                report.project_constructor_guesses += 1;
                push_external_constructor_guess_summary(
                    &mut report.top_project_constructor_guess_types,
                    &fact.type_name,
                    fact.arg_types.len(),
                );
            }
            DependencyGuessOwner::External => {
                report.external_constructor_guesses += 1;
                push_external_constructor_guess_summary(
                    &mut report.top_external_constructor_guess_types,
                    &fact.type_name,
                    fact.arg_types.len(),
                );
            }
        }
    }
    let field_guess_ownership = FieldGuessOwnership::from_conservative_field_facts(
        &semantic.facts,
        source_type_names,
        source_index,
        &resolved.program,
    );
    for fact in semantic
        .facts
        .external_field_facts
        .iter()
        .filter(|fact| fact.provenance == FactProvenance::ConservativeFallback)
    {
        match field_guess_ownership.ownership_for(&fact.field.type_name) {
            FieldGuessOwner::Source => {
                report.source_field_guesses += 1;
                push_external_field_guess_summary(
                    &mut report.top_source_field_guess_targets,
                    &fact.field.type_name,
                    &fact.field.field_name,
                    None,
                );
            }
            FieldGuessOwner::Project => {
                report.project_field_guesses += 1;
                push_external_field_guess_summary(
                    &mut report.top_project_field_guess_targets,
                    &fact.field.type_name,
                    &fact.field.field_name,
                    None,
                );
            }
            FieldGuessOwner::External => {
                report.external_field_guesses += 1;
                push_external_field_guess_summary(
                    &mut report.top_external_field_guess_targets,
                    &fact.field.type_name,
                    &fact.field.field_name,
                    None,
                );
            }
        }
    }
    report.external_api_return_nullability_facts += semantic
        .facts
        .external_api_effect_facts
        .iter()
        .filter(|fact| !external_facts.external_api_effect_facts.contains(fact))
        .filter(|fact| fact.return_nullability != crate::facts::NullabilityState::Unknown)
        .count();
    report.external_api_fluent_builder_facts += semantic
        .facts
        .external_api_effect_facts
        .iter()
        .filter(|fact| !external_facts.external_api_effect_facts.contains(fact))
        .filter(|fact| fact.role == ExternalApiRole::FluentMutator)
        .count();
    report.external_api_argument_effect_facts +=
        semantic.facts.external_api_argument_effect_facts.len();
    for fact in semantic
        .facts
        .external_api_effect_facts
        .iter()
        .filter(|fact| fact.provenance == FactProvenance::ConservativeFallback)
    {
        push_external_api_guess_summary(
            &mut report.top_external_api_guess_methods,
            fact.receiver_type.as_deref().unwrap_or("*"),
            &fact.method_name,
        );
    }
    report.total_facts += fact_count_excluding_provider_facts(&semantic.facts, external_facts);

    for fact in &resolved.facts.unresolved_target_facts {
        match fact.kind {
            UnresolvedTargetKind::MethodCall => {
                report.unresolved_method_calls += 1;
                let receiver_type = unresolved_method_receiver_type_key(&resolved.program, fact);
                if let Some(effect) = semantic.facts.external_api_effect_for_receiver_with_arity(
                    receiver_type.as_deref(),
                    &fact.target_name,
                    Some(fact.arg_types.len()),
                ) {
                    report.external_api_explained_method_calls += 1;
                    if effect.provenance == FactProvenance::ExternalLibraryPack {
                        report.external_pack_explained_method_calls += 1;
                        let mode = receiver_type.as_deref().and_then(|receiver_type| {
                            external_pack_mode_for_type(&semantic.facts, receiver_type)
                        });
                        push_external_pack_api_summary(
                            &mut report.top_external_pack_api_methods,
                            receiver_type.as_deref().unwrap_or("*"),
                            &fact.target_name,
                            mode,
                        );
                        push_external_api_method_context_summary(
                            &mut report.top_external_pack_api_method_contexts,
                            receiver_type.as_deref().unwrap_or("*"),
                            &fact.target_name,
                            mode,
                            fact.arg_types.len(),
                            &fact.enclosing_type,
                            fact.enclosing_member.as_deref().unwrap_or("*"),
                            file_label,
                        );
                        if let Some(mode) = mode {
                            increment_external_pack_explained_method_count(report, mode);
                        }
                    }
                } else if let Some(unsupported_provenance) =
                    receiver_type.as_deref().and_then(|receiver| {
                        unsupported_external_api_provenance_for_receiver(
                            &semantic.facts,
                            enable_builtin_external_packs,
                            receiver,
                            &fact.target_name,
                        )
                    })
                {
                    report.external_api_explained_method_calls += 1;
                    report.external_api_unsupported_method_calls += 1;
                    if unsupported_provenance == FactProvenance::ExternalLibraryPack {
                        report.external_pack_explained_method_calls += 1;
                        if let Some(mode) = receiver_type.as_deref().and_then(|receiver_type| {
                            external_pack_mode_for_type(&semantic.facts, receiver_type)
                        }) {
                            increment_external_pack_explained_method_count(report, mode);
                        }
                    }
                    let mode = receiver_type.as_deref().and_then(|receiver_type| {
                        external_pack_mode_for_type(&semantic.facts, receiver_type)
                    });
                    push_external_pack_api_summary(
                        &mut report.top_external_api_unsupported_methods,
                        receiver_type.as_deref().unwrap_or("*"),
                        &fact.target_name,
                        mode,
                    );
                    push_external_api_method_context_summary(
                        &mut report.top_external_api_unsupported_method_contexts,
                        receiver_type.as_deref().unwrap_or("*"),
                        &fact.target_name,
                        mode,
                        fact.arg_types.len(),
                        &fact.enclosing_type,
                        fact.enclosing_member.as_deref().unwrap_or("*"),
                        file_label,
                    );
                } else if let Some(mode) = receiver_type.as_deref().and_then(|receiver| {
                    missing_external_pack_mode_for_type(
                        &semantic.facts,
                        enable_builtin_external_packs,
                        receiver,
                    )
                }) {
                    report.external_api_explained_method_calls += 1;
                    report.external_api_missing_method_calls += 1;
                    report.external_pack_explained_method_calls += 1;
                    increment_external_pack_explained_method_count(report, mode);
                    push_external_pack_api_summary(
                        &mut report.top_external_api_missing_methods,
                        receiver_type.as_deref().unwrap_or("*"),
                        &fact.target_name,
                        Some(mode),
                    );
                    push_external_api_method_context_summary(
                        &mut report.top_external_api_missing_method_contexts,
                        receiver_type.as_deref().unwrap_or("*"),
                        &fact.target_name,
                        Some(mode),
                        fact.arg_types.len(),
                        &fact.enclosing_type,
                        fact.enclosing_member.as_deref().unwrap_or("*"),
                        file_label,
                    );
                } else {
                    report.unexplained_method_calls += 1;
                    push_unresolved_summary(
                        &mut report.top_unexplained_method_targets,
                        fact.kind,
                        &fact.target_name,
                    );
                    push_unexplained_method_receiver_summary(
                        &mut report.top_unexplained_method_receivers,
                        &fact.target_name,
                        receiver_type.as_deref().unwrap_or("*"),
                    );
                }
            }
            UnresolvedTargetKind::ConstructorCall => {
                report.unresolved_constructors += 1;
                let target_type_name = fact
                    .receiver_type
                    .as_ref()
                    .and_then(java_type_key)
                    .unwrap_or_else(|| fact.target_name.clone());
                if fact.is_anonymous_class {
                    report.anonymous_class_constructors += 1;
                    push_external_constructor_guess_summary(
                        &mut report.top_anonymous_constructor_targets,
                        &target_type_name,
                        fact.arg_types.len(),
                    );
                } else if is_source_type_reference(
                    source_type_names,
                    source_index,
                    &resolved.program,
                    &target_type_name,
                ) {
                    report.unexplained_constructors += 1;
                    report.unexplained_source_constructors += 1;
                    push_unresolved_summary(
                        &mut report.top_unexplained_constructor_targets,
                        fact.kind,
                        &fact.target_name,
                    );
                    push_unresolved_constructor_context_summary(
                        &mut report.top_unexplained_constructor_contexts,
                        &fact.target_name,
                        &target_type_name,
                        &fact
                            .arg_types
                            .iter()
                            .map(java_type_summary)
                            .collect::<Vec<_>>(),
                        &fact.enclosing_type,
                        fact.enclosing_member.as_deref().unwrap_or("*"),
                        file_label,
                    );
                } else if let Some(constructor) = semantic
                    .facts
                    .external_constructor_for(&target_type_name, &fact.arg_types)
                {
                    report.external_api_explained_constructors += 1;
                    match constructor.provenance {
                        FactProvenance::ExternalLibraryPack => {
                            report.external_pack_explained_constructors += 1;
                            let mode =
                                external_pack_mode_for_type(&semantic.facts, &target_type_name);
                            push_external_pack_constructor_summary(
                                &mut report.top_external_pack_constructor_types,
                                &target_type_name,
                                fact.arg_types.len(),
                                mode,
                            );
                            push_external_api_constructor_context_summary(
                                &mut report.top_external_pack_constructor_contexts,
                                &fact.target_name,
                                &target_type_name,
                                mode,
                                &fact
                                    .arg_types
                                    .iter()
                                    .map(java_type_summary)
                                    .collect::<Vec<_>>(),
                                &fact.enclosing_type,
                                fact.enclosing_member.as_deref().unwrap_or("*"),
                                file_label,
                            );
                            if let Some(mode) = mode {
                                increment_external_pack_explained_constructor_count(report, mode);
                            }
                        }
                        FactProvenance::ConservativeFallback => {
                            let arg_types = fact
                                .arg_types
                                .iter()
                                .map(java_type_summary)
                                .collect::<Vec<_>>();
                            match constructor_guess_ownership
                                .ownership_for_unresolved(fact, &target_type_name)
                            {
                                DependencyGuessOwner::Source => {
                                    push_unresolved_constructor_context_summary(
                                        &mut report.top_source_constructor_guess_contexts,
                                        &fact.target_name,
                                        &target_type_name,
                                        &arg_types,
                                        &fact.enclosing_type,
                                        fact.enclosing_member.as_deref().unwrap_or("*"),
                                        file_label,
                                    );
                                }
                                DependencyGuessOwner::Project => {
                                    push_unresolved_constructor_context_summary(
                                        &mut report.top_project_constructor_guess_contexts,
                                        &fact.target_name,
                                        &target_type_name,
                                        &arg_types,
                                        &fact.enclosing_type,
                                        fact.enclosing_member.as_deref().unwrap_or("*"),
                                        file_label,
                                    );
                                }
                                DependencyGuessOwner::External => {
                                    push_unresolved_constructor_context_summary(
                                        &mut report.top_external_constructor_guess_contexts,
                                        &fact.target_name,
                                        &target_type_name,
                                        &arg_types,
                                        &fact.enclosing_type,
                                        fact.enclosing_member.as_deref().unwrap_or("*"),
                                        file_label,
                                    );
                                }
                            }
                        }
                        _ => {}
                    }
                } else if let Some(unsupported_provenance) =
                    unsupported_external_constructor_provenance_for(
                        &semantic.facts,
                        enable_builtin_external_packs,
                        &target_type_name,
                        fact.arg_types.len(),
                    )
                {
                    report.external_api_explained_constructors += 1;
                    report.external_api_unsupported_constructor_calls += 1;
                    let mode = external_pack_mode_for_type(&semantic.facts, &target_type_name);
                    push_external_pack_constructor_summary(
                        &mut report.top_external_api_unsupported_constructors,
                        &target_type_name,
                        fact.arg_types.len(),
                        mode,
                    );
                    push_external_api_constructor_context_summary(
                        &mut report.top_external_api_unsupported_constructor_contexts,
                        &fact.target_name,
                        &target_type_name,
                        mode,
                        &fact
                            .arg_types
                            .iter()
                            .map(java_type_summary)
                            .collect::<Vec<_>>(),
                        &fact.enclosing_type,
                        fact.enclosing_member.as_deref().unwrap_or("*"),
                        file_label,
                    );
                    if unsupported_provenance == FactProvenance::ExternalLibraryPack {
                        report.external_pack_explained_constructors += 1;
                        if let Some(mode) = mode {
                            increment_external_pack_explained_constructor_count(report, mode);
                        }
                    }
                } else if let Some(mode) = missing_external_pack_mode_for_type(
                    &semantic.facts,
                    enable_builtin_external_packs,
                    &target_type_name,
                ) {
                    report.external_api_explained_constructors += 1;
                    report.external_api_missing_constructor_calls += 1;
                    report.external_pack_explained_constructors += 1;
                    increment_external_pack_explained_constructor_count(report, mode);
                    push_external_pack_constructor_summary(
                        &mut report.top_external_api_missing_constructors,
                        &target_type_name,
                        fact.arg_types.len(),
                        Some(mode),
                    );
                    push_external_api_constructor_context_summary(
                        &mut report.top_external_api_missing_constructor_contexts,
                        &fact.target_name,
                        &target_type_name,
                        Some(mode),
                        &fact
                            .arg_types
                            .iter()
                            .map(java_type_summary)
                            .collect::<Vec<_>>(),
                        &fact.enclosing_type,
                        fact.enclosing_member.as_deref().unwrap_or("*"),
                        file_label,
                    );
                } else {
                    report.unexplained_constructors += 1;
                    report.unexplained_non_source_constructors += 1;
                    push_unresolved_summary(
                        &mut report.top_unexplained_constructor_targets,
                        fact.kind,
                        &fact.target_name,
                    );
                    push_unresolved_constructor_context_summary(
                        &mut report.top_unexplained_constructor_contexts,
                        &fact.target_name,
                        &target_type_name,
                        &fact
                            .arg_types
                            .iter()
                            .map(java_type_summary)
                            .collect::<Vec<_>>(),
                        &fact.enclosing_type,
                        fact.enclosing_member.as_deref().unwrap_or("*"),
                        file_label,
                    );
                }
                push_unresolved_summary(
                    &mut report.top_unresolved_constructor_targets,
                    fact.kind,
                    &fact.target_name,
                );
            }
            UnresolvedTargetKind::ConstructorDelegation => {
                report.unresolved_constructor_delegations += 1;
            }
            UnresolvedTargetKind::FieldAccess => {
                report.unresolved_field_accesses += 1;
                push_unresolved_summary(
                    &mut report.top_unresolved_field_targets,
                    fact.kind,
                    &fact.target_name,
                );
                let receiver_type = fact
                    .receiver_type
                    .as_ref()
                    .and_then(java_type_key)
                    .unwrap_or_else(|| "*".to_string());
                if let Some(external_field) = semantic
                    .facts
                    .external_field_for(&receiver_type, &fact.target_name)
                {
                    report.external_api_explained_field_accesses += 1;
                    match external_field.provenance {
                        FactProvenance::ExternalLibraryPack => {
                            let mode = external_pack_mode_for_type(&semantic.facts, &receiver_type);
                            push_external_field_guess_summary(
                                &mut report.top_external_pack_field_accesses,
                                &receiver_type,
                                &fact.target_name,
                                mode,
                            );
                            push_external_api_field_context_summary(
                                &mut report.top_external_pack_field_contexts,
                                &fact.target_name,
                                &receiver_type,
                                mode,
                                &fact.enclosing_type,
                                fact.enclosing_member.as_deref().unwrap_or("*"),
                                file_label,
                            );
                        }
                        FactProvenance::ConservativeFallback => {
                            match field_guess_ownership.ownership_for(&receiver_type) {
                                FieldGuessOwner::Source => {
                                    push_unresolved_field_context_summary(
                                        &mut report.top_source_field_guess_contexts,
                                        &fact.target_name,
                                        &receiver_type,
                                        &fact.enclosing_type,
                                        fact.enclosing_member.as_deref().unwrap_or("*"),
                                        file_label,
                                    );
                                }
                                FieldGuessOwner::Project => {
                                    push_unresolved_field_context_summary(
                                        &mut report.top_project_field_guess_contexts,
                                        &fact.target_name,
                                        &receiver_type,
                                        &fact.enclosing_type,
                                        fact.enclosing_member.as_deref().unwrap_or("*"),
                                        file_label,
                                    );
                                }
                                FieldGuessOwner::External => {
                                    push_unresolved_field_context_summary(
                                        &mut report.top_external_field_guess_contexts,
                                        &fact.target_name,
                                        &receiver_type,
                                        &fact.enclosing_type,
                                        fact.enclosing_member.as_deref().unwrap_or("*"),
                                        file_label,
                                    );
                                }
                            }
                        }
                        _ => {}
                    }
                } else if unsupported_external_field_provenance_for(
                    &semantic.facts,
                    enable_builtin_external_packs,
                    &receiver_type,
                    &fact.target_name,
                )
                .is_some()
                {
                    let mode = external_pack_mode_for_type(&semantic.facts, &receiver_type);
                    report.external_api_explained_field_accesses += 1;
                    report.external_api_unsupported_field_accesses += 1;
                    push_external_field_guess_summary(
                        &mut report.top_external_api_unsupported_fields,
                        &receiver_type,
                        &fact.target_name,
                        mode,
                    );
                    push_external_api_field_context_summary(
                        &mut report.top_external_api_unsupported_field_contexts,
                        &fact.target_name,
                        &receiver_type,
                        mode,
                        &fact.enclosing_type,
                        fact.enclosing_member.as_deref().unwrap_or("*"),
                        file_label,
                    );
                } else if let Some(mode) = missing_external_pack_mode_for_type(
                    &semantic.facts,
                    enable_builtin_external_packs,
                    &receiver_type,
                ) {
                    report.external_api_explained_field_accesses += 1;
                    report.external_api_missing_field_accesses += 1;
                    push_external_field_guess_summary(
                        &mut report.top_external_api_missing_fields,
                        &receiver_type,
                        &fact.target_name,
                        Some(mode),
                    );
                    push_external_api_field_context_summary(
                        &mut report.top_external_api_missing_field_contexts,
                        &fact.target_name,
                        &receiver_type,
                        Some(mode),
                        &fact.enclosing_type,
                        fact.enclosing_member.as_deref().unwrap_or("*"),
                        file_label,
                    );
                } else {
                    report.unexplained_field_accesses += 1;
                    if source_type_names.contains(&receiver_type)
                        || source_type_names.contains(&crate::parser::to_snake_case(&receiver_type))
                    {
                        report.unexplained_source_field_accesses += 1;
                    } else {
                        report.unexplained_non_source_field_accesses += 1;
                    }
                    push_unresolved_summary(
                        &mut report.top_unexplained_field_targets,
                        fact.kind,
                        &fact.target_name,
                    );
                    push_unresolved_field_receiver_summary(
                        &mut report.top_unexplained_field_receivers,
                        &fact.target_name,
                        &receiver_type,
                    );
                    push_unresolved_field_receiver_file_summary(
                        &mut report.top_unexplained_field_receiver_files,
                        &fact.target_name,
                        &receiver_type,
                        file_label,
                    );
                    push_unresolved_field_context_summary(
                        &mut report.top_unexplained_field_contexts,
                        &fact.target_name,
                        &receiver_type,
                        &fact.enclosing_type,
                        fact.enclosing_member.as_deref().unwrap_or("*"),
                        file_label,
                    );
                }
                push_unresolved_field_receiver_summary(
                    &mut report.top_unresolved_field_receivers,
                    &fact.target_name,
                    &receiver_type,
                );
            }
        }
        push_unresolved_summary(
            &mut report.top_unresolved_targets,
            fact.kind,
            &fact.target_name,
        );
    }
    report.top_unresolved_targets.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then_with(|| left.target_name.cmp(&right.target_name))
    });
    report.top_unresolved_targets.truncate(20);
    report
        .top_unexplained_method_targets
        .sort_by(|left, right| {
            right
                .count
                .cmp(&left.count)
                .then_with(|| left.target_name.cmp(&right.target_name))
        });
    report.top_unexplained_method_targets.truncate(20);
    report
        .top_unexplained_method_receivers
        .sort_by(|left, right| {
            right
                .count
                .cmp(&left.count)
                .then_with(|| left.target_name.cmp(&right.target_name))
                .then_with(|| left.receiver_type.cmp(&right.receiver_type))
        });
    report.top_unexplained_method_receivers.truncate(20);
    report
        .top_external_api_guess_methods
        .sort_by(|left, right| {
            right
                .count
                .cmp(&left.count)
                .then_with(|| left.receiver_type.cmp(&right.receiver_type))
                .then_with(|| left.method_name.cmp(&right.method_name))
        });
    report.top_external_api_guess_methods.truncate(20);
    report.top_external_pack_api_methods.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then_with(|| left.receiver_type.cmp(&right.receiver_type))
            .then_with(|| left.method_name.cmp(&right.method_name))
            .then_with(|| {
                external_pack_summary_mode_label(left.mode)
                    .cmp(external_pack_summary_mode_label(right.mode))
            })
    });
    report.top_external_pack_api_methods.truncate(20);
    sort_external_api_method_contexts(&mut report.top_external_pack_api_method_contexts);
    report.top_external_pack_api_method_contexts.truncate(20);
    report
        .top_external_api_unsupported_methods
        .sort_by(|left, right| {
            right
                .count
                .cmp(&left.count)
                .then_with(|| left.receiver_type.cmp(&right.receiver_type))
                .then_with(|| left.method_name.cmp(&right.method_name))
                .then_with(|| {
                    external_pack_summary_mode_label(left.mode)
                        .cmp(external_pack_summary_mode_label(right.mode))
                })
        });
    report.top_external_api_unsupported_methods.truncate(20);
    report
        .top_external_api_missing_methods
        .sort_by(|left, right| {
            right
                .count
                .cmp(&left.count)
                .then_with(|| left.receiver_type.cmp(&right.receiver_type))
                .then_with(|| left.method_name.cmp(&right.method_name))
                .then_with(|| {
                    external_pack_summary_mode_label(left.mode)
                        .cmp(external_pack_summary_mode_label(right.mode))
                })
        });
    report.top_external_api_missing_methods.truncate(20);
    sort_external_api_method_contexts(&mut report.top_external_api_unsupported_method_contexts);
    report
        .top_external_api_unsupported_method_contexts
        .truncate(20);
    sort_external_api_method_contexts(&mut report.top_external_api_missing_method_contexts);
    report.top_external_api_missing_method_contexts.truncate(20);
    report
        .top_external_constructor_guess_types
        .sort_by(|left, right| {
            right
                .count
                .cmp(&left.count)
                .then_with(|| left.type_name.cmp(&right.type_name))
                .then_with(|| left.arity.cmp(&right.arity))
        });
    report.top_external_constructor_guess_types.truncate(20);
    report
        .top_external_pack_constructor_types
        .sort_by(|left, right| {
            right
                .count
                .cmp(&left.count)
                .then_with(|| left.type_name.cmp(&right.type_name))
                .then_with(|| left.arity.cmp(&right.arity))
                .then_with(|| {
                    external_pack_summary_mode_label(left.mode)
                        .cmp(external_pack_summary_mode_label(right.mode))
                })
        });
    report.top_external_pack_constructor_types.truncate(20);
    sort_external_api_constructor_contexts(&mut report.top_external_pack_constructor_contexts);
    report.top_external_pack_constructor_contexts.truncate(20);
    report
        .top_external_api_unsupported_constructors
        .sort_by(|left, right| {
            right
                .count
                .cmp(&left.count)
                .then_with(|| left.type_name.cmp(&right.type_name))
                .then_with(|| left.arity.cmp(&right.arity))
                .then_with(|| {
                    external_pack_summary_mode_label(left.mode)
                        .cmp(external_pack_summary_mode_label(right.mode))
                })
        });
    report
        .top_external_api_unsupported_constructors
        .truncate(20);
    report
        .top_external_api_missing_constructors
        .sort_by(|left, right| {
            right
                .count
                .cmp(&left.count)
                .then_with(|| left.type_name.cmp(&right.type_name))
                .then_with(|| left.arity.cmp(&right.arity))
        });
    report.top_external_api_missing_constructors.truncate(20);
    sort_external_api_constructor_contexts(
        &mut report.top_external_api_unsupported_constructor_contexts,
    );
    report
        .top_external_api_unsupported_constructor_contexts
        .truncate(20);
    sort_external_api_constructor_contexts(
        &mut report.top_external_api_missing_constructor_contexts,
    );
    report
        .top_external_api_missing_constructor_contexts
        .truncate(20);
    report
        .top_project_constructor_guess_types
        .sort_by(|left, right| {
            right
                .count
                .cmp(&left.count)
                .then_with(|| left.type_name.cmp(&right.type_name))
                .then_with(|| left.arity.cmp(&right.arity))
        });
    report.top_project_constructor_guess_types.truncate(20);
    report
        .top_source_constructor_guess_types
        .sort_by(|left, right| {
            right
                .count
                .cmp(&left.count)
                .then_with(|| left.type_name.cmp(&right.type_name))
                .then_with(|| left.arity.cmp(&right.arity))
        });
    report.top_source_constructor_guess_types.truncate(20);
    report
        .top_source_dependency_method_calls
        .sort_by(|left, right| {
            right
                .count
                .cmp(&left.count)
                .then_with(|| left.method_name.cmp(&right.method_name))
                .then_with(|| left.receiver_type.cmp(&right.receiver_type))
                .then_with(|| {
                    dependency_mode_label(Some(left.source_mode))
                        .cmp(dependency_mode_label(Some(right.source_mode)))
                })
        });
    report.top_source_dependency_method_calls.truncate(20);
    report
        .top_source_dependency_constructor_types
        .sort_by(|left, right| {
            right
                .count
                .cmp(&left.count)
                .then_with(|| left.type_name.cmp(&right.type_name))
                .then_with(|| left.arity.cmp(&right.arity))
                .then_with(|| {
                    dependency_mode_label(Some(left.source_mode))
                        .cmp(dependency_mode_label(Some(right.source_mode)))
                })
        });
    report.top_source_dependency_constructor_types.truncate(20);
    report
        .top_source_dependency_field_accesses
        .sort_by(|left, right| {
            right
                .count
                .cmp(&left.count)
                .then_with(|| left.type_name.cmp(&right.type_name))
                .then_with(|| left.field_name.cmp(&right.field_name))
                .then_with(|| {
                    dependency_mode_label(Some(left.source_mode))
                        .cmp(dependency_mode_label(Some(right.source_mode)))
                })
        });
    report.top_source_dependency_field_accesses.truncate(20);
    sort_source_dependency_method_contexts(&mut report.top_source_dependency_method_contexts);
    report.top_source_dependency_method_contexts.truncate(20);
    sort_source_dependency_constructor_contexts(
        &mut report.top_source_dependency_constructor_contexts,
    );
    report
        .top_source_dependency_constructor_contexts
        .truncate(20);
    sort_source_dependency_field_contexts(&mut report.top_source_dependency_field_contexts);
    report.top_source_dependency_field_contexts.truncate(20);
    sort_constructor_contexts(&mut report.top_external_constructor_guess_contexts);
    report.top_external_constructor_guess_contexts.truncate(20);
    sort_constructor_contexts(&mut report.top_project_constructor_guess_contexts);
    report.top_project_constructor_guess_contexts.truncate(20);
    sort_constructor_contexts(&mut report.top_source_constructor_guess_contexts);
    report.top_source_constructor_guess_contexts.truncate(20);
    report
        .top_anonymous_constructor_targets
        .sort_by(|left, right| {
            right
                .count
                .cmp(&left.count)
                .then_with(|| left.type_name.cmp(&right.type_name))
                .then_with(|| left.arity.cmp(&right.arity))
        });
    report.top_anonymous_constructor_targets.truncate(20);
    sort_external_field_guess_summaries(&mut report.top_external_field_guess_targets);
    report.top_external_field_guess_targets.truncate(20);
    sort_external_field_guess_summaries(&mut report.top_project_field_guess_targets);
    report.top_project_field_guess_targets.truncate(20);
    sort_external_field_guess_summaries(&mut report.top_source_field_guess_targets);
    report.top_source_field_guess_targets.truncate(20);
    report
        .top_external_pack_field_accesses
        .sort_by(|left, right| {
            right
                .count
                .cmp(&left.count)
                .then_with(|| left.type_name.cmp(&right.type_name))
                .then_with(|| left.field_name.cmp(&right.field_name))
                .then_with(|| {
                    external_pack_summary_mode_label(left.mode)
                        .cmp(external_pack_summary_mode_label(right.mode))
                })
        });
    report.top_external_pack_field_accesses.truncate(20);
    sort_external_api_field_contexts(&mut report.top_external_pack_field_contexts);
    report.top_external_pack_field_contexts.truncate(20);
    report
        .top_external_api_unsupported_fields
        .sort_by(|left, right| {
            right
                .count
                .cmp(&left.count)
                .then_with(|| left.type_name.cmp(&right.type_name))
                .then_with(|| left.field_name.cmp(&right.field_name))
                .then_with(|| {
                    external_pack_summary_mode_label(left.mode)
                        .cmp(external_pack_summary_mode_label(right.mode))
                })
        });
    report.top_external_api_unsupported_fields.truncate(20);
    report
        .top_external_api_missing_fields
        .sort_by(|left, right| {
            right
                .count
                .cmp(&left.count)
                .then_with(|| left.type_name.cmp(&right.type_name))
                .then_with(|| left.field_name.cmp(&right.field_name))
                .then_with(|| {
                    external_pack_summary_mode_label(left.mode)
                        .cmp(external_pack_summary_mode_label(right.mode))
                })
        });
    report.top_external_api_missing_fields.truncate(20);
    sort_external_api_field_contexts(&mut report.top_external_api_unsupported_field_contexts);
    report
        .top_external_api_unsupported_field_contexts
        .truncate(20);
    sort_external_api_field_contexts(&mut report.top_external_api_missing_field_contexts);
    report.top_external_api_missing_field_contexts.truncate(20);
    sort_field_contexts(&mut report.top_external_field_guess_contexts);
    report.top_external_field_guess_contexts.truncate(20);
    sort_field_contexts(&mut report.top_project_field_guess_contexts);
    report.top_project_field_guess_contexts.truncate(20);
    sort_field_contexts(&mut report.top_source_field_guess_contexts);
    report.top_source_field_guess_contexts.truncate(20);
    report.top_resolver_diagnostics.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then_with(|| left.code.cmp(right.code))
            .then_with(|| left.message.cmp(&right.message))
    });
    report.top_resolver_diagnostics.truncate(20);
    report.top_semantic_diagnostics.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then_with(|| left.code.cmp(right.code))
            .then_with(|| left.message.cmp(&right.message))
    });
    report.top_semantic_diagnostics.truncate(20);
    report
        .top_object_representation_diagnostics
        .sort_by(|left, right| {
            right
                .count
                .cmp(&left.count)
                .then_with(|| left.code.cmp(right.code))
                .then_with(|| left.message.cmp(&right.message))
                .then_with(|| {
                    dependency_mode_label(left.mode).cmp(dependency_mode_label(right.mode))
                })
        });
    report.top_object_representation_diagnostics.truncate(20);
    sort_and_truncate_target_summaries(&mut report.top_unresolved_constructor_targets);
    sort_and_truncate_target_summaries(&mut report.top_unexplained_constructor_targets);
    sort_constructor_contexts(&mut report.top_unexplained_constructor_contexts);
    report.top_unexplained_constructor_contexts.truncate(20);
    sort_and_truncate_target_summaries(&mut report.top_unresolved_field_targets);
    sort_and_truncate_target_summaries(&mut report.top_unexplained_field_targets);
    report
        .top_unresolved_field_receivers
        .sort_by(|left, right| {
            right
                .count
                .cmp(&left.count)
                .then_with(|| left.field_name.cmp(&right.field_name))
                .then_with(|| left.receiver_type.cmp(&right.receiver_type))
        });
    report.top_unresolved_field_receivers.truncate(20);
    report
        .top_unexplained_field_receivers
        .sort_by(|left, right| {
            right
                .count
                .cmp(&left.count)
                .then_with(|| left.field_name.cmp(&right.field_name))
                .then_with(|| left.receiver_type.cmp(&right.receiver_type))
        });
    report.top_unexplained_field_receivers.truncate(20);
    report
        .top_unexplained_field_receiver_files
        .sort_by(|left, right| {
            right
                .count
                .cmp(&left.count)
                .then_with(|| left.field_name.cmp(&right.field_name))
                .then_with(|| left.receiver_type.cmp(&right.receiver_type))
        });
    report.top_unexplained_field_receiver_files.truncate(20);
    report
        .top_unexplained_field_contexts
        .sort_by(|left, right| {
            right
                .count
                .cmp(&left.count)
                .then_with(|| left.field_name.cmp(&right.field_name))
                .then_with(|| left.receiver_type.cmp(&right.receiver_type))
                .then_with(|| left.enclosing_type.cmp(&right.enclosing_type))
                .then_with(|| left.enclosing_member.cmp(&right.enclosing_member))
        });
    report.top_unexplained_field_contexts.truncate(20);
}

fn collect_object_representation_counts_for_programs<'a>(
    programs: impl IntoIterator<Item = (&'a str, &'a Program)>,
    external_facts: &FactSet,
    report: &mut ResolverBenchmarkReport,
    benchmark_mode: ObjectRepresentationBenchmarkMode,
) {
    for (program_label, program) in programs {
        collect_object_representation_counts_for_program(
            program_label,
            None,
            program,
            program,
            external_facts,
            report,
            benchmark_mode,
        );
    }
}

fn collect_object_representation_counts_for_programs_with_context<'a>(
    programs: impl IntoIterator<Item = (&'a str, DependencyMode, Program, &'a Program)>,
    external_facts: &FactSet,
    report: &mut ResolverBenchmarkReport,
    benchmark_mode: ObjectRepresentationBenchmarkMode,
) {
    for (program_label, mode, context_program, diagnostic_program) in programs {
        collect_object_representation_counts_for_program(
            program_label,
            Some(mode),
            &context_program,
            diagnostic_program,
            external_facts,
            report,
            benchmark_mode,
        );
    }
}

fn collect_object_representation_counts_for_program(
    program_label: &str,
    mode: Option<DependencyMode>,
    context_program: &Program,
    diagnostic_program: &Program,
    external_facts: &FactSet,
    report: &mut ResolverBenchmarkReport,
    benchmark_mode: ObjectRepresentationBenchmarkMode,
) {
    let resolved_source = crate::resolve::resolve_program(
        context_program,
        crate::diagnostics::DiagnosticBag::default(),
    );
    let mut representation_facts = resolved_source.facts;
    representation_facts.extend_from(external_facts.clone());
    let representations = crate::representation::plan_representations(
        &resolved_source.program,
        &representation_facts,
    );
    if benchmark_mode.collect_diagnostics {
        let diagnostics = representations.diagnostics_for_program(diagnostic_program);
        report.object_representation_diagnostics += diagnostics.len();
        for diagnostic in diagnostics {
            push_diagnostic_summary(
                &mut report.top_object_representation_diagnostics,
                diagnostic.code,
                &diagnostic.message,
                program_label,
                mode,
            );
        }
    }
    if !benchmark_mode.count_representations {
        return;
    }
    for representation in representations.type_representations() {
        match &representation.representation {
            crate::representation::ObjectRepresentation::ConcreteStruct => {
                report.object_concrete_struct_representations += 1;
            }
            crate::representation::ObjectRepresentation::DynTrait { .. } => {
                report.object_dyn_trait_representations += 1;
            }
            crate::representation::ObjectRepresentation::ClosedEnum { .. } => {
                report.object_closed_enum_representations += 1;
            }
            crate::representation::ObjectRepresentation::ErasedObject => {
                report.object_erased_object_representations += 1;
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct ObjectRepresentationBenchmarkMode {
    count_representations: bool,
    collect_diagnostics: bool,
}

fn collect_source_dependency_resolution_counts(
    program: &Program,
    current_mode: DependencyMode,
    source_type_modes: &HashMap<String, DependencyMode>,
    source_type_fields: &HashMap<String, HashMap<String, SourceFieldInfo>>,
    file_label: &str,
    report: &mut ResolverBenchmarkReport,
) {
    for ty in &program.types {
        collect_type_source_dependency_resolution_counts(
            ty,
            current_mode,
            source_type_modes,
            source_type_fields,
            SourceDependencyWalkContext {
                file_label,
                enclosing_type: &ty.name,
                enclosing_member: "*",
            },
            report,
        );
    }
}

#[derive(Debug, Clone, Copy)]
struct SourceDependencyWalkContext<'a> {
    file_label: &'a str,
    enclosing_type: &'a str,
    enclosing_member: &'a str,
}

fn collect_type_source_dependency_resolution_counts(
    ty: &TypeDecl,
    current_mode: DependencyMode,
    source_type_modes: &HashMap<String, DependencyMode>,
    source_type_fields: &HashMap<String, HashMap<String, SourceFieldInfo>>,
    context: SourceDependencyWalkContext<'_>,
    report: &mut ResolverBenchmarkReport,
) {
    let type_context = SourceDependencyWalkContext {
        enclosing_type: &ty.name,
        ..context
    };
    let base_env = type_source_dependency_env(ty);
    for member in &ty.members {
        match member {
            Member::Field(field) => {
                if let Some(initializer) = &field.initializer {
                    let member_context = SourceDependencyWalkContext {
                        enclosing_member: &field.name,
                        ..type_context
                    };
                    collect_expr_source_dependency_resolution_counts(
                        initializer,
                        current_mode,
                        source_type_modes,
                        source_type_fields,
                        &base_env,
                        member_context,
                        report,
                    );
                }
            }
            Member::Method(method) => {
                if let Some(body) = &method.body {
                    let mut env = base_env.clone();
                    for param in &method.params {
                        env.insert_local(param.name.clone(), param.ty.clone());
                    }
                    let member_context = SourceDependencyWalkContext {
                        enclosing_member: &method.name,
                        ..type_context
                    };
                    collect_block_source_dependency_resolution_counts(
                        body,
                        current_mode,
                        source_type_modes,
                        source_type_fields,
                        &mut env,
                        member_context,
                        report,
                    );
                }
            }
            Member::Constructor(constructor) => {
                if let Some(body) = &constructor.body {
                    let mut env = base_env.clone();
                    for param in &constructor.params {
                        env.insert_local(param.name.clone(), param.ty.clone());
                    }
                    let member_context = SourceDependencyWalkContext {
                        enclosing_member: "<init>",
                        ..type_context
                    };
                    collect_block_source_dependency_resolution_counts(
                        body,
                        current_mode,
                        source_type_modes,
                        source_type_fields,
                        &mut env,
                        member_context,
                        report,
                    );
                }
            }
            Member::Type(nested) => collect_type_source_dependency_resolution_counts(
                nested,
                current_mode,
                source_type_modes,
                source_type_fields,
                type_context,
                report,
            ),
            Member::Unsupported { .. } => {}
        }
    }
}

fn collect_block_source_dependency_resolution_counts(
    block: &Block,
    current_mode: DependencyMode,
    source_type_modes: &HashMap<String, DependencyMode>,
    source_type_fields: &HashMap<String, HashMap<String, SourceFieldInfo>>,
    env: &mut TypeEnv,
    context: SourceDependencyWalkContext<'_>,
    report: &mut ResolverBenchmarkReport,
) {
    for statement in &block.statements {
        collect_stmt_source_dependency_resolution_counts(
            statement,
            current_mode,
            source_type_modes,
            source_type_fields,
            env,
            context,
            report,
        );
        env.learn_stmt(statement);
    }
}

fn collect_stmt_source_dependency_resolution_counts(
    statement: &Stmt,
    current_mode: DependencyMode,
    source_type_modes: &HashMap<String, DependencyMode>,
    source_type_fields: &HashMap<String, HashMap<String, SourceFieldInfo>>,
    env: &TypeEnv,
    context: SourceDependencyWalkContext<'_>,
    report: &mut ResolverBenchmarkReport,
) {
    match statement {
        Stmt::VarDecl { initializer, .. } => {
            if let Some(initializer) = initializer {
                collect_expr_source_dependency_resolution_counts(
                    initializer,
                    current_mode,
                    source_type_modes,
                    source_type_fields,
                    env,
                    context,
                    report,
                );
            }
        }
        Stmt::Assign { target, value } => {
            collect_expr_source_dependency_resolution_counts(
                target,
                current_mode,
                source_type_modes,
                source_type_fields,
                env,
                context,
                report,
            );
            collect_expr_source_dependency_resolution_counts(
                value,
                current_mode,
                source_type_modes,
                source_type_fields,
                env,
                context,
                report,
            );
        }
        Stmt::Expr(expr) | Stmt::Throw(expr) => collect_expr_source_dependency_resolution_counts(
            expr,
            current_mode,
            source_type_modes,
            source_type_fields,
            env,
            context,
            report,
        ),
        Stmt::Return(expr) => {
            if let Some(expr) = expr {
                collect_expr_source_dependency_resolution_counts(
                    expr,
                    current_mode,
                    source_type_modes,
                    source_type_fields,
                    env,
                    context,
                    report,
                );
            }
        }
        Stmt::If {
            condition,
            then_block,
            else_block,
        } => {
            collect_expr_source_dependency_resolution_counts(
                condition,
                current_mode,
                source_type_modes,
                source_type_fields,
                env,
                context,
                report,
            );
            let mut then_env = env.clone();
            collect_block_source_dependency_resolution_counts(
                then_block,
                current_mode,
                source_type_modes,
                source_type_fields,
                &mut then_env,
                context,
                report,
            );
            if let Some(else_block) = else_block {
                let mut else_env = env.clone();
                collect_block_source_dependency_resolution_counts(
                    else_block,
                    current_mode,
                    source_type_modes,
                    source_type_fields,
                    &mut else_env,
                    context,
                    report,
                );
            }
        }
        Stmt::While { condition, body } => {
            collect_expr_source_dependency_resolution_counts(
                condition,
                current_mode,
                source_type_modes,
                source_type_fields,
                env,
                context,
                report,
            );
            let mut body_env = env.clone();
            collect_block_source_dependency_resolution_counts(
                body,
                current_mode,
                source_type_modes,
                source_type_fields,
                &mut body_env,
                context,
                report,
            );
        }
        Stmt::For {
            init,
            condition,
            update,
            body,
            ..
        } => {
            let mut loop_env = env.clone();
            for init in init {
                collect_stmt_source_dependency_resolution_counts(
                    init,
                    current_mode,
                    source_type_modes,
                    source_type_fields,
                    &loop_env,
                    context,
                    report,
                );
                loop_env.learn_stmt(init);
            }
            if let Some(condition) = condition {
                collect_expr_source_dependency_resolution_counts(
                    condition,
                    current_mode,
                    source_type_modes,
                    source_type_fields,
                    &loop_env,
                    context,
                    report,
                );
            }
            for update in update {
                collect_expr_source_dependency_resolution_counts(
                    update,
                    current_mode,
                    source_type_modes,
                    source_type_fields,
                    &loop_env,
                    context,
                    report,
                );
            }
            collect_block_source_dependency_resolution_counts(
                body,
                current_mode,
                source_type_modes,
                source_type_fields,
                &mut loop_env,
                context,
                report,
            );
        }
        Stmt::EnhancedFor { iterable, body, .. } => {
            collect_expr_source_dependency_resolution_counts(
                iterable,
                current_mode,
                source_type_modes,
                source_type_fields,
                env,
                context,
                report,
            );
            let mut body_env = env.clone();
            if let Stmt::EnhancedFor {
                var_type, var_name, ..
            } = statement
            {
                body_env.insert_local(var_name.clone(), var_type.clone());
            }
            collect_block_source_dependency_resolution_counts(
                body,
                current_mode,
                source_type_modes,
                source_type_fields,
                &mut body_env,
                context,
                report,
            );
        }
        Stmt::Switch { selector, cases } => {
            collect_expr_source_dependency_resolution_counts(
                selector,
                current_mode,
                source_type_modes,
                source_type_fields,
                env,
                context,
                report,
            );
            for case in cases {
                let mut case_env = env.clone();
                for label in &case.labels {
                    if let crate::java_ir::SwitchLabel::Expr(expr) = label {
                        collect_expr_source_dependency_resolution_counts(
                            expr,
                            current_mode,
                            source_type_modes,
                            source_type_fields,
                            &case_env,
                            context,
                            report,
                        );
                    }
                }
                for statement in &case.statements {
                    collect_stmt_source_dependency_resolution_counts(
                        statement,
                        current_mode,
                        source_type_modes,
                        source_type_fields,
                        &case_env,
                        context,
                        report,
                    );
                    case_env.learn_stmt(statement);
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
                collect_stmt_source_dependency_resolution_counts(
                    resource,
                    current_mode,
                    source_type_modes,
                    source_type_fields,
                    env,
                    context,
                    report,
                );
            }
            let mut body_env = env.clone();
            collect_block_source_dependency_resolution_counts(
                body,
                current_mode,
                source_type_modes,
                source_type_fields,
                &mut body_env,
                context,
                report,
            );
            for catch in catches {
                let mut catch_env = env.clone();
                for ty in &catch.types {
                    catch_env.insert_local(catch.name.clone(), ty.clone());
                }
                collect_block_source_dependency_resolution_counts(
                    &catch.body,
                    current_mode,
                    source_type_modes,
                    source_type_fields,
                    &mut catch_env,
                    context,
                    report,
                );
            }
            if let Some(finally_block) = finally_block {
                let mut finally_env = env.clone();
                collect_block_source_dependency_resolution_counts(
                    finally_block,
                    current_mode,
                    source_type_modes,
                    source_type_fields,
                    &mut finally_env,
                    context,
                    report,
                );
            }
        }
        Stmt::LocalType(ty) => collect_type_source_dependency_resolution_counts(
            ty,
            current_mode,
            source_type_modes,
            source_type_fields,
            context,
            report,
        ),
        Stmt::ConstructorInvocation { args, resolved, .. } => {
            if let Some((resolved, source_mode)) = resolved.as_ref().and_then(|resolved| {
                source_dependency_mode_for_type(
                    &resolved.declaring_type,
                    current_mode,
                    source_type_modes,
                )
                .map(|mode| (resolved, mode))
            }) {
                report.source_dependency_resolved_constructors += 1;
                push_source_dependency_constructor_summary(
                    &mut report.top_source_dependency_constructor_types,
                    &resolved.declaring_type,
                    args.len(),
                    source_mode,
                );
                push_source_dependency_constructor_context_summary(
                    &mut report.top_source_dependency_constructor_contexts,
                    &resolved.declaring_type,
                    source_mode,
                    args.len(),
                    context,
                );
            }
            for arg in args {
                collect_expr_source_dependency_resolution_counts(
                    arg,
                    current_mode,
                    source_type_modes,
                    source_type_fields,
                    env,
                    context,
                    report,
                );
            }
        }
        Stmt::Block(block) => {
            let mut block_env = env.clone();
            collect_block_source_dependency_resolution_counts(
                block,
                current_mode,
                source_type_modes,
                source_type_fields,
                &mut block_env,
                context,
                report,
            )
        }
        Stmt::Break(_) | Stmt::Continue(_) | Stmt::Unsupported(_) => {}
    }
}

fn collect_expr_source_dependency_resolution_counts(
    expr: &Expr,
    current_mode: DependencyMode,
    source_type_modes: &HashMap<String, DependencyMode>,
    source_type_fields: &HashMap<String, HashMap<String, SourceFieldInfo>>,
    env: &TypeEnv,
    context: SourceDependencyWalkContext<'_>,
    report: &mut ResolverBenchmarkReport,
) {
    match expr {
        Expr::FieldAccess(receiver, field_name) => {
            if let Some(receiver_type) =
                source_dependency_field_receiver_type(receiver, source_type_fields, env)
                && let Some(source_mode) =
                    source_dependency_mode_for_type(&receiver_type, current_mode, source_type_modes)
                && let Some((resolved_field_name, field_info)) =
                    source_field_info_for(&receiver_type, field_name, source_type_fields)
            {
                report.source_dependency_resolved_field_accesses += 1;
                push_source_dependency_field_summary(
                    &mut report.top_source_dependency_field_accesses,
                    &field_info.declaring_type,
                    &resolved_field_name,
                    source_mode,
                );
                push_source_dependency_field_context_summary(
                    &mut report.top_source_dependency_field_contexts,
                    &field_info.declaring_type,
                    &resolved_field_name,
                    &receiver_type,
                    source_mode,
                    context,
                );
            }
            collect_expr_source_dependency_resolution_counts(
                receiver,
                current_mode,
                source_type_modes,
                source_type_fields,
                env,
                context,
                report,
            );
        }
        Expr::Unary { expr: receiver, .. } => {
            collect_expr_source_dependency_resolution_counts(
                receiver,
                current_mode,
                source_type_modes,
                source_type_fields,
                env,
                context,
                report,
            );
        }
        Expr::ArrayAccess(array, index)
        | Expr::Binary {
            left: array,
            right: index,
            ..
        } => {
            collect_expr_source_dependency_resolution_counts(
                array,
                current_mode,
                source_type_modes,
                source_type_fields,
                env,
                context,
                report,
            );
            collect_expr_source_dependency_resolution_counts(
                index,
                current_mode,
                source_type_modes,
                source_type_fields,
                env,
                context,
                report,
            );
        }
        Expr::ArrayCreation { dimensions, .. } | Expr::ArrayInitializer(dimensions) => {
            for expr in dimensions {
                collect_expr_source_dependency_resolution_counts(
                    expr,
                    current_mode,
                    source_type_modes,
                    source_type_fields,
                    env,
                    context,
                    report,
                );
            }
        }
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => {
            collect_expr_source_dependency_resolution_counts(
                condition,
                current_mode,
                source_type_modes,
                source_type_fields,
                env,
                context,
                report,
            );
            collect_expr_source_dependency_resolution_counts(
                then_expr,
                current_mode,
                source_type_modes,
                source_type_fields,
                env,
                context,
                report,
            );
            collect_expr_source_dependency_resolution_counts(
                else_expr,
                current_mode,
                source_type_modes,
                source_type_fields,
                env,
                context,
                report,
            );
        }
        Expr::Call {
            receiver,
            args,
            resolved,
            ..
        } => {
            if let Some((resolved, source_mode)) = resolved.as_ref().and_then(|resolved| {
                source_dependency_mode_for_type(
                    &resolved.declaring_type,
                    current_mode,
                    source_type_modes,
                )
                .map(|mode| (resolved, mode))
            }) {
                report.source_dependency_resolved_method_calls += 1;
                push_source_dependency_api_summary(
                    &mut report.top_source_dependency_method_calls,
                    &resolved.declaring_type,
                    &resolved.rust_name,
                    source_mode,
                );
                push_source_dependency_method_context_summary(
                    &mut report.top_source_dependency_method_contexts,
                    &resolved.declaring_type,
                    &resolved.rust_name,
                    source_mode,
                    args.len(),
                    context,
                );
            }
            if let Some(receiver) = receiver {
                collect_expr_source_dependency_resolution_counts(
                    receiver,
                    current_mode,
                    source_type_modes,
                    source_type_fields,
                    env,
                    context,
                    report,
                );
            }
            for arg in args {
                collect_expr_source_dependency_resolution_counts(
                    arg,
                    current_mode,
                    source_type_modes,
                    source_type_fields,
                    env,
                    context,
                    report,
                );
            }
        }
        Expr::New {
            args,
            body,
            resolved,
            ..
        } => {
            if let Some((resolved, source_mode)) = resolved.as_ref().and_then(|resolved| {
                source_dependency_mode_for_type(
                    &resolved.declaring_type,
                    current_mode,
                    source_type_modes,
                )
                .map(|mode| (resolved, mode))
            }) {
                report.source_dependency_resolved_constructors += 1;
                push_source_dependency_constructor_summary(
                    &mut report.top_source_dependency_constructor_types,
                    &resolved.declaring_type,
                    args.len(),
                    source_mode,
                );
                push_source_dependency_constructor_context_summary(
                    &mut report.top_source_dependency_constructor_contexts,
                    &resolved.declaring_type,
                    source_mode,
                    args.len(),
                    context,
                );
            }
            for arg in args {
                collect_expr_source_dependency_resolution_counts(
                    arg,
                    current_mode,
                    source_type_modes,
                    source_type_fields,
                    env,
                    context,
                    report,
                );
            }
            if let Some(body) = body {
                collect_anonymous_body_source_dependency_resolution_counts(
                    body,
                    current_mode,
                    source_type_modes,
                    source_type_fields,
                    env,
                    context,
                    report,
                );
            }
        }
        Expr::Cast { expr, .. } | Expr::InstanceOf { expr, .. } => {
            collect_expr_source_dependency_resolution_counts(
                expr,
                current_mode,
                source_type_modes,
                source_type_fields,
                env,
                context,
                report,
            );
        }
        Expr::Lambda { params, body } => {
            let mut lambda_env = lambda_body_env(env, params);
            match body {
                LambdaBody::Expr(expr) => collect_expr_source_dependency_resolution_counts(
                    expr,
                    current_mode,
                    source_type_modes,
                    source_type_fields,
                    &lambda_env,
                    context,
                    report,
                ),
                LambdaBody::Block(block) => collect_block_source_dependency_resolution_counts(
                    block,
                    current_mode,
                    source_type_modes,
                    source_type_fields,
                    &mut lambda_env,
                    context,
                    report,
                ),
            }
        }
        Expr::Literal(_)
        | Expr::Var(_)
        | Expr::This
        | Expr::Super
        | Expr::MethodReference { .. }
        | Expr::ClassLiteral(_)
        | Expr::Raw(_) => {}
    }
}

fn collect_anonymous_body_source_dependency_resolution_counts(
    members: &[Member],
    current_mode: DependencyMode,
    source_type_modes: &HashMap<String, DependencyMode>,
    source_type_fields: &HashMap<String, HashMap<String, SourceFieldInfo>>,
    env: &TypeEnv,
    context: SourceDependencyWalkContext<'_>,
    report: &mut ResolverBenchmarkReport,
) {
    let mut member_env = env.clone();
    for member in members {
        match member {
            Member::Field(field) => {
                let member_context = SourceDependencyWalkContext {
                    enclosing_member: &field.name,
                    ..context
                };
                if let Some(initializer) = &field.initializer {
                    collect_expr_source_dependency_resolution_counts(
                        initializer,
                        current_mode,
                        source_type_modes,
                        source_type_fields,
                        &member_env,
                        member_context,
                        report,
                    );
                }
                member_env.insert_field(field.name.clone(), field.ty.clone());
            }
            Member::Method(method) => {
                if let Some(body) = &method.body {
                    let mut body_env = member_env.clone();
                    for param in &method.params {
                        body_env.insert_local(param.name.clone(), param.ty.clone());
                    }
                    let member_context = SourceDependencyWalkContext {
                        enclosing_member: &method.name,
                        ..context
                    };
                    collect_block_source_dependency_resolution_counts(
                        body,
                        current_mode,
                        source_type_modes,
                        source_type_fields,
                        &mut body_env,
                        member_context,
                        report,
                    );
                }
            }
            Member::Constructor(constructor) => {
                if let Some(body) = &constructor.body {
                    let mut body_env = member_env.clone();
                    for param in &constructor.params {
                        body_env.insert_local(param.name.clone(), param.ty.clone());
                    }
                    let member_context = SourceDependencyWalkContext {
                        enclosing_member: &constructor.name,
                        ..context
                    };
                    collect_block_source_dependency_resolution_counts(
                        body,
                        current_mode,
                        source_type_modes,
                        source_type_fields,
                        &mut body_env,
                        member_context,
                        report,
                    );
                }
            }
            Member::Type(nested) => collect_type_source_dependency_resolution_counts(
                nested,
                current_mode,
                source_type_modes,
                source_type_fields,
                context,
                report,
            ),
            Member::Unsupported { .. } => {}
        }
    }
}

fn source_dependency_mode_for_type(
    declaring_type: &str,
    current_mode: DependencyMode,
    source_type_modes: &HashMap<String, DependencyMode>,
) -> Option<DependencyMode> {
    source_type_modes
        .get(declaring_type)
        .copied()
        .filter(|mode| *mode != current_mode)
}

fn collect_resolved_external_pack_field_access_counts(
    program: &Program,
    resolved_facts: &FactSet,
    semantic_facts: &FactSet,
    file_label: &str,
    report: &mut ResolverBenchmarkReport,
) {
    let unresolved_fields = resolved_facts
        .unresolved_target_facts
        .iter()
        .filter(|fact| fact.kind == UnresolvedTargetKind::FieldAccess)
        .filter_map(|fact| {
            let receiver_type = fact.receiver_type.as_ref().and_then(java_type_key)?;
            Some((
                receiver_type,
                fact.target_name.clone(),
                fact.enclosing_type.clone(),
                fact.enclosing_member
                    .clone()
                    .unwrap_or_else(|| "*".to_string()),
            ))
        })
        .collect::<HashSet<_>>();

    for ty in &program.types {
        collect_type_resolved_external_pack_field_access_counts(
            ty,
            semantic_facts,
            &unresolved_fields,
            file_label,
            report,
        );
    }
}

fn collect_type_resolved_external_pack_field_access_counts(
    ty: &TypeDecl,
    semantic_facts: &FactSet,
    unresolved_fields: &HashSet<(String, String, String, String)>,
    file_label: &str,
    report: &mut ResolverBenchmarkReport,
) {
    let mut env = type_source_dependency_env(ty);
    for member in &ty.members {
        match member {
            Member::Field(field) => {
                if let Some(init) = &field.initializer {
                    let context = SourceDependencyWalkContext {
                        file_label,
                        enclosing_type: &ty.name,
                        enclosing_member: &field.name,
                    };
                    collect_expr_resolved_external_pack_field_access_counts(
                        init,
                        semantic_facts,
                        unresolved_fields,
                        &env,
                        context,
                        report,
                    );
                }
            }
            Member::Method(method) => {
                let mut method_env = type_source_dependency_env(ty);
                for param in &method.params {
                    method_env.insert_local(param.name.clone(), param.ty.clone());
                }
                if let Some(body) = &method.body {
                    let context = SourceDependencyWalkContext {
                        file_label,
                        enclosing_type: &ty.name,
                        enclosing_member: &method.name,
                    };
                    collect_block_resolved_external_pack_field_access_counts(
                        body,
                        semantic_facts,
                        unresolved_fields,
                        &mut method_env,
                        context,
                        report,
                    );
                }
            }
            Member::Constructor(constructor) => {
                let mut constructor_env = type_source_dependency_env(ty);
                for param in &constructor.params {
                    constructor_env.insert_local(param.name.clone(), param.ty.clone());
                }
                let context = SourceDependencyWalkContext {
                    file_label,
                    enclosing_type: &ty.name,
                    enclosing_member: &constructor.name,
                };
                if let Some(body) = &constructor.body {
                    collect_block_resolved_external_pack_field_access_counts(
                        body,
                        semantic_facts,
                        unresolved_fields,
                        &mut constructor_env,
                        context,
                        report,
                    );
                }
            }
            Member::Type(nested) => collect_type_resolved_external_pack_field_access_counts(
                nested,
                semantic_facts,
                unresolved_fields,
                file_label,
                report,
            ),
            Member::Unsupported { .. } => {}
        }
        if let Member::Field(field) = member {
            env.insert_field(field.name.clone(), field.ty.clone());
        }
    }
}

fn collect_block_resolved_external_pack_field_access_counts(
    block: &Block,
    semantic_facts: &FactSet,
    unresolved_fields: &HashSet<(String, String, String, String)>,
    env: &mut TypeEnv,
    context: SourceDependencyWalkContext<'_>,
    report: &mut ResolverBenchmarkReport,
) {
    for statement in &block.statements {
        collect_stmt_resolved_external_pack_field_access_counts(
            statement,
            semantic_facts,
            unresolved_fields,
            env,
            context,
            report,
        );
        env.learn_stmt(statement);
    }
}

fn collect_stmt_resolved_external_pack_field_access_counts(
    statement: &Stmt,
    semantic_facts: &FactSet,
    unresolved_fields: &HashSet<(String, String, String, String)>,
    env: &mut TypeEnv,
    context: SourceDependencyWalkContext<'_>,
    report: &mut ResolverBenchmarkReport,
) {
    match statement {
        Stmt::VarDecl { initializer, .. } => {
            if let Some(init) = initializer {
                collect_expr_resolved_external_pack_field_access_counts(
                    init,
                    semantic_facts,
                    unresolved_fields,
                    env,
                    context,
                    report,
                );
            }
        }
        Stmt::Assign { target, value } => {
            collect_expr_resolved_external_pack_field_access_counts(
                target,
                semantic_facts,
                unresolved_fields,
                env,
                context,
                report,
            );
            collect_expr_resolved_external_pack_field_access_counts(
                value,
                semantic_facts,
                unresolved_fields,
                env,
                context,
                report,
            );
        }
        Stmt::Expr(expr) | Stmt::Return(Some(expr)) | Stmt::Throw(expr) => {
            collect_expr_resolved_external_pack_field_access_counts(
                expr,
                semantic_facts,
                unresolved_fields,
                env,
                context,
                report,
            );
        }
        Stmt::If {
            condition,
            then_block,
            else_block,
        } => {
            collect_expr_resolved_external_pack_field_access_counts(
                condition,
                semantic_facts,
                unresolved_fields,
                env,
                context,
                report,
            );
            collect_block_resolved_external_pack_field_access_counts(
                then_block,
                semantic_facts,
                unresolved_fields,
                &mut env.clone(),
                context,
                report,
            );
            if let Some(else_block) = else_block {
                collect_block_resolved_external_pack_field_access_counts(
                    else_block,
                    semantic_facts,
                    unresolved_fields,
                    &mut env.clone(),
                    context,
                    report,
                );
            }
        }
        Stmt::While { condition, body } => {
            collect_expr_resolved_external_pack_field_access_counts(
                condition,
                semantic_facts,
                unresolved_fields,
                env,
                context,
                report,
            );
            collect_block_resolved_external_pack_field_access_counts(
                body,
                semantic_facts,
                unresolved_fields,
                &mut env.clone(),
                context,
                report,
            );
        }
        Stmt::For {
            init,
            condition,
            update,
            body,
            ..
        } => {
            let mut loop_env = env.clone();
            for init in init {
                collect_stmt_resolved_external_pack_field_access_counts(
                    init,
                    semantic_facts,
                    unresolved_fields,
                    &mut loop_env,
                    context,
                    report,
                );
                loop_env.learn_stmt(init);
            }
            if let Some(condition) = condition {
                collect_expr_resolved_external_pack_field_access_counts(
                    condition,
                    semantic_facts,
                    unresolved_fields,
                    &loop_env,
                    context,
                    report,
                );
            }
            for update in update {
                collect_expr_resolved_external_pack_field_access_counts(
                    update,
                    semantic_facts,
                    unresolved_fields,
                    &loop_env,
                    context,
                    report,
                );
            }
            collect_block_resolved_external_pack_field_access_counts(
                body,
                semantic_facts,
                unresolved_fields,
                &mut loop_env,
                context,
                report,
            );
        }
        Stmt::EnhancedFor {
            var_type,
            var_name,
            iterable,
            body,
        } => {
            collect_expr_resolved_external_pack_field_access_counts(
                iterable,
                semantic_facts,
                unresolved_fields,
                env,
                context,
                report,
            );
            let mut body_env = env.clone();
            body_env.insert_local(var_name.clone(), var_type.clone());
            collect_block_resolved_external_pack_field_access_counts(
                body,
                semantic_facts,
                unresolved_fields,
                &mut body_env,
                context,
                report,
            );
        }
        Stmt::Switch { selector, cases } => {
            collect_expr_resolved_external_pack_field_access_counts(
                selector,
                semantic_facts,
                unresolved_fields,
                env,
                context,
                report,
            );
            for case in cases {
                let mut case_env = env.clone();
                for label in &case.labels {
                    if let SwitchLabel::Expr(expr) = label {
                        collect_expr_resolved_external_pack_field_access_counts(
                            expr,
                            semantic_facts,
                            unresolved_fields,
                            &case_env,
                            context,
                            report,
                        );
                    }
                }
                for statement in &case.statements {
                    collect_stmt_resolved_external_pack_field_access_counts(
                        statement,
                        semantic_facts,
                        unresolved_fields,
                        &mut case_env,
                        context,
                        report,
                    );
                    case_env.learn_stmt(statement);
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
                collect_stmt_resolved_external_pack_field_access_counts(
                    resource,
                    semantic_facts,
                    unresolved_fields,
                    env,
                    context,
                    report,
                );
            }
            let mut body_env = env.clone();
            collect_block_resolved_external_pack_field_access_counts(
                body,
                semantic_facts,
                unresolved_fields,
                &mut body_env,
                context,
                report,
            );
            for catch in catches {
                let mut catch_env = env.clone();
                for ty in &catch.types {
                    catch_env.insert_local(catch.name.clone(), ty.clone());
                }
                collect_block_resolved_external_pack_field_access_counts(
                    &catch.body,
                    semantic_facts,
                    unresolved_fields,
                    &mut catch_env,
                    context,
                    report,
                );
            }
            if let Some(finally_block) = finally_block {
                let mut finally_env = env.clone();
                collect_block_resolved_external_pack_field_access_counts(
                    finally_block,
                    semantic_facts,
                    unresolved_fields,
                    &mut finally_env,
                    context,
                    report,
                );
            }
        }
        Stmt::ConstructorInvocation { args, .. } => {
            for arg in args {
                collect_expr_resolved_external_pack_field_access_counts(
                    arg,
                    semantic_facts,
                    unresolved_fields,
                    env,
                    context,
                    report,
                );
            }
        }
        Stmt::LocalType(ty) => collect_type_resolved_external_pack_field_access_counts(
            ty,
            semantic_facts,
            unresolved_fields,
            context.file_label,
            report,
        ),
        Stmt::Block(block) => {
            collect_block_resolved_external_pack_field_access_counts(
                block,
                semantic_facts,
                unresolved_fields,
                &mut env.clone(),
                context,
                report,
            );
        }
        Stmt::Return(None) | Stmt::Break(_) | Stmt::Continue(_) | Stmt::Unsupported(_) => {}
    }
}

fn collect_expr_resolved_external_pack_field_access_counts(
    expr: &Expr,
    semantic_facts: &FactSet,
    unresolved_fields: &HashSet<(String, String, String, String)>,
    env: &TypeEnv,
    context: SourceDependencyWalkContext<'_>,
    report: &mut ResolverBenchmarkReport,
) {
    match expr {
        Expr::FieldAccess(receiver, field_name) => {
            if let Some(receiver_type) =
                external_pack_field_receiver_type(receiver, semantic_facts, env)
                && let Some(field) = semantic_facts.external_field_for(&receiver_type, field_name)
                && field.provenance == FactProvenance::ExternalLibraryPack
            {
                let key = (
                    receiver_type.clone(),
                    field_name.clone(),
                    context.enclosing_type.to_string(),
                    context.enclosing_member.to_string(),
                );
                if !unresolved_fields.contains(&key) {
                    let fact_receiver_type = field.field.type_name.clone();
                    let mode = external_pack_mode_for_type(semantic_facts, &fact_receiver_type);
                    report.external_api_explained_field_accesses += 1;
                    push_external_field_guess_summary(
                        &mut report.top_external_pack_field_accesses,
                        &fact_receiver_type,
                        field_name,
                        mode,
                    );
                    push_external_api_field_context_summary(
                        &mut report.top_external_pack_field_contexts,
                        field_name,
                        &fact_receiver_type,
                        mode,
                        context.enclosing_type,
                        context.enclosing_member,
                        context.file_label,
                    );
                }
            }
            collect_expr_resolved_external_pack_field_access_counts(
                receiver,
                semantic_facts,
                unresolved_fields,
                env,
                context,
                report,
            );
        }
        Expr::ArrayAccess(array, index)
        | Expr::Binary {
            left: array,
            right: index,
            ..
        } => {
            collect_expr_resolved_external_pack_field_access_counts(
                array,
                semantic_facts,
                unresolved_fields,
                env,
                context,
                report,
            );
            collect_expr_resolved_external_pack_field_access_counts(
                index,
                semantic_facts,
                unresolved_fields,
                env,
                context,
                report,
            );
        }
        Expr::ArrayCreation { dimensions, .. } => {
            for dimension in dimensions {
                collect_expr_resolved_external_pack_field_access_counts(
                    dimension,
                    semantic_facts,
                    unresolved_fields,
                    env,
                    context,
                    report,
                );
            }
        }
        Expr::ArrayInitializer(elements) => {
            for element in elements {
                collect_expr_resolved_external_pack_field_access_counts(
                    element,
                    semantic_facts,
                    unresolved_fields,
                    env,
                    context,
                    report,
                );
            }
        }
        Expr::Call { receiver, args, .. } => {
            if let Some(receiver) = receiver {
                collect_expr_resolved_external_pack_field_access_counts(
                    receiver,
                    semantic_facts,
                    unresolved_fields,
                    env,
                    context,
                    report,
                );
            }
            for arg in args {
                collect_expr_resolved_external_pack_field_access_counts(
                    arg,
                    semantic_facts,
                    unresolved_fields,
                    env,
                    context,
                    report,
                );
            }
        }
        Expr::New { args, body, .. } => {
            for arg in args {
                collect_expr_resolved_external_pack_field_access_counts(
                    arg,
                    semantic_facts,
                    unresolved_fields,
                    env,
                    context,
                    report,
                );
            }
            if let Some(body) = body {
                collect_anonymous_body_resolved_external_pack_field_access_counts(
                    body,
                    semantic_facts,
                    unresolved_fields,
                    env,
                    context,
                    report,
                );
            }
        }
        Expr::Cast { expr, .. } | Expr::Unary { expr, .. } => {
            collect_expr_resolved_external_pack_field_access_counts(
                expr,
                semantic_facts,
                unresolved_fields,
                env,
                context,
                report,
            );
        }
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => {
            collect_expr_resolved_external_pack_field_access_counts(
                condition,
                semantic_facts,
                unresolved_fields,
                env,
                context,
                report,
            );
            collect_expr_resolved_external_pack_field_access_counts(
                then_expr,
                semantic_facts,
                unresolved_fields,
                env,
                context,
                report,
            );
            collect_expr_resolved_external_pack_field_access_counts(
                else_expr,
                semantic_facts,
                unresolved_fields,
                env,
                context,
                report,
            );
        }
        Expr::Lambda { params, body } => {
            let mut lambda_env = lambda_body_env(env, params);
            match body {
                LambdaBody::Expr(expr) => collect_expr_resolved_external_pack_field_access_counts(
                    expr,
                    semantic_facts,
                    unresolved_fields,
                    &lambda_env,
                    context,
                    report,
                ),
                LambdaBody::Block(block) => {
                    collect_block_resolved_external_pack_field_access_counts(
                        block,
                        semantic_facts,
                        unresolved_fields,
                        &mut lambda_env,
                        context,
                        report,
                    )
                }
            }
        }
        _ => {}
    }
}

fn collect_anonymous_body_resolved_external_pack_field_access_counts(
    members: &[Member],
    semantic_facts: &FactSet,
    unresolved_fields: &HashSet<(String, String, String, String)>,
    env: &TypeEnv,
    context: SourceDependencyWalkContext<'_>,
    report: &mut ResolverBenchmarkReport,
) {
    let mut member_env = env.clone();
    for member in members {
        match member {
            Member::Field(field) => {
                let member_context = SourceDependencyWalkContext {
                    enclosing_member: &field.name,
                    ..context
                };
                if let Some(init) = &field.initializer {
                    collect_expr_resolved_external_pack_field_access_counts(
                        init,
                        semantic_facts,
                        unresolved_fields,
                        &member_env,
                        member_context,
                        report,
                    );
                }
                member_env.insert_field(field.name.clone(), field.ty.clone());
            }
            Member::Method(method) => {
                if let Some(body) = &method.body {
                    let mut body_env = member_env.clone();
                    for param in &method.params {
                        body_env.insert_local(param.name.clone(), param.ty.clone());
                    }
                    let member_context = SourceDependencyWalkContext {
                        enclosing_member: &method.name,
                        ..context
                    };
                    collect_block_resolved_external_pack_field_access_counts(
                        body,
                        semantic_facts,
                        unresolved_fields,
                        &mut body_env,
                        member_context,
                        report,
                    );
                }
            }
            Member::Constructor(constructor) => {
                if let Some(body) = &constructor.body {
                    let mut body_env = member_env.clone();
                    for param in &constructor.params {
                        body_env.insert_local(param.name.clone(), param.ty.clone());
                    }
                    let member_context = SourceDependencyWalkContext {
                        enclosing_member: &constructor.name,
                        ..context
                    };
                    collect_block_resolved_external_pack_field_access_counts(
                        body,
                        semantic_facts,
                        unresolved_fields,
                        &mut body_env,
                        member_context,
                        report,
                    );
                }
            }
            Member::Type(nested) => collect_type_resolved_external_pack_field_access_counts(
                nested,
                semantic_facts,
                unresolved_fields,
                context.file_label,
                report,
            ),
            Member::Unsupported { .. } => {}
        }
    }
}

fn lambda_body_env(env: &TypeEnv, params: &[crate::java_ir::LambdaParam]) -> TypeEnv {
    let mut lambda_env = env.clone();
    for param in params {
        let ty = param
            .ty
            .clone()
            .unwrap_or_else(|| JavaType::Unknown(format!("lambda_param:{}", param.name)));
        lambda_env.insert_local(param.name.clone(), ty);
    }
    lambda_env
}

fn external_pack_field_receiver_type(
    receiver: &Expr,
    semantic_facts: &FactSet,
    env: &TypeEnv,
) -> Option<String> {
    match external_pack_expr_type(receiver, semantic_facts, env) {
        Some(JavaType::Named { name, .. }) | Some(JavaType::Unknown(name)) => Some(name),
        Some(JavaType::String) => Some("String".to_string()),
        Some(JavaType::Array(_)) => Some("Array".to_string()),
        Some(JavaType::Primitive(primitive)) => Some(format!("{primitive:?}")),
        Some(JavaType::Void | JavaType::Wildcard(_)) | None => None,
    }
}

fn external_pack_expr_type(
    expr: &Expr,
    semantic_facts: &FactSet,
    env: &TypeEnv,
) -> Option<JavaType> {
    match expr {
        Expr::Var(name) => env
            .local_type(name)
            .cloned()
            .or_else(|| env.field_type(name).cloned())
            .or_else(|| Some(JavaType::Unknown(name.clone()))),
        Expr::This | Expr::Super => None,
        Expr::FieldAccess(receiver, field_name) => {
            let receiver_type = external_pack_field_receiver_type(receiver, semantic_facts, env)?;
            semantic_facts
                .external_field_for(&receiver_type, field_name)
                .and_then(|field| field.field_type.clone())
        }
        Expr::Call {
            resolved: Some(call),
            ..
        } => Some(call.return_type.clone()),
        Expr::New { ty, .. } | Expr::Cast { ty, .. } | Expr::ClassLiteral(ty) => Some(ty.clone()),
        Expr::ArrayAccess(array, _) => crate::types::sequence_element_type(
            &external_pack_expr_type(array, semantic_facts, env)?,
        ),
        Expr::Ternary { then_expr, .. } => external_pack_expr_type(then_expr, semantic_facts, env),
        Expr::Literal(literal) => Some(crate::types::literal_type(literal)),
        _ => None,
    }
}

fn type_source_dependency_env(ty: &TypeDecl) -> TypeEnv {
    let mut env = TypeEnv::new();
    for (name, java_ty) in direct_field_types(ty) {
        env.insert_field(name, java_ty);
    }
    env
}

fn direct_field_types(ty: &TypeDecl) -> HashMap<String, JavaType> {
    ty.members
        .iter()
        .filter_map(|member| match member {
            Member::Field(field) => Some((field.name.clone(), field.ty.clone())),
            _ => None,
        })
        .collect()
}

fn source_dependency_field_receiver_type(
    receiver: &Expr,
    source_type_fields: &HashMap<String, HashMap<String, SourceFieldInfo>>,
    env: &TypeEnv,
) -> Option<String> {
    match source_dependency_expr_type(receiver, source_type_fields, env) {
        Some(JavaType::Named { name, .. }) | Some(JavaType::Unknown(name)) => Some(name),
        Some(JavaType::String) => Some("String".to_string()),
        Some(JavaType::Array(_)) => Some("Array".to_string()),
        Some(JavaType::Primitive(primitive)) => Some(format!("{primitive:?}")),
        Some(JavaType::Void | JavaType::Wildcard(_)) | None => None,
    }
}

fn source_dependency_expr_type(
    expr: &Expr,
    source_type_fields: &HashMap<String, HashMap<String, SourceFieldInfo>>,
    env: &TypeEnv,
) -> Option<JavaType> {
    match expr {
        Expr::Var(name) => env
            .local_type(name)
            .cloned()
            .or_else(|| env.field_type(name).cloned())
            .or_else(|| Some(JavaType::Unknown(name.clone()))),
        Expr::This | Expr::Super => None,
        Expr::FieldAccess(receiver, field_name) => {
            let receiver_type =
                source_dependency_field_receiver_type(receiver, source_type_fields, env)?;
            source_field_info_for(&receiver_type, field_name, source_type_fields)
                .map(|(_, field)| field.ty)
        }
        Expr::Call {
            resolved: Some(call),
            ..
        } => Some(call.return_type.clone()),
        Expr::New { ty, .. } | Expr::Cast { ty, .. } | Expr::ClassLiteral(ty) => Some(ty.clone()),
        Expr::ArrayAccess(array, _) => crate::types::sequence_element_type(
            &source_dependency_expr_type(array, source_type_fields, env)?,
        ),
        Expr::Ternary { then_expr, .. } => {
            source_dependency_expr_type(then_expr, source_type_fields, env)
        }
        Expr::Literal(literal) => Some(crate::types::literal_type(literal)),
        _ => None,
    }
}

fn source_field_info_for(
    receiver_type: &str,
    field_name: &str,
    source_type_fields: &HashMap<String, HashMap<String, SourceFieldInfo>>,
) -> Option<(String, SourceFieldInfo)> {
    let fields = source_type_fields.get(receiver_type)?;
    fields
        .get(field_name)
        .cloned()
        .map(|field| (field_name.to_string(), field))
        .or_else(|| {
            let normalized = crate::parser::to_snake_case(field_name);
            fields
                .get(&normalized)
                .cloned()
                .map(|field| (normalized, field))
        })
}

fn collect_configured_provider_fact_counts(
    providers: &[DependencyFactProvider],
    report: &mut ResolverBenchmarkReport,
) {
    for provider in providers {
        report.configured_provider_facts += fact_count(&provider.facts);
        report.configured_provider_external_api_effect_facts +=
            provider.facts.external_api_effect_facts.len();
        report.configured_provider_external_api_argument_effect_facts +=
            provider.facts.external_api_argument_effect_facts.len();
        report.configured_provider_capture_facts += provider.facts.captures.owners.len();
        report.configured_provider_trivial_accessor_facts +=
            provider.facts.trivial_accessor_facts.len();
        report.configured_provider_method_effect_facts += provider.facts.method_effect_facts.len();
        report.configured_provider_recursion_facts += provider.facts.recursion_facts.len();
        report.configured_provider_control_flow_facts += provider.facts.control_flow_facts.len();
        report.configured_provider_dispatch_requirement_facts +=
            provider.facts.dispatch_requirement_facts.len();
        report.configured_provider_catch_facts += provider.facts.catch_facts.len();
        report.configured_provider_external_crate_api_mapping_facts +=
            provider.facts.external_crate_api_mapping_facts.len();
        report.configured_provider_external_crate_constructor_mapping_facts += provider
            .facts
            .external_crate_constructor_mapping_facts
            .len();
        report.configured_provider_external_constructor_facts +=
            provider.facts.external_constructor_facts.len();
        report.configured_provider_external_field_facts +=
            provider.facts.external_field_facts.len();
        report.configured_provider_hierarchy_override_facts +=
            provider.facts.hierarchy_overrides.len();
        report.configured_provider_closed_hierarchy_facts +=
            provider.facts.closed_hierarchy_facts.len();
        report.configured_provider_object_safety_facts += provider.facts.object_safety_facts.len();
        report.configured_provider_type_capability_facts +=
            provider.facts.type_capability_overrides.len();
        report.configured_provider_field_nullability_facts +=
            provider.facts.field_nullability_facts.len();
        report.configured_provider_exception_facts += provider.facts.exception_facts.len();
        report.configured_provider_external_library_pack_facts +=
            provider.facts.external_library_packs.len();
        report.configured_provider_unsupported_external_api_facts +=
            provider.facts.unsupported_external_api_facts.len();
        report.configured_provider_unsupported_external_constructor_facts +=
            provider.facts.unsupported_external_constructor_facts.len();
        report.configured_provider_unsupported_external_field_facts +=
            provider.facts.unsupported_external_field_facts.len();
    }
}

fn collect_source_summary_provider_fact_counts(
    providers: &[DependencyFactProvider],
    report: &mut ResolverBenchmarkReport,
) {
    for provider in providers
        .iter()
        .filter(|provider| provider.mode == DependencyMode::SourceSummary)
    {
        report.source_summary_provider_facts += fact_count(&provider.facts);
        report.source_summary_provider_external_api_effect_facts +=
            provider.facts.external_api_effect_facts.len();
        report.source_summary_provider_external_api_argument_effect_facts +=
            provider.facts.external_api_argument_effect_facts.len();
        report.source_summary_provider_capture_facts += provider.facts.captures.owners.len();
        report.source_summary_provider_trivial_accessor_facts +=
            provider.facts.trivial_accessor_facts.len();
        report.source_summary_provider_method_effect_facts +=
            provider.facts.method_effect_facts.len();
        report.source_summary_provider_recursion_facts += provider.facts.recursion_facts.len();
        report.source_summary_provider_control_flow_facts +=
            provider.facts.control_flow_facts.len();
        report.source_summary_provider_dispatch_requirement_facts +=
            provider.facts.dispatch_requirement_facts.len();
        report.source_summary_provider_catch_facts += provider.facts.catch_facts.len();
        report.source_summary_provider_external_crate_api_mapping_facts +=
            provider.facts.external_crate_api_mapping_facts.len();
        report.source_summary_provider_external_crate_constructor_mapping_facts += provider
            .facts
            .external_crate_constructor_mapping_facts
            .len();
        report.source_summary_provider_external_constructor_facts +=
            provider.facts.external_constructor_facts.len();
        report.source_summary_provider_external_field_facts +=
            provider.facts.external_field_facts.len();
        report.source_summary_provider_hierarchy_override_facts +=
            provider.facts.hierarchy_overrides.len();
        report.source_summary_provider_closed_hierarchy_facts +=
            provider.facts.closed_hierarchy_facts.len();
        report.source_summary_provider_object_safety_facts +=
            provider.facts.object_safety_facts.len();
        report.source_summary_provider_type_capability_facts +=
            provider.facts.type_capability_overrides.len();
        report.source_summary_provider_field_nullability_facts +=
            provider.facts.field_nullability_facts.len();
        report.source_summary_provider_exception_facts += provider.facts.exception_facts.len();
        report.source_summary_provider_external_library_pack_facts +=
            provider.facts.external_library_packs.len();
        report.source_summary_provider_unsupported_external_api_facts +=
            provider.facts.unsupported_external_api_facts.len();
        report.source_summary_provider_unsupported_external_constructor_facts +=
            provider.facts.unsupported_external_constructor_facts.len();
        report.source_summary_provider_unsupported_external_field_facts +=
            provider.facts.unsupported_external_field_facts.len();
    }
}

fn external_pack_summary_mode_label(mode: ExternalReportMode) -> &'static str {
    match mode {
        ExternalReportMode::Known(ExternalLibraryMode::ExternalFactModel) => "ExternalFactModel",
        ExternalReportMode::Known(ExternalLibraryMode::RustCrateMapping) => "RustCrateMapping",
        ExternalReportMode::Known(ExternalLibraryMode::FfiBoundary) => "FfiBoundary",
        ExternalReportMode::Known(ExternalLibraryMode::ConservativeBoundary) => {
            "ConservativeBoundary"
        }
        ExternalReportMode::Unknown => "UnknownExternalMode",
    }
}

fn dependency_mode_for_external_library_mode(mode: ExternalLibraryMode) -> DependencyMode {
    match mode {
        ExternalLibraryMode::ExternalFactModel => DependencyMode::ExternalFactModel,
        ExternalLibraryMode::RustCrateMapping => DependencyMode::RustCrateMapping,
        ExternalLibraryMode::FfiBoundary => DependencyMode::FfiBoundary,
        ExternalLibraryMode::ConservativeBoundary => DependencyMode::ConservativeBoundary,
    }
}

fn dependency_mode_label(mode: Option<DependencyMode>) -> &'static str {
    match mode {
        Some(DependencyMode::PrimarySource) => "PrimarySource",
        Some(DependencyMode::CoTranslateSource) => "CoTranslateSource",
        Some(DependencyMode::SourceSummary) => "SourceSummary",
        Some(DependencyMode::ExternalFactModel) => "ExternalFactModel",
        Some(DependencyMode::RustCrateMapping) => "RustCrateMapping",
        Some(DependencyMode::FfiBoundary) => "FfiBoundary",
        Some(DependencyMode::ConservativeBoundary) => "ConservativeBoundary",
        None => "*",
    }
}

pub fn render_resolver_benchmark_report(report: &ResolverBenchmarkReport) -> String {
    let mut lines = vec![
        format!("root\t{}", report.root.display()),
        format!("files_seen\t{}", report.files_seen),
        format!(
            "files_parsed_without_errors\t{}",
            report.files_parsed_without_errors
        ),
        format!(
            "files_resolved_without_errors\t{}",
            report.files_resolved_without_errors
        ),
        format!("parser_diagnostics\t{}", report.parser_diagnostics),
        format!("resolver_diagnostics\t{}", report.resolver_diagnostics),
        format!("semantic_diagnostics\t{}", report.semantic_diagnostics),
        format!(
            "external_pack_manifest_coverage_diagnostics\t{}",
            report.external_pack_manifest_coverage_diagnostics
        ),
        format!(
            "configured_provider_facts\t{}",
            report.configured_provider_facts
        ),
        format!(
            "configured_provider_external_api_effect_facts\t{}",
            report.configured_provider_external_api_effect_facts
        ),
        format!(
            "configured_provider_external_api_argument_effect_facts\t{}",
            report.configured_provider_external_api_argument_effect_facts
        ),
        format!(
            "configured_provider_capture_facts\t{}",
            report.configured_provider_capture_facts
        ),
        format!(
            "configured_provider_trivial_accessor_facts\t{}",
            report.configured_provider_trivial_accessor_facts
        ),
        format!(
            "configured_provider_method_effect_facts\t{}",
            report.configured_provider_method_effect_facts
        ),
        format!(
            "configured_provider_recursion_facts\t{}",
            report.configured_provider_recursion_facts
        ),
        format!(
            "configured_provider_control_flow_facts\t{}",
            report.configured_provider_control_flow_facts
        ),
        format!(
            "configured_provider_dispatch_requirement_facts\t{}",
            report.configured_provider_dispatch_requirement_facts
        ),
        format!(
            "configured_provider_catch_facts\t{}",
            report.configured_provider_catch_facts
        ),
        format!(
            "configured_provider_external_crate_api_mapping_facts\t{}",
            report.configured_provider_external_crate_api_mapping_facts
        ),
        format!(
            "configured_provider_external_crate_constructor_mapping_facts\t{}",
            report.configured_provider_external_crate_constructor_mapping_facts
        ),
        format!(
            "configured_provider_external_constructor_facts\t{}",
            report.configured_provider_external_constructor_facts
        ),
        format!(
            "configured_provider_external_field_facts\t{}",
            report.configured_provider_external_field_facts
        ),
        format!(
            "configured_provider_hierarchy_override_facts\t{}",
            report.configured_provider_hierarchy_override_facts
        ),
        format!(
            "configured_provider_closed_hierarchy_facts\t{}",
            report.configured_provider_closed_hierarchy_facts
        ),
        format!(
            "configured_provider_object_safety_facts\t{}",
            report.configured_provider_object_safety_facts
        ),
        format!(
            "configured_provider_type_capability_facts\t{}",
            report.configured_provider_type_capability_facts
        ),
        format!(
            "configured_provider_field_nullability_facts\t{}",
            report.configured_provider_field_nullability_facts
        ),
        format!(
            "configured_provider_exception_facts\t{}",
            report.configured_provider_exception_facts
        ),
        format!(
            "configured_provider_external_library_pack_facts\t{}",
            report.configured_provider_external_library_pack_facts
        ),
        format!(
            "configured_provider_unsupported_external_api_facts\t{}",
            report.configured_provider_unsupported_external_api_facts
        ),
        format!(
            "configured_provider_unsupported_external_constructor_facts\t{}",
            report.configured_provider_unsupported_external_constructor_facts
        ),
        format!(
            "configured_provider_unsupported_external_field_facts\t{}",
            report.configured_provider_unsupported_external_field_facts
        ),
        format!("source_summary_facts\t{}", report.source_summary_facts),
        format!(
            "source_summary_root_type_capability_facts\t{}",
            report.source_summary_root_type_capability_facts
        ),
        format!(
            "source_summary_root_object_safety_facts\t{}",
            report.source_summary_root_object_safety_facts
        ),
        format!(
            "source_summary_root_closed_hierarchy_facts\t{}",
            report.source_summary_root_closed_hierarchy_facts
        ),
        format!(
            "source_summary_provider_facts\t{}",
            report.source_summary_provider_facts
        ),
        format!(
            "source_summary_provider_external_api_effect_facts\t{}",
            report.source_summary_provider_external_api_effect_facts
        ),
        format!(
            "source_summary_provider_external_api_argument_effect_facts\t{}",
            report.source_summary_provider_external_api_argument_effect_facts
        ),
        format!(
            "source_summary_provider_capture_facts\t{}",
            report.source_summary_provider_capture_facts
        ),
        format!(
            "source_summary_provider_trivial_accessor_facts\t{}",
            report.source_summary_provider_trivial_accessor_facts
        ),
        format!(
            "source_summary_provider_method_effect_facts\t{}",
            report.source_summary_provider_method_effect_facts
        ),
        format!(
            "source_summary_provider_recursion_facts\t{}",
            report.source_summary_provider_recursion_facts
        ),
        format!(
            "source_summary_provider_control_flow_facts\t{}",
            report.source_summary_provider_control_flow_facts
        ),
        format!(
            "source_summary_provider_dispatch_requirement_facts\t{}",
            report.source_summary_provider_dispatch_requirement_facts
        ),
        format!(
            "source_summary_provider_catch_facts\t{}",
            report.source_summary_provider_catch_facts
        ),
        format!(
            "source_summary_provider_external_crate_api_mapping_facts\t{}",
            report.source_summary_provider_external_crate_api_mapping_facts
        ),
        format!(
            "source_summary_provider_external_crate_constructor_mapping_facts\t{}",
            report.source_summary_provider_external_crate_constructor_mapping_facts
        ),
        format!(
            "source_summary_provider_external_constructor_facts\t{}",
            report.source_summary_provider_external_constructor_facts
        ),
        format!(
            "source_summary_provider_external_field_facts\t{}",
            report.source_summary_provider_external_field_facts
        ),
        format!(
            "source_summary_provider_hierarchy_override_facts\t{}",
            report.source_summary_provider_hierarchy_override_facts
        ),
        format!(
            "source_summary_provider_closed_hierarchy_facts\t{}",
            report.source_summary_provider_closed_hierarchy_facts
        ),
        format!(
            "source_summary_provider_object_safety_facts\t{}",
            report.source_summary_provider_object_safety_facts
        ),
        format!(
            "source_summary_provider_type_capability_facts\t{}",
            report.source_summary_provider_type_capability_facts
        ),
        format!(
            "source_summary_provider_field_nullability_facts\t{}",
            report.source_summary_provider_field_nullability_facts
        ),
        format!(
            "source_summary_provider_exception_facts\t{}",
            report.source_summary_provider_exception_facts
        ),
        format!(
            "source_summary_provider_external_library_pack_facts\t{}",
            report.source_summary_provider_external_library_pack_facts
        ),
        format!(
            "source_summary_provider_unsupported_external_api_facts\t{}",
            report.source_summary_provider_unsupported_external_api_facts
        ),
        format!(
            "source_summary_provider_unsupported_external_constructor_facts\t{}",
            report.source_summary_provider_unsupported_external_constructor_facts
        ),
        format!(
            "source_summary_provider_unsupported_external_field_facts\t{}",
            report.source_summary_provider_unsupported_external_field_facts
        ),
        format!(
            "source_dependency_resolved_method_calls\t{}",
            report.source_dependency_resolved_method_calls
        ),
        format!(
            "source_dependency_resolved_constructors\t{}",
            report.source_dependency_resolved_constructors
        ),
        format!(
            "source_dependency_resolved_field_accesses\t{}",
            report.source_dependency_resolved_field_accesses
        ),
        format!(
            "unresolved_method_calls\t{}",
            report.unresolved_method_calls
        ),
        format!(
            "external_api_explained_method_calls\t{}",
            report.external_api_explained_method_calls
        ),
        format!(
            "external_api_unsupported_method_calls\t{}",
            report.external_api_unsupported_method_calls
        ),
        format!(
            "external_api_missing_method_calls\t{}",
            report.external_api_missing_method_calls
        ),
        format!(
            "unexplained_method_calls\t{}",
            report.unexplained_method_calls
        ),
        format!(
            "unresolved_constructors\t{}",
            report.unresolved_constructors
        ),
        format!(
            "external_api_explained_constructors\t{}",
            report.external_api_explained_constructors
        ),
        format!(
            "external_api_unsupported_constructor_calls\t{}",
            report.external_api_unsupported_constructor_calls
        ),
        format!(
            "external_api_missing_constructor_calls\t{}",
            report.external_api_missing_constructor_calls
        ),
        format!(
            "anonymous_class_constructors\t{}",
            report.anonymous_class_constructors
        ),
        format!(
            "unexplained_constructors\t{}",
            report.unexplained_constructors
        ),
        format!(
            "unexplained_source_constructors\t{}",
            report.unexplained_source_constructors
        ),
        format!(
            "unexplained_non_source_constructors\t{}",
            report.unexplained_non_source_constructors
        ),
        format!(
            "unresolved_constructor_delegations\t{}",
            report.unresolved_constructor_delegations
        ),
        format!(
            "unresolved_field_accesses\t{}",
            report.unresolved_field_accesses
        ),
        format!(
            "external_api_explained_field_accesses\t{}",
            report.external_api_explained_field_accesses
        ),
        format!(
            "external_api_unsupported_field_accesses\t{}",
            report.external_api_unsupported_field_accesses
        ),
        format!(
            "external_api_missing_field_accesses\t{}",
            report.external_api_missing_field_accesses
        ),
        format!(
            "unexplained_field_accesses\t{}",
            report.unexplained_field_accesses
        ),
        format!(
            "unexplained_source_field_accesses\t{}",
            report.unexplained_source_field_accesses
        ),
        format!(
            "unexplained_non_source_field_accesses\t{}",
            report.unexplained_non_source_field_accesses
        ),
        format!("external_api_guesses\t{}", report.external_api_guesses),
        format!(
            "external_pack_api_facts\t{}",
            report.external_pack_api_facts
        ),
        format!(
            "external_pack_constructor_facts\t{}",
            report.external_pack_constructor_facts
        ),
        format!(
            "external_fact_model_pack_api_facts\t{}",
            report.external_fact_model_pack_api_facts
        ),
        format!(
            "external_fact_model_pack_constructor_facts\t{}",
            report.external_fact_model_pack_constructor_facts
        ),
        format!(
            "rust_crate_mapping_pack_api_facts\t{}",
            report.rust_crate_mapping_pack_api_facts
        ),
        format!(
            "rust_crate_mapping_pack_constructor_facts\t{}",
            report.rust_crate_mapping_pack_constructor_facts
        ),
        format!(
            "ffi_boundary_pack_api_facts\t{}",
            report.ffi_boundary_pack_api_facts
        ),
        format!(
            "ffi_boundary_pack_constructor_facts\t{}",
            report.ffi_boundary_pack_constructor_facts
        ),
        format!(
            "conservative_boundary_pack_api_facts\t{}",
            report.conservative_boundary_pack_api_facts
        ),
        format!(
            "conservative_boundary_pack_constructor_facts\t{}",
            report.conservative_boundary_pack_constructor_facts
        ),
        format!(
            "external_pack_explained_method_calls\t{}",
            report.external_pack_explained_method_calls
        ),
        format!(
            "external_pack_explained_constructors\t{}",
            report.external_pack_explained_constructors
        ),
        format!(
            "external_fact_model_explained_method_calls\t{}",
            report.external_fact_model_explained_method_calls
        ),
        format!(
            "external_fact_model_explained_constructors\t{}",
            report.external_fact_model_explained_constructors
        ),
        format!(
            "rust_crate_mapping_explained_method_calls\t{}",
            report.rust_crate_mapping_explained_method_calls
        ),
        format!(
            "rust_crate_mapping_explained_constructors\t{}",
            report.rust_crate_mapping_explained_constructors
        ),
        format!(
            "ffi_boundary_explained_method_calls\t{}",
            report.ffi_boundary_explained_method_calls
        ),
        format!(
            "ffi_boundary_explained_constructors\t{}",
            report.ffi_boundary_explained_constructors
        ),
        format!(
            "conservative_boundary_explained_method_calls\t{}",
            report.conservative_boundary_explained_method_calls
        ),
        format!(
            "conservative_boundary_explained_constructors\t{}",
            report.conservative_boundary_explained_constructors
        ),
        format!(
            "external_constructor_guesses\t{}",
            report.external_constructor_guesses
        ),
        format!(
            "project_constructor_guesses\t{}",
            report.project_constructor_guesses
        ),
        format!(
            "source_constructor_guesses\t{}",
            report.source_constructor_guesses
        ),
        format!("external_field_guesses\t{}", report.external_field_guesses),
        format!("project_field_guesses\t{}", report.project_field_guesses),
        format!("source_field_guesses\t{}", report.source_field_guesses),
        format!(
            "external_api_return_nullability_facts\t{}",
            report.external_api_return_nullability_facts
        ),
        format!(
            "external_api_fluent_builder_facts\t{}",
            report.external_api_fluent_builder_facts
        ),
        format!(
            "external_api_argument_effect_facts\t{}",
            report.external_api_argument_effect_facts
        ),
        format!("total_facts\t{}", report.total_facts),
        format!("elapsed_ms\t{}", report.elapsed_ms),
        format!(
            "object_concrete_struct_representations\t{}",
            report.object_concrete_struct_representations
        ),
        format!(
            "object_dyn_trait_representations\t{}",
            report.object_dyn_trait_representations
        ),
        format!(
            "object_closed_enum_representations\t{}",
            report.object_closed_enum_representations
        ),
        format!(
            "object_erased_object_representations\t{}",
            report.object_erased_object_representations
        ),
        format!(
            "object_representation_diagnostics\t{}",
            report.object_representation_diagnostics
        ),
    ];
    if !report.source_roots.is_empty() {
        lines.push("source_roots".to_string());
        for source_root in &report.source_roots {
            lines.push(format!(
                "{:?}\t{}\t{}",
                source_root.mode,
                source_root.files_seen,
                source_root.path.display()
            ));
        }
    }
    if !report.fact_providers.is_empty() {
        lines.push("fact_providers".to_string());
        lines.push(
            "name\tmode\tfacts\texternal_api_effect_facts\texternal_api_argument_effect_facts\texternal_crate_api_mapping_facts\texternal_crate_constructor_mapping_facts\texternal_constructor_facts\texternal_field_facts\thierarchy_override_facts\tclosed_hierarchy_facts\tobject_safety_facts\ttype_capability_facts\tfield_nullability_facts\texception_facts\texternal_library_pack_facts\tunsupported_external_api_facts\tunsupported_external_constructor_facts\tunsupported_external_field_facts".to_string(),
        );
        for provider in &report.fact_providers {
            lines.push(format!(
                "{}\t{:?}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
                provider.name,
                provider.mode,
                provider.facts,
                provider.external_api_effect_facts,
                provider.external_api_argument_effect_facts,
                provider.external_crate_api_mapping_facts,
                provider.external_crate_constructor_mapping_facts,
                provider.external_constructor_facts,
                provider.external_field_facts,
                provider.hierarchy_override_facts,
                provider.closed_hierarchy_facts,
                provider.object_safety_facts,
                provider.type_capability_facts,
                provider.field_nullability_facts,
                provider.exception_facts,
                provider.external_library_pack_facts,
                provider.unsupported_external_api_facts,
                provider.unsupported_external_constructor_facts,
                provider.unsupported_external_field_facts
            ));
        }
    }
    if !report
        .top_external_pack_manifest_coverage_diagnostics
        .is_empty()
    {
        lines.push("top_external_pack_manifest_coverage_diagnostics".to_string());
        for diagnostic in &report.top_external_pack_manifest_coverage_diagnostics {
            lines.push(format!(
                "{}\t{}\t{}\t{}\t{}",
                diagnostic.code,
                dependency_mode_label(diagnostic.mode),
                diagnostic.count,
                diagnostic.message,
                diagnostic.sample_files.join(",")
            ));
        }
    }
    if !report.top_object_representation_diagnostics.is_empty() {
        lines.push("top_object_representation_diagnostics".to_string());
        for diagnostic in &report.top_object_representation_diagnostics {
            lines.push(format!(
                "{}\t{}\t{}\t{}\t{}",
                diagnostic.code,
                dependency_mode_label(diagnostic.mode),
                diagnostic.count,
                diagnostic.message,
                diagnostic.sample_files.join(",")
            ));
        }
    }
    if !report.top_unresolved_targets.is_empty() {
        lines.push("top_unresolved_targets".to_string());
        for target in &report.top_unresolved_targets {
            lines.push(format!(
                "{:?}\t{}\t{}",
                target.kind, target.target_name, target.count
            ));
        }
    }
    if !report.top_unexplained_method_targets.is_empty() {
        lines.push("top_unexplained_method_targets".to_string());
        for target in &report.top_unexplained_method_targets {
            lines.push(format!(
                "{:?}\t{}\t{}",
                target.kind, target.target_name, target.count
            ));
        }
    }
    if !report.top_unexplained_method_receivers.is_empty() {
        lines.push("top_unexplained_method_receivers".to_string());
        for target in &report.top_unexplained_method_receivers {
            lines.push(format!(
                "{}\t{}\t{}",
                target.target_name, target.receiver_type, target.count
            ));
        }
    }
    if !report.top_external_api_guess_methods.is_empty() {
        lines.push("top_external_api_guess_methods".to_string());
        for target in &report.top_external_api_guess_methods {
            lines.push(format!(
                "{}\t{}\t{}",
                target.receiver_type, target.method_name, target.count
            ));
        }
    }
    if !report.top_external_pack_api_methods.is_empty() {
        lines.push("top_external_pack_api_methods".to_string());
        for target in &report.top_external_pack_api_methods {
            lines.push(format!(
                "{}\t{}\t{}\t{}",
                target.receiver_type,
                target.method_name,
                external_pack_summary_mode_label(target.mode),
                target.count
            ));
        }
    }
    if !report.top_external_pack_api_method_contexts.is_empty() {
        lines.push("top_external_pack_api_method_contexts".to_string());
        lines.push(
            "receiver_type\tmethod\tmode\targ_count\tenclosing_type\tenclosing_member\tcount\tsample_files"
                .to_string(),
        );
        for context in &report.top_external_pack_api_method_contexts {
            lines.push(format_external_api_method_context_summary(context));
        }
    }
    if !report.top_external_api_unsupported_methods.is_empty() {
        lines.push("top_external_api_unsupported_methods".to_string());
        for target in &report.top_external_api_unsupported_methods {
            lines.push(format!(
                "{}\t{}\t{}\t{}",
                target.receiver_type,
                target.method_name,
                external_pack_summary_mode_label(target.mode),
                target.count
            ));
        }
    }
    if !report.top_external_api_missing_methods.is_empty() {
        lines.push("top_external_api_missing_methods".to_string());
        for target in &report.top_external_api_missing_methods {
            lines.push(format!(
                "{}\t{}\t{}\t{}",
                target.receiver_type,
                target.method_name,
                external_pack_summary_mode_label(target.mode),
                target.count
            ));
        }
    }
    if !report
        .top_external_api_unsupported_method_contexts
        .is_empty()
    {
        lines.push("top_external_api_unsupported_method_contexts".to_string());
        lines.push(
            "receiver_type\tmethod\tmode\targ_count\tenclosing_type\tenclosing_member\tcount\tsample_files"
                .to_string(),
        );
        for context in &report.top_external_api_unsupported_method_contexts {
            lines.push(format_external_api_method_context_summary(context));
        }
    }
    if !report.top_external_api_missing_method_contexts.is_empty() {
        lines.push("top_external_api_missing_method_contexts".to_string());
        lines.push(
            "receiver_type\tmethod\tmode\targ_count\tenclosing_type\tenclosing_member\tcount\tsample_files"
                .to_string(),
        );
        for context in &report.top_external_api_missing_method_contexts {
            lines.push(format_external_api_method_context_summary(context));
        }
    }
    if !report.top_external_constructor_guess_types.is_empty() {
        lines.push("top_external_constructor_guess_types".to_string());
        for target in &report.top_external_constructor_guess_types {
            lines.push(format!(
                "{}\t{}\t{}",
                target.type_name, target.arity, target.count
            ));
        }
    }
    if !report.top_external_pack_constructor_types.is_empty() {
        lines.push("top_external_pack_constructor_types".to_string());
        for target in &report.top_external_pack_constructor_types {
            lines.push(format!(
                "{}\t{}\t{}\t{}",
                target.type_name,
                target.arity,
                external_pack_summary_mode_label(target.mode),
                target.count
            ));
        }
    }
    if !report.top_external_pack_constructor_contexts.is_empty() {
        lines.push("top_external_pack_constructor_contexts".to_string());
        lines.push(
            "target\treceiver\tmode\targ_types\tenclosing_type\tenclosing_member\tcount\tsample_files"
                .to_string(),
        );
        for context in &report.top_external_pack_constructor_contexts {
            lines.push(format_external_api_constructor_context_summary(context));
        }
    }
    if !report.top_external_api_unsupported_constructors.is_empty() {
        lines.push("top_external_api_unsupported_constructors".to_string());
        for target in &report.top_external_api_unsupported_constructors {
            lines.push(format!(
                "{}\t{}\t{}\t{}",
                target.type_name,
                target.arity,
                external_pack_summary_mode_label(target.mode),
                target.count
            ));
        }
    }
    if !report.top_external_api_missing_constructors.is_empty() {
        lines.push("top_external_api_missing_constructors".to_string());
        for target in &report.top_external_api_missing_constructors {
            lines.push(format!(
                "{}\t{}\t{}\t{}",
                target.type_name,
                target.arity,
                external_pack_summary_mode_label(target.mode),
                target.count
            ));
        }
    }
    if !report
        .top_external_api_unsupported_constructor_contexts
        .is_empty()
    {
        lines.push("top_external_api_unsupported_constructor_contexts".to_string());
        lines.push(
            "target\treceiver\tmode\targ_types\tenclosing_type\tenclosing_member\tcount\tsample_files"
                .to_string(),
        );
        for context in &report.top_external_api_unsupported_constructor_contexts {
            lines.push(format_external_api_constructor_context_summary(context));
        }
    }
    if !report
        .top_external_api_missing_constructor_contexts
        .is_empty()
    {
        lines.push("top_external_api_missing_constructor_contexts".to_string());
        lines.push(
            "target\treceiver\tmode\targ_types\tenclosing_type\tenclosing_member\tcount\tsample_files"
                .to_string(),
        );
        for context in &report.top_external_api_missing_constructor_contexts {
            lines.push(format_external_api_constructor_context_summary(context));
        }
    }
    if !report.top_project_constructor_guess_types.is_empty() {
        lines.push("top_project_constructor_guess_types".to_string());
        for target in &report.top_project_constructor_guess_types {
            lines.push(format!(
                "{}\t{}\t{}",
                target.type_name, target.arity, target.count
            ));
        }
    }
    if !report.top_source_constructor_guess_types.is_empty() {
        lines.push("top_source_constructor_guess_types".to_string());
        for target in &report.top_source_constructor_guess_types {
            lines.push(format!(
                "{}\t{}\t{}",
                target.type_name, target.arity, target.count
            ));
        }
    }
    if !report.top_source_dependency_method_calls.is_empty() {
        lines.push("top_source_dependency_method_calls".to_string());
        for target in &report.top_source_dependency_method_calls {
            lines.push(format!(
                "{}\t{}\t{}\t{}",
                target.receiver_type,
                target.method_name,
                dependency_mode_label(Some(target.source_mode)),
                target.count
            ));
        }
    }
    if !report.top_source_dependency_constructor_types.is_empty() {
        lines.push("top_source_dependency_constructor_types".to_string());
        for target in &report.top_source_dependency_constructor_types {
            lines.push(format!(
                "{}\t{}\t{}\t{}",
                target.type_name,
                target.arity,
                dependency_mode_label(Some(target.source_mode)),
                target.count
            ));
        }
    }
    if !report.top_source_dependency_field_accesses.is_empty() {
        lines.push("top_source_dependency_field_accesses".to_string());
        for target in &report.top_source_dependency_field_accesses {
            lines.push(format!(
                "{}\t{}\t{}\t{}",
                target.type_name,
                target.field_name,
                dependency_mode_label(Some(target.source_mode)),
                target.count
            ));
        }
    }
    if !report.top_source_dependency_method_contexts.is_empty() {
        lines.push("top_source_dependency_method_contexts".to_string());
        lines.push(
            "declaring_type\tmethod\tmode\targ_count\tenclosing_type\tenclosing_member\tcount\tsample_files"
                .to_string(),
        );
        for context in &report.top_source_dependency_method_contexts {
            lines.push(format_source_dependency_method_context_summary(context));
        }
    }
    if !report.top_source_dependency_constructor_contexts.is_empty() {
        lines.push("top_source_dependency_constructor_contexts".to_string());
        lines.push(
            "declaring_type\tmode\targ_count\tenclosing_type\tenclosing_member\tcount\tsample_files"
                .to_string(),
        );
        for context in &report.top_source_dependency_constructor_contexts {
            lines.push(format_source_dependency_constructor_context_summary(
                context,
            ));
        }
    }
    if !report.top_source_dependency_field_contexts.is_empty() {
        lines.push("top_source_dependency_field_contexts".to_string());
        lines.push(
            "declaring_type\tfield\treceiver_type\tmode\tenclosing_type\tenclosing_member\tcount\tsample_files"
                .to_string(),
        );
        for context in &report.top_source_dependency_field_contexts {
            lines.push(format_source_dependency_field_context_summary(context));
        }
    }
    if !report.top_external_constructor_guess_contexts.is_empty() {
        lines.push("top_external_constructor_guess_contexts".to_string());
        lines.push(
            "target\treceiver\targ_types\tenclosing_type\tenclosing_member\tcount\tsample_files"
                .to_string(),
        );
        for context in &report.top_external_constructor_guess_contexts {
            lines.push(format_constructor_context_summary(context));
        }
    }
    if !report.top_project_constructor_guess_contexts.is_empty() {
        lines.push("top_project_constructor_guess_contexts".to_string());
        lines.push(
            "target\treceiver\targ_types\tenclosing_type\tenclosing_member\tcount\tsample_files"
                .to_string(),
        );
        for context in &report.top_project_constructor_guess_contexts {
            lines.push(format_constructor_context_summary(context));
        }
    }
    if !report.top_source_constructor_guess_contexts.is_empty() {
        lines.push("top_source_constructor_guess_contexts".to_string());
        lines.push(
            "target\treceiver\targ_types\tenclosing_type\tenclosing_member\tcount\tsample_files"
                .to_string(),
        );
        for context in &report.top_source_constructor_guess_contexts {
            lines.push(format_constructor_context_summary(context));
        }
    }
    if !report.top_anonymous_constructor_targets.is_empty() {
        lines.push("top_anonymous_constructor_targets".to_string());
        for target in &report.top_anonymous_constructor_targets {
            lines.push(format!(
                "{}\t{}\t{}",
                target.type_name, target.arity, target.count
            ));
        }
    }
    if !report.top_external_field_guess_targets.is_empty() {
        lines.push("top_external_field_guess_targets".to_string());
        for target in &report.top_external_field_guess_targets {
            lines.push(format!(
                "{}\t{}\t{}",
                target.type_name, target.field_name, target.count
            ));
        }
    }
    if !report.top_project_field_guess_targets.is_empty() {
        lines.push("top_project_field_guess_targets".to_string());
        for target in &report.top_project_field_guess_targets {
            lines.push(format!(
                "{}\t{}\t{}",
                target.type_name, target.field_name, target.count
            ));
        }
    }
    if !report.top_source_field_guess_targets.is_empty() {
        lines.push("top_source_field_guess_targets".to_string());
        for target in &report.top_source_field_guess_targets {
            lines.push(format!(
                "{}\t{}\t{}",
                target.type_name, target.field_name, target.count
            ));
        }
    }
    if !report.top_external_api_unsupported_fields.is_empty() {
        lines.push("top_external_api_unsupported_fields".to_string());
        for target in &report.top_external_api_unsupported_fields {
            lines.push(format!(
                "{}\t{}\t{}\t{}",
                target.type_name,
                target.field_name,
                external_pack_summary_mode_label(target.mode),
                target.count
            ));
        }
    }
    if !report.top_external_api_missing_fields.is_empty() {
        lines.push("top_external_api_missing_fields".to_string());
        for target in &report.top_external_api_missing_fields {
            lines.push(format!(
                "{}\t{}\t{}\t{}",
                target.type_name,
                target.field_name,
                external_pack_summary_mode_label(target.mode),
                target.count
            ));
        }
    }
    if !report.top_external_pack_field_accesses.is_empty() {
        lines.push("top_external_pack_field_accesses".to_string());
        for target in &report.top_external_pack_field_accesses {
            lines.push(format!(
                "{}\t{}\t{}\t{}",
                target.type_name,
                target.field_name,
                external_pack_summary_mode_label(target.mode),
                target.count
            ));
        }
    }
    if !report.top_external_pack_field_contexts.is_empty() {
        lines.push("top_external_pack_field_contexts".to_string());
        lines.push(
            "field\treceiver\tmode\tenclosing_type\tenclosing_member\tcount\tsample_files"
                .to_string(),
        );
        for target in &report.top_external_pack_field_contexts {
            lines.push(format_external_api_field_context_summary(target));
        }
    }
    if !report
        .top_external_api_unsupported_field_contexts
        .is_empty()
    {
        lines.push("top_external_api_unsupported_field_contexts".to_string());
        lines.push(
            "field\treceiver\tmode\tenclosing_type\tenclosing_member\tcount\tsample_files"
                .to_string(),
        );
        for target in &report.top_external_api_unsupported_field_contexts {
            lines.push(format_external_api_field_context_summary(target));
        }
    }
    if !report.top_external_api_missing_field_contexts.is_empty() {
        lines.push("top_external_api_missing_field_contexts".to_string());
        lines.push(
            "field\treceiver\tmode\tenclosing_type\tenclosing_member\tcount\tsample_files"
                .to_string(),
        );
        for target in &report.top_external_api_missing_field_contexts {
            lines.push(format_external_api_field_context_summary(target));
        }
    }
    if !report.top_external_field_guess_contexts.is_empty() {
        lines.push("top_external_field_guess_contexts".to_string());
        for target in &report.top_external_field_guess_contexts {
            lines.push(format_field_context_summary(target));
        }
    }
    if !report.top_project_field_guess_contexts.is_empty() {
        lines.push("top_project_field_guess_contexts".to_string());
        for target in &report.top_project_field_guess_contexts {
            lines.push(format_field_context_summary(target));
        }
    }
    if !report.top_source_field_guess_contexts.is_empty() {
        lines.push("top_source_field_guess_contexts".to_string());
        for target in &report.top_source_field_guess_contexts {
            lines.push(format_field_context_summary(target));
        }
    }
    if !report.top_resolver_diagnostics.is_empty() {
        lines.push("top_resolver_diagnostics".to_string());
        for diagnostic in &report.top_resolver_diagnostics {
            lines.push(format!(
                "{}\t{}\t{}",
                diagnostic.code, diagnostic.message, diagnostic.count
            ));
        }
        lines.push("top_resolver_diagnostic_files".to_string());
        for diagnostic in &report.top_resolver_diagnostics {
            lines.push(format!(
                "{}\t{}\t{}\t{}",
                diagnostic.code,
                diagnostic.message,
                diagnostic.count,
                diagnostic.sample_files.join(",")
            ));
        }
    }
    if !report.top_semantic_diagnostics.is_empty() {
        lines.push("top_semantic_diagnostics".to_string());
        for diagnostic in &report.top_semantic_diagnostics {
            lines.push(format!(
                "{}\t{}\t{}",
                diagnostic.code, diagnostic.message, diagnostic.count
            ));
        }
        lines.push("top_semantic_diagnostic_files".to_string());
        for diagnostic in &report.top_semantic_diagnostics {
            lines.push(format!(
                "{}\t{}\t{}\t{}",
                diagnostic.code,
                diagnostic.message,
                diagnostic.count,
                diagnostic.sample_files.join(",")
            ));
        }
    }
    if !report.top_unresolved_constructor_targets.is_empty() {
        lines.push("top_unresolved_constructor_targets".to_string());
        for target in &report.top_unresolved_constructor_targets {
            lines.push(format!(
                "{:?}\t{}\t{}",
                target.kind, target.target_name, target.count
            ));
        }
    }
    if !report.top_unexplained_constructor_targets.is_empty() {
        lines.push("top_unexplained_constructor_targets".to_string());
        for target in &report.top_unexplained_constructor_targets {
            lines.push(format!(
                "{:?}\t{}\t{}",
                target.kind, target.target_name, target.count
            ));
        }
    }
    if !report.top_unexplained_constructor_contexts.is_empty() {
        lines.push("top_unexplained_constructor_contexts".to_string());
        for target in &report.top_unexplained_constructor_contexts {
            lines.push(format_constructor_context_summary(target));
        }
    }
    if !report.top_unresolved_field_targets.is_empty() {
        lines.push("top_unresolved_field_targets".to_string());
        for target in &report.top_unresolved_field_targets {
            lines.push(format!(
                "{:?}\t{}\t{}",
                target.kind, target.target_name, target.count
            ));
        }
    }
    if !report.top_unexplained_field_targets.is_empty() {
        lines.push("top_unexplained_field_targets".to_string());
        for target in &report.top_unexplained_field_targets {
            lines.push(format!(
                "{:?}\t{}\t{}",
                target.kind, target.target_name, target.count
            ));
        }
    }
    if !report.top_unresolved_field_receivers.is_empty() {
        lines.push("top_unresolved_field_receivers".to_string());
        for target in &report.top_unresolved_field_receivers {
            lines.push(format!(
                "{}\t{}\t{}",
                target.field_name, target.receiver_type, target.count
            ));
        }
    }
    if !report.top_unexplained_field_receivers.is_empty() {
        lines.push("top_unexplained_field_receivers".to_string());
        for target in &report.top_unexplained_field_receivers {
            lines.push(format!(
                "{}\t{}\t{}",
                target.field_name, target.receiver_type, target.count
            ));
        }
    }
    if !report.top_unexplained_field_receiver_files.is_empty() {
        lines.push("top_unexplained_field_receiver_files".to_string());
        for target in &report.top_unexplained_field_receiver_files {
            lines.push(format!(
                "{}\t{}\t{}\t{}",
                target.field_name,
                target.receiver_type,
                target.count,
                target.sample_files.join(",")
            ));
        }
    }
    if !report.top_unexplained_field_contexts.is_empty() {
        lines.push("top_unexplained_field_contexts".to_string());
        for target in &report.top_unexplained_field_contexts {
            lines.push(format!(
                "{}\t{}\t{}\t{}\t{}\t{}",
                target.field_name,
                target.receiver_type,
                target.enclosing_type,
                target.enclosing_member,
                target.count,
                target.sample_files.join(",")
            ));
        }
    }
    lines.join("\n")
}

fn java_files_under(root: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    collect_java_files(root, &mut out)?;
    out.sort();
    Ok(out)
}

fn collect_java_files(path: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    if path.is_file() {
        if path
            .extension()
            .is_some_and(|extension| extension == "java")
        {
            out.push(path.to_path_buf());
        }
        return Ok(());
    }
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_java_files(&path, out)?;
        } else if path
            .extension()
            .is_some_and(|extension| extension == "java")
        {
            out.push(path);
        }
    }
    Ok(())
}

fn source_type_names(program: &crate::java_ir::Program) -> HashSet<String> {
    let mut names = HashSet::new();
    for ty in &program.types {
        collect_source_type_names(ty, &mut names);
    }
    names
}

fn collect_source_type_names(
    ty: &crate::java_ir::TypeDecl,
    names: &mut std::collections::HashSet<String>,
) {
    names.insert(ty.name.clone());
    names.insert(crate::parser::to_snake_case(&ty.name));
    for member in &ty.members {
        collect_source_member_type_names(member, names);
    }
}

fn collect_source_member_type_names(member: &Member, names: &mut HashSet<String>) {
    match member {
        Member::Type(nested) => collect_source_type_names(nested, names),
        Member::Method(method) => {
            if let Some(body) = &method.body {
                collect_source_block_type_names(body, names);
            }
        }
        Member::Constructor(constructor) => {
            if let Some(body) = &constructor.body {
                collect_source_block_type_names(body, names);
            }
        }
        _ => {}
    }
}

fn collect_source_block_type_names(block: &Block, names: &mut HashSet<String>) {
    for stmt in &block.statements {
        collect_source_stmt_type_names(stmt, names);
    }
}

fn collect_source_stmt_type_names(stmt: &Stmt, names: &mut HashSet<String>) {
    match stmt {
        Stmt::LocalType(local) => collect_source_type_names(local, names),
        Stmt::VarDecl {
            initializer: Some(initializer),
            ..
        } => collect_source_expr_type_names(initializer, names),
        Stmt::Assign { target, value } => {
            collect_source_expr_type_names(target, names);
            collect_source_expr_type_names(value, names);
        }
        Stmt::Expr(expr) | Stmt::Throw(expr) => collect_source_expr_type_names(expr, names),
        Stmt::Return(Some(expr)) => collect_source_expr_type_names(expr, names),
        Stmt::If {
            condition,
            then_block,
            else_block,
        } => {
            collect_source_expr_type_names(condition, names);
            collect_source_block_type_names(then_block, names);
            if let Some(else_block) = else_block {
                collect_source_block_type_names(else_block, names);
            }
        }
        Stmt::While { condition, body } => {
            collect_source_expr_type_names(condition, names);
            collect_source_block_type_names(body, names);
        }
        Stmt::For {
            init,
            condition,
            update,
            body,
            ..
        } => {
            for stmt in init {
                collect_source_stmt_type_names(stmt, names);
            }
            if let Some(condition) = condition {
                collect_source_expr_type_names(condition, names);
            }
            for expr in update {
                collect_source_expr_type_names(expr, names);
            }
            collect_source_block_type_names(body, names);
        }
        Stmt::EnhancedFor { iterable, body, .. } => {
            collect_source_expr_type_names(iterable, names);
            collect_source_block_type_names(body, names);
        }
        Stmt::Switch {
            selector, cases, ..
        } => {
            collect_source_expr_type_names(selector, names);
            for case in cases {
                for stmt in &case.statements {
                    collect_source_stmt_type_names(stmt, names);
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
                collect_source_stmt_type_names(resource, names);
            }
            collect_source_block_type_names(body, names);
            for catch in catches {
                collect_source_block_type_names(&catch.body, names);
            }
            if let Some(finally_block) = finally_block {
                collect_source_block_type_names(finally_block, names);
            }
        }
        Stmt::ConstructorInvocation { args, .. } => {
            for arg in args {
                collect_source_expr_type_names(arg, names);
            }
        }
        Stmt::Block(block) => collect_source_block_type_names(block, names),
        Stmt::VarDecl {
            initializer: None, ..
        }
        | Stmt::Return(None)
        | Stmt::Break(_)
        | Stmt::Continue(_)
        | Stmt::Unsupported(_) => {}
    }
}

fn collect_source_expr_type_names(expr: &Expr, names: &mut HashSet<String>) {
    match expr {
        Expr::FieldAccess(receiver, _) | Expr::Unary { expr: receiver, .. } => {
            collect_source_expr_type_names(receiver, names);
        }
        Expr::ArrayAccess(array, index)
        | Expr::Binary {
            left: array,
            right: index,
            ..
        } => {
            collect_source_expr_type_names(array, names);
            collect_source_expr_type_names(index, names);
        }
        Expr::ArrayCreation { dimensions, .. } | Expr::ArrayInitializer(dimensions) => {
            for expr in dimensions {
                collect_source_expr_type_names(expr, names);
            }
        }
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => {
            collect_source_expr_type_names(condition, names);
            collect_source_expr_type_names(then_expr, names);
            collect_source_expr_type_names(else_expr, names);
        }
        Expr::Call { receiver, args, .. } => {
            if let Some(receiver) = receiver {
                collect_source_expr_type_names(receiver, names);
            }
            for arg in args {
                collect_source_expr_type_names(arg, names);
            }
        }
        Expr::New { args, body, .. } => {
            for arg in args {
                collect_source_expr_type_names(arg, names);
            }
            if let Some(members) = body {
                for member in members {
                    collect_source_member_type_names(member, names);
                }
            }
        }
        Expr::Cast { expr, .. } | Expr::InstanceOf { expr, .. } => {
            collect_source_expr_type_names(expr, names);
        }
        Expr::Lambda { body, .. } => match body {
            LambdaBody::Expr(expr) => collect_source_expr_type_names(expr, names),
            LambdaBody::Block(block) => collect_source_block_type_names(block, names),
        },
        Expr::Literal(_)
        | Expr::Var(_)
        | Expr::This
        | Expr::Super
        | Expr::MethodReference { .. }
        | Expr::ClassLiteral(_)
        | Expr::Raw(_) => {}
    }
}

fn is_source_type_name(source_type_names: &HashSet<String>, type_name: &str) -> bool {
    source_type_names.contains(type_name)
        || source_type_names.contains(&crate::parser::to_snake_case(type_name))
}

fn is_source_type_reference(
    source_type_names: &HashSet<String>,
    source_index: Option<&SourceTypeIndex>,
    program: &Program,
    type_name: &str,
) -> bool {
    if let Some(source_index) = source_index {
        if let Some(is_source) = source_index.explicit_import_source_ownership(program, type_name) {
            return is_source;
        }
    }
    is_source_type_name(source_type_names, type_name)
}

fn is_project_type_reference(
    source_index: Option<&SourceTypeIndex>,
    program: &Program,
    type_name: &str,
) -> bool {
    source_index.is_some_and(|source_index| {
        source_index.explicit_import_project_ownership(program, type_name)
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FieldGuessOwner {
    Source,
    Project,
    External,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DependencyGuessOwner {
    Source,
    Project,
    External,
}

#[derive(Debug, Default)]
struct ConstructorGuessOwnership {
    source_types: HashSet<String>,
    project_types: HashSet<String>,
}

impl ConstructorGuessOwnership {
    fn from_conservative_constructor_facts(
        facts: &FactSet,
        source_type_names: &HashSet<String>,
        source_index: Option<&SourceTypeIndex>,
        program: &Program,
    ) -> Self {
        let mut ownership = Self::default();

        for fact in facts
            .external_constructor_facts
            .iter()
            .filter(|fact| fact.provenance == FactProvenance::ConservativeFallback)
        {
            ownership.record_type_ownership(
                &fact.type_name,
                None,
                source_type_names,
                source_index,
                program,
            );
        }

        for fact in facts.unresolved_target_facts.iter().filter(|fact| {
            fact.kind == UnresolvedTargetKind::ConstructorCall && !fact.is_anonymous_class
        }) {
            let canonical_name = fact
                .receiver_type
                .as_ref()
                .and_then(java_type_key)
                .unwrap_or_else(|| fact.target_name.clone());
            ownership.record_type_ownership(
                &canonical_name,
                fact.receiver_type.as_ref().and_then(java_type_raw_name),
                source_type_names,
                source_index,
                program,
            );
        }

        ownership
    }

    fn ownership_for(&self, type_name: &str) -> DependencyGuessOwner {
        if self.is_source_type(type_name) {
            DependencyGuessOwner::Source
        } else if self.is_project_type(type_name) {
            DependencyGuessOwner::Project
        } else {
            DependencyGuessOwner::External
        }
    }

    fn ownership_for_unresolved(
        &self,
        fact: &crate::facts::UnresolvedTargetFact,
        canonical_type_name: &str,
    ) -> DependencyGuessOwner {
        if self
            .type_aliases_for_unresolved(fact, canonical_type_name)
            .iter()
            .any(|alias| self.is_source_type(alias))
        {
            DependencyGuessOwner::Source
        } else if self
            .type_aliases_for_unresolved(fact, canonical_type_name)
            .iter()
            .any(|alias| self.is_project_type(alias))
        {
            DependencyGuessOwner::Project
        } else {
            DependencyGuessOwner::External
        }
    }

    fn record_type_ownership(
        &mut self,
        canonical_type_name: &str,
        raw_type_name: Option<&str>,
        source_type_names: &HashSet<String>,
        source_index: Option<&SourceTypeIndex>,
        program: &Program,
    ) {
        if is_source_type_reference(
            source_type_names,
            source_index,
            program,
            canonical_type_name,
        ) || raw_type_name.is_some_and(|raw| {
            nested_type_owner_candidates(raw)
                .into_iter()
                .any(|candidate| {
                    is_source_type_reference(source_type_names, source_index, program, &candidate)
                })
        }) {
            self.insert_source_type(canonical_type_name);
            if let Some(raw_type_name) = raw_type_name {
                self.insert_source_type(raw_type_name);
            }
        } else if is_project_type_reference(source_index, program, canonical_type_name)
            || raw_type_name.is_some_and(|raw| {
                nested_type_owner_candidates(raw)
                    .into_iter()
                    .any(|candidate| is_project_type_reference(source_index, program, &candidate))
            })
        {
            self.insert_project_type(canonical_type_name);
            if let Some(raw_type_name) = raw_type_name {
                self.insert_project_type(raw_type_name);
            }
        }
    }

    fn type_aliases_for_unresolved(
        &self,
        fact: &crate::facts::UnresolvedTargetFact,
        canonical_type_name: &str,
    ) -> Vec<String> {
        let mut aliases = constructor_type_aliases(canonical_type_name);
        if let Some(raw_type_name) = fact.receiver_type.as_ref().and_then(java_type_raw_name) {
            aliases.extend(constructor_type_aliases(raw_type_name));
        }
        aliases.sort();
        aliases.dedup();
        aliases
    }

    fn is_source_type(&self, type_name: &str) -> bool {
        constructor_type_aliases(type_name)
            .into_iter()
            .any(|alias| self.source_types.contains(&alias))
    }

    fn is_project_type(&self, type_name: &str) -> bool {
        constructor_type_aliases(type_name)
            .into_iter()
            .any(|alias| self.project_types.contains(&alias))
    }

    fn insert_source_type(&mut self, type_name: &str) {
        self.source_types
            .extend(constructor_type_aliases(type_name));
    }

    fn insert_project_type(&mut self, type_name: &str) {
        self.project_types
            .extend(constructor_type_aliases(type_name));
    }
}

fn constructor_type_aliases(type_name: &str) -> Vec<String> {
    let simple_name = type_name.rsplit('.').next().unwrap_or(type_name);
    let canonical = crate::stdlib::canonical_external_receiver_type_key(simple_name);
    let mut aliases = vec![
        type_name.to_string(),
        simple_name.to_string(),
        canonical.clone(),
        crate::parser::to_snake_case(type_name),
        crate::parser::to_snake_case(simple_name),
        crate::parser::to_snake_case(&canonical),
    ];
    aliases.sort();
    aliases.dedup();
    aliases
}

fn nested_type_owner_candidates(type_name: &str) -> Vec<String> {
    let segments = type_name.split('.').collect::<Vec<_>>();
    if segments.len() <= 1 {
        return constructor_type_aliases(type_name);
    }

    let mut candidates = constructor_type_aliases(type_name);
    for end in 1..segments.len() {
        let prefix = segments[..end].join(".");
        if let Some(segment) = segments.get(end - 1) {
            if segment
                .chars()
                .next()
                .is_some_and(|ch| ch.is_ascii_uppercase())
            {
                candidates.extend(constructor_type_aliases(&prefix));
            }
        }
    }
    candidates.sort();
    candidates.dedup();
    candidates
}

#[derive(Debug, Default)]
struct FieldGuessOwnership {
    source_receivers: HashSet<String>,
    project_receivers: HashSet<String>,
}

impl FieldGuessOwnership {
    fn from_conservative_field_facts(
        facts: &FactSet,
        source_type_names: &HashSet<String>,
        source_index: Option<&SourceTypeIndex>,
        program: &Program,
    ) -> Self {
        let conservative_fields = facts
            .external_field_facts
            .iter()
            .filter(|fact| fact.provenance == FactProvenance::ConservativeFallback)
            .collect::<Vec<_>>();
        let mut ownership = Self::default();

        for fact in &conservative_fields {
            if is_source_type_reference(
                source_type_names,
                source_index,
                program,
                &fact.field.type_name,
            ) {
                ownership.insert_source_receiver(&fact.field.type_name);
            } else if is_project_type_reference(source_index, program, &fact.field.type_name) {
                ownership.insert_project_receiver(&fact.field.type_name);
            }
        }

        let mut changed = true;
        while changed {
            changed = false;
            for fact in &conservative_fields {
                if !is_type_like_static_field_name(&fact.field.field_name) {
                    continue;
                }
                if ownership.is_source_receiver(&fact.field.type_name) {
                    changed |= ownership.insert_source_receiver(&fact.field.field_name);
                } else if ownership.is_project_receiver(&fact.field.type_name) {
                    changed |= ownership.insert_project_receiver(&fact.field.field_name);
                }
            }
        }

        ownership
    }

    fn ownership_for(&self, receiver_type: &str) -> FieldGuessOwner {
        if self.is_source_receiver(receiver_type) {
            FieldGuessOwner::Source
        } else if self.is_project_receiver(receiver_type) {
            FieldGuessOwner::Project
        } else {
            FieldGuessOwner::External
        }
    }

    fn is_source_receiver(&self, receiver_type: &str) -> bool {
        field_guess_receiver_aliases(receiver_type)
            .into_iter()
            .any(|alias| self.source_receivers.contains(&alias))
    }

    fn is_project_receiver(&self, receiver_type: &str) -> bool {
        field_guess_receiver_aliases(receiver_type)
            .into_iter()
            .any(|alias| self.project_receivers.contains(&alias))
    }

    fn insert_source_receiver(&mut self, receiver_type: &str) -> bool {
        let mut changed = false;
        for alias in field_guess_receiver_aliases(receiver_type) {
            changed |= self.source_receivers.insert(alias);
        }
        changed
    }

    fn insert_project_receiver(&mut self, receiver_type: &str) -> bool {
        let mut changed = false;
        for alias in field_guess_receiver_aliases(receiver_type) {
            changed |= self.project_receivers.insert(alias);
        }
        changed
    }
}

fn field_guess_receiver_aliases(receiver_type: &str) -> Vec<String> {
    let mut aliases = vec![
        receiver_type.to_string(),
        crate::parser::to_snake_case(receiver_type),
    ];
    aliases.sort();
    aliases.dedup();
    aliases
}

fn is_type_like_static_field_name(field_name: &str) -> bool {
    field_name
        .chars()
        .next()
        .is_some_and(|ch| ch.is_ascii_uppercase())
}

fn push_unresolved_summary(
    summaries: &mut Vec<UnresolvedTargetSummary>,
    kind: UnresolvedTargetKind,
    target_name: &str,
) {
    if let Some(summary) = summaries
        .iter_mut()
        .find(|summary| summary.kind == kind && summary.target_name == target_name)
    {
        summary.count += 1;
    } else {
        summaries.push(UnresolvedTargetSummary {
            kind,
            target_name: target_name.to_string(),
            count: 1,
        });
    }
}

fn sort_and_truncate_target_summaries(summaries: &mut Vec<UnresolvedTargetSummary>) {
    summaries.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then_with(|| left.target_name.cmp(&right.target_name))
    });
    summaries.truncate(20);
}

fn push_unexplained_method_receiver_summary(
    summaries: &mut Vec<UnresolvedMethodReceiverSummary>,
    target_name: &str,
    receiver_type: &str,
) {
    if let Some(summary) = summaries.iter_mut().find(|summary| {
        summary.target_name == target_name && summary.receiver_type == receiver_type
    }) {
        summary.count += 1;
    } else {
        summaries.push(UnresolvedMethodReceiverSummary {
            target_name: target_name.to_string(),
            receiver_type: receiver_type.to_string(),
            count: 1,
        });
    }
}

fn push_external_api_guess_summary(
    summaries: &mut Vec<ExternalApiGuessSummary>,
    receiver_type: &str,
    method_name: &str,
) {
    if let Some(summary) = summaries.iter_mut().find(|summary| {
        summary.receiver_type == receiver_type && summary.method_name == method_name
    }) {
        summary.count += 1;
    } else {
        summaries.push(ExternalApiGuessSummary {
            receiver_type: receiver_type.to_string(),
            method_name: method_name.to_string(),
            count: 1,
        });
    }
}

fn push_external_pack_api_summary(
    summaries: &mut Vec<ExternalApiPackSummary>,
    receiver_type: &str,
    method_name: &str,
    mode: Option<ExternalLibraryMode>,
) {
    let mode = ExternalReportMode::from(mode);
    if let Some(summary) = summaries.iter_mut().find(|summary| {
        summary.receiver_type == receiver_type
            && summary.method_name == method_name
            && summary.mode == mode
    }) {
        summary.count += 1;
    } else {
        summaries.push(ExternalApiPackSummary {
            receiver_type: receiver_type.to_string(),
            method_name: method_name.to_string(),
            mode,
            count: 1,
        });
    }
}

fn push_source_dependency_api_summary(
    summaries: &mut Vec<SourceDependencyMethodSummary>,
    receiver_type: &str,
    method_name: &str,
    source_mode: DependencyMode,
) {
    if let Some(summary) = summaries.iter_mut().find(|summary| {
        summary.receiver_type == receiver_type
            && summary.method_name == method_name
            && summary.source_mode == source_mode
    }) {
        summary.count += 1;
    } else {
        summaries.push(SourceDependencyMethodSummary {
            receiver_type: receiver_type.to_string(),
            method_name: method_name.to_string(),
            source_mode,
            count: 1,
        });
    }
}

fn push_external_api_method_context_summary(
    summaries: &mut Vec<ExternalApiMethodContextSummary>,
    receiver_type: &str,
    method_name: &str,
    mode: Option<ExternalLibraryMode>,
    arg_count: usize,
    enclosing_type: &str,
    enclosing_member: &str,
    file_label: &str,
) {
    let mode = ExternalReportMode::from(mode);
    if let Some(summary) = summaries.iter_mut().find(|summary| {
        summary.receiver_type == receiver_type
            && summary.method_name == method_name
            && summary.mode == mode
            && summary.arg_count == arg_count
            && summary.enclosing_type == enclosing_type
            && summary.enclosing_member == enclosing_member
    }) {
        summary.count += 1;
        if summary.sample_files.len() < 5
            && !summary.sample_files.iter().any(|file| file == file_label)
        {
            summary.sample_files.push(file_label.to_string());
        }
    } else {
        summaries.push(ExternalApiMethodContextSummary {
            receiver_type: receiver_type.to_string(),
            method_name: method_name.to_string(),
            mode,
            arg_count,
            enclosing_type: enclosing_type.to_string(),
            enclosing_member: enclosing_member.to_string(),
            count: 1,
            sample_files: vec![file_label.to_string()],
        });
    }
}

fn push_external_pack_constructor_summary(
    summaries: &mut Vec<ExternalPackConstructorSummary>,
    type_name: &str,
    arity: usize,
    mode: Option<ExternalLibraryMode>,
) {
    let mode = ExternalReportMode::from(mode);
    if let Some(summary) = summaries.iter_mut().find(|summary| {
        summary.type_name == type_name && summary.arity == arity && summary.mode == mode
    }) {
        summary.count += 1;
    } else {
        summaries.push(ExternalPackConstructorSummary {
            type_name: type_name.to_string(),
            arity,
            mode,
            count: 1,
        });
    }
}

fn push_source_dependency_constructor_summary(
    summaries: &mut Vec<SourceDependencyConstructorSummary>,
    type_name: &str,
    arity: usize,
    source_mode: DependencyMode,
) {
    if let Some(summary) = summaries.iter_mut().find(|summary| {
        summary.type_name == type_name
            && summary.arity == arity
            && summary.source_mode == source_mode
    }) {
        summary.count += 1;
    } else {
        summaries.push(SourceDependencyConstructorSummary {
            type_name: type_name.to_string(),
            arity,
            source_mode,
            count: 1,
        });
    }
}

fn push_external_constructor_guess_summary(
    summaries: &mut Vec<ExternalConstructorGuessSummary>,
    type_name: &str,
    arity: usize,
) {
    if let Some(summary) = summaries
        .iter_mut()
        .find(|summary| summary.type_name == type_name && summary.arity == arity)
    {
        summary.count += 1;
    } else {
        summaries.push(ExternalConstructorGuessSummary {
            type_name: type_name.to_string(),
            arity,
            count: 1,
        });
    }
}

fn sort_constructor_contexts(summaries: &mut [UnresolvedConstructorContextSummary]) {
    summaries.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then_with(|| left.target_name.cmp(&right.target_name))
            .then_with(|| left.receiver_type.cmp(&right.receiver_type))
            .then_with(|| left.enclosing_type.cmp(&right.enclosing_type))
            .then_with(|| left.enclosing_member.cmp(&right.enclosing_member))
            .then_with(|| left.arg_types.cmp(&right.arg_types))
    });
}

fn sort_external_api_method_contexts(summaries: &mut [ExternalApiMethodContextSummary]) {
    summaries.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then_with(|| left.receiver_type.cmp(&right.receiver_type))
            .then_with(|| left.method_name.cmp(&right.method_name))
            .then_with(|| {
                external_pack_summary_mode_label(left.mode)
                    .cmp(external_pack_summary_mode_label(right.mode))
            })
            .then_with(|| left.enclosing_type.cmp(&right.enclosing_type))
            .then_with(|| left.enclosing_member.cmp(&right.enclosing_member))
            .then_with(|| left.arg_count.cmp(&right.arg_count))
    });
}

fn sort_external_api_constructor_contexts(summaries: &mut [ExternalApiConstructorContextSummary]) {
    summaries.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then_with(|| left.target_name.cmp(&right.target_name))
            .then_with(|| left.receiver_type.cmp(&right.receiver_type))
            .then_with(|| {
                external_pack_summary_mode_label(left.mode)
                    .cmp(external_pack_summary_mode_label(right.mode))
            })
            .then_with(|| left.enclosing_type.cmp(&right.enclosing_type))
            .then_with(|| left.enclosing_member.cmp(&right.enclosing_member))
            .then_with(|| left.arg_types.cmp(&right.arg_types))
    });
}

fn sort_external_api_field_contexts(summaries: &mut [ExternalApiFieldContextSummary]) {
    summaries.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then_with(|| left.field_name.cmp(&right.field_name))
            .then_with(|| left.receiver_type.cmp(&right.receiver_type))
            .then_with(|| {
                external_pack_summary_mode_label(left.mode)
                    .cmp(external_pack_summary_mode_label(right.mode))
            })
            .then_with(|| left.enclosing_type.cmp(&right.enclosing_type))
            .then_with(|| left.enclosing_member.cmp(&right.enclosing_member))
    });
}

fn sort_external_field_guess_summaries(summaries: &mut [ExternalFieldGuessSummary]) {
    summaries.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then_with(|| left.type_name.cmp(&right.type_name))
            .then_with(|| left.field_name.cmp(&right.field_name))
    });
}

fn sort_field_contexts(summaries: &mut [UnresolvedFieldContextSummary]) {
    summaries.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then_with(|| left.field_name.cmp(&right.field_name))
            .then_with(|| left.receiver_type.cmp(&right.receiver_type))
            .then_with(|| left.enclosing_type.cmp(&right.enclosing_type))
            .then_with(|| left.enclosing_member.cmp(&right.enclosing_member))
    });
}

fn sort_source_dependency_method_contexts(summaries: &mut [SourceDependencyMethodContextSummary]) {
    summaries.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then_with(|| left.declaring_type.cmp(&right.declaring_type))
            .then_with(|| left.method_name.cmp(&right.method_name))
            .then_with(|| {
                dependency_mode_label(Some(left.source_mode))
                    .cmp(dependency_mode_label(Some(right.source_mode)))
            })
            .then_with(|| left.enclosing_type.cmp(&right.enclosing_type))
            .then_with(|| left.enclosing_member.cmp(&right.enclosing_member))
            .then_with(|| left.arg_count.cmp(&right.arg_count))
    });
}

fn sort_source_dependency_constructor_contexts(
    summaries: &mut [SourceDependencyConstructorContextSummary],
) {
    summaries.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then_with(|| left.declaring_type.cmp(&right.declaring_type))
            .then_with(|| {
                dependency_mode_label(Some(left.source_mode))
                    .cmp(dependency_mode_label(Some(right.source_mode)))
            })
            .then_with(|| left.enclosing_type.cmp(&right.enclosing_type))
            .then_with(|| left.enclosing_member.cmp(&right.enclosing_member))
            .then_with(|| left.arg_count.cmp(&right.arg_count))
    });
}

fn sort_source_dependency_field_contexts(summaries: &mut [SourceDependencyFieldContextSummary]) {
    summaries.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then_with(|| left.declaring_type.cmp(&right.declaring_type))
            .then_with(|| left.field_name.cmp(&right.field_name))
            .then_with(|| left.receiver_type.cmp(&right.receiver_type))
            .then_with(|| {
                dependency_mode_label(Some(left.source_mode))
                    .cmp(dependency_mode_label(Some(right.source_mode)))
            })
            .then_with(|| left.enclosing_type.cmp(&right.enclosing_type))
            .then_with(|| left.enclosing_member.cmp(&right.enclosing_member))
    });
}

fn format_constructor_context_summary(target: &UnresolvedConstructorContextSummary) -> String {
    format!(
        "{}\t{}\t{}\t{}\t{}\t{}\t{}",
        target.target_name,
        target.receiver_type,
        target.arg_types.join(","),
        target.enclosing_type,
        target.enclosing_member,
        target.count,
        target.sample_files.join(",")
    )
}

fn format_external_api_method_context_summary(target: &ExternalApiMethodContextSummary) -> String {
    format!(
        "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
        target.receiver_type,
        target.method_name,
        external_pack_summary_mode_label(target.mode),
        target.arg_count,
        target.enclosing_type,
        target.enclosing_member,
        target.count,
        target.sample_files.join(",")
    )
}

fn format_external_api_constructor_context_summary(
    target: &ExternalApiConstructorContextSummary,
) -> String {
    format!(
        "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
        target.target_name,
        target.receiver_type,
        external_pack_summary_mode_label(target.mode),
        target.arg_types.join(","),
        target.enclosing_type,
        target.enclosing_member,
        target.count,
        target.sample_files.join(",")
    )
}

fn format_source_dependency_method_context_summary(
    target: &SourceDependencyMethodContextSummary,
) -> String {
    format!(
        "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
        target.declaring_type,
        target.method_name,
        dependency_mode_label(Some(target.source_mode)),
        target.arg_count,
        target.enclosing_type,
        target.enclosing_member,
        target.count,
        target.sample_files.join(",")
    )
}

fn format_source_dependency_constructor_context_summary(
    target: &SourceDependencyConstructorContextSummary,
) -> String {
    format!(
        "{}\t{}\t{}\t{}\t{}\t{}\t{}",
        target.declaring_type,
        dependency_mode_label(Some(target.source_mode)),
        target.arg_count,
        target.enclosing_type,
        target.enclosing_member,
        target.count,
        target.sample_files.join(",")
    )
}

fn format_source_dependency_field_context_summary(
    target: &SourceDependencyFieldContextSummary,
) -> String {
    format!(
        "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
        target.declaring_type,
        target.field_name,
        target.receiver_type,
        dependency_mode_label(Some(target.source_mode)),
        target.enclosing_type,
        target.enclosing_member,
        target.count,
        target.sample_files.join(",")
    )
}

fn format_field_context_summary(target: &UnresolvedFieldContextSummary) -> String {
    format!(
        "{}\t{}\t{}\t{}\t{}\t{}",
        target.field_name,
        target.receiver_type,
        target.enclosing_type,
        target.enclosing_member,
        target.count,
        target.sample_files.join(",")
    )
}

fn format_external_api_field_context_summary(target: &ExternalApiFieldContextSummary) -> String {
    format!(
        "{}\t{}\t{}\t{}\t{}\t{}\t{}",
        target.field_name,
        target.receiver_type,
        external_pack_summary_mode_label(target.mode),
        target.enclosing_type,
        target.enclosing_member,
        target.count,
        target.sample_files.join(",")
    )
}

fn push_external_field_guess_summary(
    summaries: &mut Vec<ExternalFieldGuessSummary>,
    type_name: &str,
    field_name: &str,
    mode: Option<ExternalLibraryMode>,
) {
    let mode = ExternalReportMode::from(mode);
    if let Some(summary) = summaries.iter_mut().find(|summary| {
        summary.type_name == type_name && summary.field_name == field_name && summary.mode == mode
    }) {
        summary.count += 1;
    } else {
        summaries.push(ExternalFieldGuessSummary {
            type_name: type_name.to_string(),
            field_name: field_name.to_string(),
            mode,
            count: 1,
        });
    }
}

fn push_source_dependency_field_summary(
    summaries: &mut Vec<SourceDependencyFieldSummary>,
    type_name: &str,
    field_name: &str,
    source_mode: DependencyMode,
) {
    if let Some(summary) = summaries.iter_mut().find(|summary| {
        summary.type_name == type_name
            && summary.field_name == field_name
            && summary.source_mode == source_mode
    }) {
        summary.count += 1;
    } else {
        summaries.push(SourceDependencyFieldSummary {
            type_name: type_name.to_string(),
            field_name: field_name.to_string(),
            source_mode,
            count: 1,
        });
    }
}

fn push_unresolved_field_receiver_summary(
    summaries: &mut Vec<UnresolvedFieldReceiverSummary>,
    field_name: &str,
    receiver_type: &str,
) {
    if let Some(summary) = summaries
        .iter_mut()
        .find(|summary| summary.field_name == field_name && summary.receiver_type == receiver_type)
    {
        summary.count += 1;
    } else {
        summaries.push(UnresolvedFieldReceiverSummary {
            field_name: field_name.to_string(),
            receiver_type: receiver_type.to_string(),
            count: 1,
        });
    }
}

fn push_unresolved_field_receiver_file_summary(
    summaries: &mut Vec<UnresolvedFieldReceiverFileSummary>,
    field_name: &str,
    receiver_type: &str,
    file_label: &str,
) {
    if let Some(summary) = summaries
        .iter_mut()
        .find(|summary| summary.field_name == field_name && summary.receiver_type == receiver_type)
    {
        summary.count += 1;
        if summary.sample_files.len() < 5
            && !summary.sample_files.iter().any(|file| file == file_label)
        {
            summary.sample_files.push(file_label.to_string());
        }
    } else {
        summaries.push(UnresolvedFieldReceiverFileSummary {
            field_name: field_name.to_string(),
            receiver_type: receiver_type.to_string(),
            count: 1,
            sample_files: vec![file_label.to_string()],
        });
    }
}

fn push_unresolved_constructor_context_summary(
    summaries: &mut Vec<UnresolvedConstructorContextSummary>,
    target_name: &str,
    receiver_type: &str,
    arg_types: &[String],
    enclosing_type: &str,
    enclosing_member: &str,
    file_label: &str,
) {
    if let Some(summary) = summaries.iter_mut().find(|summary| {
        summary.target_name == target_name
            && summary.receiver_type == receiver_type
            && summary.arg_types == arg_types
            && summary.enclosing_type == enclosing_type
            && summary.enclosing_member == enclosing_member
    }) {
        summary.count += 1;
        if summary.sample_files.len() < 5
            && !summary.sample_files.iter().any(|file| file == file_label)
        {
            summary.sample_files.push(file_label.to_string());
        }
    } else {
        summaries.push(UnresolvedConstructorContextSummary {
            target_name: target_name.to_string(),
            receiver_type: receiver_type.to_string(),
            arg_types: arg_types.to_vec(),
            enclosing_type: enclosing_type.to_string(),
            enclosing_member: enclosing_member.to_string(),
            count: 1,
            sample_files: vec![file_label.to_string()],
        });
    }
}

fn push_external_api_constructor_context_summary(
    summaries: &mut Vec<ExternalApiConstructorContextSummary>,
    target_name: &str,
    receiver_type: &str,
    mode: Option<ExternalLibraryMode>,
    arg_types: &[String],
    enclosing_type: &str,
    enclosing_member: &str,
    file_label: &str,
) {
    let mode = ExternalReportMode::from(mode);
    if let Some(summary) = summaries.iter_mut().find(|summary| {
        summary.target_name == target_name
            && summary.receiver_type == receiver_type
            && summary.mode == mode
            && summary.arg_types == arg_types
            && summary.enclosing_type == enclosing_type
            && summary.enclosing_member == enclosing_member
    }) {
        summary.count += 1;
        if summary.sample_files.len() < 5
            && !summary.sample_files.iter().any(|file| file == file_label)
        {
            summary.sample_files.push(file_label.to_string());
        }
    } else {
        summaries.push(ExternalApiConstructorContextSummary {
            target_name: target_name.to_string(),
            receiver_type: receiver_type.to_string(),
            mode,
            arg_types: arg_types.to_vec(),
            enclosing_type: enclosing_type.to_string(),
            enclosing_member: enclosing_member.to_string(),
            count: 1,
            sample_files: vec![file_label.to_string()],
        });
    }
}

fn push_source_dependency_method_context_summary(
    summaries: &mut Vec<SourceDependencyMethodContextSummary>,
    declaring_type: &str,
    method_name: &str,
    source_mode: DependencyMode,
    arg_count: usize,
    context: SourceDependencyWalkContext<'_>,
) {
    if let Some(summary) = summaries.iter_mut().find(|summary| {
        summary.declaring_type == declaring_type
            && summary.method_name == method_name
            && summary.source_mode == source_mode
            && summary.arg_count == arg_count
            && summary.enclosing_type == context.enclosing_type
            && summary.enclosing_member == context.enclosing_member
    }) {
        summary.count += 1;
        if summary.sample_files.len() < 5
            && !summary
                .sample_files
                .iter()
                .any(|file| file == context.file_label)
        {
            summary.sample_files.push(context.file_label.to_string());
        }
    } else {
        summaries.push(SourceDependencyMethodContextSummary {
            declaring_type: declaring_type.to_string(),
            method_name: method_name.to_string(),
            source_mode,
            arg_count,
            enclosing_type: context.enclosing_type.to_string(),
            enclosing_member: context.enclosing_member.to_string(),
            count: 1,
            sample_files: vec![context.file_label.to_string()],
        });
    }
}

fn push_source_dependency_constructor_context_summary(
    summaries: &mut Vec<SourceDependencyConstructorContextSummary>,
    declaring_type: &str,
    source_mode: DependencyMode,
    arg_count: usize,
    context: SourceDependencyWalkContext<'_>,
) {
    if let Some(summary) = summaries.iter_mut().find(|summary| {
        summary.declaring_type == declaring_type
            && summary.source_mode == source_mode
            && summary.arg_count == arg_count
            && summary.enclosing_type == context.enclosing_type
            && summary.enclosing_member == context.enclosing_member
    }) {
        summary.count += 1;
        if summary.sample_files.len() < 5
            && !summary
                .sample_files
                .iter()
                .any(|file| file == context.file_label)
        {
            summary.sample_files.push(context.file_label.to_string());
        }
    } else {
        summaries.push(SourceDependencyConstructorContextSummary {
            declaring_type: declaring_type.to_string(),
            source_mode,
            arg_count,
            enclosing_type: context.enclosing_type.to_string(),
            enclosing_member: context.enclosing_member.to_string(),
            count: 1,
            sample_files: vec![context.file_label.to_string()],
        });
    }
}

fn push_source_dependency_field_context_summary(
    summaries: &mut Vec<SourceDependencyFieldContextSummary>,
    declaring_type: &str,
    field_name: &str,
    receiver_type: &str,
    source_mode: DependencyMode,
    context: SourceDependencyWalkContext<'_>,
) {
    if let Some(summary) = summaries.iter_mut().find(|summary| {
        summary.declaring_type == declaring_type
            && summary.field_name == field_name
            && summary.receiver_type == receiver_type
            && summary.source_mode == source_mode
            && summary.enclosing_type == context.enclosing_type
            && summary.enclosing_member == context.enclosing_member
    }) {
        summary.count += 1;
        if summary.sample_files.len() < 5
            && !summary
                .sample_files
                .iter()
                .any(|file| file == context.file_label)
        {
            summary.sample_files.push(context.file_label.to_string());
        }
    } else {
        summaries.push(SourceDependencyFieldContextSummary {
            declaring_type: declaring_type.to_string(),
            field_name: field_name.to_string(),
            receiver_type: receiver_type.to_string(),
            source_mode,
            enclosing_type: context.enclosing_type.to_string(),
            enclosing_member: context.enclosing_member.to_string(),
            count: 1,
            sample_files: vec![context.file_label.to_string()],
        });
    }
}

fn push_unresolved_field_context_summary(
    summaries: &mut Vec<UnresolvedFieldContextSummary>,
    field_name: &str,
    receiver_type: &str,
    enclosing_type: &str,
    enclosing_member: &str,
    file_label: &str,
) {
    if let Some(summary) = summaries.iter_mut().find(|summary| {
        summary.field_name == field_name
            && summary.receiver_type == receiver_type
            && summary.enclosing_type == enclosing_type
            && summary.enclosing_member == enclosing_member
    }) {
        summary.count += 1;
        if summary.sample_files.len() < 5
            && !summary.sample_files.iter().any(|file| file == file_label)
        {
            summary.sample_files.push(file_label.to_string());
        }
    } else {
        summaries.push(UnresolvedFieldContextSummary {
            field_name: field_name.to_string(),
            receiver_type: receiver_type.to_string(),
            enclosing_type: enclosing_type.to_string(),
            enclosing_member: enclosing_member.to_string(),
            count: 1,
            sample_files: vec![file_label.to_string()],
        });
    }
}

fn push_external_api_field_context_summary(
    summaries: &mut Vec<ExternalApiFieldContextSummary>,
    field_name: &str,
    receiver_type: &str,
    mode: Option<ExternalLibraryMode>,
    enclosing_type: &str,
    enclosing_member: &str,
    file_label: &str,
) {
    let mode = ExternalReportMode::from(mode);
    if let Some(summary) = summaries.iter_mut().find(|summary| {
        summary.field_name == field_name
            && summary.receiver_type == receiver_type
            && summary.mode == mode
            && summary.enclosing_type == enclosing_type
            && summary.enclosing_member == enclosing_member
    }) {
        summary.count += 1;
        if summary.sample_files.len() < 5
            && !summary.sample_files.iter().any(|file| file == file_label)
        {
            summary.sample_files.push(file_label.to_string());
        }
    } else {
        summaries.push(ExternalApiFieldContextSummary {
            field_name: field_name.to_string(),
            receiver_type: receiver_type.to_string(),
            mode,
            enclosing_type: enclosing_type.to_string(),
            enclosing_member: enclosing_member.to_string(),
            count: 1,
            sample_files: vec![file_label.to_string()],
        });
    }
}

fn push_diagnostic_summary(
    summaries: &mut Vec<DiagnosticSummary>,
    code: &'static str,
    message: &str,
    file_label: &str,
    mode: Option<DependencyMode>,
) {
    if let Some(summary) = summaries
        .iter_mut()
        .find(|summary| summary.code == code && summary.message == message && summary.mode == mode)
    {
        summary.count += 1;
        if summary.sample_files.len() < 5
            && !summary
                .sample_files
                .iter()
                .any(|existing| existing == file_label)
        {
            summary.sample_files.push(file_label.to_string());
        }
    } else {
        summaries.push(DiagnosticSummary {
            code,
            message: message.to_string(),
            mode,
            count: 1,
            sample_files: vec![file_label.to_string()],
        });
    }
}

fn collect_builtin_external_pack_manifest_coverage_audit(report: &mut ResolverBenchmarkReport) {
    for manifest in crate::external_packs::builtin_external_pack_manifests() {
        let Some(facts) = crate::external_packs::builtin_external_pack_facts(manifest.pack_name)
        else {
            continue;
        };
        let mode = Some(dependency_mode_for_external_library_mode(manifest.mode));
        push_external_pack_manifest_audit(
            report,
            manifest.pack_name,
            mode,
            crate::external_packs::audit_external_pack_manifest_coverage(&manifest, &facts),
        );
        push_external_pack_manifest_audit(
            report,
            manifest.pack_name,
            mode,
            crate::external_packs::audit_external_pack_fact_boundaries(&facts),
        );
    }
}

fn collect_external_pack_provider_boundary_audits(
    config: &DependencyModeConfig,
    report: &mut ResolverBenchmarkReport,
) {
    for provider in &config.fact_providers {
        if provider.facts.external_library_packs.is_empty() {
            continue;
        }
        if let Some(manifest) =
            crate::external_packs::builtin_external_pack_manifest_for_provider_name(&provider.name)
        {
            push_external_pack_manifest_audit(
                report,
                &provider.name,
                Some(provider.mode),
                crate::external_packs::audit_external_pack_manifest_coverage(
                    &manifest,
                    &provider.facts,
                ),
            );
        }
        push_external_pack_manifest_audit(
            report,
            &provider.name,
            Some(provider.mode),
            crate::external_packs::audit_external_pack_fact_boundaries(&provider.facts),
        );
    }
}

fn push_external_pack_manifest_audit(
    report: &mut ResolverBenchmarkReport,
    source_name: &str,
    mode: Option<DependencyMode>,
    audit: crate::diagnostics::DiagnosticBag,
) {
    report.external_pack_manifest_coverage_diagnostics += audit.len();
    for diagnostic in audit.iter() {
        push_diagnostic_summary(
            &mut report.top_external_pack_manifest_coverage_diagnostics,
            diagnostic.code,
            &diagnostic.message,
            source_name,
            mode,
        );
    }
}

fn fact_count(facts: &FactSet) -> usize {
    facts.notes.len()
        + facts.captures.owners.len()
        + facts.hierarchy_overrides.len()
        + facts.closed_hierarchy_facts.len()
        + facts.object_safety_facts.len()
        + facts.type_capability_overrides.len()
        + facts.field_nullability_facts.len()
        + facts.trivial_accessor_facts.len()
        + facts.external_api_effect_facts.len()
        + facts.external_api_argument_effect_facts.len()
        + facts.external_crate_api_mapping_facts.len()
        + facts.external_crate_constructor_mapping_facts.len()
        + facts.external_constructor_facts.len()
        + facts.external_field_facts.len()
        + facts.method_effect_facts.len()
        + facts.recursion_facts.len()
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

fn fact_count_excluding_provider_facts(facts: &FactSet, provider_facts: &FactSet) -> usize {
    fact_count(facts).saturating_sub(fact_count(provider_facts))
}

fn java_type_key(java_ty: &crate::java_ir::JavaType) -> Option<String> {
    match java_ty {
        crate::java_ir::JavaType::Named { name, .. } => {
            Some(crate::stdlib::canonical_external_receiver_type_key(name))
        }
        crate::java_ir::JavaType::String => Some("String".to_string()),
        crate::java_ir::JavaType::Array(_) => Some("Array".to_string()),
        crate::java_ir::JavaType::Unknown(name) => {
            Some(crate::stdlib::canonical_external_receiver_type_key(name))
        }
        crate::java_ir::JavaType::Void
        | crate::java_ir::JavaType::Primitive(_)
        | crate::java_ir::JavaType::Wildcard(_) => None,
    }
}

fn java_type_raw_name(java_ty: &crate::java_ir::JavaType) -> Option<&str> {
    match java_ty {
        crate::java_ir::JavaType::Named { name, .. } | crate::java_ir::JavaType::Unknown(name) => {
            Some(name)
        }
        crate::java_ir::JavaType::String
        | crate::java_ir::JavaType::Array(_)
        | crate::java_ir::JavaType::Void
        | crate::java_ir::JavaType::Primitive(_)
        | crate::java_ir::JavaType::Wildcard(_) => None,
    }
}

fn unresolved_method_receiver_type_key(
    program: &Program,
    fact: &crate::facts::UnresolvedTargetFact,
) -> Option<String> {
    let receiver_type = fact.receiver_type.as_ref()?;
    if let JavaType::Named { name, args } = receiver_type
        && args.is_empty()
        && let Some(bound) = unresolved_target_type_param_bound(program, fact, name)
    {
        return java_type_key(&bound);
    }
    java_type_key(receiver_type)
}

fn unresolved_target_type_param_bound(
    program: &Program,
    fact: &crate::facts::UnresolvedTargetFact,
    type_param: &str,
) -> Option<JavaType> {
    let ty = find_type_decl(program, &fact.enclosing_type)?;
    type_param_bound_from_decl(&ty.type_params, &ty.type_param_bounds, type_param).or_else(|| {
        let member_name = fact.enclosing_member.as_deref()?;
        type_param_bound_from_member(ty, member_name, type_param)
    })
}

fn type_param_bound_from_member(
    ty: &TypeDecl,
    member_name: &str,
    type_param: &str,
) -> Option<JavaType> {
    for member in &ty.members {
        match member {
            Member::Method(method) if method.name == member_name => {
                if let Some(bound) = type_param_bound_from_decl(
                    &method.type_params,
                    &method.type_param_bounds,
                    type_param,
                ) {
                    return Some(bound);
                }
            }
            Member::Constructor(constructor) if member_name == "<init>" => {
                if let Some(bound) = type_param_bound_from_decl(
                    &constructor.type_params,
                    &constructor.type_param_bounds,
                    type_param,
                ) {
                    return Some(bound);
                }
            }
            _ => {}
        }
    }
    None
}

fn type_param_bound_from_decl(
    type_params: &[String],
    type_param_bounds: &[Option<JavaType>],
    type_param: &str,
) -> Option<JavaType> {
    type_params
        .iter()
        .zip(type_param_bounds.iter())
        .find_map(|(name, bound)| (name == type_param).then(|| bound.clone()).flatten())
}

fn find_type_decl<'a>(program: &'a Program, type_name: &str) -> Option<&'a TypeDecl> {
    program
        .types
        .iter()
        .find_map(|ty| find_type_decl_in_type(ty, type_name))
}

fn find_type_decl_in_type<'a>(ty: &'a TypeDecl, type_name: &str) -> Option<&'a TypeDecl> {
    if ty.name == type_name {
        return Some(ty);
    }
    ty.members.iter().find_map(|member| match member {
        Member::Type(nested) => find_type_decl_in_type(nested, type_name),
        _ => None,
    })
}

fn java_type_summary(java_ty: &crate::java_ir::JavaType) -> String {
    match java_ty {
        crate::java_ir::JavaType::Named { name, args } if args.is_empty() => name.clone(),
        crate::java_ir::JavaType::Named { name, args } => format!(
            "{}<{}>",
            name,
            args.iter()
                .map(java_type_summary)
                .collect::<Vec<_>>()
                .join(",")
        ),
        crate::java_ir::JavaType::String => "String".to_string(),
        crate::java_ir::JavaType::Array(element) => {
            format!("{}[]", java_type_summary(element))
        }
        crate::java_ir::JavaType::Unknown(name) => format!("?{name}"),
        crate::java_ir::JavaType::Primitive(primitive) => format!("{primitive:?}"),
        crate::java_ir::JavaType::Void => "void".to_string(),
        crate::java_ir::JavaType::Wildcard(None) => "?".to_string(),
        crate::java_ir::JavaType::Wildcard(Some(bound)) => match bound {
            crate::java_ir::WildcardBound::Extends(ty) => {
                format!("? extends {}", java_type_summary(ty))
            }
            crate::java_ir::WildcardBound::Super(ty) => {
                format!("? super {}", java_type_summary(ty))
            }
        },
    }
}
