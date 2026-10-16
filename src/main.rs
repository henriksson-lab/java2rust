use std::collections::HashMap;
use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use new_java2rust::output_markers;

fn main() -> ExitCode {
    let Some(args) = parse_args(env::args().skip(1)) else {
        eprintln!(
            "usage: new-java2rust [--context-dir dir] [--context-dir-emitted] [--context-source-summary-dir dir] [--context-dir-package dir:package.prefix] [--facts facts.txt] [--external-pack-manifest pack.txt] <file.java>\n       new-java2rust translate-batch --primary dir [--primary-file relative/or/absolute.java ...] [--emit-primary-file relative/or/absolute.java ...] --out-dir dir [--generated-dir dir] [--jobs n, default 10] [--phase-events path] [--worker-events path] [--context-dir dir] [--context-dir-emitted] [--context-source-summary-dir dir] [--context-dir-package dir:package.prefix] [--facts facts.txt] [--external-pack-manifest pack.txt]\n       new-java2rust resolver-bench [--source-root mode:path] [--source-root-package mode:path:package.prefix] [--no-builtin-external-packs] [--builtin-external-pack name] [--facts facts.txt] [--external-pack-manifest pack.txt] <dir>\n       new-java2rust count-invalid-patterns <file.rs>"
        );
        return ExitCode::from(2);
    };

    if let Args::CountInvalidPatterns { rust_path } = args {
        return match fs::read_to_string(&rust_path) {
            Ok(rust) => {
                println!("{}", output_markers::count_invalid_patterns(&rust));
                ExitCode::SUCCESS
            }
            Err(err) => {
                eprintln!("failed to read {rust_path}: {err}");
                ExitCode::from(1)
            }
        };
    }

    if let Args::ResolverBench {
        root_path,
        external_inputs,
        source_roots,
    } = args
    {
        let mut config =
            new_java2rust::dependency_mode::DependencyModeConfig::for_primary_source_root(
                &root_path,
            );
        for source_root in source_roots {
            if source_root.package_prefixes.is_empty() {
                config.add_source_root(source_root.path, source_root.mode);
            } else {
                config.add_source_root_with_package_prefixes(
                    source_root.path,
                    source_root.mode,
                    source_root.package_prefixes,
                );
            }
        }
        config.set_builtin_external_packs_enabled(external_inputs.enable_builtin_external_packs);
        let fact_diagnostics = match load_external_fact_providers(&external_inputs, &mut config) {
            Ok(diagnostics) => diagnostics,
            Err(err) => {
                eprintln!("{err}");
                return ExitCode::from(1);
            }
        };
        print_diagnostics(&fact_diagnostics);
        return match new_java2rust::resolver_benchmark::benchmark_java_resolver_config(&config) {
            Ok(report) => {
                println!(
                    "{}",
                    new_java2rust::resolver_benchmark::render_resolver_benchmark_report(&report)
                );
                ExitCode::SUCCESS
            }
            Err(err) => {
                eprintln!("failed to benchmark {root_path}: {err}");
                ExitCode::from(1)
            }
        };
    }

    if let Args::TranslateBatch {
        primary_path,
        primary_files,
        emit_primary_files,
        out_dir,
        generated_dir,
        external_inputs,
        context_dirs,
        context_summary_dirs,
        context_dir_packages,
        context_dirs_are_emitted,
        jobs,
        phase_events_path,
        worker_events_path,
    } = args
    {
        return match run_translate_batch(
            &primary_path,
            &primary_files,
            &emit_primary_files,
            &out_dir,
            generated_dir.as_deref(),
            &external_inputs,
            &context_dirs,
            &context_summary_dirs,
            &context_dir_packages,
            context_dirs_are_emitted,
            jobs,
            phase_events_path.as_deref(),
            worker_events_path.as_deref(),
        ) {
            Ok(()) => ExitCode::SUCCESS,
            Err(err) => {
                eprintln!("{err}");
                ExitCode::from(1)
            }
        };
    }

    let Args::Translate {
        source_path,
        external_inputs,
        context_dirs,
        context_summary_dirs,
        context_dir_packages,
        context_dirs_are_emitted,
    } = args
    else {
        unreachable!();
    };

    let source = match fs::read_to_string(&source_path) {
        Ok(source) => source,
        Err(err) => {
            eprintln!("failed to read {}: {err}", source_path);
            return ExitCode::from(1);
        }
    };

    let (external_facts, fact_diagnostics) = match load_external_inputs(&external_inputs) {
        Ok(inputs) => inputs,
        Err(err) => {
            eprintln!("{err}");
            return ExitCode::from(1);
        }
    };

    let context_sources = match load_context_sources(&context_dirs, &[]) {
        Ok(sources) => sources,
        Err(err) => {
            eprintln!("{err}");
            return ExitCode::from(1);
        }
    };
    let context_summary_sources =
        match load_context_sources(&context_summary_dirs, &context_dir_packages) {
            Ok(sources) => sources,
            Err(err) => {
                eprintln!("{err}");
                return ExitCode::from(1);
            }
        };
    let translation = if context_sources.is_empty() && context_summary_sources.is_empty() {
        new_java2rust::translate_source_with_facts(&source, &external_facts)
    } else {
        new_java2rust::translate_source_with_context_sources_and_summary_sources(
            &source,
            &context_sources,
            &context_summary_sources,
            context_dirs_are_emitted,
            &external_facts,
        )
    };
    print_diagnostics(&fact_diagnostics);
    print_diagnostics(&translation.diagnostics);
    println!("{}", translation.rust);
    if fact_diagnostics.has_errors() || translation.diagnostics.has_errors() {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Args {
    Translate {
        source_path: String,
        external_inputs: ExternalInputPaths,
        context_dirs: Vec<String>,
        context_summary_dirs: Vec<String>,
        context_dir_packages: Vec<ContextDirPackage>,
        context_dirs_are_emitted: bool,
    },
    TranslateBatch {
        primary_path: String,
        primary_files: Vec<String>,
        emit_primary_files: Vec<String>,
        out_dir: String,
        generated_dir: Option<String>,
        external_inputs: ExternalInputPaths,
        context_dirs: Vec<String>,
        context_summary_dirs: Vec<String>,
        context_dir_packages: Vec<ContextDirPackage>,
        context_dirs_are_emitted: bool,
        jobs: usize,
        phase_events_path: Option<String>,
        worker_events_path: Option<String>,
    },
    ResolverBench {
        root_path: String,
        external_inputs: ExternalInputPaths,
        source_roots: Vec<CliSourceRoot>,
    },
    CountInvalidPatterns {
        rust_path: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CliSourceRoot {
    mode: new_java2rust::dependency_mode::DependencyMode,
    path: String,
    package_prefixes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ContextDirPackage {
    path: String,
    package_prefix: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ExternalInputPaths {
    facts_paths: Vec<String>,
    pack_manifest_paths: Vec<String>,
    builtin_pack_names: Vec<String>,
    enable_builtin_external_packs: bool,
}

impl Default for ExternalInputPaths {
    fn default() -> Self {
        Self {
            facts_paths: Vec::new(),
            pack_manifest_paths: Vec::new(),
            builtin_pack_names: Vec::new(),
            enable_builtin_external_packs: true,
        }
    }
}

fn parse_args(args: impl Iterator<Item = String>) -> Option<Args> {
    let mut source_path = None;
    let mut external_inputs = ExternalInputPaths::default();
    let mut context_dirs = Vec::new();
    let mut context_summary_dirs = Vec::new();
    let mut context_dir_packages = Vec::new();
    let mut context_dirs_are_emitted = false;
    let mut args = args.peekable();
    if args
        .peek()
        .is_some_and(|arg| arg == "count-invalid-patterns")
    {
        args.next();
        let rust_path = args.next()?;
        if args.next().is_some() {
            return None;
        }
        return Some(Args::CountInvalidPatterns { rust_path });
    }
    if args.peek().is_some_and(|arg| arg == "translate-batch") {
        args.next();
        let mut primary_path = None;
        let mut primary_files = Vec::new();
        let mut emit_primary_files = Vec::new();
        let mut out_dir = None;
        let mut generated_dir = None;
        let mut jobs = 10usize;
        let mut phase_events_path = None;
        let mut worker_events_path = None;
        while let Some(arg) = args.next() {
            if arg == "--primary" {
                primary_path = Some(args.next()?);
            } else if arg == "--primary-file" {
                primary_files.push(args.next()?);
            } else if arg == "--emit-primary-file" {
                emit_primary_files.push(args.next()?);
            } else if arg == "--out-dir" {
                out_dir = Some(args.next()?);
            } else if arg == "--generated-dir" {
                generated_dir = Some(args.next()?);
            } else if arg == "--jobs" {
                jobs = parse_positive_usize(&args.next()?)?;
            } else if arg == "--phase-events" {
                phase_events_path = Some(args.next()?);
            } else if arg == "--worker-events" {
                worker_events_path = Some(args.next()?);
            } else if arg == "--facts" {
                external_inputs.facts_paths.push(args.next()?);
            } else if arg == "--external-pack-manifest" {
                external_inputs.pack_manifest_paths.push(args.next()?);
            } else if arg == "--builtin-external-pack" {
                external_inputs.builtin_pack_names.push(args.next()?);
            } else if arg == "--context-dir" {
                context_dirs.push(args.next()?);
            } else if arg == "--context-dir-emitted" {
                context_dirs_are_emitted = true;
            } else if arg == "--context-source-summary-dir" {
                context_summary_dirs.push(args.next()?);
            } else if arg == "--context-dir-package" {
                context_dir_packages.push(parse_context_dir_package_spec(&args.next()?)?);
            } else if arg == "--no-builtin-external-packs" {
                external_inputs.enable_builtin_external_packs = false;
            } else {
                return None;
            }
        }
        return Some(Args::TranslateBatch {
            primary_path: primary_path?,
            primary_files,
            emit_primary_files,
            out_dir: out_dir?,
            generated_dir,
            external_inputs,
            context_dirs,
            context_summary_dirs,
            context_dir_packages,
            context_dirs_are_emitted,
            jobs,
            phase_events_path,
            worker_events_path,
        });
    }
    if args.peek().is_some_and(|arg| arg == "resolver-bench") {
        args.next();
        let mut root_path = None;
        let mut source_roots = Vec::new();
        while let Some(arg) = args.next() {
            if arg == "--facts" {
                external_inputs.facts_paths.push(args.next()?);
            } else if arg == "--external-pack-manifest" {
                external_inputs.pack_manifest_paths.push(args.next()?);
            } else if arg == "--builtin-external-pack" {
                external_inputs.builtin_pack_names.push(args.next()?);
            } else if arg == "--no-builtin-external-packs" {
                external_inputs.enable_builtin_external_packs = false;
            } else if arg == "--source-root" {
                source_roots.push(parse_source_root_spec(&args.next()?)?);
            } else if arg == "--source-root-package" {
                source_roots.push(parse_source_root_package_spec(&args.next()?)?);
            } else if root_path.is_none() {
                root_path = Some(arg);
            } else {
                return None;
            }
        }
        return Some(Args::ResolverBench {
            root_path: root_path?,
            external_inputs,
            source_roots,
        });
    }
    while let Some(arg) = args.next() {
        if arg == "--facts" {
            external_inputs.facts_paths.push(args.next()?);
        } else if arg == "--external-pack-manifest" {
            external_inputs.pack_manifest_paths.push(args.next()?);
        } else if arg == "--builtin-external-pack" {
            external_inputs.builtin_pack_names.push(args.next()?);
        } else if arg == "--context-dir" {
            context_dirs.push(args.next()?);
        } else if arg == "--context-dir-emitted" {
            context_dirs_are_emitted = true;
        } else if arg == "--context-source-summary-dir" {
            context_summary_dirs.push(args.next()?);
        } else if arg == "--context-dir-package" {
            context_dir_packages.push(parse_context_dir_package_spec(&args.next()?)?);
        } else if arg == "--no-builtin-external-packs" {
            external_inputs.enable_builtin_external_packs = false;
        } else if source_path.is_none() {
            source_path = Some(arg);
        } else {
            return None;
        }
    }
    Some(Args::Translate {
        source_path: source_path?,
        external_inputs,
        context_dirs,
        context_summary_dirs,
        context_dir_packages,
        context_dirs_are_emitted,
    })
}

fn parse_source_root_spec(spec: &str) -> Option<CliSourceRoot> {
    let (mode, path) = spec.split_once(':')?;
    if path.is_empty() {
        return None;
    }
    Some(CliSourceRoot {
        mode: parse_dependency_mode(mode)?,
        path: path.to_string(),
        package_prefixes: Vec::new(),
    })
}

fn parse_source_root_package_spec(spec: &str) -> Option<CliSourceRoot> {
    let (mode, rest) = spec.split_once(':')?;
    let (path, package_prefix) = rest.rsplit_once(':')?;
    if path.is_empty() || package_prefix.is_empty() {
        return None;
    }
    Some(CliSourceRoot {
        mode: parse_dependency_mode(mode)?,
        path: path.to_string(),
        package_prefixes: vec![package_prefix.to_string()],
    })
}

fn parse_context_dir_package_spec(spec: &str) -> Option<ContextDirPackage> {
    let (path, package_prefix) = spec.rsplit_once(':')?;
    if path.is_empty() || package_prefix.is_empty() {
        return None;
    }
    Some(ContextDirPackage {
        path: path.to_string(),
        package_prefix: package_prefix.to_string(),
    })
}

fn parse_dependency_mode(mode: &str) -> Option<new_java2rust::dependency_mode::DependencyMode> {
    use new_java2rust::dependency_mode::DependencyMode;
    match mode {
        "primary" | "primary-source" | "primary_source" => Some(DependencyMode::PrimarySource),
        "co-translate" | "co_translate" | "co-translate-source" | "co_translate_source" => {
            Some(DependencyMode::CoTranslateSource)
        }
        "source-summary" | "source_summary" | "summary" => Some(DependencyMode::SourceSummary),
        _ => None,
    }
}

fn parse_positive_usize(value: &str) -> Option<usize> {
    let value = value.parse::<usize>().ok()?;
    (value > 0).then_some(value)
}

fn load_context_sources(
    context_dirs: &[String],
    context_dir_packages: &[ContextDirPackage],
) -> Result<Vec<String>, String> {
    let mut paths = Vec::new();
    for context_dir in context_dirs {
        collect_java_paths(context_dir, &mut paths)?;
    }
    for context_dir_package in context_dir_packages {
        collect_java_paths_for_package(
            &context_dir_package.path,
            &context_dir_package.package_prefix,
            &mut paths,
        )?;
    }
    paths.sort();
    paths.dedup();
    paths
        .into_iter()
        .map(|path| {
            fs::read_to_string(&path).map_err(|err| format!("failed to read {path}: {err}"))
        })
        .collect()
}

fn load_batch_sources(path: &str) -> Result<Vec<new_java2rust::BatchSource>, String> {
    let mut paths = Vec::new();
    collect_java_paths(path, &mut paths)?;
    paths.sort();
    paths.dedup();
    paths
        .into_iter()
        .map(|path| {
            fs::read_to_string(&path)
                .map(|source| new_java2rust::BatchSource::new(path.clone(), source))
                .map_err(|err| format!("failed to read {path}: {err}"))
        })
        .collect()
}

fn load_selected_batch_sources(
    primary_path: &str,
    primary_files: &[String],
) -> Result<Vec<new_java2rust::BatchSource>, String> {
    if primary_files.is_empty() {
        return load_batch_sources(primary_path);
    }
    let primary_root = Path::new(primary_path);
    let mut paths = primary_files
        .iter()
        .map(|path| {
            let path = Path::new(path);
            if path.is_absolute() {
                path.to_path_buf()
            } else {
                primary_root.join(path)
            }
        })
        .collect::<Vec<_>>();
    paths.sort();
    paths.dedup();
    paths
        .into_iter()
        .map(|path| {
            if path.extension().and_then(|value| value.to_str()) != Some("java") {
                return Err(format!(
                    "selected primary is not a Java source file: {}",
                    path.display()
                ));
            }
            fs::read_to_string(&path)
                .map(|source| {
                    new_java2rust::BatchSource::new(path.to_string_lossy().into_owned(), source)
                })
                .map_err(|err| format!("failed to read {}: {err}", path.display()))
        })
        .collect()
}

fn run_translate_batch(
    primary_path: &str,
    primary_files: &[String],
    emit_primary_files: &[String],
    out_dir: &str,
    generated_dir_override: Option<&str>,
    external_inputs: &ExternalInputPaths,
    context_dirs: &[String],
    context_summary_dirs: &[String],
    context_dir_packages: &[ContextDirPackage],
    context_dirs_are_emitted: bool,
    jobs: usize,
    phase_events_path: Option<&str>,
    worker_events_path: Option<&str>,
) -> Result<(), String> {
    let command_start = Instant::now();
    fs::create_dir_all(out_dir).map_err(|err| format!("failed to create {out_dir}: {err}"))?;
    append_batch_phase_event(
        phase_events_path,
        "batch_command_start",
        "running",
        0,
        primary_path,
    )?;

    let input_start = Instant::now();
    let primary_sources = load_selected_batch_sources(primary_path, primary_files)?;
    if primary_sources.is_empty() {
        return Err(format!("no Java files found under: {primary_path}"));
    }
    append_batch_phase_event(
        phase_events_path,
        "batch_input_collection",
        "ok",
        input_start.elapsed().as_millis(),
        &format!("{} primary files", primary_sources.len()),
    )?;

    let fact_start = Instant::now();
    let (external_facts, fact_diagnostics) = load_external_inputs(external_inputs)?;
    append_batch_phase_event(
        phase_events_path,
        "batch_external_facts",
        "ok",
        fact_start.elapsed().as_millis(),
        "loaded",
    )?;

    let context_load_start = Instant::now();
    let context_sources = load_context_sources(context_dirs, &[])?;
    let context_summary_sources = load_context_sources(context_summary_dirs, context_dir_packages)?;
    append_batch_phase_event(
        phase_events_path,
        "batch_context_load",
        "ok",
        context_load_start.elapsed().as_millis(),
        &format!(
            "{} context files, {} summary files",
            context_sources.len(),
            context_summary_sources.len()
        ),
    )?;

    let generated_dir = generated_dir_override
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(out_dir).join("generated"));
    fs::create_dir_all(&generated_dir)
        .map_err(|err| format!("failed to create {}: {err}", generated_dir.display()))?;

    append_batch_phase_event(
        phase_events_path,
        "batch_parse_global_emit_start",
        "running",
        command_start.elapsed().as_millis(),
        &format!("jobs={jobs}"),
    )?;
    let event_sink = worker_events_path.map(|event_path| {
        let event_path = event_path.to_string();
        let generated_dir = generated_dir.clone();
        let event_write_lock = Arc::new(Mutex::new(()));
        Arc::new(move |event: new_java2rust::BatchTranslationEvent| {
            let _guard = event_write_lock
                .lock()
                .expect("batch event write lock poisoned");
            let rust_path = generated_dir.join(batch_stem(event.index(), event.path()) + ".rs");
            if let Err(err) = append_batch_worker_event(
                Some(&event_path),
                event.phase(),
                event.index(),
                event.path(),
                event.elapsed_ms(),
                event.status(),
                &rust_path,
            ) {
                eprintln!("{err}");
            }
        }) as Arc<dyn Fn(new_java2rust::BatchTranslationEvent) + Send + Sync>
    });
    let checkpoint_error = Arc::new(Mutex::new(None::<String>));
    let completion_sink = {
        let generated_dir = generated_dir.clone();
        let checkpoint_error = Arc::clone(&checkpoint_error);
        Arc::new(
            move |index: usize, translation: &new_java2rust::BatchTranslation| {
                let rust_path = generated_dir.join(batch_stem(index, translation.path()) + ".rs");
                let temporary_path = rust_path.with_extension("rs.part");
                let result = fs::write(&temporary_path, translation.rust())
                    .and_then(|()| fs::rename(&temporary_path, &rust_path));
                if let Err(err) = result {
                    let message = format!(
                        "failed to checkpoint completed translation {}: {err}",
                        rust_path.display()
                    );
                    eprintln!("{message}");
                    *checkpoint_error
                        .lock()
                        .expect("batch checkpoint error lock poisoned") = Some(message);
                }
            },
        ) as Arc<new_java2rust::BatchTranslationCompletionSink>
    };
    let emission_paths = if emit_primary_files.is_empty() {
        None
    } else {
        let paths = load_selected_batch_sources(primary_path, emit_primary_files)?
            .into_iter()
            .map(|source| source.path().to_string())
            .collect::<Vec<_>>();
        let primary_paths = primary_sources
            .iter()
            .map(new_java2rust::BatchSource::path)
            .collect::<std::collections::BTreeSet<_>>();
        if let Some(path) = paths
            .iter()
            .find(|path| !primary_paths.contains(path.as_str()))
        {
            return Err(format!(
                "selected emission file is not in the analyzed primary set: {path}"
            ));
        }
        Some(paths)
    };
    let mut translation_options = new_java2rust::BatchTranslationOptions::new(jobs, event_sink)
        .with_completion_sink(completion_sink);
    if let Some(emission_paths) = emission_paths {
        translation_options = translation_options.with_emission_paths(emission_paths);
    }
    let report =
        new_java2rust::translate_batch_sources_with_context_sources_summary_sources_and_options(
            &primary_sources,
            &context_sources,
            &context_summary_sources,
            context_dirs_are_emitted,
            &external_facts,
            &translation_options,
        );
    if let Some(err) = checkpoint_error
        .lock()
        .expect("batch checkpoint error lock poisoned")
        .take()
    {
        return Err(err);
    }
    append_batch_phase_event(
        phase_events_path,
        "batch_primary_parse",
        "ok",
        report.timings().primary_parse_ms(),
        &format!("{} primary files", primary_sources.len()),
    )?;
    append_batch_phase_event(
        phase_events_path,
        "batch_context_parse",
        "ok",
        report.timings().context_parse_ms(),
        &format!("{} context files", context_sources.len()),
    )?;
    append_batch_phase_event(
        phase_events_path,
        "batch_summary_parse",
        "ok",
        report.timings().summary_parse_ms(),
        &format!("{} summary files", context_summary_sources.len()),
    )?;
    append_batch_phase_event(
        phase_events_path,
        "batch_global_analysis",
        "ok",
        report.timings().global_analysis_ms(),
        "resolver/semantic/representation/signature",
    )?;
    append_batch_phase_event(
        phase_events_path,
        "batch_global_resolve_context",
        "ok",
        report.timings().global_resolve_ms(),
        "resolve_program_with_source_program_and_facts",
    )?;
    append_batch_phase_event(
        phase_events_path,
        "batch_global_semantic",
        "ok",
        report.timings().global_semantic_ms(),
        "semantic_analysis::analyze_program",
    )?;
    append_batch_phase_event(
        phase_events_path,
        "batch_global_representation",
        "ok",
        report.timings().global_representation_ms(),
        "representation::plan_representations",
    )?;
    append_batch_phase_event(
        phase_events_path,
        "batch_global_signature",
        "ok",
        report.timings().global_signature_ms(),
        "signature::solve_signatures",
    )?;
    append_signature_profile_phase_events(phase_events_path, &report.analysis_profile().signature)?;
    append_batch_phase_event(
        phase_events_path,
        "batch_emission",
        "ok",
        report.timings().emission_ms(),
        &format!("jobs={jobs}"),
    )?;

    let status_path = Path::new(out_dir).join("status.tsv");
    let manifest_path = Path::new(out_dir).join("batch-manifest.tsv");
    let analysis_profile_path = Path::new(out_dir).join("analysis-profile.tsv");
    write_analysis_profile(
        &analysis_profile_path,
        report.analysis_profile(),
        report.timings(),
        report.translations(),
    )?;
    let mut status = String::from(
        "source\trust_path\ttranslate_status\tdiagnostics\tinvalid_patterns\tunsupported_members\tstubbed_constants\n",
    );
    let mut manifest = String::from(
        "index\tsource\trust_path\trust_hash\tdiagnostics\tinvalid_patterns\tstubbed_constants\n",
    );
    let write_start = Instant::now();
    for translation in report.translations() {
        let index = primary_sources
            .iter()
            .position(|source| source.path() == translation.path())
            .ok_or_else(|| {
                format!(
                    "emitted translation is not in the analyzed primary set: {}",
                    translation.path()
                )
            })?
            + 1;
        let rust_path = generated_dir.join(batch_stem(index, translation.path()) + ".rs");
        fs::write(&rust_path, translation.rust())
            .map_err(|err| format!("failed to write {}: {err}", rust_path.display()))?;
        for diagnostic in translation.diagnostics().iter() {
            eprintln!(
                "{:?} {}: {}",
                diagnostic.severity(),
                diagnostic.code(),
                diagnostic.message()
            );
        }
        let invalid_patterns = output_markers::count_invalid_patterns(translation.rust());
        let stubbed_constants = output_markers::count_stubbed_constants(translation.rust());
        let unsupported_members = translation
            .diagnostics()
            .iter()
            .filter(|diagnostic| diagnostic.code() == "unsupported_member")
            .count();
        let translate_status = translate_status_for_diagnostics(translation.diagnostics());
        status.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
            translation.path(),
            rust_path.display(),
            translate_status,
            translation.diagnostics().len(),
            invalid_patterns,
            unsupported_members,
            stubbed_constants
        ));
        manifest.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
            index,
            translation.path(),
            rust_path.display(),
            stable_hash_hex(translation.rust()),
            translation.diagnostics().len(),
            invalid_patterns,
            stubbed_constants
        ));
    }
    if worker_events_path.is_none() {
        for event in report.events() {
            append_batch_worker_event(
                worker_events_path,
                event.phase(),
                event.index(),
                event.path(),
                event.elapsed_ms(),
                event.status(),
                &generated_dir.join(batch_stem(event.index(), event.path()) + ".rs"),
            )?;
        }
    }
    fs::write(&status_path, status)
        .map_err(|err| format!("failed to write {}: {err}", status_path.display()))?;
    fs::write(&manifest_path, manifest)
        .map_err(|err| format!("failed to write {}: {err}", manifest_path.display()))?;
    append_batch_phase_event(
        phase_events_path,
        "batch_status_write",
        "ok",
        write_start.elapsed().as_millis(),
        &status_path.display().to_string(),
    )?;
    append_batch_phase_event(
        phase_events_path,
        "batch_command_end",
        "ok",
        command_start.elapsed().as_millis(),
        primary_path,
    )?;
    print_diagnostics(&fact_diagnostics);
    Ok(())
}

fn stable_hash_hex(value: &str) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in value.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

fn append_signature_profile_phase_events(
    path: Option<&str>,
    profile: &new_java2rust::signature::SignatureSolverProfile,
) -> Result<(), String> {
    append_batch_phase_event(
        path,
        "batch_global_signature_collect_constraints",
        "ok",
        profile.collect_constraints_ms(),
        "signature::collect_signature_constraints",
    )?;
    append_batch_phase_event(
        path,
        "batch_global_signature_dependency_graph",
        "ok",
        profile.dependency_graph_ms(),
        "signature::build_signature_dependency_graph",
    )?;
    append_batch_phase_event(
        path,
        "batch_global_signature_scc",
        "ok",
        profile.scc_analysis_ms(),
        "signature::analyze_signature_dependency_sccs",
    )?;
    append_batch_phase_event(
        path,
        "batch_global_signature_pass_plan",
        "ok",
        profile.pass_plan_ms(),
        "signature::plan_signature_solver_passes",
    )?;
    append_batch_phase_event(
        path,
        "batch_global_signature_fixpoint",
        "ok",
        profile.fixpoint_ms(),
        "signature constraint saturation",
    )?;
    append_batch_phase_event(
        path,
        "batch_global_signature_plan_build",
        "ok",
        profile.plan_build_ms(),
        "signature plan build",
    )
}

fn write_analysis_profile(
    path: &Path,
    profile: &new_java2rust::BatchAnalysisProfile,
    timings: &new_java2rust::BatchTranslationTimings,
    translations: &[new_java2rust::BatchTranslation],
) -> Result<(), String> {
    let mut out = String::from("metric\tvalue\n");
    push_profile_metric(&mut out, "primary_files", profile.primary_files());
    push_profile_metric(&mut out, "context_files", profile.context_files());
    push_profile_metric(&mut out, "summary_files", profile.summary_files());
    push_profile_metric(&mut out, "program_types", profile.program_types());
    push_profile_metric(&mut out, "program_methods", profile.program_methods());
    push_profile_metric(
        &mut out,
        "program_constructors",
        profile.program_constructors(),
    );
    push_profile_metric(&mut out, "program_fields", profile.program_fields());
    push_profile_metric(
        &mut out,
        "external_fact_count",
        profile.external_fact_count(),
    );
    push_profile_metric(&mut out, "global_analysis_ms", timings.global_analysis_ms());
    push_profile_metric(&mut out, "global_resolve_ms", timings.global_resolve_ms());
    push_profile_metric(&mut out, "global_semantic_ms", timings.global_semantic_ms());
    push_profile_metric(
        &mut out,
        "global_representation_ms",
        timings.global_representation_ms(),
    );
    push_profile_metric(
        &mut out,
        "global_signature_ms",
        timings.global_signature_ms(),
    );
    let signature = &profile.signature;
    push_profile_metric(
        &mut out,
        "signature_collect_constraints_ms",
        signature.collect_constraints_ms(),
    );
    push_profile_metric(
        &mut out,
        "signature_dependency_graph_ms",
        signature.dependency_graph_ms(),
    );
    push_profile_metric(
        &mut out,
        "signature_scc_analysis_ms",
        signature.scc_analysis_ms(),
    );
    push_profile_metric(&mut out, "signature_pass_plan_ms", signature.pass_plan_ms());
    push_profile_metric(&mut out, "signature_fixpoint_ms", signature.fixpoint_ms());
    push_profile_metric(
        &mut out,
        "signature_plan_build_ms",
        signature.plan_build_ms(),
    );
    push_profile_metric(&mut out, "signature_slot_count", signature.slot_count());
    push_profile_metric(
        &mut out,
        "signature_unique_constraint_count",
        signature.unique_constraint_count(),
    );
    push_profile_metric(
        &mut out,
        "signature_iteration_budget",
        signature.iteration_budget(),
    );
    push_profile_metric(
        &mut out,
        "signature_graph_node_count",
        signature.graph_node_count(),
    );
    push_profile_metric(
        &mut out,
        "signature_graph_edge_count",
        signature.graph_edge_count(),
    );
    push_profile_metric(
        &mut out,
        "signature_component_count",
        signature.component_count(),
    );
    push_profile_metric(
        &mut out,
        "signature_cyclic_component_count",
        signature.cyclic_component_count(),
    );
    push_profile_metric(
        &mut out,
        "signature_max_component_size",
        signature.max_component_size(),
    );
    push_profile_metric(
        &mut out,
        "signature_planned_components",
        signature.planned_components(),
    );
    push_profile_metric(
        &mut out,
        "signature_planned_mutable_components",
        signature.planned_mutable_components(),
    );
    push_profile_metric(
        &mut out,
        "signature_fixpoint_iterations",
        signature.fixpoint_iterations(),
    );
    push_profile_metric(&mut out, "signature_diagnostics", signature.diagnostics());
    push_profile_metric(
        &mut out,
        "signature_component_sweeps",
        signature.component_sweeps(),
    );
    push_profile_metric(
        &mut out,
        "signature_changed_component_sweeps",
        signature.changed_component_sweeps(),
    );
    push_profile_metric(&mut out, "signature_state_clones", signature.state_clones());
    push_profile_metric(
        &mut out,
        "signature_state_clone_ms",
        signature.state_clone_ms(),
    );
    push_profile_metric(
        &mut out,
        "signature_propagation_sweep_ms",
        signature.propagation_sweep_ms(),
    );
    push_profile_metric(
        &mut out,
        "signature_component_rollback_ms",
        signature.component_rollback_ms(),
    );
    push_profile_metric(
        &mut out,
        "signature_component_change_check_ms",
        signature.component_change_check_ms(),
    );
    for (label, arm) in signature.propagation_arms() {
        push_profile_metric(
            &mut out,
            &format!("signature_propagation_arm_{label}_sweeps"),
            arm.sweeps(),
        );
        push_profile_metric(
            &mut out,
            &format!("signature_propagation_arm_{label}_changed_sweeps"),
            arm.changed_sweeps(),
        );
        push_profile_metric(
            &mut out,
            &format!("signature_propagation_arm_{label}_ms"),
            arm.elapsed_ns() / 1_000_000,
        );
        push_profile_metric(
            &mut out,
            &format!("signature_propagation_arm_{label}_us"),
            arm.elapsed_ns() / 1_000,
        );
    }
    // A compile-error count cannot see silent failures: a no-op facade, an
    // `unimplemented!` body and an erased construct all lower cleanly while the
    // behaviour they stood for is gone, so erasing is scored as an improvement.
    // These counters put that cost back into the batch summary.
    let mut noop_facade_methods = 0;
    // The *cost* of a no-op facade, as distinct from its surface: one facade
    // constructor may be called from a hundred places
    // ([0018](../claude_audits/0018-the-compile-error-metric-cannot-see-silent-failures.md)).
    let mut noop_facade_call_sites = 0;
    let mut unimplemented_facade_methods = 0;
    let mut compile_error_markers = 0;
    for translation in translations {
        noop_facade_methods += output_markers::count_noop_facade_methods(translation.rust());
        noop_facade_call_sites += output_markers::count_noop_facade_call_sites(translation.rust());
        unimplemented_facade_methods +=
            output_markers::count_unimplemented_facade_methods(translation.rust());
        compile_error_markers += output_markers::count_compile_error_markers(translation.rust());
    }
    push_profile_metric(&mut out, "noop_facade_methods", noop_facade_methods);
    push_profile_metric(&mut out, "noop_facade_call_sites", noop_facade_call_sites);
    push_profile_metric(
        &mut out,
        "unimplemented_facade_methods",
        unimplemented_facade_methods,
    );
    push_profile_metric(&mut out, "compile_error_markers", compile_error_markers);
    fs::write(path, out).map_err(|err| format!("failed to write {}: {err}", path.display()))
}

fn push_profile_metric(out: &mut String, name: &str, value: impl std::fmt::Display) {
    out.push_str(name);
    out.push('\t');
    out.push_str(&value.to_string());
    out.push('\n');
}

fn append_batch_phase_event(
    path: Option<&str>,
    phase: &str,
    status: &str,
    elapsed_ms: u128,
    detail: &str,
) -> Result<(), String> {
    let Some(path) = path else {
        return Ok(());
    };
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|err| format!("failed to append {path}: {err}"))?;
    writeln!(file, "{phase}\t{status}\t{elapsed_ms}\t{detail}")
        .map_err(|err| format!("failed to write {path}: {err}"))
}

