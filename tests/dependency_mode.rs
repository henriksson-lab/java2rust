use std::fs;
use std::path::{Path, PathBuf};

use new_java2rust::dependency_mode::{DependencyMode, DependencyModeConfig};
use new_java2rust::facts::{
    ExternalApiCoverage, ExternalApiEffectFact, ExternalApiRole, ExternalLibraryMode,
    ExternalLibraryPackFact, FactProvenance, FactSet, NullabilityState, ReceiverMutabilityFact,
};
use new_java2rust::java_ir::{JavaType, Program};
use new_java2rust::object_model::ObjectRepresentation;
use new_java2rust::parser;
use new_java2rust::representation::plan_representations;
use new_java2rust::resolve;
use new_java2rust::resolver_benchmark::{
    benchmark_java_resolver_config, render_resolver_benchmark_report,
};

#[test]
fn dependency_mode_config_tracks_roots_and_fact_provider_modes() {
    let mut config = DependencyModeConfig::for_primary_source_root("app/src/main/java");
    config.add_source_root(
        "deps/htsjdk/src/main/java",
        DependencyMode::CoTranslateSource,
    );
    config.add_source_root("deps/barclay-summary", DependencyMode::SourceSummary);
    config.add_fact_provider(
        "htsjdk.variantcontext",
        DependencyMode::ExternalFactModel,
        Default::default(),
    );

    let resolution_roots = config
        .source_roots_for_resolution()
        .map(|root| (root.path.to_string_lossy().into_owned(), root.mode))
        .collect::<Vec<_>>();
    assert_eq!(
        resolution_roots,
        vec![
            (
                "app/src/main/java".to_string(),
                DependencyMode::PrimarySource
            ),
            (
                "deps/htsjdk/src/main/java".to_string(),
                DependencyMode::CoTranslateSource
            )
        ]
    );
    assert_eq!(
        config.mode_for_source_path(Path::new("deps/barclay-summary/Barclay.fact")),
        Some(DependencyMode::SourceSummary)
    );
    assert_eq!(
        config
            .source_roots_for_source_index()
            .map(|root| (root.path.to_string_lossy().into_owned(), root.mode))
            .collect::<Vec<_>>(),
        vec![
            (
                "app/src/main/java".to_string(),
                DependencyMode::PrimarySource,
            ),
            (
                "deps/htsjdk/src/main/java".to_string(),
                DependencyMode::CoTranslateSource,
            ),
            (
                "deps/barclay-summary".to_string(),
                DependencyMode::SourceSummary,
            ),
        ]
    );
    assert_eq!(config.fact_provider_facts().count(), 1);
}

#[test]
fn resolver_benchmark_uses_dependency_root_for_cross_root_resolution() {
    let root = unique_temp_root("dependency-mode-cross-root");
    let app_root = root.join("app");
    let dep_root = root.join("htsjdk");
    fs::create_dir_all(app_root.join("org/example/gatk")).unwrap();
    fs::create_dir_all(dep_root.join("htsjdk/variant/variantcontext")).unwrap();
    fs::write(
        app_root.join("org/example/gatk/UsesVariant.java"),
        r#"
package org.example.gatk;

import htsjdk.variant.variantcontext.AlleleLike;

class UsesVariant {
    AlleleLike create(String bases) {
        return new AlleleLike(bases);
    }

    int score(AlleleLike allele) {
        return allele.length + allele.inheritedScore();
    }

    String label(AlleleLike allele) {
        return allele.labelBox().get();
    }
}
"#,
    )
    .unwrap();
    fs::write(
        dep_root.join("htsjdk/variant/variantcontext/AlleleLike.java"),
        r#"
package htsjdk.variant.variantcontext;

public class AlleleLike extends AlleleBase {
    public int length;

    public AlleleLike(String bases) {}

    public Box<String> labelBox() {
        return new Box<String>("ref");
    }
}

class AlleleBase {
    public int inheritedScore() {
        return 1;
    }
}

class Box<T> {
    private T value;

    Box(T value) {
        this.value = value;
    }

    T get() {
        return value;
    }
}
"#,
    )
    .unwrap();

    let mut config = DependencyModeConfig::for_primary_source_root(&app_root);
    config.add_source_root(&dep_root, DependencyMode::CoTranslateSource);
    let report = benchmark_java_resolver_config(&config).unwrap();
    let rendered = render_resolver_benchmark_report(&report);

    assert_eq!(report.files_seen, 2);
    assert_eq!(report.unresolved_method_calls, 0, "{rendered}");
    assert_eq!(report.unresolved_constructors, 0);
    assert_eq!(report.unresolved_field_accesses, 0, "{rendered}");
    assert_eq!(report.external_api_explained_constructors, 0);
    assert!(rendered.contains("source_roots"));
    assert!(rendered.contains("PrimarySource\t1"));
    assert!(rendered.contains("CoTranslateSource\t1"));
}

