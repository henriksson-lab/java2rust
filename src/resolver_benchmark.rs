use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use crate::dependency_mode::{DependencyMode, DependencyModeConfig};
use crate::diagnostics::Severity;
use crate::facts::{ExternalLibraryMode, FactProvenance, FactSet, UnresolvedTargetKind};
use crate::java_ir::{
    Block, Expr, ImportDecl, JavaType, LambdaBody, Member, Param, Program, Stmt, TypeDecl,
};
use crate::parser::ParsedProgram;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResolverBenchmarkReport {
    pub root: PathBuf,
    pub source_roots: Vec<ResolverBenchmarkSourceRoot>,
    pub fact_providers: Vec<ResolverBenchmarkFactProvider>,
    pub files_seen: usize,
    pub files_parsed_without_errors: usize,
    pub files_resolved_without_errors: usize,
    pub parser_diagnostics: usize,
    pub resolver_diagnostics: usize,
    pub semantic_diagnostics: usize,
    pub top_resolver_diagnostics: Vec<DiagnosticSummary>,
    pub unresolved_method_calls: usize,
    pub external_api_explained_method_calls: usize,
    pub unexplained_method_calls: usize,
    pub unresolved_constructors: usize,
    pub external_api_explained_constructors: usize,
    pub anonymous_class_constructors: usize,
    pub unexplained_constructors: usize,
    pub unexplained_source_constructors: usize,
    pub unexplained_non_source_constructors: usize,
    pub unresolved_constructor_delegations: usize,
    pub unresolved_field_accesses: usize,
    pub external_api_explained_field_accesses: usize,
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
    pub external_api_return_nullability_facts: usize,
    pub total_facts: usize,
    pub top_external_api_guess_methods: Vec<ExternalApiGuessSummary>,
    pub top_external_pack_api_methods: Vec<ExternalApiPackSummary>,
    pub top_external_pack_constructor_types: Vec<ExternalPackConstructorSummary>,
    pub top_external_constructor_guess_types: Vec<ExternalConstructorGuessSummary>,
    pub top_project_constructor_guess_types: Vec<ExternalConstructorGuessSummary>,
    pub top_source_constructor_guess_types: Vec<ExternalConstructorGuessSummary>,
    pub top_external_field_guess_targets: Vec<ExternalFieldGuessSummary>,
    pub top_external_field_guess_contexts: Vec<UnresolvedFieldContextSummary>,
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
pub struct ExternalApiGuessSummary {
    pub receiver_type: String,
    pub method_name: String,
    pub count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalApiPackSummary {
    pub receiver_type: String,
    pub method_name: String,
    pub count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalPackConstructorSummary {
    pub type_name: String,
    pub arity: usize,
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
    pub count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticSummary {
    pub code: &'static str,
    pub message: String,
    pub count: usize,
    pub sample_files: Vec<String>,
}

pub fn benchmark_java_resolver_dir(root: &Path) -> std::io::Result<ResolverBenchmarkReport> {
    benchmark_java_resolver_config(&DependencyModeConfig::for_primary_source_root(root))
}

pub fn benchmark_java_resolver_config(
    config: &DependencyModeConfig,
) -> std::io::Result<ResolverBenchmarkReport> {
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
            })
            .collect(),
        ..ResolverBenchmarkReport::default()
    };
    let mut external_facts = FactSet::default();
    for provider_facts in config.fact_provider_facts() {
        external_facts.extend_from(provider_facts.clone());
    }
    let mut parsed_files = Vec::new();
    for source_root in config.source_roots_for_source_index() {
        let source_root_index = config
            .source_roots
            .iter()
            .position(|root| root.path == source_root.path && root.mode == source_root.mode);
        for path in java_files_under(&source_root.path)? {
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
    let source_programs = combined_source_programs_by_package(&parsed_files);
    let source_index = SourceTypeIndex::from_programs(&source_programs);
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
            &mut report,
            &parsed_file.file_label,
        );
    }
    Ok(report)
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
    benchmark_parsed_java_file(
        parsed,
        &source_symbols,
        &source_type_names,
        None,
        &FactSet::default(),
        report,
        &file_label,
    );
    Ok(())
}

#[derive(Debug, Clone)]
struct ParsedJavaFile {
    parsed: ParsedProgram,
    mode: DependencyMode,
    file_label: String,
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
        program.imports.iter().find_map(|import| {
            if import.is_static || import.is_wildcard {
                return None;
            }
            (import.path.rsplit('.').next() == Some(simple_name))
                .then(|| self.by_qualified_name.contains_key(&import.path))
        })
    }

    fn explicit_import_project_ownership(&self, program: &Program, type_name: &str) -> bool {
        let simple_name = type_name.rsplit('.').next().unwrap_or(type_name);
        program.imports.iter().any(|import| {
            !import.is_static
                && !import.is_wildcard
                && import.path.rsplit('.').next() == Some(simple_name)
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
        collect_type_name_dependencies(&crate::parser::parse_type(super_type), &mut out);
    }
    for interface in &ty.implements {
        collect_type_name_dependencies(&crate::parser::parse_type(interface), &mut out);
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
        .or_else(|| {
            crate::external_packs::external_library_pack_for_type(type_name).map(|pack| pack.mode)
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
    report: &mut ResolverBenchmarkReport,
    file_label: &str,
) {
    let resolved = crate::resolve::resolve_program_with_source_symbols(
        &parsed.program,
        parsed.diagnostics,
        source_symbols,
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
        );
    }
    if !resolved.diagnostics.has_errors() {
        report.files_resolved_without_errors += 1;
    }

    let semantic = crate::semantic_analysis::analyze_program(
        &resolved.program,
        &resolved.facts,
        external_facts,
    );
    report.semantic_diagnostics += semantic.diagnostics.len();
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
        .count();
    report.external_pack_constructor_facts += semantic
        .facts
        .external_constructor_facts
        .iter()
        .filter(|fact| fact.provenance == FactProvenance::ExternalLibraryPack)
        .count();
    for fact in semantic
        .facts
        .external_api_effect_facts
        .iter()
        .filter(|fact| fact.provenance == FactProvenance::ExternalLibraryPack)
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
    {
        if let Some(mode) = external_pack_mode_for_type(&semantic.facts, &fact.type_name) {
            increment_external_pack_constructor_mode_count(report, mode);
        }
    }
    for fact in semantic
        .facts
        .external_constructor_facts
        .iter()
        .filter(|fact| fact.provenance == FactProvenance::ConservativeFallback)
    {
        if is_source_type_reference(
            source_type_names,
            source_index,
            &resolved.program,
            &fact.type_name,
        ) {
            report.source_constructor_guesses += 1;
            push_external_constructor_guess_summary(
                &mut report.top_source_constructor_guess_types,
                &fact.type_name,
                fact.arg_types.len(),
            );
        } else if is_project_type_reference(source_index, &resolved.program, &fact.type_name) {
            report.project_constructor_guesses += 1;
            push_external_constructor_guess_summary(
                &mut report.top_project_constructor_guess_types,
                &fact.type_name,
                fact.arg_types.len(),
            );
        } else {
            report.external_constructor_guesses += 1;
            push_external_constructor_guess_summary(
                &mut report.top_external_constructor_guess_types,
                &fact.type_name,
                fact.arg_types.len(),
            );
        }
    }
    report.external_field_guesses += semantic
        .facts
        .external_field_facts
        .iter()
        .filter(|fact| fact.provenance == FactProvenance::ConservativeFallback)
        .count();
    report.external_api_return_nullability_facts += semantic
        .facts
        .external_api_effect_facts
        .iter()
        .filter(|fact| fact.return_nullability != crate::facts::NullabilityState::Unknown)
        .count();
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
    for fact in semantic
        .facts
        .external_api_effect_facts
        .iter()
        .filter(|fact| fact.provenance == FactProvenance::ExternalLibraryPack)
    {
        push_external_pack_api_summary(
            &mut report.top_external_pack_api_methods,
            fact.receiver_type.as_deref().unwrap_or("*"),
            &fact.method_name,
        );
    }
    for fact in semantic
        .facts
        .external_constructor_facts
        .iter()
        .filter(|fact| fact.provenance == FactProvenance::ExternalLibraryPack)
    {
        push_external_pack_constructor_summary(
            &mut report.top_external_pack_constructor_types,
            &fact.type_name,
            fact.arg_types.len(),
        );
    }
    for fact in semantic
        .facts
        .external_field_facts
        .iter()
        .filter(|fact| fact.provenance == FactProvenance::ConservativeFallback)
    {
        push_external_field_guess_summary(
            &mut report.top_external_field_guess_targets,
            &fact.field.type_name,
            &fact.field.field_name,
        );
    }
    report.total_facts += fact_count(&semantic.facts);

    for fact in &resolved.facts.unresolved_target_facts {
        match fact.kind {
            UnresolvedTargetKind::MethodCall => {
                report.unresolved_method_calls += 1;
                let receiver_type = unresolved_method_receiver_type_key(&resolved.program, fact);
                if let Some(effect) = semantic
                    .facts
                    .external_api_effect_for_receiver(receiver_type.as_deref(), &fact.target_name)
                {
                    report.external_api_explained_method_calls += 1;
                    if effect.provenance == FactProvenance::ExternalLibraryPack {
                        report.external_pack_explained_method_calls += 1;
                        if let Some(mode) = receiver_type.as_deref().and_then(|receiver_type| {
                            external_pack_mode_for_type(&semantic.facts, receiver_type)
                        }) {
                            increment_external_pack_explained_method_count(report, mode);
                        }
                    }
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
                    if constructor.provenance == FactProvenance::ExternalLibraryPack {
                        report.external_pack_explained_constructors += 1;
                        if let Some(mode) =
                            external_pack_mode_for_type(&semantic.facts, &target_type_name)
                        {
                            increment_external_pack_explained_constructor_count(report, mode);
                        }
                    }
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
                    if external_field.provenance == FactProvenance::ConservativeFallback {
                        push_unresolved_field_context_summary(
                            &mut report.top_external_field_guess_contexts,
                            &fact.target_name,
                            &receiver_type,
                            &fact.enclosing_type,
                            fact.enclosing_member.as_deref().unwrap_or("*"),
                            file_label,
                        );
                    }
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
    });
    report.top_external_pack_api_methods.truncate(20);
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
        });
    report.top_external_pack_constructor_types.truncate(20);
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
        .top_anonymous_constructor_targets
        .sort_by(|left, right| {
            right
                .count
                .cmp(&left.count)
                .then_with(|| left.type_name.cmp(&right.type_name))
                .then_with(|| left.arity.cmp(&right.arity))
        });
    report.top_anonymous_constructor_targets.truncate(20);
    report
        .top_external_field_guess_targets
        .sort_by(|left, right| {
            right
                .count
                .cmp(&left.count)
                .then_with(|| left.type_name.cmp(&right.type_name))
                .then_with(|| left.field_name.cmp(&right.field_name))
        });
    report.top_external_field_guess_targets.truncate(20);
    report
        .top_external_field_guess_contexts
        .sort_by(|left, right| {
            right
                .count
                .cmp(&left.count)
                .then_with(|| left.field_name.cmp(&right.field_name))
                .then_with(|| left.receiver_type.cmp(&right.receiver_type))
                .then_with(|| left.enclosing_type.cmp(&right.enclosing_type))
                .then_with(|| left.enclosing_member.cmp(&right.enclosing_member))
        });
    report.top_external_field_guess_contexts.truncate(20);
    report.top_resolver_diagnostics.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then_with(|| left.code.cmp(right.code))
            .then_with(|| left.message.cmp(&right.message))
    });
    report.top_resolver_diagnostics.truncate(20);
    sort_and_truncate_target_summaries(&mut report.top_unresolved_constructor_targets);
    sort_and_truncate_target_summaries(&mut report.top_unexplained_constructor_targets);
    report
        .top_unexplained_constructor_contexts
        .sort_by(|left, right| {
            right
                .count
                .cmp(&left.count)
                .then_with(|| left.target_name.cmp(&right.target_name))
                .then_with(|| left.receiver_type.cmp(&right.receiver_type))
                .then_with(|| left.enclosing_type.cmp(&right.enclosing_type))
                .then_with(|| left.enclosing_member.cmp(&right.enclosing_member))
                .then_with(|| left.arg_types.cmp(&right.arg_types))
        });
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
            "unresolved_method_calls\t{}",
            report.unresolved_method_calls
        ),
        format!(
            "external_api_explained_method_calls\t{}",
            report.external_api_explained_method_calls
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
        format!(
            "external_api_return_nullability_facts\t{}",
            report.external_api_return_nullability_facts
        ),
        format!("total_facts\t{}", report.total_facts),
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
        for provider in &report.fact_providers {
            lines.push(format!(
                "{}\t{:?}\t{}",
                provider.name, provider.mode, provider.facts
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
                "{}\t{}\t{}",
                target.receiver_type, target.method_name, target.count
            ));
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
                "{}\t{}\t{}",
                target.type_name, target.arity, target.count
            ));
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
    if !report.top_external_field_guess_contexts.is_empty() {
        lines.push("top_external_field_guess_contexts".to_string());
        for target in &report.top_external_field_guess_contexts {
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
            lines.push(format!(
                "{}\t{}\t{}\t{}\t{}\t{}\t{}",
                target.target_name,
                target.receiver_type,
                target.arg_types.join(","),
                target.enclosing_type,
                target.enclosing_member,
                target.count,
                target.sample_files.join(",")
            ));
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
) {
    if let Some(summary) = summaries.iter_mut().find(|summary| {
        summary.receiver_type == receiver_type && summary.method_name == method_name
    }) {
        summary.count += 1;
    } else {
        summaries.push(ExternalApiPackSummary {
            receiver_type: receiver_type.to_string(),
            method_name: method_name.to_string(),
            count: 1,
        });
    }
}

fn push_external_pack_constructor_summary(
    summaries: &mut Vec<ExternalPackConstructorSummary>,
    type_name: &str,
    arity: usize,
) {
    if let Some(summary) = summaries
        .iter_mut()
        .find(|summary| summary.type_name == type_name && summary.arity == arity)
    {
        summary.count += 1;
    } else {
        summaries.push(ExternalPackConstructorSummary {
            type_name: type_name.to_string(),
            arity,
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

fn push_external_field_guess_summary(
    summaries: &mut Vec<ExternalFieldGuessSummary>,
    type_name: &str,
    field_name: &str,
) {
    if let Some(summary) = summaries
        .iter_mut()
        .find(|summary| summary.type_name == type_name && summary.field_name == field_name)
    {
        summary.count += 1;
    } else {
        summaries.push(ExternalFieldGuessSummary {
            type_name: type_name.to_string(),
            field_name: field_name.to_string(),
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

fn push_diagnostic_summary(
    summaries: &mut Vec<DiagnosticSummary>,
    code: &'static str,
    message: &str,
    file_label: &str,
) {
    if let Some(summary) = summaries
        .iter_mut()
        .find(|summary| summary.code == code && summary.message == message)
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
            count: 1,
            sample_files: vec![file_label.to_string()],
        });
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
        + facts.external_constructor_facts.len()
        + facts.external_field_facts.len()
        + facts.method_effect_facts.len()
        + facts.recursion_facts.len()
        + facts.control_flow_facts.len()
        + facts.unresolved_target_facts.len()
        + facts.dispatch_requirement_facts.len()
        + facts.exception_facts.len()
        + facts.catch_facts.len()
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