fn translate_status_for_diagnostics(
    diagnostics: &new_java2rust::diagnostics::DiagnosticBag,
) -> &'static str {
    if diagnostics.has_errors() {
        "errors"
    } else if diagnostics.is_empty() {
        "ok"
    } else {
        "diagnostics"
    }
}

fn append_batch_worker_event(
    path: Option<&str>,
    event: &str,
    index: usize,
    source: &str,
    elapsed_ms: u128,
    status: &str,
    rust_path: &Path,
) -> Result<(), String> {
    let Some(path) = path else {
        return Ok(());
    };
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|err| format!("failed to append {path}: {err}"))?;
    writeln!(
        file,
        "{event}\t{}\t{index}\t{source}\t{elapsed_ms}\t{status}\t{}",
        std::process::id(),
        rust_path.display()
    )
    .map_err(|err| format!("failed to write {path}: {err}"))
}

fn batch_stem(index: usize, java_file: &str) -> String {
    let base = Path::new(java_file)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("Source");
    sanitize_rust_file_stem(&format!("{index:05}_{base}"))
}

fn sanitize_rust_file_stem(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

fn collect_java_paths(path: &str, out: &mut Vec<String>) -> Result<(), String> {
    let metadata = fs::metadata(path).map_err(|err| format!("failed to stat {path}: {err}"))?;
    if metadata.is_file() {
        if path.ends_with(".java") {
            out.push(path.to_string());
        }
        return Ok(());
    }
    if !metadata.is_dir() {
        return Ok(());
    }
    let entries = fs::read_dir(path).map_err(|err| format!("failed to read dir {path}: {err}"))?;
    for entry in entries {
        let entry = entry.map_err(|err| format!("failed to read dir entry under {path}: {err}"))?;
        collect_java_paths(&entry.path().to_string_lossy(), out)?;
    }
    Ok(())
}

fn collect_java_paths_for_package(
    path: &str,
    package_prefix: &str,
    out: &mut Vec<String>,
) -> Result<(), String> {
    let mut candidates = Vec::new();
    let package_dir =
        package_prefix
            .split('.')
            .fold(Path::new(path).to_path_buf(), |mut dir, segment| {
                dir.push(segment);
                dir
            });
    if package_dir.is_dir() {
        collect_java_paths(&package_dir.to_string_lossy(), &mut candidates)?;
    } else {
        collect_java_paths(path, &mut candidates)?;
    }
    for candidate in candidates {
        let source = fs::read_to_string(&candidate)
            .map_err(|err| format!("failed to read {candidate}: {err}"))?;
        if java_source_package_matches(&source, package_prefix) {
            out.push(candidate);
        }
    }
    Ok(())
}

fn java_source_package_matches(source: &str, package_prefix: &str) -> bool {
    let Some(package_name) = java_source_package_name(source) else {
        return false;
    };
    package_name == package_prefix
        || package_name
            .strip_prefix(package_prefix)
            .is_some_and(|suffix| suffix.starts_with('.'))
}

fn java_source_package_name(source: &str) -> Option<&str> {
    for line in source.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with("//") {
            continue;
        }
        if let Some(rest) = line.strip_prefix("package ") {
            return rest.strip_suffix(';').map(str::trim);
        }
    }
    None
}

