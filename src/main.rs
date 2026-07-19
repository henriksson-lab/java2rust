use std::env;
use std::fs;
use std::path::Path;
use std::process::ExitCode;

fn main() -> ExitCode {
    let Some(args) = parse_args(env::args().skip(1)) else {
        eprintln!(
            "usage: new-java2rust [--context-dir dir] [--context-dir-package dir:package.prefix] [--facts facts.txt] [--external-pack-manifest pack.txt] <file.java>\n       new-java2rust resolver-bench [--source-root mode:path] [--source-root-package mode:path:package.prefix] [--no-builtin-external-packs] [--builtin-external-pack name] [--facts facts.txt] [--external-pack-manifest pack.txt] <dir>"
        );
        return ExitCode::from(2);
    };

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

    let Args::Translate {
        source_path,
        external_inputs,
        context_dirs,
        context_dir_packages,
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
    let context_summary_sources = match load_context_sources(&[], &context_dir_packages) {
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
            &external_facts,
        )
    };
    print_diagnostics(&fact_diagnostics);
    print_diagnostics(&translation.diagnostics);
    println!("{}", translation.rust);
    ExitCode::SUCCESS
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Args {
    Translate {
        source_path: String,
        external_inputs: ExternalInputPaths,
        context_dirs: Vec<String>,
        context_dir_packages: Vec<ContextDirPackage>,
    },
    ResolverBench {
        root_path: String,
        external_inputs: ExternalInputPaths,
        source_roots: Vec<CliSourceRoot>,
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
    let mut context_dir_packages = Vec::new();
    let mut args = args.peekable();
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
        } else if arg == "--context-dir-package" {
            context_dir_packages.push(parse_context_dir_package_spec(&args.next()?)?);
        } else if arg == "--no-builtin-external-packs" {
            return None;
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
        context_dir_packages,
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
    let mut diagnostics = new_java2rust::diagnostics::DiagnosticBag::default();

    for facts_path in &external_inputs.facts_paths {
        let fact_source = fs::read_to_string(facts_path)
            .map_err(|err| format!("failed to read {facts_path}: {err}"))?;
        let parsed_facts = new_java2rust::external_facts::parse_external_facts(&fact_source);
        diagnostics.extend(parsed_facts.diagnostics);
        facts.extend_from(parsed_facts.facts);
    }

    for manifest_path in &external_inputs.pack_manifest_paths {
        let manifest_source = fs::read_to_string(manifest_path)
            .map_err(|err| format!("failed to read {manifest_path}: {err}"))?;
        let parsed_manifest =
            new_java2rust::external_packs::parse_external_pack_manifest(&manifest_source);
        diagnostics.extend(parsed_manifest.diagnostics);
        facts.extend_from(parsed_manifest.facts);
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
        diagnostics.extend(parsed_facts.diagnostics);
        config.add_fact_provider(
            format!("facts:{facts_path}"),
            new_java2rust::dependency_mode::DependencyMode::ExternalFactModel,
            parsed_facts.facts,
        );
    }

    for manifest_path in &external_inputs.pack_manifest_paths {
        let manifest_source = fs::read_to_string(manifest_path)
            .map_err(|err| format!("failed to read {manifest_path}: {err}"))?;
        let parsed_manifest =
            new_java2rust::external_packs::parse_external_pack_manifest(&manifest_source);
        diagnostics.extend(parsed_manifest.diagnostics);
        config
            .add_external_pack_provider(format!("manifest:{manifest_path}"), parsed_manifest.facts)
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

fn print_diagnostics(diagnostics: &new_java2rust::diagnostics::DiagnosticBag) {
    for diagnostic in diagnostics.iter() {
        eprintln!(
            "{:?} {}: {}",
            diagnostic.severity, diagnostic.code, diagnostic.message
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
                context_dir_packages: Vec::new(),
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
                context_dir_packages: Vec::new(),
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
                context_dir_packages: Vec::new(),
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
                context_dir_packages: vec![ContextDirPackage {
                    path: "src/main/java".to_string(),
                    package_prefix: "org.example.engine".to_string(),
                }],
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
    fn parse_args_rejects_builtin_pack_baseline_flag_for_translation() {
        assert!(
            parse_args(
                ["--no-builtin-external-packs", "Example.java"]
                    .into_iter()
                    .map(str::to_string)
            )
            .is_none()
        );
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
        assert_eq!(config.fact_providers.len(), 3);
        assert_eq!(
            config.fact_providers[0].mode,
            new_java2rust::dependency_mode::DependencyMode::ExternalFactModel
        );
        assert_eq!(
            config.fact_providers[1].mode,
            new_java2rust::dependency_mode::DependencyMode::RustCrateMapping
        );
        assert_eq!(
            config.fact_providers[1]
                .facts
                .external_library_pack_for_type("dep.Record")
                .unwrap()
                .rust_target
                .as_deref(),
            Some("dep_runtime")
        );
        assert_eq!(
            config.fact_providers[2].name,
            "builtin-pack:hdf5".to_string()
        );
        assert_eq!(
            config.fact_providers[2].mode,
            new_java2rust::dependency_mode::DependencyMode::RustCrateMapping
        );
        assert!(
            config.fact_providers[2]
                .facts
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
        assert!(config.fact_providers.is_empty());

        std::fs::remove_file(manifest_path).unwrap();
        std::fs::remove_dir(temp_dir).unwrap();
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
