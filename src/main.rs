use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

fn main() -> ExitCode {
    let Some(args) = parse_args(env::args().skip(1)) else {
        eprintln!(
            "usage: new-java2rust [--context-dir dir] [--context-source-summary-dir dir] [--context-dir-package dir:package.prefix] [--facts facts.txt] [--external-pack-manifest pack.txt] <file.java>\n       new-java2rust resolver-bench [--source-root mode:path] [--source-root-package mode:path:package.prefix] [--no-builtin-external-packs] [--builtin-external-pack name] [--facts facts.txt] [--external-pack-manifest pack.txt] <dir>"
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
        context_summary_dirs,
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
        context_summary_dirs: Vec<String>,
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
    let mut context_summary_dirs = Vec::new();
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
        let mut manifest_facts = parsed_manifest.facts;
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
        let mut manifest_facts = parsed_manifest.facts;
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
    generic_params: Vec<String>,
    generic_bounds: HashMap<String, String>,
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
                new_java2rust::call_plan::external_method_rust_name(
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
        let Some(abi) = abi_map.fns.get(&(owner, rust_name.clone())) else {
            continue;
        };
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
        let _ = crate_name;
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
                new_java2rust::call_plan::unresolved_constructor_rust_name_for_facts(
                    &new_java2rust::java_ir::JavaType::Named {
                        name: constructor.type_name.clone(),
                        args: Vec::new(),
                    },
                    facts,
                    arg_count,
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
                argument_mutability: if param.trim().starts_with("&mut ") {
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
    let owner = new_java2rust::slot_render::rust_name_for_java_named_type(java_simple_name);
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
    let params = split_top_level(&rest[params_start + 1..params_end], ',')
        .into_iter()
        .filter_map(|param| {
            let param = param.trim();
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
    ty.trim().starts_with("Result<")
        || ty.trim().starts_with("std::result::Result<")
        || ty.trim().contains("Result<")
}

fn rust_return_payload_is_optional(ty: &str) -> bool {
    rust_return_payload_type(ty).trim().starts_with("Option<")
}

fn rust_return_payload_type(ty: &str) -> String {
    let ty = ty.trim().trim_end_matches(';').trim();
    if let Some(inner) = result_payload(ty) {
        return split_top_level(inner, ',')
            .first()
            .map(|payload| payload.trim().to_string())
            .unwrap_or_else(|| ty.to_string());
    }
    ty.to_string()
}

fn rust_type_accepts_none(ty: &str, abi: &FacadeFnAbi) -> bool {
    let ty = ty.trim();
    ty.starts_with("Option<")
        || ty.starts_with("std::option::Option<")
        || (abi.generic_params.iter().any(|param| param == ty)
            && !abi.generic_bounds.contains_key(ty))
}

fn result_payload(ty: &str) -> Option<&str> {
    let generic_start = ty.find('<')?;
    let outer = ty[..generic_start].trim();
    if outer == "Result" || outer == "std::result::Result" || outer.ends_with("Result") {
        let inner = &ty[generic_start + 1..];
        return inner.strip_suffix('>');
    }
    None
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
    if let Some(inner) = ty
        .strip_prefix("Option<")
        .or_else(|| ty.strip_prefix("std::option::Option<"))
        .and_then(|inner| inner.strip_suffix('>'))
    {
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
                context_summary_dirs: Vec::new(),
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
                context_summary_dirs: Vec::new(),
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
                context_summary_dirs: Vec::new(),
                context_dir_packages: Vec::new(),
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
             impl Assert {\n\
                 pub fn assert_null<T>(_actual: T) {}\n\
                 pub fn set_string<S: Into<String>>(_actual: S) {}\n\
             }\n",
        )
        .unwrap();
        let manifest_path = temp_dir.join("dep.pack");
        std::fs::write(
            &manifest_path,
            "external-pack dep Assert rust-crate-mapping mapped target=dep_facade path=facade\n\
             external-pack-api dep Assert assertNull mapped side-effect read-only definitely-non-null args=1 return=void rust=assert_null\n\
             external-pack-argument dep Assert assertNull 0 maybe-null read-only borrowed type=Object\n\
             external-pack-api dep Assert setString mapped side-effect read-only definitely-non-null args=1 return=void rust=set_string\n\
             external-pack-argument dep Assert setString 0 maybe-null read-only borrowed type=String\n",
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