fn load_external_inputs(
    external_inputs: &ExternalInputPaths,
) -> Result<
    (
        new_java2rust::facts::FactSet,
        new_java2rust::diagnostics::DiagnosticBag,
    ),
    String,
> {
    let mut facts = new_java2rust::facts::FactSet::default();
    facts.disable_builtin_external_packs = !external_inputs.enable_builtin_external_packs;
    let mut diagnostics = new_java2rust::diagnostics::DiagnosticBag::default();

    for facts_path in &external_inputs.facts_paths {
        let fact_source = fs::read_to_string(facts_path)
            .map_err(|err| format!("failed to read {facts_path}: {err}"))?;
        let parsed_facts = new_java2rust::external_facts::parse_external_facts(&fact_source);
        let (parsed_fact_set, parsed_diagnostics) = parsed_facts.into_parts();
        diagnostics.extend(parsed_diagnostics);
        facts.extend_from(parsed_fact_set);
    }

    for manifest_path in &external_inputs.pack_manifest_paths {
        let manifest_source = fs::read_to_string(manifest_path)
            .map_err(|err| format!("failed to read {manifest_path}: {err}"))?;
        let parsed_manifest =
            new_java2rust::external_packs::parse_external_pack_manifest(&manifest_source);
        let (mut manifest_facts, manifest_diagnostics) = parsed_manifest.into_parts();
        diagnostics.extend(manifest_diagnostics);
        enrich_external_pack_facade_abi(manifest_path, &manifest_source, &mut manifest_facts);
        facts.extend_from(manifest_facts);
    }

    for pack_name in &external_inputs.builtin_pack_names {
        let pack_facts = new_java2rust::external_packs::builtin_external_pack_facts(pack_name)
            .ok_or_else(|| format!("unknown built-in external pack: {pack_name}"))?;
        facts.extend_from(pack_facts);
    }

    Ok((facts, diagnostics))
}

