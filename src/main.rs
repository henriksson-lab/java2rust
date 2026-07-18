use std::env;
use std::fs;
use std::process::ExitCode;

fn main() -> ExitCode {
    let Some(args) = parse_args(env::args().skip(1)) else {
        eprintln!(
            "usage: new-java2rust [--facts facts.txt] [--external-pack-manifest pack.txt] <file.java>\n       new-java2rust resolver-bench [--source-root mode:path] [--no-builtin-external-packs] [--builtin-external-pack name] [--facts facts.txt] [--external-pack-manifest pack.txt] <dir>"
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
            config.add_source_root(source_root.path, source_root.mode);
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

    let translation = new_java2rust::translate_source_with_facts(&source, &external_facts);
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
        } else if arg == "--no-builtin-external-packs" || arg == "--builtin-external-pack" {
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
                    },
                    CliSourceRoot {
                        mode: new_java2rust::dependency_mode::DependencyMode::SourceSummary,
                        path: "deps/barclay-summary".to_string(),
                    },
                ],
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
                ["--builtin-external-pack", "hdf5", "Example.java"]
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
            builtin_pack_names: Vec::new(),
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