#[test]
fn resolver_benchmark_without_dependency_root_keeps_import_as_external_boundary() {
    let root = unique_temp_root("dependency-mode-external-boundary");
    let app_root = root.join("app");
    fs::create_dir_all(app_root.join("org/example/gatk")).unwrap();
    fs::write(
        app_root.join("org/example/gatk/UsesVariant.java"),
        r#"
package org.example.gatk;

import htsjdk.variant.variantcontext.AlleleLike;

class UsesVariant {
    AlleleLike create(String bases) {
        return new AlleleLike(bases);
    }

    int score(AlleleLike allele) {
        return allele.length + allele.inheritedScore();
    }

    String label(AlleleLike allele) {
        return allele.labelBox().get();
    }
}
"#,
    )
    .unwrap();

    let config = DependencyModeConfig::for_primary_source_root(&app_root);
    let report = benchmark_java_resolver_config(&config).unwrap();

    assert_eq!(report.files_seen, 1);
    assert_eq!(report.unexplained_source_constructors, 0);
    assert_eq!(report.external_api_explained_constructors, 1);
    assert!(
        report.external_api_explained_method_calls > 0,
        "omitted dependency methods should be explained as external API facts"
    );
    assert!(
        report.external_field_guesses > 0,
        "omitted dependency fields should stay external boundary facts"
    );
}

#[test]
fn source_summary_root_contributes_declarations_without_benchmarking_bodies() {
    let root = unique_temp_root("dependency-mode-source-summary-root");
    let app_root = root.join("app");
    let summary_root = root.join("summary");
    fs::create_dir_all(app_root.join("app/usecase")).unwrap();
    fs::create_dir_all(summary_root.join("dep/summary")).unwrap();
    fs::write(
        app_root.join("app/usecase/UsesSummary.java"),
        r#"
package app.usecase;

import dep.summary.SummaryType;

class UsesSummary {
    SummaryType create(String value) {
        return new SummaryType(value);
    }

    int score(SummaryType summary) {
        return summary.length + summary.inheritedScore();
    }

    String label(SummaryType summary) {
        return summary.labelBox().get();
    }
}
"#,
    )
    .unwrap();
    fs::write(
        summary_root.join("dep/summary/SummaryType.java"),
        r#"
package dep.summary;

public class SummaryType extends SummaryBase {
    public int length;

    public SummaryType(String value) {}

    public Box<String> labelBox() {
        return new Box<String>("label");
    }
}

class SummaryBase {
    public int inheritedScore() {
        return 1;
    }
}

class Box<T> {
    private T value;

    Box(T value) {
        this.value = value;
    }

    T get() {
        return value;
    }
}
"#,
    )
    .unwrap();

    let mut config = DependencyModeConfig::for_primary_source_root(&app_root);
    config.add_source_root(&summary_root, DependencyMode::SourceSummary);
    let report = benchmark_java_resolver_config(&config).unwrap();
    let rendered = render_resolver_benchmark_report(&report);

    assert_eq!(report.files_seen, 2);
    assert_eq!(report.files_parsed_without_errors, 2);
    assert_eq!(
        report.files_resolved_without_errors, 1,
        "summary roots should feed declarations into the source index without becoming benchmarked/emitted source files: {rendered}"
    );
    assert_eq!(report.unresolved_method_calls, 0, "{rendered}");
    assert_eq!(report.unresolved_constructors, 0, "{rendered}");
    assert_eq!(report.unresolved_field_accesses, 0, "{rendered}");
    assert_eq!(report.external_api_explained_method_calls, 0, "{rendered}");
    assert_eq!(report.external_api_explained_constructors, 0, "{rendered}");
    assert!(rendered.contains("SourceSummary\t1"), "{rendered}");
}