fn load_external_fact_providers(
    external_inputs: &ExternalInputPaths,
    config: &mut new_java2rust::dependency_mode::DependencyModeConfig,
) -> Result<new_java2rust::diagnostics::DiagnosticBag, String> {
    let mut diagnostics = new_java2rust::diagnostics::DiagnosticBag::default();

    for facts_path in &external_inputs.facts_paths {
        let fact_source = fs::read_to_string(facts_path)
            .map_err(|err| format!("failed to read {facts_path}: {err}"))?;
        let parsed_facts = new_java2rust::external_facts::parse_external_facts(&fact_source);
        let (parsed_fact_set, parsed_diagnostics) = parsed_facts.into_parts();
        diagnostics.extend(parsed_diagnostics);
        config.add_fact_provider(
            format!("facts:{facts_path}"),
            new_java2rust::dependency_mode::DependencyMode::ExternalFactModel,
            parsed_fact_set,
        );
    }

    for manifest_path in &external_inputs.pack_manifest_paths {
        let manifest_source = fs::read_to_string(manifest_path)
            .map_err(|err| format!("failed to read {manifest_path}: {err}"))?;
        let parsed_manifest =
            new_java2rust::external_packs::parse_external_pack_manifest(&manifest_source);
        let (mut manifest_facts, manifest_diagnostics) = parsed_manifest.into_parts();
        diagnostics.extend(manifest_diagnostics);
        enrich_external_pack_facade_abi(manifest_path, &manifest_source, &mut manifest_facts);
        config
            .add_external_pack_provider(format!("manifest:{manifest_path}"), manifest_facts)
            .map_err(|err| {
                format!("failed to register external pack manifest {manifest_path}: {err}")
            })?;
    }

    for pack_name in &external_inputs.builtin_pack_names {
        let facts = new_java2rust::external_packs::builtin_external_pack_facts(pack_name)
            .ok_or_else(|| format!("unknown built-in external pack: {pack_name}"))?;
        config
            .add_external_pack_provider(format!("builtin-pack:{pack_name}"), facts)
            .map_err(|err| {
                format!("failed to register built-in external pack {pack_name}: {err}")
            })?;
    }

    Ok(diagnostics)
}

#[derive(Debug, Clone)]
struct FacadeFnAbi {
    return_type: Option<String>,
    params: Vec<String>,
    receiver: FacadeFnReceiver,
    generic_params: Vec<String>,
    generic_bounds: HashMap<String, String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FacadeFnReceiver {
    None,
    BorrowedSelf,
    MutBorrowedSelf,
    OwnedSelf,
}

#[derive(Debug, Clone)]
struct FacadeAbi {
    fns: HashMap<(String, String), FacadeFnAbi>,
    aliases: HashMap<String, String>,
    from_conversions: Vec<(String, String)>,
}

fn enrich_external_pack_facade_abi(
    manifest_path: &str,
    manifest_source: &str,
    facts: &mut new_java2rust::facts::FactSet,
) {
    let manifest_path = Path::new(manifest_path);
    let crate_roots = manifest_crate_roots(manifest_path, manifest_source);
    if crate_roots.is_empty() {
        return;
    }
    let mut parsed_crates = HashMap::<String, FacadeAbi>::new();

    append_facade_conversion_facts(facts, &crate_roots, &mut parsed_crates);

    let api_facts = facts.external_api_effect_facts.clone();
    for effect in api_facts {
        let Some(receiver_type) = effect.receiver_type.as_deref() else {
            continue;
        };
        let Some((crate_name, abi_map)) =
            facade_abi_map_for_type(facts, receiver_type, &crate_roots, &mut parsed_crates)
        else {
            continue;
        };
        let arg_count = effect.arg_count.unwrap_or(0);
        let rust_name = facts
            .external_crate_api_mapping_for_receiver(
                receiver_type,
                &effect.method_name,
                effect.arg_count,
            )
            .map(|mapping| mapping.rust_name.clone())
            .unwrap_or_else(|| {
                new_java2rust::call_plan::external_facade_method_rust_name_for_abi(
                    receiver_type,
                    &effect.method_name,
                    arg_count,
                    facts,
                )
            });
        let owner = facade_owner_name(
            abi_map,
            receiver_type.rsplit('.').next().unwrap_or(receiver_type),
        );
        let Some(abi) = abi_map.fns.get(&(owner.clone(), rust_name.clone())) else {
            continue;
        };
        append_facade_structured_method_abi_fact(
            facts,
            &effect,
            crate_name.as_str(),
            abi,
            &owner,
            &rust_name,
        );
        append_facade_method_abi_facts(facts, &effect, abi);
        append_facade_argument_abi_facts(
            facts,
            &effect,
            &effect.method_name,
            &effect.method_name,
            abi,
        );
        if rust_name != effect.method_name {
            append_facade_argument_abi_facts(facts, &effect, &effect.method_name, &rust_name, abi);
        }
    }

    let constructors = facts.external_constructor_facts.clone();
    for constructor in constructors {
        let arg_count = constructor.arg_types.len();
        let Some((_crate_name, abi_map)) = facade_abi_map_for_type(
            facts,
            &constructor.type_name,
            &crate_roots,
            &mut parsed_crates,
        ) else {
            continue;
        };
        let owner = facade_owner_name(
            abi_map,
            constructor
                .type_name
                .rsplit('.')
                .next()
                .unwrap_or(&constructor.type_name),
        );
        let rust_name = facts
            .external_crate_constructor_mapping_for(&constructor.type_name, arg_count)
            .map(|mapping| mapping.rust_name.clone())
            .unwrap_or_else(|| {
                new_java2rust::call_plan::external_facade_constructor_rust_name_for_abi(
                    &constructor.type_name,
                    arg_count,
                    facts,
                )
            });
        let Some(abi) = abi_map.fns.get(&(owner, rust_name)) else {
            continue;
        };
        if let Some(return_type) = abi.return_type.as_deref() {
            facts.external_constructor_rust_result_facts.push(
                new_java2rust::facts::ExternalConstructorRustResultFact {
                    type_name: constructor.type_name.clone(),
                    arg_count,
                    returns_result: rust_type_is_result(return_type),
                    provenance: new_java2rust::facts::FactProvenance::ExternalLibraryPack,
                },
            );
        }
        append_facade_constructor_argument_abi_facts(facts, &constructor, arg_count, abi, abi_map);
    }
}

fn append_facade_conversion_facts(
    facts: &mut new_java2rust::facts::FactSet,
    crate_roots: &HashMap<String, PathBuf>,
    parsed_crates: &mut HashMap<String, FacadeAbi>,
) {
    let type_mappings = facade_type_mappings(facts, crate_roots, parsed_crates);
    for (crate_name, mappings) in type_mappings {
        let Some(abi) = parsed_crates.get(&crate_name) else {
            continue;
        };
        for (from_rust, to_rust) in &abi.from_conversions {
            for from_java in java_types_for_facade_owner(&mappings, from_rust) {
                for to_java in java_types_for_facade_owner(&mappings, to_rust) {
                    if facts
                        .external_conversion_adapter_for(&from_java, &to_java)
                        .is_some()
                    {
                        continue;
                    }
                    facts.external_conversion_adapter_facts.push(
                        new_java2rust::facts::ExternalConversionAdapterFact {
                            from_type: from_java.clone(),
                            to_type: to_java,
                            kind: new_java2rust::facts::ExternalConversionAdapterKind::Into,
                            provenance: new_java2rust::facts::FactProvenance::ExternalLibraryPack,
                        },
                    );
                }
            }
        }
    }
}

fn append_facade_method_abi_facts(
    facts: &mut new_java2rust::facts::FactSet,
    effect: &new_java2rust::facts::ExternalApiEffectFact,
    abi: &FacadeFnAbi,
) {
    let Some(return_type) = abi.return_type.as_deref() else {
        return;
    };
    facts
        .external_rust_result_facts
        .push(new_java2rust::facts::ExternalRustResultFact {
            receiver_type: effect.receiver_type.clone(),
            method_name: effect.method_name.clone(),
            arg_count: effect.arg_count,
            returns_result: rust_type_is_result(return_type),
            provenance: new_java2rust::facts::FactProvenance::ExternalLibraryPack,
        });
    let unwrapped = rust_return_payload_type(return_type);
    if unwrapped != "()" {
        facts.external_rust_return_type_facts.push(
            new_java2rust::facts::ExternalRustReturnTypeFact {
                receiver_type: effect.receiver_type.clone(),
                method_name: effect.method_name.clone(),
                arg_count: effect.arg_count,
                rust_type_name: unwrapped,
                return_slot: facade_return_slot_for_rust_type(return_type),
                borrowed_return_recovery: None,
                provenance: new_java2rust::facts::FactProvenance::ExternalLibraryPack,
            },
        );
    }
    if !rust_return_payload_is_optional(return_type)
        && !matches!(
            effect.return_nullability,
            new_java2rust::facts::NullabilityState::DefinitelyNonNull
        )
    {
        let mut adjusted = effect.clone();
        adjusted.return_nullability = new_java2rust::facts::NullabilityState::DefinitelyNonNull;
        adjusted.provenance = new_java2rust::facts::FactProvenance::ExternalLibraryPack;
        facts.external_api_effect_facts.push(adjusted);
    }
}

fn append_facade_structured_method_abi_fact(
    facts: &mut new_java2rust::facts::FactSet,
    effect: &new_java2rust::facts::ExternalApiEffectFact,
    crate_name: &str,
    abi: &FacadeFnAbi,
    owner: &str,
    rust_name: &str,
) {
    let Some(receiver_type) = effect.receiver_type.as_deref() else {
        return;
    };
    let arg_count = effect.arg_count.unwrap_or(abi.params.len());
    if facts
        .facade_abi_for_external_method(receiver_type, &effect.method_name, arg_count)
        .is_some()
    {
        return;
    }
    let key = new_java2rust::facts::CallableKey::method(
        receiver_type.to_string(),
        effect.method_name.clone(),
        new_java2rust::facts::Staticness::Unknown,
        arg_count,
    );

    let Some(return_type) = abi.return_type.as_deref() else {
        return;
    };
    let return_slot = facade_return_slot_for_rust_type(return_type);
    let borrow_source = return_slot
        .contains_shared_borrow()
        .then_some(new_java2rust::facts::BorrowSource::Receiver);
    facts
        .facade_abi_facts
        .push(new_java2rust::facts::FacadeAbiFact {
            key,
            rust_path: format!("{crate_name}::{owner}::{rust_name}"),
            receiver: facade_receiver_abi(abi.receiver),
            args: abi
                .params
                .iter()
                .map(|param| new_java2rust::facts::facade_slot_for_rust_type(param))
                .collect(),
            return_slot,
            result: facade_result_abi(return_type),
            borrow_source,
            borrowed_return_recovery: None,
            provenance: new_java2rust::facts::FactProvenance::ExternalLibraryPack,
        });
}

fn facade_receiver_abi(receiver: FacadeFnReceiver) -> new_java2rust::facts::ReceiverAbi {
    match receiver {
        FacadeFnReceiver::None => new_java2rust::facts::ReceiverAbi::AssociatedFunction,
        FacadeFnReceiver::BorrowedSelf => new_java2rust::facts::ReceiverAbi::BorrowedSelf,
        FacadeFnReceiver::MutBorrowedSelf => new_java2rust::facts::ReceiverAbi::MutBorrowedSelf,
        FacadeFnReceiver::OwnedSelf => new_java2rust::facts::ReceiverAbi::OwnedSelf,
    }
}

fn facade_result_abi(return_type: &str) -> new_java2rust::facts::ResultAbi {
    if rust_type_is_result(return_type) {
        new_java2rust::facts::ResultAbi::ResultWrapped
    } else if rust_return_payload_is_optional(return_type) {
        new_java2rust::facts::ResultAbi::OptionWrapped
    } else {
        new_java2rust::facts::ResultAbi::Plain
    }
}

fn facade_return_slot_for_rust_type(return_type: &str) -> new_java2rust::facts::FacadeSlot {
    new_java2rust::facts::facade_slot_for_rust_type(&rust_return_payload_type(return_type))
}

fn append_facade_argument_abi_facts(
    facts: &mut new_java2rust::facts::FactSet,
    effect: &new_java2rust::facts::ExternalApiEffectFact,
    lookup_method_name: &str,
    published_method_name: &str,
    abi: &FacadeFnAbi,
) {
    let Some(receiver_type) = effect.receiver_type.as_deref() else {
        return;
    };
    for (index, param) in abi.params.iter().enumerate() {
        let Some(existing) = facts.external_api_argument_effect_for_receiver(
            Some(receiver_type),
            lookup_method_name,
            index,
        ) else {
            continue;
        };
        if rust_type_accepts_none(param, abi) {
            if published_method_name != lookup_method_name {
                let mut bridged = existing.clone();
                bridged.method_name = published_method_name.to_string();
                bridged.provenance = new_java2rust::facts::FactProvenance::ExternalLibraryPack;
                facts.external_api_argument_effect_facts.push(bridged);
            }
            continue;
        }
        if matches!(
            existing.parameter_nullability,
            new_java2rust::facts::NullabilityState::DefinitelyNonNull
        ) {
            if published_method_name != lookup_method_name {
                let mut bridged = existing.clone();
                bridged.method_name = published_method_name.to_string();
                bridged.provenance = new_java2rust::facts::FactProvenance::ExternalLibraryPack;
                facts.external_api_argument_effect_facts.push(bridged);
            }
            continue;
        }
        let mut adjusted = existing.clone();
        adjusted.method_name = published_method_name.to_string();
        adjusted.parameter_nullability = new_java2rust::facts::NullabilityState::DefinitelyNonNull;
        adjusted.provenance = new_java2rust::facts::FactProvenance::ExternalLibraryPack;
        facts.external_api_argument_effect_facts.push(adjusted);
    }
}

fn append_facade_constructor_argument_abi_facts(
    facts: &mut new_java2rust::facts::FactSet,
    constructor: &new_java2rust::facts::ExternalConstructorFact,
    arg_count: usize,
    fn_abi: &FacadeFnAbi,
    facade_abi: &FacadeAbi,
) {
    let type_mappings = facade_type_mappings_for_crate(facts, &constructor.type_name, facade_abi);
    for (index, param) in fn_abi.params.iter().enumerate() {
        if facts
            .external_constructor_argument_effect_for(&constructor.type_name, arg_count, index)
            .is_some()
        {
            continue;
        }
        facts.external_constructor_argument_effect_facts.push(
            new_java2rust::facts::ExternalConstructorArgumentEffectFact {
                type_name: constructor.type_name.clone(),
                arg_count,
                parameter_index: index,
                parameter_type: java_type_for_rust_param(param, &type_mappings, fn_abi),
                parameter_nullability: if rust_type_accepts_none(param, fn_abi) {
                    new_java2rust::facts::NullabilityState::MaybeNull
                } else {
                    new_java2rust::facts::NullabilityState::DefinitelyNonNull
                },
                argument_mutability: if new_java2rust::facts::facade_slot_for_rust_type(param)
                    .contains_mut_borrow()
                {
                    new_java2rust::facts::ReceiverMutabilityFact::MutatesReceiver
                } else {
                    new_java2rust::facts::ReceiverMutabilityFact::ReadOnly
                },
                alias_policy: if rust_param_is_borrowed_or_borrowable(param, fn_abi) {
                    new_java2rust::facts::ExternalArgumentAliasPolicy::Borrowed
                } else {
                    new_java2rust::facts::ExternalArgumentAliasPolicy::NoAlias
                },
                provenance: new_java2rust::facts::FactProvenance::ExternalLibraryPack,
            },
        );
    }
}

fn facade_abi_map_for_type<'a>(
    facts: &new_java2rust::facts::FactSet,
    type_name: &str,
    crate_roots: &HashMap<String, PathBuf>,
    parsed_crates: &'a mut HashMap<String, FacadeAbi>,
) -> Option<(String, &'a FacadeAbi)> {
    let pack = facts.external_library_pack_for_type(type_name)?;
    let crate_name = pack.rust_target.as_ref()?.clone();
    let crate_root = crate_roots.get(&crate_name)?;
    if !parsed_crates.contains_key(&crate_name) {
        let source = fs::read_to_string(crate_root.join("src/lib.rs")).ok()?;
        parsed_crates.insert(crate_name.clone(), parse_facade_abi(&source));
    }
    let map = parsed_crates.get(&crate_name)?;
    Some((crate_name, map))
}

fn facade_type_mappings(
    facts: &new_java2rust::facts::FactSet,
    crate_roots: &HashMap<String, PathBuf>,
    parsed_crates: &mut HashMap<String, FacadeAbi>,
) -> HashMap<String, Vec<(String, String)>> {
    let mut by_crate = HashMap::<String, Vec<(String, String)>>::new();
    for pack in &facts.external_library_packs {
        let Some(crate_name) = pack.rust_target.as_ref() else {
            continue;
        };
        if !crate_roots.contains_key(crate_name) {
            continue;
        }
        let Some((_, abi)) =
            facade_abi_map_for_type(facts, &pack.package_prefix, crate_roots, parsed_crates)
        else {
            continue;
        };
        let simple = pack
            .package_prefix
            .rsplit('.')
            .next()
            .unwrap_or(&pack.package_prefix);
        by_crate
            .entry(crate_name.clone())
            .or_default()
            .push((facade_owner_name(abi, simple), pack.package_prefix.clone()));
    }
    by_crate
}

fn facade_type_mappings_for_crate(
    facts: &new_java2rust::facts::FactSet,
    type_name: &str,
    facade_abi: &FacadeAbi,
) -> Vec<(String, String)> {
    let Some(pack) = facts.external_library_pack_for_type(type_name) else {
        return Vec::new();
    };
    let Some(crate_name) = pack.rust_target.as_ref() else {
        return Vec::new();
    };
    facts
        .external_library_packs
        .iter()
        .filter(|candidate| candidate.rust_target.as_ref() == Some(crate_name))
        .map(|candidate| {
            let simple = candidate
                .package_prefix
                .rsplit('.')
                .next()
                .unwrap_or(&candidate.package_prefix);
            (
                facade_owner_name(facade_abi, simple),
                candidate.package_prefix.clone(),
            )
        })
        .collect()
}

fn java_types_for_facade_owner(mappings: &[(String, String)], rust_type: &str) -> Vec<String> {
    mappings
        .iter()
        .filter(|(owner, _)| owner == rust_type)
        .map(|(_, java_type)| java_type.clone())
        .collect()
}

fn facade_owner_name(abi: &FacadeAbi, java_simple_name: &str) -> String {
    let owner = java_simple_name.to_string();
    abi.aliases.get(&owner).cloned().unwrap_or(owner)
}

fn manifest_crate_roots(manifest_path: &Path, manifest_source: &str) -> HashMap<String, PathBuf> {
    let mut roots = HashMap::new();
    for line in manifest_source.lines() {
        let line = line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        let tokens = line.split_whitespace().collect::<Vec<_>>();
        if tokens.len() < 5 || tokens[0] != "external-pack" {
            continue;
        }
        if !matches!(tokens[3], "rust-crate-mapping" | "rust_crate_mapping") {
            continue;
        }
        let attrs = tokens[5..]
            .iter()
            .filter_map(|token| token.split_once('='))
            .collect::<HashMap<_, _>>();
        let Some(target) = attrs.get("target").copied() else {
            continue;
        };
        if roots.contains_key(target) {
            continue;
        }
        if let Some(path) = attrs.get("path") {
            roots.insert(
                target.to_string(),
                manifest_path.parent().unwrap_or(Path::new(".")).join(path),
            );
            continue;
        }
        if manifest_path
            .parent()
            .and_then(Path::file_name)
            .and_then(|name| name.to_str())
            == Some("manifests")
            && let Some(root) = manifest_path.parent().and_then(Path::parent)
        {
            let package = attrs
                .get("package")
                .copied()
                .map(str::to_string)
                .unwrap_or_else(|| target.replace('_', "-"));
            let candidate = root.join("crates").join(package);
            if candidate.exists() {
                roots.insert(target.to_string(), candidate);
            }
        }
    }
    roots
}