#[test]
fn resolver_benchmark_attributes_configured_fact_provider_facts() {
    let root = unique_temp_root("dependency-mode-fact-provider");
    let app_root = root.join("app");
    fs::create_dir_all(app_root.join("org/example/gatk")).unwrap();
    fs::write(
        app_root.join("org/example/gatk/UsesSummary.java"),
        r#"
package org.example.gatk;

import external.summary.SummaryType;

class UsesSummary {
    Object read(SummaryType summary) {
        return summary.value();
    }
}
"#,
    )
    .unwrap();

    let mut provider_facts = FactSet::default();
    provider_facts
        .external_library_packs
        .push(ExternalLibraryPackFact {
            pack_name: "external.summary".to_string(),
            package_prefix: "SummaryType".to_string(),
            mode: ExternalLibraryMode::ExternalFactModel,
            coverage: ExternalApiCoverage::Semantic,
            rust_target: Some("crate::external::summary".to_string()),
            provenance: FactProvenance::ExternalLibraryPack,
        });
    provider_facts
        .external_api_effect_facts
        .push(ExternalApiEffectFact {
            receiver_type: Some("SummaryType".to_string()),
            method_name: "value".to_string(),
            role: ExternalApiRole::Getter,
            receiver_mutability: ReceiverMutabilityFact::ReadOnly,
            return_type: None,
            return_nullability: NullabilityState::DefinitelyNonNull,
            provenance: FactProvenance::ExternalLibraryPack,
        });

    let mut config = DependencyModeConfig::for_primary_source_root(&app_root);
    config.add_fact_provider(
        "external.summary",
        DependencyMode::SourceSummary,
        provider_facts,
    );
    let report = benchmark_java_resolver_config(&config).unwrap();
    let rendered = render_resolver_benchmark_report(&report);

    assert_eq!(report.external_pack_explained_method_calls, 1, "{rendered}");
    assert_eq!(
        report.external_fact_model_explained_method_calls, 1,
        "{rendered}"
    );
    assert_eq!(report.external_api_guesses, 0, "{rendered}");
    assert!(rendered.contains("fact_providers"), "{rendered}");
    assert!(
        rendered.contains("external.summary\tSourceSummary\t1"),
        "{rendered}"
    );
}

#[test]
fn co_translated_dependency_hierarchy_can_drive_closed_enum_representation() {
    let root = unique_temp_root("dependency-mode-object-model");
    let app_root = root.join("app");
    let dep_root = root.join("dep");
    fs::create_dir_all(app_root.join("app/usecase")).unwrap();
    fs::create_dir_all(dep_root.join("dep/model")).unwrap();

    let app_source = r#"
package app.usecase;

import dep.model.Pet;
import dep.model.Dog;

class PetUse {
    Pet create() {
        return new Dog();
    }
}
"#;
    let dep_source = r#"
package dep.model;

public sealed interface Pet permits Dog, Cat {}

public final class Dog implements Pet {
    public Dog() {}
}

public final class Cat implements Pet {
    public Cat() {}
}
"#;
    fs::write(app_root.join("app/usecase/PetUse.java"), app_source).unwrap();
    fs::write(dep_root.join("dep/model/Pet.java"), dep_source).unwrap();

    let mut config = DependencyModeConfig::for_primary_source_root(&app_root);
    config.add_source_root(&dep_root, DependencyMode::CoTranslateSource);
    let report = benchmark_java_resolver_config(&config).unwrap();
    let rendered = render_resolver_benchmark_report(&report);

    assert_eq!(report.files_seen, 2);
    assert_eq!(report.unresolved_method_calls, 0, "{rendered}");
    assert_eq!(report.unexplained_constructors, 0, "{rendered}");

    let app = parser::parse_java_source(app_source);
    let dep = parser::parse_java_source(dep_source);
    let mut merged = Program {
        package: dep.program.package.clone(),
        imports: dep.program.imports.clone(),
        types: dep.program.types.clone(),
    };
    merged.imports.extend(app.program.imports);
    merged.types.extend(app.program.types);
    let resolved = resolve::resolve_program(&merged, dep.diagnostics);
    let representations = plan_representations(&resolved.program, &resolved.facts);

    assert_eq!(
        representations.representation_for(&JavaType::Named {
            name: "Pet".to_string(),
            args: Vec::new(),
        }),
        ObjectRepresentation::ClosedEnum {
            enum_name: "PetValue".to_string(),
            variants: vec!["Cat".to_string(), "Dog".to_string()],
        }
    );
}

fn unique_temp_root(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "new_java2rust_{name}_{}_{}",
        std::process::id(),
        std::thread::current().name().unwrap_or("test")
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    root
}