fn parse_facade_abi(source: &str) -> FacadeAbi {
    let mut out = HashMap::new();
    let mut aliases = HashMap::new();
    let mut from_conversions = Vec::new();
    let mut owner: Option<String> = None;
    let mut pending = String::new();
    for raw_line in source.lines() {
        let line = raw_line.trim();
        if let Some((alias, target)) = parse_facade_type_alias(line) {
            aliases.insert(alias, target);
        }
        if let Some(conversion) = parse_facade_from_conversion(line) {
            from_conversions.push(conversion);
        }
        if let Some(rest) = line.strip_prefix("impl ") {
            if let Some(name) = rest.split_whitespace().next() {
                if name.chars().next().is_some_and(char::is_uppercase) {
                    owner = Some(name.trim_matches('{').to_string());
                }
            }
        }
        if owner.is_none() {
            continue;
        }
        if line.starts_with("pub fn ") || !pending.is_empty() {
            if !pending.is_empty() {
                pending.push(' ');
            }
            pending.push_str(line);
            if line.contains('{') || line.ends_with(';') {
                if let (Some(owner), Some((name, abi))) =
                    (owner.clone(), parse_facade_fn_signature(&pending))
                {
                    out.insert((owner, name), abi);
                }
                pending.clear();
            }
        }
    }
    FacadeAbi {
        fns: out,
        aliases,
        from_conversions,
    }
}

fn parse_facade_type_alias(line: &str) -> Option<(String, String)> {
    let rest = line.strip_prefix("pub type ")?;
    let (alias, target) = rest.split_once('=')?;
    Some((
        alias.trim().to_string(),
        target.trim().trim_end_matches(';').trim().to_string(),
    ))
}

fn parse_facade_from_conversion(line: &str) -> Option<(String, String)> {
    let rest = line.strip_prefix("impl From<")?;
    let (from, rest) = rest.split_once("> for ")?;
    let to = rest.split_whitespace().next()?.trim_matches('{');
    Some((from.trim().to_string(), to.trim().to_string()))
}

fn parse_facade_fn_signature(signature: &str) -> Option<(String, FacadeFnAbi)> {
    let rest = signature.trim().strip_prefix("pub fn ")?;
    let name_end = rest.find(['(', '<'])?;
    let name = rest[..name_end].trim().to_string();
    let generic_params = parse_facade_generic_params(rest);
    let generic_bounds = parse_facade_generic_bounds(rest);
    let params_start = rest.find('(')?;
    let params_end = matching_paren(rest, params_start)?;
    let mut receiver = FacadeFnReceiver::None;
    let params = split_top_level(&rest[params_start + 1..params_end], ',')
        .into_iter()
        .filter_map(|param| {
            let param = param.trim();
            receiver = match param {
                "self" => FacadeFnReceiver::OwnedSelf,
                "&self" => FacadeFnReceiver::BorrowedSelf,
                "&mut self" => FacadeFnReceiver::MutBorrowedSelf,
                _ => receiver,
            };
            if param == "self" || param == "&self" || param == "&mut self" || param.is_empty() {
                return None;
            }
            param.split_once(':').map(|(_, ty)| ty.trim().to_string())
        })
        .collect::<Vec<_>>();
    let after_params = &rest[params_end + 1..];
    let return_type = after_params
        .split_once("->")
        .map(|(_, ty)| ty.split('{').next().unwrap_or(ty).trim().to_string());
    Some((
        name,
        FacadeFnAbi {
            return_type,
            params,
            receiver,
            generic_params,
            generic_bounds,
        },
    ))
}

fn parse_facade_generic_params(rest: &str) -> Vec<String> {
    parse_facade_generic_entries(rest)
        .into_iter()
        .map(|generic| {
            generic
                .split_once(':')
                .map(|(name, _)| name)
                .unwrap_or(generic)
                .trim()
                .to_string()
        })
        .filter(|name| !name.is_empty())
        .collect()
}

fn parse_facade_generic_bounds(rest: &str) -> HashMap<String, String> {
    parse_facade_generic_entries(rest)
        .into_iter()
        .filter_map(|generic| {
            let (name, bound) = generic.split_once(':')?;
            Some((name.trim().to_string(), bound.trim().to_string()))
        })
        .collect()
}

fn parse_facade_generic_entries(rest: &str) -> Vec<&str> {
    let Some(generic_start) = rest.find('<') else {
        return Vec::new();
    };
    let Some(params_start) = rest.find('(') else {
        return Vec::new();
    };
    if generic_start > params_start {
        return Vec::new();
    }
    let Some(generic_end) = matching_angle(rest, generic_start) else {
        return Vec::new();
    };
    split_top_level(&rest[generic_start + 1..generic_end], ',')
}

fn matching_angle(s: &str, open: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (idx, ch) in s.char_indices().skip_while(|(idx, _)| *idx < open) {
        match ch {
            '<' => depth += 1,
            '>' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(idx);
                }
            }
            _ => {}
        }
    }
    None
}

fn matching_paren(s: &str, open: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (idx, ch) in s.char_indices().skip_while(|(idx, _)| *idx < open) {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(idx);
                }
            }
            _ => {}
        }
    }
    None
}

fn split_top_level(s: &str, sep: char) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0usize;
    let mut angle_depth = 0i32;
    let mut paren_depth = 0i32;
    for (idx, ch) in s.char_indices() {
        match ch {
            '<' => angle_depth += 1,
            '>' => angle_depth -= 1,
            '(' => paren_depth += 1,
            ')' => paren_depth -= 1,
            ch if ch == sep && angle_depth == 0 && paren_depth == 0 => {
                out.push(&s[start..idx]);
                start = idx + ch.len_utf8();
            }
            _ => {}
        }
    }
    out.push(&s[start..]);
    out
}

fn rust_type_is_result(ty: &str) -> bool {
    new_java2rust::types::rendered_rust_type_is_result(ty)
}

fn rust_return_payload_is_optional(ty: &str) -> bool {
    new_java2rust::types::rendered_rust_option_inner(&rust_return_payload_type(ty)).is_some()
}

fn rust_return_payload_type(ty: &str) -> String {
    let ty = ty.trim().trim_end_matches(';').trim();
    if let Some(args) = new_java2rust::types::rendered_rust_result_args(ty) {
        return args
            .first()
            .map(|payload| (*payload).to_string())
            .unwrap_or_else(|| ty.to_string());
    }
    ty.to_string()
}

fn rust_type_accepts_none(ty: &str, abi: &FacadeFnAbi) -> bool {
    let ty = ty.trim();
    new_java2rust::types::rendered_rust_type_is_option(ty)
        || (abi.generic_params.iter().any(|param| param == ty)
            && !abi.generic_bounds.contains_key(ty))
}

fn rust_param_is_borrowed_or_borrowable(ty: &str, abi: &FacadeFnAbi) -> bool {
    let ty = ty.trim();
    ty.starts_with('&')
        || ty.contains("Into<String>")
        || ty.contains("AsRef<str>")
        || abi
            .generic_bounds
            .get(ty)
            .is_some_and(|bound| bound.contains("Into<String>") || bound.contains("AsRef<str>"))
}

fn java_type_for_rust_param(
    ty: &str,
    type_mappings: &[(String, String)],
    abi: &FacadeFnAbi,
) -> Option<new_java2rust::java_ir::JavaType> {
    let ty = rust_param_payload_type(ty);
    let bound = abi
        .generic_bounds
        .get(&ty)
        .map(String::as_str)
        .unwrap_or("");
    if ty == "String"
        || ty == "str"
        || ty.contains("Into<String>")
        || ty.contains("AsRef<str>")
        || bound.contains("Into<String>")
        || bound.contains("AsRef<str>")
    {
        return Some(new_java2rust::java_ir::JavaType::String);
    }
    match ty.as_str() {
        "bool" => Some(new_java2rust::java_ir::JavaType::Primitive(
            new_java2rust::java_ir::PrimitiveType::Boolean,
        )),
        "i8" => Some(new_java2rust::java_ir::JavaType::Primitive(
            new_java2rust::java_ir::PrimitiveType::Byte,
        )),
        "i16" => Some(new_java2rust::java_ir::JavaType::Primitive(
            new_java2rust::java_ir::PrimitiveType::Short,
        )),
        "i32" | "usize" => Some(new_java2rust::java_ir::JavaType::Primitive(
            new_java2rust::java_ir::PrimitiveType::Int,
        )),
        "i64" => Some(new_java2rust::java_ir::JavaType::Primitive(
            new_java2rust::java_ir::PrimitiveType::Long,
        )),
        "f32" => Some(new_java2rust::java_ir::JavaType::Primitive(
            new_java2rust::java_ir::PrimitiveType::Float,
        )),
        "f64" => Some(new_java2rust::java_ir::JavaType::Primitive(
            new_java2rust::java_ir::PrimitiveType::Double,
        )),
        _ => type_mappings
            .iter()
            .find(|(rust_type, _)| rust_type == &ty)
            .map(|(_, java_type)| new_java2rust::java_ir::JavaType::Named {
                name: java_type.clone(),
                args: Vec::new(),
            }),
    }
}

fn rust_param_payload_type(ty: &str) -> String {
    let mut ty = ty.trim();
    if let Some(inner) = new_java2rust::types::rendered_rust_option_inner(ty) {
        ty = inner.trim();
    }
    ty.trim_start_matches("&mut ")
        .trim_start_matches('&')
        .trim()
        .to_string()
}

fn print_diagnostics(diagnostics: &new_java2rust::diagnostics::DiagnosticBag) {
    for diagnostic in diagnostics.iter() {
        eprintln!(
            "{:?} {}: {}",
            diagnostic.severity(),
            diagnostic.code(),
            diagnostic.message()
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use new_java2rust::facts::{FactProvenance, NullabilityState};

    #[test]
    fn parse_args_accepts_facts_and_external_pack_manifest_inputs() {
        let args = parse_args(
            [
                "--facts",
                "overrides.facts",
                "--external-pack-manifest",
                "dep.pack",
                "--external-pack-manifest",
                "native.pack",
                "Example.java",
            ]
            .into_iter()
            .map(str::to_string),
        );

        assert_eq!(
            args,
            Some(Args::Translate {
                source_path: "Example.java".to_string(),
                external_inputs: ExternalInputPaths {
                    facts_paths: vec!["overrides.facts".to_string()],
                    pack_manifest_paths: vec!["dep.pack".to_string(), "native.pack".to_string()],
                    builtin_pack_names: Vec::new(),
                    enable_builtin_external_packs: true,
                },
                context_dirs: Vec::new(),
                context_summary_dirs: Vec::new(),
                context_dir_packages: Vec::new(),
                context_dirs_are_emitted: false,
            })
        );
    }

    #[test]
    fn parse_args_rejects_missing_external_pack_manifest_path() {
        assert!(parse_args(["--external-pack-manifest"].into_iter().map(str::to_string)).is_none());
    }

    #[test]
    fn parse_args_accepts_resolver_bench_external_inputs() {
        let args = parse_args(
            [
                "resolver-bench",
                "--facts",
                "overrides.facts",
                "--external-pack-manifest",
                "dep.pack",
                "--builtin-external-pack",
                "hdf5",
                "src/main/java",
            ]
            .into_iter()
            .map(str::to_string),
        );

        assert_eq!(
            args,
            Some(Args::ResolverBench {
                root_path: "src/main/java".to_string(),
                external_inputs: ExternalInputPaths {
                    facts_paths: vec!["overrides.facts".to_string()],
                    pack_manifest_paths: vec!["dep.pack".to_string()],
                    builtin_pack_names: vec!["hdf5".to_string()],
                    enable_builtin_external_packs: true,
                },
                source_roots: Vec::new(),
            })
        );
    }

    #[test]
    fn parse_args_accepts_resolver_bench_dependency_source_roots() {
        let args = parse_args(
            [
                "resolver-bench",
                "--source-root",
                "co-translate:deps/picard/src/main/java",
                "--source-root",
                "source-summary:deps/barclay-summary",
                "src/main/java",
            ]
            .into_iter()
            .map(str::to_string),
        );

        assert_eq!(
            args,
            Some(Args::ResolverBench {
                root_path: "src/main/java".to_string(),
                external_inputs: ExternalInputPaths::default(),
                source_roots: vec![
                    CliSourceRoot {
                        mode: new_java2rust::dependency_mode::DependencyMode::CoTranslateSource,
                        path: "deps/picard/src/main/java".to_string(),
                        package_prefixes: Vec::new(),
                    },
                    CliSourceRoot {
                        mode: new_java2rust::dependency_mode::DependencyMode::SourceSummary,
                        path: "deps/barclay-summary".to_string(),
                        package_prefixes: Vec::new(),
                    },
                ],
            })
        );
    }

    #[test]
    fn parse_args_accepts_resolver_bench_dependency_source_root_package_prefixes() {
        let args = parse_args(
            [
                "resolver-bench",
                "--source-root-package",
                "source-summary:deps/htsjdk/src/main/java:htsjdk.variant.variantcontext",
                "src/main/java",
            ]
            .into_iter()
            .map(str::to_string),
        );

        assert_eq!(
            args,
            Some(Args::ResolverBench {
                root_path: "src/main/java".to_string(),
                external_inputs: ExternalInputPaths::default(),
                source_roots: vec![CliSourceRoot {
                    mode: new_java2rust::dependency_mode::DependencyMode::SourceSummary,
                    path: "deps/htsjdk/src/main/java".to_string(),
                    package_prefixes: vec!["htsjdk.variant.variantcontext".to_string()],
                }],
            })
        );
    }

    #[test]
    fn parse_args_rejects_invalid_resolver_bench_source_root_specs() {
        assert!(
            parse_args(
                [
                    "resolver-bench",
                    "--source-root",
                    "external-fact-model:deps/runtime",
                    "src/main/java"
                ]
                .into_iter()
                .map(str::to_string)
            )
            .is_none()
        );
        assert!(
            parse_args(
                [
                    "resolver-bench",
                    "--source-root",
                    "source-summary:",
                    "src/main/java"
                ]
                .into_iter()
                .map(str::to_string)
            )
            .is_none()
        );
        assert!(
            parse_args(
                [
                    "resolver-bench",
                    "--source-root-package",
                    "source-summary:deps/runtime:",
                    "src/main/java"
                ]
                .into_iter()
                .map(str::to_string)
            )
            .is_none()
        );
        assert!(
            parse_args(
                [
                    "resolver-bench",
                    "--source-root-package",
                    "source-summary::dep.keep",
                    "src/main/java"
                ]
                .into_iter()
                .map(str::to_string)
            )
            .is_none()
        );
    }

    #[test]
    fn parse_args_accepts_resolver_bench_builtin_pack_baseline_flag() {
        let args = parse_args(
            [
                "resolver-bench",
                "--no-builtin-external-packs",
                "src/main/java",
            ]
            .into_iter()
            .map(str::to_string),
        );

        assert_eq!(
            args,
            Some(Args::ResolverBench {
                root_path: "src/main/java".to_string(),
                external_inputs: ExternalInputPaths {
                    facts_paths: Vec::new(),
                    pack_manifest_paths: Vec::new(),
                    builtin_pack_names: Vec::new(),
                    enable_builtin_external_packs: false,
                },
                source_roots: Vec::new(),
            })
        );
    }

    #[test]
    fn parse_args_accepts_explicit_builtin_pack_for_translation() {
        assert_eq!(
            parse_args(
                [
                    "--builtin-external-pack",
                    "htsjdk.tribble.feature",
                    "Example.java",
                ]
                .into_iter()
                .map(str::to_string)
            ),
            Some(Args::Translate {
                source_path: "Example.java".to_string(),
                external_inputs: ExternalInputPaths {
                    facts_paths: Vec::new(),
                    pack_manifest_paths: Vec::new(),
                    builtin_pack_names: vec!["htsjdk.tribble.feature".to_string()],
                    enable_builtin_external_packs: true,
                },
                context_dirs: Vec::new(),
                context_summary_dirs: Vec::new(),
                context_dir_packages: Vec::new(),
                context_dirs_are_emitted: false,
            })
        );
    }

    #[test]
    fn parse_args_accepts_context_dir_for_translation() {
        assert_eq!(
            parse_args(
                ["--context-dir", "src/main/java/pkg", "Example.java"]
                    .into_iter()
                    .map(str::to_string)
            ),
            Some(Args::Translate {
                source_path: "Example.java".to_string(),
                external_inputs: ExternalInputPaths::default(),
                context_dirs: vec!["src/main/java/pkg".to_string()],
                context_summary_dirs: Vec::new(),
                context_dir_packages: Vec::new(),
                context_dirs_are_emitted: false,
            })
        );
    }

    #[test]
    fn parse_args_accepts_emitted_context_dir_for_translation() {
        assert_eq!(
            parse_args(
                [
                    "--context-dir",
                    "src/main/java/pkg",
                    "--context-dir-emitted",
                    "Example.java",
                ]
                .into_iter()
                .map(str::to_string)
            ),
            Some(Args::Translate {
                source_path: "Example.java".to_string(),
                external_inputs: ExternalInputPaths::default(),
                context_dirs: vec!["src/main/java/pkg".to_string()],
                context_summary_dirs: Vec::new(),
                context_dir_packages: Vec::new(),
                context_dirs_are_emitted: true,
            })
        );
    }

    #[test]
    fn parse_args_accepts_source_summary_context_dir_for_translation() {
        assert_eq!(
            parse_args(
                [
                    "--context-source-summary-dir",
                    "deps/summary/src/main/java",
                    "Example.java",
                ]
                .into_iter()
                .map(str::to_string)
            ),
            Some(Args::Translate {
                source_path: "Example.java".to_string(),
                external_inputs: ExternalInputPaths::default(),
                context_dirs: Vec::new(),
                context_summary_dirs: vec!["deps/summary/src/main/java".to_string()],
                context_dir_packages: Vec::new(),
                context_dirs_are_emitted: false,
            })
        );
    }

    #[test]
    fn parse_args_accepts_selected_primary_files_for_batch_translation() {
        assert_eq!(
            parse_args(
                [
                    "translate-batch",
                    "--primary",
                    "src/main/java",
                    "--primary-file",
                    "pkg/A.java",
                    "--primary-file",
                    "/checkout/pkg/B.java",
                    "--emit-primary-file",
                    "pkg/A.java",
                    "--out-dir",
                    "out",
                ]
                .into_iter()
                .map(str::to_string),
            ),
            Some(Args::TranslateBatch {
                primary_path: "src/main/java".to_string(),
                primary_files: vec!["pkg/A.java".to_string(), "/checkout/pkg/B.java".to_string(),],
                emit_primary_files: vec!["pkg/A.java".to_string()],
                out_dir: "out".to_string(),
                generated_dir: None,
                external_inputs: ExternalInputPaths::default(),
                context_dirs: Vec::new(),
                context_summary_dirs: Vec::new(),
                context_dir_packages: Vec::new(),
                context_dirs_are_emitted: false,
                jobs: 10,
                phase_events_path: None,
                worker_events_path: None,
            })
        );
    }

    #[test]
    fn parse_args_accepts_package_filtered_context_dir_for_translation() {
        assert_eq!(
            parse_args(
                [
                    "--context-dir-package",
                    "src/main/java:org.example.engine",
                    "Example.java",
                ]
                .into_iter()
                .map(str::to_string)
            ),
            Some(Args::Translate {
                source_path: "Example.java".to_string(),
                external_inputs: ExternalInputPaths::default(),
                context_dirs: Vec::new(),
                context_summary_dirs: Vec::new(),
                context_dir_packages: vec![ContextDirPackage {
                    path: "src/main/java".to_string(),
                    package_prefix: "org.example.engine".to_string(),
                }],
                context_dirs_are_emitted: false,
            })
        );
    }

    #[test]
    fn package_filtered_context_sources_only_load_matching_packages() {
        let temp_dir = std::env::temp_dir().join(format!(
            "new_java2rust_context_filter_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(temp_dir.join("src/org/example/engine")).unwrap();
        std::fs::create_dir_all(temp_dir.join("src/org/example/other")).unwrap();
        std::fs::write(
            temp_dir.join("src/org/example/engine/Base.java"),
            "package org.example.engine;\nclass Base {}\n",
        )
        .unwrap();
        std::fs::create_dir_all(temp_dir.join("src/org/example/engine/nested")).unwrap();
        std::fs::write(
            temp_dir.join("src/org/example/engine/nested/Nested.java"),
            "package org.example.engine.nested;\nclass Nested {}\n",
        )
        .unwrap();
        std::fs::write(
            temp_dir.join("src/org/example/other/Other.java"),
            "package org.example.other;\nclass Other {}\n",
        )
        .unwrap();

        let sources = load_context_sources(
            &[],
            &[ContextDirPackage {
                path: temp_dir.join("src").to_string_lossy().to_string(),
                package_prefix: "org.example.engine".to_string(),
            }],
        )
        .unwrap();

        assert_eq!(sources.len(), 2);
        assert!(sources.iter().any(|source| source.contains("class Base")));
        assert!(sources.iter().any(|source| source.contains("class Nested")));
        assert!(!sources.iter().any(|source| source.contains("class Other")));
    }

    #[test]
    fn parse_args_accepts_builtin_pack_baseline_flag_for_translation() {
        let Args::Translate {
            external_inputs, ..
        } = parse_args(
            ["--no-builtin-external-packs", "Example.java"]
                .into_iter()
                .map(str::to_string),
        )
        .expect("translation should accept disabling builtin external packs")
        else {
            panic!("expected translation args");
        };
        assert!(!external_inputs.enable_builtin_external_packs);
        assert!(
            parse_args(
                ["--source-root", "source-summary:deps", "Example.java"]
                    .into_iter()
                    .map(str::to_string)
            )
            .is_none()
        );
    }

    #[test]
    fn external_input_files_are_merged_into_one_fact_set() {
        let temp_dir =
            std::env::temp_dir().join(format!("new_java2rust_cli_inputs_{}", std::process::id()));
        std::fs::create_dir_all(&temp_dir).unwrap();
        let facts_path = temp_dir.join("overrides.facts");
        let manifest_path = temp_dir.join("dep.pack");
        std::fs::write(
            &facts_path,
            "external-api getName DepRecord getter read-only definitely-non-null return=String\n",
        )
        .unwrap();
        std::fs::write(
            &manifest_path,
            "external-pack dep dep external-fact-model semantic target=crate::external::dep\n\
             external-pack dep DepReader external-fact-model semantic target=crate::external::dep\n\
             external-pack-api dep DepReader read semantic getter read-only maybe-null return=DepRecord\n",
        )
        .unwrap();

        let inputs = ExternalInputPaths {
            facts_paths: vec![facts_path.to_string_lossy().to_string()],
            pack_manifest_paths: vec![manifest_path.to_string_lossy().to_string()],
            builtin_pack_names: vec!["htsjdk.tribble.feature".to_string()],
            enable_builtin_external_packs: true,
        };
        let (facts, diagnostics) = load_external_inputs(&inputs).unwrap();

        assert!(diagnostics.is_empty());
        assert_eq!(
            facts
                .external_api_effect_for_receiver(Some("DepRecord"), "getName")
                .unwrap()
                .provenance,
            FactProvenance::UserOverride
        );
        let read = facts
            .external_api_effect_for_receiver(Some("DepReader"), "read")
            .unwrap();
        assert_eq!(read.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(read.return_nullability, NullabilityState::MaybeNull);
        assert!(facts.external_library_pack_for_type("dep.Record").is_some());
        assert!(
            facts
                .external_library_pack_for_type("htsjdk.tribble.FeatureReader")
                .is_some()
        );

        std::fs::remove_file(facts_path).unwrap();
        std::fs::remove_file(manifest_path).unwrap();
        std::fs::remove_dir(temp_dir).unwrap();
    }

    #[test]
    fn resolver_bench_external_inputs_register_fact_providers() {
        let temp_dir = std::env::temp_dir().join(format!(
            "new_java2rust_cli_bench_inputs_{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&temp_dir).unwrap();
        let facts_path = temp_dir.join("overrides.facts");
        let manifest_path = temp_dir.join("dep.pack");
        std::fs::write(
            &facts_path,
            "external-api getName DepRecord getter read-only definitely-non-null return=String\n",
        )
        .unwrap();
        std::fs::write(
            &manifest_path,
            "external-pack dep dep rust-crate-mapping mapped target=dep_runtime\n\
             external-pack dep DepReader rust-crate-mapping mapped target=dep_runtime\n\
             external-pack-api dep DepReader read mapped getter read-only maybe-null return=DepRecord\n",
        )
        .unwrap();

        let inputs = ExternalInputPaths {
            facts_paths: vec![facts_path.to_string_lossy().to_string()],
            pack_manifest_paths: vec![manifest_path.to_string_lossy().to_string()],
            builtin_pack_names: vec!["hdf5".to_string()],
            enable_builtin_external_packs: true,
        };
        let mut config =
            new_java2rust::dependency_mode::DependencyModeConfig::for_primary_source_root(
                temp_dir.join("src"),
            );
        let diagnostics = load_external_fact_providers(&inputs, &mut config).unwrap();

        assert!(diagnostics.is_empty());
        assert_eq!(config.fact_providers().len(), 3);
        assert_eq!(
            config.fact_providers()[0].mode(),
            new_java2rust::dependency_mode::DependencyMode::ExternalFactModel
        );
        assert_eq!(
            config.fact_providers()[1].mode(),
            new_java2rust::dependency_mode::DependencyMode::RustCrateMapping
        );
        assert_eq!(
            config.fact_providers()[1]
                .facts()
                .external_library_pack_for_type("dep.Record")
                .unwrap()
                .rust_target
                .as_deref(),
            Some("dep_runtime")
        );
        assert_eq!(
            config.fact_providers()[2].name(),
            "builtin-pack:hdf5".to_string()
        );
        assert_eq!(
            config.fact_providers()[2].mode(),
            new_java2rust::dependency_mode::DependencyMode::RustCrateMapping
        );
        assert!(
            config.fact_providers()[2]
                .facts()
                .external_library_pack_for_type("HDF5File")
                .is_some()
        );

        std::fs::remove_file(facts_path).unwrap();
        std::fs::remove_file(manifest_path).unwrap();
        std::fs::remove_dir(temp_dir).unwrap();
    }

    #[test]
    fn resolver_bench_rejects_mixed_mode_manifest_provider() {
        let temp_dir = std::env::temp_dir().join(format!(
            "new_java2rust_cli_mixed_manifest_{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&temp_dir).unwrap();
        let manifest_path = temp_dir.join("mixed.pack");
        std::fs::write(
            &manifest_path,
            "external-pack dep.a dep.a external-fact-model semantic\n\
             external-pack dep.b dep.b rust-crate-mapping mapped target=dep_b\n",
        )
        .unwrap();

        let inputs = ExternalInputPaths {
            facts_paths: Vec::new(),
            pack_manifest_paths: vec![manifest_path.to_string_lossy().to_string()],
            builtin_pack_names: Vec::new(),
            enable_builtin_external_packs: true,
        };
        let mut config =
            new_java2rust::dependency_mode::DependencyModeConfig::for_primary_source_root("src");
        let err = load_external_fact_providers(&inputs, &mut config).unwrap_err();

        assert!(
            err.contains("failed to register external pack manifest"),
            "{err}"
        );
        assert!(
            err.contains("manifest:") && err.contains("mixed.pack"),
            "{err}"
        );
        assert!(
            err.contains("external-fact-model, rust-crate-mapping"),
            "{err}"
        );
        assert!(
            err.contains("split mixed-mode manifests into separate providers"),
            "{err}"
        );
        assert!(config.fact_providers().is_empty());

        std::fs::remove_file(manifest_path).unwrap();
        std::fs::remove_dir(temp_dir).unwrap();
    }

    #[test]
    fn external_pack_ingestion_derives_facade_constructor_args_and_conversions() {
        let temp_dir = std::env::temp_dir().join(format!(
            "new_java2rust_cli_facade_abi_{}",
            std::process::id()
        ));
        let facade_dir = temp_dir.join("facade/src");
        std::fs::create_dir_all(&facade_dir).unwrap();
        std::fs::write(
            facade_dir.join("lib.rs"),
            "pub struct RootAllocator;\n\
             pub struct IntVector;\n\
             pub enum FieldVector { Int(IntVector) }\n\
             impl IntVector {\n\
                 pub fn new(name: impl Into<String>, allocator: &RootAllocator) -> Self { IntVector }\n\
             }\n\
             impl From<IntVector> for FieldVector {\n\
                 fn from(value: IntVector) -> Self { FieldVector::Int(value) }\n\
             }\n",
        )
        .unwrap();
        let manifest_path = temp_dir.join("dep.pack");
        std::fs::write(
            &manifest_path,
            "external-pack dep RootAllocator rust-crate-mapping mapped target=dep_facade path=facade\n\
             external-pack dep IntVector rust-crate-mapping mapped target=dep_facade path=facade\n\
             external-pack dep FieldVector rust-crate-mapping mapped target=dep_facade path=facade\n\
             external-pack-constructor dep IntVector 2 mapped rust-type=IntVector rust=new\n",
        )
        .unwrap();

        let inputs = ExternalInputPaths {
            facts_paths: Vec::new(),
            pack_manifest_paths: vec![manifest_path.to_string_lossy().to_string()],
            builtin_pack_names: Vec::new(),
            enable_builtin_external_packs: false,
        };
        let (facts, diagnostics) = load_external_inputs(&inputs).unwrap();

        assert!(diagnostics.is_empty());
        let allocator_arg = facts
            .external_constructor_argument_effect_for("IntVector", 2, 1)
            .expect("facade &RootAllocator parameter should become a constructor arg fact");
        assert_eq!(
            allocator_arg.parameter_nullability,
            NullabilityState::DefinitelyNonNull
        );
        assert_eq!(
            allocator_arg.alias_policy,
            new_java2rust::facts::ExternalArgumentAliasPolicy::Borrowed
        );
        assert!(
            facts
                .external_conversion_adapter_for("IntVector", "FieldVector")
                .is_some()
        );

        std::fs::remove_dir_all(temp_dir).unwrap();
    }

    #[test]
    fn external_pack_ingestion_keeps_nullable_unconstrained_generic_arguments() {
        let temp_dir = std::env::temp_dir().join(format!(
            "new_java2rust_cli_facade_generic_{}",
            std::process::id()
        ));
        let facade_dir = temp_dir.join("facade/src");
        std::fs::create_dir_all(&facade_dir).unwrap();
        std::fs::write(
            facade_dir.join("lib.rs"),
            "pub struct Assert;\n\
             pub struct Schema;\n\
             pub struct Buffer;\n\
             impl Assert {\n\
                 pub fn get_schema(&self) -> Option<&Schema> { None }\n\
                 pub fn assert_null<T>(_actual: T) {}\n\
                 pub fn set_string<S: Into<String>>(_actual: S) {}\n\
             }\n\
             impl Buffer {\n\
                 pub fn new(_bytes: &mut [u8]) -> Self { Self }\n\
             }\n",
        )
        .unwrap();
        let manifest_path = temp_dir.join("dep.pack");
        std::fs::write(
            &manifest_path,
            "external-pack dep Assert rust-crate-mapping mapped target=dep_facade path=facade\n\
             external-pack dep Schema rust-crate-mapping mapped target=dep_facade path=facade\n\
             external-pack-api dep Assert getSchema mapped getter read-only maybe-null args=0 return=Schema rust=get_schema\n\
             external-pack-api dep Assert assertNull mapped side-effect read-only definitely-non-null args=1 return=void rust=assert_null\n\
             external-pack-argument dep Assert assertNull 0 maybe-null read-only borrowed type=Object\n\
             external-pack-api dep Assert setString mapped side-effect read-only definitely-non-null args=1 return=void rust=set_string\n\
             external-pack-argument dep Assert setString 0 maybe-null read-only borrowed type=String\n\
             external-pack dep Buffer rust-crate-mapping mapped target=dep_facade path=facade\n\
             external-pack-constructor dep Buffer 1 mapped rust-type=Buffer rust=new\n",
        )
        .unwrap();

        let inputs = ExternalInputPaths {
            facts_paths: Vec::new(),
            pack_manifest_paths: vec![manifest_path.to_string_lossy().to_string()],
            builtin_pack_names: Vec::new(),
            enable_builtin_external_packs: false,
        };
        let (facts, diagnostics) = load_external_inputs(&inputs).unwrap();

        assert!(diagnostics.is_empty());
        assert!(
            facts
                .external_api_argument_effect_for_receiver(Some("Assert"), "setString", 0)
                .is_some(),
            "{:?}",
            facts.external_api_argument_effect_facts
        );
        assert_eq!(
            facts
                .external_api_argument_effect_for_receiver(Some("Assert"), "assert_null", 0)
                .expect("unconstrained generic keeps nullable fact")
                .parameter_nullability,
            NullabilityState::MaybeNull
        );
        assert_eq!(
            facts
                .external_api_argument_effect_for_receiver(Some("Assert"), "set_string", 0)
                .expect("bounded generic should infer a required string facade argument")
                .parameter_nullability,
            NullabilityState::DefinitelyNonNull
        );
        let abi = facts
            .facade_abi_for_external_method("Assert", "getSchema", 0)
            .unwrap_or_else(|| {
                panic!(
                    "facade crate signature should generate a structured ABI fact; got {:?}",
                    facts.facade_abi_facts
                )
            });
        assert_eq!(
            abi.receiver,
            new_java2rust::facts::ReceiverAbi::BorrowedSelf
        );
        assert_eq!(
            abi.return_slot,
            new_java2rust::facts::FacadeSlot::Optional(Box::new(
                new_java2rust::facts::FacadeSlot::Borrowed,
            ))
        );
        assert_eq!(abi.result, new_java2rust::facts::ResultAbi::OptionWrapped);
        assert_eq!(abi.rust_path, "dep_facade::Assert::get_schema");
        assert_eq!(
            abi.borrow_source,
            Some(new_java2rust::facts::BorrowSource::Receiver)
        );
        assert_eq!(
            facts
                .external_constructor_argument_effect_for("Buffer", 1, 0)
                .expect("constructor &mut [u8] should become a mutating borrowed argument")
                .argument_mutability,
            new_java2rust::facts::ReceiverMutabilityFact::MutatesReceiver
        );

        std::fs::remove_dir_all(temp_dir).unwrap();
    }

    #[test]
    fn resolver_bench_rejects_unknown_builtin_pack_name() {
        let inputs = ExternalInputPaths {
            facts_paths: Vec::new(),
            pack_manifest_paths: Vec::new(),
            builtin_pack_names: vec!["missing-pack".to_string()],
            enable_builtin_external_packs: true,
        };
        let mut config =
            new_java2rust::dependency_mode::DependencyModeConfig::for_primary_source_root("src");
        let err = load_external_fact_providers(&inputs, &mut config).unwrap_err();
        assert!(err.contains("unknown built-in external pack: missing-pack"));
    }
}
