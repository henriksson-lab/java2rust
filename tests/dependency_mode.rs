use std::fs;
use std::path::{Path, PathBuf};

use new_java2rust::dependency_mode::{
    DependencyFactProviderError, DependencyMode, DependencyModeConfig,
};
use new_java2rust::external_facts::parse_external_facts;
use new_java2rust::external_packs::{
    hdf5_pack_facts, htsjdk_variantcontext_pack_facts, java_sql_pack_facts,
    parse_external_pack_manifest, sqlite_pack_facts,
};
use new_java2rust::facts::{
    ExternalApiCoverage, ExternalApiEffectFact, ExternalApiRole, ExternalFieldFact,
    ExternalFieldKind, ExternalLibraryMode, ExternalLibraryPackFact, FactProvenance, FactSet,
    FieldKey, HierarchyOverride, HierarchyStrategy, NullabilityState, ReceiverMutabilityFact,
};
use new_java2rust::java_ir::{JavaType, PrimitiveType, Program};
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
fn resolver_benchmark_can_disable_builtin_packs_and_readd_them_as_providers() {
    let root = unique_temp_root("dependency-mode-builtin-pack-policy");
    let app_root = root.join("app");
    fs::create_dir_all(app_root.join("org/example/gatk")).unwrap();
    fs::write(
        app_root.join("org/example/gatk/UsesVariantContext.java"),
        r#"
package org.example.gatk;

import htsjdk.variant.variantcontext.VariantContext;

class UsesVariantContext {
    String contig(VariantContext variant) {
        return variant.getContig();
    }
}
"#,
    )
    .unwrap();

    let disabled_config =
        DependencyModeConfig::for_primary_source_root(&app_root).without_builtin_external_packs();
    let disabled_report = benchmark_java_resolver_config(&disabled_config).unwrap();
    let disabled_rendered = render_resolver_benchmark_report(&disabled_report);

    assert_eq!(
        disabled_report.external_pack_manifest_coverage_diagnostics, 0,
        "{disabled_rendered}"
    );
    assert_eq!(
        disabled_report.external_pack_explained_method_calls, 0,
        "{disabled_rendered}"
    );
    assert_eq!(
        disabled_report.external_fact_model_explained_method_calls, 0,
        "{disabled_rendered}"
    );
    assert_eq!(
        disabled_report.external_pack_api_facts, 0,
        "without built-in packs or configured providers the call may use generic Java getter policy, but must not receive HTSJDK pack attribution: {disabled_rendered}"
    );

    let mut provider_config =
        DependencyModeConfig::for_primary_source_root(&app_root).without_builtin_external_packs();
    provider_config.add_fact_provider(
        "htsjdk.variant.variantcontext",
        DependencyMode::ExternalFactModel,
        htsjdk_variantcontext_pack_facts(),
    );
    let provider_report = benchmark_java_resolver_config(&provider_config).unwrap();
    let provider_rendered = render_resolver_benchmark_report(&provider_report);

    assert_eq!(
        provider_report.external_pack_manifest_coverage_diagnostics, 0,
        "{provider_rendered}"
    );
    assert_eq!(
        provider_report.external_pack_explained_method_calls, 1,
        "{provider_rendered}"
    );
    assert_eq!(
        provider_report.external_pack_api_facts, 0,
        "selected provider facts should be counted in fact_providers, not multiplied as generated per-file facts: {provider_rendered}"
    );
    assert_eq!(
        provider_report.external_fact_model_explained_method_calls, 1,
        "{provider_rendered}"
    );
    assert_eq!(
        provider_report.external_api_guesses, 0,
        "{provider_rendered}"
    );
    assert!(
        provider_rendered.contains("htsjdk.variant.variantcontext\tExternalFactModel"),
        "{provider_rendered}"
    );
    assert!(
        provider_rendered.contains("VariantContext\tgetContig\tExternalFactModel\t1"),
        "top external-pack API methods should summarize explained call sites, not all available provider facts: {provider_rendered}"
    );

    let builtin_report =
        benchmark_java_resolver_config(&DependencyModeConfig::for_primary_source_root(&app_root))
            .unwrap();
    let builtin_rendered = render_resolver_benchmark_report(&builtin_report);
    assert_eq!(
        builtin_report.external_pack_manifest_coverage_diagnostics, 0,
        "{builtin_rendered}"
    );
    assert!(
        builtin_rendered.contains("external_pack_manifest_coverage_diagnostics\t0"),
        "{builtin_rendered}"
    );
    assert!(
        !builtin_rendered.contains("top_external_pack_manifest_coverage_diagnostics"),
        "{builtin_rendered}"
    );
}

#[test]
fn resolver_benchmark_compares_baseline_fact_model_and_crate_mapping_modes() {
    let root = unique_temp_root("dependency-mode-benchmark-modes");
    let app_root = root.join("app");
    fs::create_dir_all(app_root.join("org/example/gatk")).unwrap();
    fs::write(
        app_root.join("org/example/gatk/DependencyModeProbe.java"),
        r#"
package org.example.gatk;

import java.io.File;
import htsjdk.variant.variantcontext.Genotype;
import htsjdk.variant.variantcontext.GenotypesContext;
import htsjdk.variant.variantcontext.VariantContext;
import org.sqlite.SQLiteConfig;

class DependencyModeProbe {
    String contig(VariantContext variant) {
        return variant.getContig();
    }

    int genotypeCount(GenotypesContext genotypes) {
        Genotype first = genotypes.get(0);
        return genotypes.size();
    }

    double metric(File file) {
        HDF5File hdf5 = new HDF5File(file, HDF5File.OpenMode.CREATE);
        return hdf5.readDouble("/metrics/value");
    }

    Object connect(String path) {
        SQLiteConfig config = new SQLiteConfig();
        config.setReadOnly(true);
        return config.createConnection(path);
    }
}
"#,
    )
    .unwrap();

    let baseline_config =
        DependencyModeConfig::for_primary_source_root(&app_root).without_builtin_external_packs();
    let baseline_report = benchmark_java_resolver_config(&baseline_config).unwrap();
    let baseline_rendered = render_resolver_benchmark_report(&baseline_report);
    assert_eq!(
        baseline_report.external_fact_model_explained_method_calls, 0,
        "{baseline_rendered}"
    );
    assert_eq!(
        baseline_report.rust_crate_mapping_explained_method_calls, 0,
        "{baseline_rendered}"
    );
    assert_eq!(
        baseline_report.external_pack_explained_method_calls, 0,
        "{baseline_rendered}"
    );

    let mut htsjdk_config =
        DependencyModeConfig::for_primary_source_root(&app_root).without_builtin_external_packs();
    htsjdk_config
        .add_external_pack_provider(
            "builtin-pack:htsjdk.variant.variantcontext",
            htsjdk_variantcontext_pack_facts(),
        )
        .unwrap();
    let htsjdk_report = benchmark_java_resolver_config(&htsjdk_config).unwrap();
    let htsjdk_rendered = render_resolver_benchmark_report(&htsjdk_report);
    assert_eq!(
        htsjdk_report.external_fact_model_explained_method_calls, 3,
        "{htsjdk_rendered}"
    );
    assert_eq!(
        htsjdk_report.rust_crate_mapping_explained_method_calls, 0,
        "{htsjdk_rendered}"
    );
    assert!(
        htsjdk_rendered.contains("VariantContext\tgetContig\tExternalFactModel\t1"),
        "{htsjdk_rendered}"
    );
    assert!(
        htsjdk_rendered.contains("GenotypesContext\tget\tExternalFactModel\t1"),
        "{htsjdk_rendered}"
    );
    assert!(
        htsjdk_rendered.contains("GenotypesContext\tsize\tExternalFactModel\t1"),
        "{htsjdk_rendered}"
    );

    let mut crate_config =
        DependencyModeConfig::for_primary_source_root(&app_root).without_builtin_external_packs();
    crate_config
        .add_external_pack_provider("builtin-pack:hdf5", hdf5_pack_facts())
        .unwrap();
    crate_config
        .add_external_pack_provider("builtin-pack:sqlite", sqlite_pack_facts())
        .unwrap();
    let crate_report = benchmark_java_resolver_config(&crate_config).unwrap();
    let crate_rendered = render_resolver_benchmark_report(&crate_report);
    assert_eq!(
        crate_report.external_fact_model_explained_method_calls, 0,
        "{crate_rendered}"
    );
    assert_eq!(
        crate_report.rust_crate_mapping_explained_method_calls, 3,
        "{crate_rendered}"
    );
    assert_eq!(
        crate_report.rust_crate_mapping_explained_constructors, 2,
        "{crate_rendered}"
    );
    assert!(
        crate_rendered.contains("HDF5File\treadDouble\tRustCrateMapping\t1"),
        "{crate_rendered}"
    );
    assert!(
        crate_rendered.contains("SQLiteConfig\tcreateConnection\tRustCrateMapping\t1"),
        "{crate_rendered}"
    );
    assert!(
        crate_rendered.contains("SQLiteConfig\tsetReadOnly\tRustCrateMapping\t1"),
        "{crate_rendered}"
    );
    assert!(
        crate_rendered.contains("HDF5File\t2\tRustCrateMapping\t1"),
        "{crate_rendered}"
    );
}

#[test]
fn resolver_benchmark_reports_explained_external_pack_field_contexts() {
    let root = unique_temp_root("dependency-mode-explained-pack-field-contexts");
    let app_root = root.join("app");
    fs::create_dir_all(app_root.join("org/example/gatk")).unwrap();
    fs::write(
        app_root.join("org/example/gatk/UsesHdf5OpenMode.java"),
        r#"
package org.example.gatk;

class UsesHdf5OpenMode {
    Object createMode() {
        return HDF5File.OpenMode.CREATE;
    }
}
"#,
    )
    .unwrap();

    let mut config =
        DependencyModeConfig::for_primary_source_root(&app_root).without_builtin_external_packs();
    config
        .add_external_pack_provider("builtin-pack:hdf5", hdf5_pack_facts())
        .unwrap();
    let report = benchmark_java_resolver_config(&config).unwrap();
    let rendered = render_resolver_benchmark_report(&report);

    assert_eq!(
        report.external_api_explained_field_accesses, 2,
        "{rendered}"
    );
    assert_eq!(report.unexplained_field_accesses, 0, "{rendered}");
    assert!(
        rendered.contains("top_external_pack_field_accesses"),
        "{rendered}"
    );
    assert!(
        rendered.contains("HDF5File\tOpenMode\tRustCrateMapping\t1"),
        "{rendered}"
    );
    assert!(
        rendered.contains("OpenMode\tCREATE\tRustCrateMapping\t1"),
        "{rendered}"
    );
    assert!(
        rendered.contains("top_external_pack_field_contexts"),
        "{rendered}"
    );
    assert!(
        rendered.contains(
            "OpenMode\tHDF5File\tRustCrateMapping\tUsesHdf5OpenMode\tcreateMode\t1\torg/example/gatk/UsesHdf5OpenMode.java"
        ),
        "{rendered}"
    );
    assert!(
        rendered.contains(
            "CREATE\tOpenMode\tRustCrateMapping\tUsesHdf5OpenMode\tcreateMode\t1\torg/example/gatk/UsesHdf5OpenMode.java"
        ),
        "{rendered}"
    );
}

#[test]
fn resolver_benchmark_reports_resolved_pack_fields_across_statement_shapes() {
    let root = unique_temp_root("dependency-mode-pack-field-context-statement-shapes");
    let app_root = root.join("app");
    fs::create_dir_all(app_root.join("org/example/gatk")).unwrap();
    fs::write(
        app_root.join("org/example/gatk/UsesHdf5OpenModeStatements.java"),
        r#"
package org.example.gatk;

import java.util.Arrays;

class UsesHdf5OpenModeStatements {
    Object createMode(boolean choose, int count) {
        Object mode = null;
        mode = HDF5File.OpenMode.CREATE;
        for (int i = 0; i < count; i = i + 1) {
            mode = HDF5File.OpenMode.READ_ONLY;
        }
        for (Object item : Arrays.asList(HDF5File.OpenMode.CREATE)) {
            mode = item;
        }
        switch (count) {
            case 0:
                mode = HDF5File.OpenMode.READ_ONLY;
                break;
            default:
                mode = HDF5File.OpenMode.CREATE;
        }
        try {
            mode = HDF5File.OpenMode.READ_ONLY;
        } catch (RuntimeException ex) {
            mode = HDF5File.OpenMode.CREATE;
        } finally {
            mode = HDF5File.OpenMode.READ_ONLY;
        }
        return mode;
    }
}
"#,
    )
    .unwrap();

    let mut config =
        DependencyModeConfig::for_primary_source_root(&app_root).without_builtin_external_packs();
    config
        .add_external_pack_provider("builtin-pack:hdf5", hdf5_pack_facts())
        .unwrap();
    let report = benchmark_java_resolver_config(&config).unwrap();
    let rendered = render_resolver_benchmark_report(&report);

    assert_eq!(report.parser_diagnostics, 0, "{rendered}");
    assert_eq!(report.resolver_diagnostics, 0, "{rendered}");
    assert_eq!(
        report.external_api_explained_field_accesses, 16,
        "{rendered}"
    );
    assert_eq!(report.unexplained_field_accesses, 0, "{rendered}");
    assert!(
        rendered.contains(
            "OpenMode\tHDF5File\tRustCrateMapping\tUsesHdf5OpenModeStatements\tcreateMode\t8\torg/example/gatk/UsesHdf5OpenModeStatements.java"
        ),
        "{rendered}"
    );
    assert!(
        rendered.contains(
            "READ_ONLY\tOpenMode\tRustCrateMapping\tUsesHdf5OpenModeStatements\tcreateMode\t4\torg/example/gatk/UsesHdf5OpenModeStatements.java"
        ),
        "{rendered}"
    );
    assert!(
        rendered.contains(
            "CREATE\tOpenMode\tRustCrateMapping\tUsesHdf5OpenModeStatements\tcreateMode\t4\torg/example/gatk/UsesHdf5OpenModeStatements.java"
        ),
        "{rendered}"
    );
}

#[test]
fn resolver_benchmark_does_not_treat_lambda_params_as_static_pack_receivers() {
    let root = unique_temp_root("dependency-mode-pack-field-lambda-param-shadow");
    let app_root = root.join("app");
    fs::create_dir_all(app_root.join("org/example/gatk")).unwrap();
    fs::write(
        app_root.join("org/example/gatk/UsesHdf5OpenModeLambda.java"),
        r#"
package org.example.gatk;

import java.util.List;

class UsesHdf5OpenModeLambda {
    Object createMode(List<Object> values) {
        Object direct = HDF5File.OpenMode.CREATE;
        values.stream().map(HDF5File -> HDF5File.OpenMode.CREATE);
        return direct;
    }
}
"#,
    )
    .unwrap();

    let mut config =
        DependencyModeConfig::for_primary_source_root(&app_root).without_builtin_external_packs();
    config
        .add_external_pack_provider("builtin-pack:hdf5", hdf5_pack_facts())
        .unwrap();
    let report = benchmark_java_resolver_config(&config).unwrap();
    let rendered = render_resolver_benchmark_report(&report);

    assert_eq!(report.parser_diagnostics, 0, "{rendered}");
    assert_eq!(report.resolver_diagnostics, 0, "{rendered}");
    assert_eq!(
        report.external_api_explained_field_accesses, 2,
        "only the direct static field chain should be attributed to the HDF5 pack; the lambda parameter shadows the type-like owner: {rendered}"
    );
    assert!(
        rendered.contains(
            "OpenMode\tHDF5File\tRustCrateMapping\tUsesHdf5OpenModeLambda\tcreateMode\t1\torg/example/gatk/UsesHdf5OpenModeLambda.java"
        ),
        "{rendered}"
    );
    assert!(
        rendered.contains(
            "CREATE\tOpenMode\tRustCrateMapping\tUsesHdf5OpenModeLambda\tcreateMode\t1\torg/example/gatk/UsesHdf5OpenModeLambda.java"
        ),
        "{rendered}"
    );
}

#[test]
fn resolver_benchmark_reports_pack_fields_inside_anonymous_class_bodies() {
    let root = unique_temp_root("dependency-mode-pack-field-anonymous-body");
    let app_root = root.join("app");
    fs::create_dir_all(app_root.join("org/example/gatk")).unwrap();
    fs::write(
        app_root.join("org/example/gatk/UsesHdf5OpenModeAnonymous.java"),
        r#"
package org.example.gatk;

class UsesHdf5OpenModeAnonymous {
    Object createMode() {
        return new Object() {
            Object mode() {
                return HDF5File.OpenMode.CREATE;
            }
        };
    }
}
"#,
    )
    .unwrap();

    let mut config =
        DependencyModeConfig::for_primary_source_root(&app_root).without_builtin_external_packs();
    config
        .add_external_pack_provider("builtin-pack:hdf5", hdf5_pack_facts())
        .unwrap();
    let report = benchmark_java_resolver_config(&config).unwrap();
    let rendered = render_resolver_benchmark_report(&report);

    assert_eq!(report.parser_diagnostics, 0, "{rendered}");
    assert_eq!(report.resolver_diagnostics, 0, "{rendered}");
    assert_eq!(
        report.external_api_explained_field_accesses, 2,
        "anonymous class bodies should be walked for resolved external-pack field provenance: {rendered}"
    );
    assert!(
        rendered.contains(
            "OpenMode\tHDF5File\tRustCrateMapping\tUsesHdf5OpenModeAnonymous\tmode\t1\torg/example/gatk/UsesHdf5OpenModeAnonymous.java"
        ),
        "{rendered}"
    );
    assert!(
        rendered.contains(
            "CREATE\tOpenMode\tRustCrateMapping\tUsesHdf5OpenModeAnonymous\tmode\t1\torg/example/gatk/UsesHdf5OpenModeAnonymous.java"
        ),
        "{rendered}"
    );
}

#[test]
fn resolver_benchmark_reports_pack_methods_inside_anonymous_class_bodies() {
    let root = unique_temp_root("dependency-mode-pack-method-anonymous-body");
    let app_root = root.join("app");
    fs::create_dir_all(app_root.join("org/example/gatk")).unwrap();
    fs::write(
        app_root.join("org/example/gatk/UsesHdf5MethodAnonymous.java"),
        r#"
package org.example.gatk;

class UsesHdf5MethodAnonymous {
    Object createReader(HDF5File hdf5) {
        return new Object() {
            double readMetric() {
                return hdf5.readDouble("/metrics/value");
            }
        };
    }
}
"#,
    )
    .unwrap();

    let mut config =
        DependencyModeConfig::for_primary_source_root(&app_root).without_builtin_external_packs();
    config
        .add_external_pack_provider("builtin-pack:hdf5", hdf5_pack_facts())
        .unwrap();
    let report = benchmark_java_resolver_config(&config).unwrap();
    let rendered = render_resolver_benchmark_report(&report);

    assert_eq!(report.parser_diagnostics, 0, "{rendered}");
    assert_eq!(report.resolver_diagnostics, 0, "{rendered}");
    assert_eq!(
        report.external_pack_explained_method_calls, 1,
        "anonymous class bodies should preserve external-pack method provenance: {rendered}"
    );
    assert!(
        rendered.contains(
            "HDF5File\treadDouble\tRustCrateMapping\t1\tUsesHdf5MethodAnonymous\treadMetric\t1\torg/example/gatk/UsesHdf5MethodAnonymous.java"
        ),
        "{rendered}"
    );
}

#[test]
fn resolver_benchmark_reports_pack_constructors_inside_anonymous_class_bodies() {
    let root = unique_temp_root("dependency-mode-pack-constructor-anonymous-body");
    let app_root = root.join("app");
    fs::create_dir_all(app_root.join("org/example/gatk")).unwrap();
    fs::write(
        app_root.join("org/example/gatk/UsesHdf5ConstructorAnonymous.java"),
        r#"
package org.example.gatk;

import java.io.File;

class UsesHdf5ConstructorAnonymous {
    Object createFactory(File file) {
        return new Object() {
            HDF5File open() {
                return new HDF5File(file);
            }
        };
    }
}
"#,
    )
    .unwrap();

    let mut config =
        DependencyModeConfig::for_primary_source_root(&app_root).without_builtin_external_packs();
    config
        .add_external_pack_provider("builtin-pack:hdf5", hdf5_pack_facts())
        .unwrap();
    let report = benchmark_java_resolver_config(&config).unwrap();
    let rendered = render_resolver_benchmark_report(&report);

    assert_eq!(report.parser_diagnostics, 0, "{rendered}");
    assert_eq!(report.resolver_diagnostics, 0, "{rendered}");
    assert_eq!(
        report.external_pack_explained_constructors, 1,
        "anonymous class bodies should preserve external-pack constructor provenance: {rendered}"
    );
    assert!(
        rendered.contains(
            "HDF5File\tHDF5File\tRustCrateMapping\tFile\tUsesHdf5ConstructorAnonymous\topen\t1\torg/example/gatk/UsesHdf5ConstructorAnonymous.java"
        ),
        "{rendered}"
    );
}

#[test]
fn resolver_benchmark_sqlite_pack_covers_gatk_shaped_database_workflow() {
    let root = unique_temp_root("dependency-mode-sqlite-workflow-pack");
    let app_root = root.join("app");
    fs::create_dir_all(app_root.join("org/example/gatk")).unwrap();
    fs::write(
        app_root.join("org/example/gatk/SqliteWorkflowProbe.java"),
        r#"
package org.example.gatk;

import org.sqlite.SQLiteConfig;
import org.sqlite.SQLiteConnection;
import org.sqlite.SQLiteResultSet;
import org.sqlite.SQLiteStatement;

class SqliteWorkflowProbe {
    Object configure() {
        SQLiteConfig config = new SQLiteConfig();
        config.setReadOnly(false);
        config.setBusyTimeout(1000);
        config.setSharedCache(true);
        config.setJournalMode("WAL");
        return config.toProperties();
    }

    void transact(String path, String query) {
        SQLiteConfig config = new SQLiteConfig();
        SQLiteConnection connection = config.createConnection(path);
        SQLiteStatement prepared = connection.prepareStatement(query);
        prepared.bindString(1, "sample");
        prepared.bindInt(2, 10);
        prepared.bindDouble(3, 99.5);
        prepared.execute();
        prepared.executeUpdate();
        prepared.clearParameters();
        SQLiteResultSet results = prepared.executeQuery();
        if (results.next()) {
            results.getString("sample");
            results.getInt(1);
            results.getLong(2);
            results.getBoolean("active");
            results.getDouble("score");
        }
        results.close();
        prepared.close();
        SQLiteStatement direct = connection.createStatement();
        direct.execute();
        connection.setAutoCommit(false);
        connection.commit();
        connection.rollback();
        connection.close();
    }
}
"#,
    )
    .unwrap();

    let baseline_config =
        DependencyModeConfig::for_primary_source_root(&app_root).without_builtin_external_packs();
    let baseline_report = benchmark_java_resolver_config(&baseline_config).unwrap();
    let baseline_rendered = render_resolver_benchmark_report(&baseline_report);
    assert_eq!(
        baseline_report.rust_crate_mapping_explained_method_calls, 0,
        "{baseline_rendered}"
    );
    assert_eq!(
        baseline_report.rust_crate_mapping_explained_constructors, 0,
        "{baseline_rendered}"
    );

    let mut sqlite_config =
        DependencyModeConfig::for_primary_source_root(&app_root).without_builtin_external_packs();
    sqlite_config
        .add_external_pack_provider("builtin-pack:sqlite", sqlite_pack_facts())
        .unwrap();
    let sqlite_report = benchmark_java_resolver_config(&sqlite_config).unwrap();
    let sqlite_rendered = render_resolver_benchmark_report(&sqlite_report);
    assert_eq!(
        sqlite_report.rust_crate_mapping_explained_method_calls, 28,
        "{sqlite_rendered}"
    );
    assert_eq!(
        sqlite_report.rust_crate_mapping_explained_constructors, 2,
        "{sqlite_rendered}"
    );
    assert_eq!(
        sqlite_report.external_pack_manifest_coverage_diagnostics, 0,
        "{sqlite_rendered}"
    );
    assert!(
        sqlite_report.external_api_argument_effect_facts >= 1,
        "SQLite bind/getter argument boundary facts should be visible as a benchmark comparison axis: {sqlite_rendered}"
    );
    assert!(
        sqlite_rendered.contains("external_api_argument_effect_facts\t"),
        "{sqlite_rendered}"
    );
    assert!(
        sqlite_rendered.contains("SQLiteConfig\tsetReadOnly\tRustCrateMapping\t1"),
        "{sqlite_rendered}"
    );
    assert!(
        sqlite_rendered.contains("SQLiteConfig\tcreateConnection\tRustCrateMapping\t1"),
        "{sqlite_rendered}"
    );
    assert!(
        sqlite_rendered.contains("SQLiteConnection\tprepareStatement\tRustCrateMapping\t1"),
        "{sqlite_rendered}"
    );
    assert!(
        sqlite_rendered.contains("SQLiteConnection\tcreateStatement\tRustCrateMapping\t1"),
        "{sqlite_rendered}"
    );
    assert!(
        sqlite_rendered.contains("SQLiteConnection\tsetAutoCommit\tRustCrateMapping\t1"),
        "{sqlite_rendered}"
    );
    assert!(
        sqlite_rendered.contains("SQLiteResultSet\tnext\tRustCrateMapping\t1"),
        "{sqlite_rendered}"
    );
    assert!(
        sqlite_rendered.contains("SQLiteResultSet\tgetString\tRustCrateMapping\t1"),
        "{sqlite_rendered}"
    );
    assert!(
        sqlite_rendered.contains("SQLiteResultSet\tgetLong\tRustCrateMapping\t1"),
        "{sqlite_rendered}"
    );
    assert!(
        sqlite_rendered.contains("SQLiteResultSet\tgetBoolean\tRustCrateMapping\t1"),
        "{sqlite_rendered}"
    );
    assert!(
        sqlite_rendered.contains("SQLiteResultSet\tclose\tRustCrateMapping\t1"),
        "{sqlite_rendered}"
    );
    assert!(
        !sqlite_rendered.contains("external_pack_missing_api"),
        "{sqlite_rendered}"
    );
}

#[test]
fn resolver_benchmark_java_sql_pack_composes_with_sqlite_configuration_workflow() {
    let root = unique_temp_root("dependency-mode-java-sql-sqlite-workflow");
    let app_root = root.join("app");
    fs::create_dir_all(app_root.join("org/example/gatk")).unwrap();
    fs::write(
        app_root.join("org/example/gatk/JdbcSqliteWorkflowProbe.java"),
        r#"
package org.example.gatk;

import org.sqlite.SQLiteConfig;
import java.sql.*;

class JdbcSqliteWorkflowProbe {
    void dump(String path, String catalog, String schemaPattern, String tableName, String[] tableTypes) {
        SQLiteConfig config = new SQLiteConfig();
        config.setReadOnly(true);
        Connection connection = DriverManager.getConnection("jdbc:sqlite:" + path, config.toProperties());
        DatabaseMetaData database = connection.getMetaData();
        String product = database.getDatabaseProductName();
        ResultSet tables = database.getTables(catalog, schemaPattern, tableName, tableTypes);
        while (tables.next()) {
            tables.getString("TABLE_NAME");
        }
        tables.close();
        ResultSet columns = database.getColumns(catalog, schemaPattern, tableName, "GENE");
        while (columns.next()) {
            columns.getString("COLUMN_NAME");
        }
        columns.close();
        Statement statement = connection.createStatement();
        ResultSet resultSet = statement.executeQuery("select gene, score from variants");
        ResultSetMetaData metadata = resultSet.getMetaData();
        int columnCount = metadata.getColumnCount();
        for (int i = 1; i <= columnCount; i++) {
            metadata.getColumnName(i);
            metadata.getColumnType(i);
            metadata.getColumnTypeName(i);
            metadata.getColumnClassName(i);
            metadata.isNullable(i);
            metadata.isSigned(i);
            metadata.isAutoIncrement(i);
            metadata.getPrecision(i);
            metadata.getScale(i);
            metadata.getColumnDisplaySize(i);
        }
        PreparedStatement update = connection.prepareStatement("update variants set score = ?, depth = ?, active = ? where gene = ?");
        update.setDouble(1, 7.5);
        update.setString(2, path);
        update.setInt(3, columnCount);
        update.setLong(4, 123L);
        update.setBoolean(5, true);
        update.executeUpdate();
        update.close();
        while (resultSet.next()) {
            resultSet.getString(1);
            resultSet.getString("gene");
            resultSet.getInt("depth");
            resultSet.getLong(2);
            resultSet.getDouble("score");
            resultSet.getFloat("quality");
            resultSet.getShort(3);
            resultSet.getByte("flags");
            resultSet.getBoolean("active");
            resultSet.getObject("payload");
            resultSet.getBytes("blob");
            resultSet.getDate("created_on");
            resultSet.getTimestamp("updated_at");
            resultSet.wasNull();
        }
        resultSet.close();
        statement.close();
        connection.close();
    }
}
"#,
    )
    .unwrap();

    let baseline_config =
        DependencyModeConfig::for_primary_source_root(&app_root).without_builtin_external_packs();
    let baseline_report = benchmark_java_resolver_config(&baseline_config).unwrap();
    let baseline_rendered = render_resolver_benchmark_report(&baseline_report);
    assert_eq!(
        baseline_report.external_fact_model_explained_method_calls, 0,
        "{baseline_rendered}"
    );
    assert_eq!(
        baseline_report.rust_crate_mapping_explained_method_calls, 0,
        "{baseline_rendered}"
    );

    let mut sqlite_only_config =
        DependencyModeConfig::for_primary_source_root(&app_root).without_builtin_external_packs();
    sqlite_only_config
        .add_external_pack_provider("builtin-pack:sqlite", sqlite_pack_facts())
        .unwrap();
    let sqlite_only_report = benchmark_java_resolver_config(&sqlite_only_config).unwrap();
    let sqlite_only_rendered = render_resolver_benchmark_report(&sqlite_only_report);
    assert_eq!(
        sqlite_only_report.rust_crate_mapping_explained_method_calls, 2,
        "{sqlite_only_rendered}"
    );
    assert_eq!(
        sqlite_only_report.external_fact_model_explained_method_calls, 0,
        "{sqlite_only_rendered}"
    );
    assert!(
        sqlite_only_rendered.contains("SQLiteConfig\tsetReadOnly\tRustCrateMapping\t1"),
        "{sqlite_only_rendered}"
    );
    assert!(
        sqlite_only_rendered.contains("SQLiteConfig\ttoProperties\tRustCrateMapping\t1"),
        "{sqlite_only_rendered}"
    );

    let mut composed_config =
        DependencyModeConfig::for_primary_source_root(&app_root).without_builtin_external_packs();
    composed_config
        .add_external_pack_provider("builtin-pack:sqlite", sqlite_pack_facts())
        .unwrap();
    composed_config
        .add_external_pack_provider("builtin-pack:java.sql", java_sql_pack_facts())
        .unwrap();
    let composed_report = benchmark_java_resolver_config(&composed_config).unwrap();
    let composed_rendered = render_resolver_benchmark_report(&composed_report);
    assert_eq!(
        composed_report.rust_crate_mapping_explained_method_calls, 2,
        "{composed_rendered}"
    );
    assert_eq!(
        composed_report.external_fact_model_explained_method_calls, 51,
        "{composed_rendered}"
    );
    assert_eq!(
        composed_report.external_pack_manifest_coverage_diagnostics, 0,
        "{composed_rendered}"
    );
    assert!(
        composed_rendered.contains("driver_manager\tgetConnection\tExternalFactModel\t1"),
        "{composed_rendered}"
    );
    assert!(
        composed_rendered.contains("java.sql.Connection\tcreateStatement\tExternalFactModel\t1"),
        "{composed_rendered}"
    );
    assert!(
        composed_rendered.contains("java.sql.Connection\tprepareStatement\tExternalFactModel\t1"),
        "{composed_rendered}"
    );
    assert!(
        composed_rendered.contains("java.sql.Connection\tgetMetaData\tExternalFactModel\t1"),
        "{composed_rendered}"
    );
    assert!(
        composed_rendered.contains("java.sql.DatabaseMetaData\tgetTables\tExternalFactModel\t1"),
        "{composed_rendered}"
    );
    assert!(
        composed_rendered.contains("java.sql.DatabaseMetaData\tgetColumns\tExternalFactModel\t1"),
        "{composed_rendered}"
    );
    assert!(
        composed_rendered.contains("java.sql.PreparedStatement\tsetDouble\tExternalFactModel\t1"),
        "{composed_rendered}"
    );
    assert!(
        composed_rendered.contains("java.sql.PreparedStatement\tsetString\tExternalFactModel\t1"),
        "{composed_rendered}"
    );
    assert!(
        composed_rendered.contains("java.sql.PreparedStatement\tsetInt\tExternalFactModel\t1"),
        "{composed_rendered}"
    );
    assert!(
        composed_rendered.contains("java.sql.PreparedStatement\tsetLong\tExternalFactModel\t1"),
        "{composed_rendered}"
    );
    assert!(
        composed_rendered.contains("java.sql.PreparedStatement\tsetBoolean\tExternalFactModel\t1"),
        "{composed_rendered}"
    );
    assert!(
        composed_rendered
            .contains("java.sql.PreparedStatement\texecuteUpdate\tExternalFactModel\t1"),
        "{composed_rendered}"
    );
    assert!(
        composed_rendered.contains("java.sql.ResultSet\tnext\tExternalFactModel\t3"),
        "{composed_rendered}"
    );
    assert!(
        !composed_rendered.contains("external_pack_missing_api"),
        "{composed_rendered}"
    );
}

#[test]
fn resolver_benchmark_java_sql_pack_reports_missing_owned_members() {
    let root = unique_temp_root("dependency-mode-java-sql-missing-members");
    let app_root = root.join("app");
    fs::create_dir_all(app_root.join("org/example/gatk")).unwrap();
    fs::write(
        app_root.join("org/example/gatk/MissingJdbcBoundaryProbe.java"),
        r#"
package org.example.gatk;

import java.sql.*;

class MissingJdbcBoundaryProbe {
    Object inspect(Connection connection, ResultSet results) {
        connection.unmodeledConnectionMethod();
        results.unmodeledResultSetMethod();
        Object opened = new Connection();
        return results.unmodeledColumn;
    }
}
"#,
    )
    .unwrap();

    let mut config =
        DependencyModeConfig::for_primary_source_root(&app_root).without_builtin_external_packs();
    config
        .add_external_pack_provider("builtin-pack:java.sql", java_sql_pack_facts())
        .unwrap();
    let report = benchmark_java_resolver_config(&config).unwrap();
    let rendered = render_resolver_benchmark_report(&report);

    assert_eq!(report.external_api_missing_method_calls, 2, "{rendered}");
    assert_eq!(
        report.external_api_missing_constructor_calls, 1,
        "{rendered}"
    );
    assert_eq!(report.external_api_missing_field_accesses, 1, "{rendered}");
    assert_eq!(
        report.unexplained_method_calls, 0,
        "missing JDBC methods should stay pack-owned instead of becoming generic guesses: {rendered}"
    );
    assert_eq!(
        report.unexplained_constructors, 0,
        "missing JDBC constructors should stay pack-owned instead of becoming generic guesses: {rendered}"
    );
    assert_eq!(
        report.unexplained_field_accesses, 0,
        "missing JDBC fields should stay pack-owned instead of becoming generic guesses: {rendered}"
    );
    assert!(
        rendered.contains("java.sql.Connection\tunmodeledConnectionMethod\tExternalFactModel\t1"),
        "{rendered}"
    );
    assert!(
        rendered.contains("java.sql.ResultSet\tunmodeledResultSetMethod\tExternalFactModel\t1"),
        "{rendered}"
    );
    assert!(
        rendered.contains("java.sql.Connection\t0\tExternalFactModel\t1"),
        "{rendered}"
    );
    assert!(
        rendered.contains("java.sql.ResultSet\tunmodeledColumn\tExternalFactModel\t1"),
        "{rendered}"
    );
}

#[test]
fn resolver_benchmark_counts_builtin_fluent_builder_facts() {
    let root = unique_temp_root("dependency-mode-builder-fact-counts");
    let app_root = root.join("app");
    fs::create_dir_all(app_root.join("org/example/gatk")).unwrap();
    fs::write(
        app_root.join("org/example/gatk/BuilderProbe.java"),
        r#"
package org.example.gatk;

import htsjdk.variant.variantcontext.VariantContextBuilder;

class BuilderProbe {
    VariantContextBuilder contig(VariantContextBuilder builder, String contig) {
        return builder.chr(contig);
    }
}
"#,
    )
    .unwrap();

    let report =
        benchmark_java_resolver_config(&DependencyModeConfig::for_primary_source_root(&app_root))
            .unwrap();
    let rendered = render_resolver_benchmark_report(&report);

    assert_eq!(report.semantic_diagnostics, 0, "{rendered}");
    assert!(
        report.external_api_fluent_builder_facts >= 1,
        "builtin fluent builder facts should be visible as a benchmark comparison axis: {rendered}"
    );
    assert!(
        rendered.contains("external_api_fluent_builder_facts\t"),
        "{rendered}"
    );
    assert!(
        rendered.contains("external_api_argument_effect_facts\t"),
        "benchmark reports should expose API argument boundary facts as a comparison axis: {rendered}"
    );
    assert!(
        rendered.contains("elapsed_ms\t"),
        "benchmark reports should expose saturation/runtime timing as a comparison axis: {rendered}"
    );
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
        return allele.length + allele.inheritedLength + allele.inheritedScore();
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
    public int inheritedLength;

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
    assert!(
        report.source_dependency_resolved_method_calls >= 2,
        "cross-root source method resolutions should be reported: {rendered}"
    );
    assert!(
        report.source_dependency_resolved_constructors >= 1,
        "cross-root source constructor resolutions should be reported: {rendered}"
    );
    assert!(
        report.source_dependency_resolved_field_accesses >= 2,
        "cross-root source field resolutions should be reported: {rendered}"
    );
    assert!(
        rendered.contains("source_dependency_resolved_method_calls\t"),
        "{rendered}"
    );
    assert!(
        rendered.contains("source_dependency_resolved_constructors\t"),
        "{rendered}"
    );
    assert!(
        rendered.contains("source_dependency_resolved_field_accesses\t"),
        "{rendered}"
    );
    assert!(
        report
            .top_source_dependency_method_calls
            .iter()
            .any(|summary| summary.receiver_type == "AlleleLike"
                && summary.method_name == "label_box"
                && summary.source_mode == DependencyMode::CoTranslateSource),
        "source dependency method summary should identify resolved dependency receivers: {rendered}"
    );
    assert!(
        report
            .top_source_dependency_constructor_types
            .iter()
            .any(|summary| summary.type_name == "AlleleLike"
                && summary.arity == 1
                && summary.source_mode == DependencyMode::CoTranslateSource),
        "source dependency constructor summary should identify resolved dependency constructors: {rendered}"
    );
    assert!(
        report
            .top_source_dependency_field_accesses
            .iter()
            .any(|summary| summary.type_name == "AlleleLike"
                && summary.field_name == "length"
                && summary.source_mode == DependencyMode::CoTranslateSource),
        "source dependency field summary should identify resolved dependency fields: {rendered}"
    );
    assert!(
        report
            .top_source_dependency_field_accesses
            .iter()
            .any(|summary| summary.type_name == "AlleleBase"
                && summary.field_name == "inherited_length"
                && summary.source_mode == DependencyMode::CoTranslateSource),
        "source dependency field summary should identify inherited dependency fields by declaring type: {rendered}"
    );
    assert!(
        report
            .top_source_dependency_field_contexts
            .iter()
            .any(|summary| summary.declaring_type == "AlleleBase"
                && summary.field_name == "inherited_length"
                && summary.receiver_type == "AlleleLike"
                && summary.enclosing_type == "UsesVariant"
                && summary.enclosing_member == "score"),
        "source dependency field context should show where inherited dependency fields are consumed: {rendered}"
    );
    assert!(
        rendered.contains("top_source_dependency_method_calls"),
        "{rendered}"
    );
    assert!(
        rendered.contains("AlleleLike\tlabel_box\tCoTranslateSource"),
        "{rendered}"
    );
    assert!(
        rendered.contains("top_source_dependency_constructor_types"),
        "{rendered}"
    );
    assert!(
        rendered.contains("AlleleLike\t1\tCoTranslateSource"),
        "{rendered}"
    );
    assert!(
        rendered.contains("top_source_dependency_field_accesses"),
        "{rendered}"
    );
    assert!(
        rendered.contains("AlleleBase\tinherited_length\tCoTranslateSource"),
        "{rendered}"
    );
    assert!(
        rendered.contains("top_source_dependency_field_contexts"),
        "{rendered}"
    );
    assert!(
        rendered.contains(
            "AlleleBase\tinherited_length\tAlleleLike\tCoTranslateSource\tUsesVariant\tscore"
        ),
        "{rendered}"
    );
    assert!(rendered.contains("source_roots"));
    assert!(rendered.contains("PrimarySource\t1"));
    assert!(rendered.contains("CoTranslateSource\t1"));
    assert!(
        !rendered.contains("\tSourceDependency\t"),
        "source dependency aggregate rows should always carry the configured source-root mode: {rendered}"
    );
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
        return summary.length + summary.inheritedLength + summary.inheritedScore();
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
    public int inheritedLength;

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

sealed interface SummaryChoice permits SummarySelected {}

final class SummarySelected implements SummaryChoice {}
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
    assert!(
        report.source_dependency_resolved_method_calls >= 2,
        "source-summary methods should be resolved as dependency source declarations: {rendered}"
    );
    assert!(
        report.source_dependency_resolved_constructors >= 1,
        "source-summary constructors should be resolved as dependency source declarations: {rendered}"
    );
    assert!(
        report.source_dependency_resolved_field_accesses >= 2,
        "source-summary fields should be resolved as dependency source declarations: {rendered}"
    );
    assert!(
        report
            .top_source_dependency_field_accesses
            .iter()
            .any(|summary| summary.type_name == "SummaryType"
                && summary.field_name == "length"
                && summary.source_mode == DependencyMode::SourceSummary),
        "source-summary field table should identify direct dependency fields: {rendered}"
    );
    assert!(
        report
            .top_source_dependency_field_contexts
            .iter()
            .any(|summary| summary.declaring_type == "SummaryBase"
                && summary.field_name == "inherited_length"
                && summary.receiver_type == "SummaryType"
                && summary.enclosing_type == "UsesSummary"
                && summary.enclosing_member == "score"),
        "source-summary field contexts should identify inherited dependency fields and consumers: {rendered}"
    );
    assert!(
        report.source_summary_facts >= 7,
        "summary roots should emit declaration facts for hierarchy/object strategy consumers: {rendered}"
    );
    assert!(
        report.source_summary_root_type_capability_facts >= 5,
        "summary roots should expose type capability declaration facts by kind: {rendered}"
    );
    assert!(
        report.source_summary_root_object_safety_facts >= 1,
        "summary roots should expose interface object-safety declaration facts by kind: {rendered}"
    );
    assert!(
        report.source_summary_root_closed_hierarchy_facts >= 1,
        "summary roots should expose sealed hierarchy declaration facts by kind: {rendered}"
    );
    assert!(
        rendered.contains("source_summary_facts\t"),
        "source-summary declaration facts should be visible in benchmark reports: {rendered}"
    );
    assert!(
        rendered.contains("source_summary_root_type_capability_facts\t"),
        "{rendered}"
    );
    assert!(
        rendered.contains("source_summary_root_object_safety_facts\t"),
        "{rendered}"
    );
    assert!(
        rendered.contains("source_summary_root_closed_hierarchy_facts\t"),
        "{rendered}"
    );
    assert!(
        rendered.contains("top_source_dependency_field_accesses")
            && rendered.contains("top_source_dependency_field_contexts"),
        "{rendered}"
    );
    assert!(rendered.contains("SourceSummary\t1"), "{rendered}");
    assert!(
        !rendered.contains("\tSourceDependency\t"),
        "source-summary aggregate rows should always carry the configured source-root mode: {rendered}"
    );
}

#[test]
fn resolver_benchmark_separates_project_field_guesses_from_external_field_guesses() {
    let root = unique_temp_root("dependency-mode-project-field-guesses");
    let app_root = root.join("app");
    fs::create_dir_all(app_root.join("app/usecase")).unwrap();
    fs::write(
        app_root.join("app/usecase/UsesArgs.java"),
        r#"
package app.usecase;

import app.helper.FindArgs;

class UsesArgs {
    int score(FindArgs args) {
        return args.kSize + args.minEvidenceMapQ;
    }
}
"#,
    )
    .unwrap();

    let config = DependencyModeConfig::for_primary_source_root(&app_root);
    let report = benchmark_java_resolver_config(&config).unwrap();
    let rendered = render_resolver_benchmark_report(&report);

    assert_eq!(report.unexplained_field_accesses, 0, "{rendered}");
    assert_eq!(
        report.external_field_guesses, 0,
        "omitted sibling-project fields should not be reported as external library pressure: {rendered}"
    );
    assert_eq!(
        report.project_field_guesses, 2,
        "explicit imports under the active project root should be project fallback facts: {rendered}"
    );
    assert_eq!(report.source_field_guesses, 0, "{rendered}");
    assert!(
        rendered.contains("top_project_field_guess_targets")
            && rendered.contains("FindArgs\tkSize\t1")
            && rendered.contains("FindArgs\tminEvidenceMapQ\t1"),
        "project field fallback targets should be rendered separately: {rendered}"
    );
    assert!(
        rendered.contains("top_project_field_guess_contexts")
            && rendered.contains("kSize\tFindArgs\tUsesArgs\tscore\t1\t")
            && rendered.contains("app/usecase/UsesArgs.java"),
        "project field fallback contexts should be rendered separately: {rendered}"
    );
}

#[test]
fn resolver_benchmark_recommends_source_summary_prefix_for_project_guesses() {
    let root = unique_temp_root("dependency-mode-source-summary-recommendations");
    let app_root = root.join("app");
    fs::create_dir_all(app_root.join("app/usecase")).unwrap();
    fs::write(
        app_root.join("app/usecase/UsesArgs.java"),
        r#"
package app.usecase;

import app.helper.FindArgs;

class UsesArgs {
    int score(FindArgs args) {
        FindArgs created = new FindArgs();
        return args.kSize + created.minEvidenceMapQ + args.score();
    }
}
"#,
    )
    .unwrap();

    let config = DependencyModeConfig::for_primary_source_root(&app_root);
    let report = benchmark_java_resolver_config(&config).unwrap();
    let rendered = render_resolver_benchmark_report(&report);

    assert!(
        report.project_constructor_guesses >= 1,
        "constructor pressure should be classified before recommendation: {rendered}"
    );
    assert!(
        report.project_field_guesses >= 2,
        "field pressure should be classified before recommendation: {rendered}"
    );
    assert!(
        report
            .top_source_summary_recommendations
            .iter()
            .any(|recommendation| {
                recommendation.package_prefix == "app.helper"
                    && recommendation.target_type == "FindArgs"
                    && recommendation.constructor_guesses >= 1
                    && recommendation.field_guesses >= 2
                    && recommendation.count >= 3
                    && recommendation
                        .sample_files
                        .contains(&"app/usecase/UsesArgs.java".to_string())
            }),
        "project constructor/field guesses should recommend a source-summary root: {rendered}"
    );
    assert!(rendered.contains("top_source_summary_recommendations"));
    assert!(rendered.contains("app.helper\tFindArgs"));
}

#[test]
fn resolver_benchmark_classifies_snake_case_project_field_receiver_as_project() {
    let root = unique_temp_root("dependency-mode-snake-project-field-receiver");
    let app_root = root.join("app");
    fs::create_dir_all(app_root.join("app/usecase")).unwrap();
    fs::write(
        app_root.join("app/usecase/UsesBaseUtils.java"),
        r#"
package app.usecase;

import app.helper.BaseUtils;

class UsesBaseUtils {
    int score() {
        return BaseUtils.MAX_BASE_SCORE;
    }
}
"#,
    )
    .unwrap();

    let config = DependencyModeConfig::for_primary_source_root(&app_root);
    let report = benchmark_java_resolver_config(&config).unwrap();
    let rendered = render_resolver_benchmark_report(&report);

    assert_eq!(report.unexplained_field_accesses, 0, "{rendered}");
    assert_eq!(
        report.external_field_guesses, 0,
        "snake-cased receiver keys for explicitly imported project utilities should not become external library pressure: {rendered}"
    );
    assert_eq!(report.project_field_guesses, 1, "{rendered}");
    assert!(
        rendered.contains("top_project_field_guess_targets")
            && rendered.contains("BaseUtils\tMAX_BASE_SCORE\t1"),
        "project fallback should retain the source receiver key in the project bucket: {rendered}"
    );
}

#[test]
fn resolver_benchmark_propagates_project_field_guess_ownership_through_static_field_chains() {
    let root = unique_temp_root("dependency-mode-project-field-chain");
    let app_root = root.join("app");
    fs::create_dir_all(app_root.join("app/usecase")).unwrap();
    fs::write(
        app_root.join("app/usecase/UsesBaseUtils.java"),
        r#"
package app.usecase;

import app.helper.BaseUtils;

class UsesBaseUtils {
    int score() {
        return BaseUtils.Base.N.base;
    }
}
"#,
    )
    .unwrap();

    let config = DependencyModeConfig::for_primary_source_root(&app_root);
    let report = benchmark_java_resolver_config(&config).unwrap();
    let rendered = render_resolver_benchmark_report(&report);

    assert_eq!(report.unexplained_field_accesses, 0, "{rendered}");
    assert_eq!(
        report.external_field_guesses, 0,
        "field-chain ownership from an omitted project utility should propagate through type-like static fields: {rendered}"
    );
    assert_eq!(report.project_field_guesses, 3, "{rendered}");
    assert!(
        rendered.contains("BaseUtils\tBase\t1")
            && rendered.contains("Base\tN\t1")
            && rendered.contains("N\tbase\t1"),
        "all links in the project static-field chain should stay in the project bucket: {rendered}"
    );
}

#[test]
fn resolver_benchmark_classifies_nested_project_constructor_guess_by_outer_import() {
    let root = unique_temp_root("dependency-mode-project-nested-constructor");
    let app_root = root.join("app");
    fs::create_dir_all(app_root.join("app/usecase")).unwrap();
    fs::write(
        app_root.join("app/usecase/UsesAppException.java"),
        r#"
package app.usecase;

import app.errors.AppException;

class UsesAppException {
    Object create(String label) {
        return new AppException.BadInput(label);
    }
}
"#,
    )
    .unwrap();

    let config = DependencyModeConfig::for_primary_source_root(&app_root);
    let report = benchmark_java_resolver_config(&config).unwrap();
    let rendered = render_resolver_benchmark_report(&report);

    assert_eq!(report.unexplained_constructors, 0, "{rendered}");
    assert_eq!(
        report.external_constructor_guesses, 0,
        "nested constructors owned by an explicitly imported project outer type should not become external pressure: {rendered}"
    );
    assert_eq!(report.project_constructor_guesses, 1, "{rendered}");
    assert!(
        rendered.contains("top_project_constructor_guess_types")
            && rendered.contains("AppException.BadInput\t1\t1")
            && rendered.contains("top_project_constructor_guess_contexts")
            && rendered.contains("AppException.BadInput\tAppException.BadInput"),
        "project nested constructor fallback should retain canonical target context: {rendered}"
    );
}

#[test]
fn resolver_benchmark_keeps_nested_non_project_constructor_guess_external() {
    let root = unique_temp_root("dependency-mode-external-nested-constructor");
    let app_root = root.join("app");
    fs::create_dir_all(app_root.join("app/usecase")).unwrap();
    fs::write(
        app_root.join("app/usecase/UsesExternalException.java"),
        r#"
package app.usecase;

import external.errors.ExternalException;

class UsesExternalException {
    Object create(String label) {
        return new ExternalException.BadInput(label);
    }
}
"#,
    )
    .unwrap();

    let config = DependencyModeConfig::for_primary_source_root(&app_root);
    let report = benchmark_java_resolver_config(&config).unwrap();
    let rendered = render_resolver_benchmark_report(&report);

    assert_eq!(report.unexplained_constructors, 0, "{rendered}");
    assert_eq!(report.project_constructor_guesses, 0, "{rendered}");
    assert_eq!(
        report.external_constructor_guesses, 1,
        "nested constructors under non-project imports remain external fallback pressure: {rendered}"
    );
    assert!(
        rendered.contains("top_external_constructor_guess_types")
            && rendered.contains("BadInput\t1\t1"),
        "external nested constructor fallback should stay in the external bucket: {rendered}"
    );
}

#[test]
fn source_summary_root_resolves_imported_nested_graph_types() {
    let root = unique_temp_root("dependency-mode-source-summary-nested-graph");
    let app_root = root.join("app");
    let summary_root = root.join("summary");
    fs::create_dir_all(app_root.join("app/usecase")).unwrap();
    fs::create_dir_all(summary_root.join("dep/assembly")).unwrap();
    fs::write(
        app_root.join("app/usecase/UsesAssemblyGraph.java"),
        r#"
package app.usecase;

import dep.assembly.AssemblyGraph;
import dep.assembly.AssemblyGraph.Contig;
import dep.assembly.AssemblyGraph.Connection;

import java.util.List;

class UsesAssemblyGraph {
    AssemblyGraph rewrite(AssemblyGraph graph, Contig first) {
        Connection connection = first.getSoleSuccessor();
        Contig target = connection.getTarget();
        for (Connection conn : target.getConnections()) {
            Connection replacement =
                    new Connection(conn.getTarget(), conn.getOverlapLen(), false, conn.isTargetReverse());
            replacement.rcConnection(first);
        }
        first.setConnections(target.getConnections());
        Contig joined = new Contig(first.getSequence(), null, first.getReadCount());
        return new AssemblyGraph(joined);
    }
}
"#,
    )
    .unwrap();
    fs::write(
        summary_root.join("dep/assembly/AssemblyGraph.java"),
        r#"
package dep.assembly;

import java.util.List;

public final class AssemblyGraph {
    public AssemblyGraph(List<Contig> contigs) {}
    public AssemblyGraph(Contig contig) {}

    public List<Contig> getContigs() { return null; }

    public static final class Contig {
        public Contig(byte[] sequence, byte[] coverage, int readCount) {}

        public byte[] getSequence() { return null; }
        public int getReadCount() { return 0; }
        public List<Connection> getConnections() { return null; }
        public void setConnections(List<Connection> connections) {}
        public Connection getSoleSuccessor() { return null; }
    }

    public static final class Connection {
        public Connection(Contig target, int overlapLen, boolean sourceReverse, boolean targetReverse) {}

        public Contig getTarget() { return null; }
        public int getOverlapLen() { return 0; }
        public boolean isTargetReverse() { return false; }
        public Connection rcConnection(Contig replacementTarget) { return null; }
    }
}
"#,
    )
    .unwrap();

    let baseline_config =
        DependencyModeConfig::for_primary_source_root(&app_root).without_builtin_external_packs();
    let baseline = benchmark_java_resolver_config(&baseline_config).unwrap();
    assert!(
        baseline.project_constructor_guesses > 0 || baseline.external_constructor_guesses > 0,
        "{}",
        render_resolver_benchmark_report(&baseline)
    );

    let mut config = DependencyModeConfig::for_primary_source_root(&app_root);
    config.add_source_root(&summary_root, DependencyMode::SourceSummary);
    let report = benchmark_java_resolver_config(&config).unwrap();
    let rendered = render_resolver_benchmark_report(&report);

    assert_eq!(report.files_seen, 2);
    assert_eq!(report.files_resolved_without_errors, 1, "{rendered}");
    assert_eq!(report.unresolved_method_calls, 0, "{rendered}");
    assert_eq!(report.unresolved_constructors, 0, "{rendered}");
    assert!(
        report.source_dependency_resolved_method_calls >= 7,
        "nested dependency methods should resolve through source-summary declarations: {rendered}"
    );
    assert!(
        report.source_dependency_resolved_constructors >= 3,
        "nested dependency constructors should resolve through source-summary declarations: {rendered}"
    );
    assert!(
        rendered.contains("Connection\t4") && rendered.contains("Contig\t3"),
        "nested dependency constructor targets should be attributed as source dependencies, not pack guesses: {rendered}"
    );
    assert!(
        rendered.contains("Connection\tget_target") && rendered.contains("Contig\tget_connections"),
        "nested dependency method targets should be attributed as source dependencies: {rendered}"
    );
}

#[test]
fn source_summary_root_resolves_fermilite_shaped_assembler_graph_boundary() {
    let root = unique_temp_root("dependency-mode-source-summary-fermilite-graph");
    let app_root = root.join("app");
    let summary_root = root.join("summary");
    fs::create_dir_all(app_root.join("app/evidence")).unwrap();
    fs::create_dir_all(summary_root.join("org/example/fermi")).unwrap();
    fs::write(
        app_root.join("app/evidence/UsesFermiAssembly.java"),
        r#"
package app.evidence;

import org.example.fermi.FermiLiteAssembler;
import org.example.fermi.FermiLiteAssembly;
import org.example.fermi.FermiLiteAssembly.Contig;
import org.example.fermi.FermiLiteAssembly.Connection;

import java.util.List;

class UsesFermiAssembly {
    FermiLiteAssembly assemble(List<byte[]> reads) {
        FermiLiteAssembler assembler = new FermiLiteAssembler();
        FermiLiteAssembly assembly = assembler.createAssembly(reads);
        if (assembly.getNContigs() == 0) {
            return new FermiLiteAssembly(reads);
        }
        return assembly;
    }

    FermiLiteAssembly rewrite(FermiLiteAssembly assembly, Contig first) {
        Connection connection = first.getSoleSuccessor();
        Contig target = connection.getTarget();
        for (Connection conn : target.getConnections()) {
            Connection replacement =
                    new Connection(conn.getTarget(), conn.getOverlapLen(), false, conn.isTargetRC());
            replacement.rcConnection(first);
        }
        first.setConnections(target.getConnections());
        Contig joined = new Contig(first.getSequence(), null, first.getNSupportingReads());
        return new FermiLiteAssembly(joined);
    }
}
"#,
    )
    .unwrap();
    fs::write(
        summary_root.join("org/example/fermi/FermiLiteAssembler.java"),
        r#"
package org.example.fermi;

import java.util.List;

public final class FermiLiteAssembler {
    public FermiLiteAssembler() {}
    public FermiLiteAssembly createAssembly(List<byte[]> reads) { return null; }
}
"#,
    )
    .unwrap();
    fs::write(
        summary_root.join("org/example/fermi/FermiLiteAssembly.java"),
        r#"
package org.example.fermi;

import java.util.List;

public final class FermiLiteAssembly {
    public FermiLiteAssembly(List<byte[]> reads) {}
    public FermiLiteAssembly(Contig contig) {}

    public int getNContigs() { return 0; }
    public List<Contig> getContigs() { return null; }

    public static final class Contig {
        public Contig(byte[] sequence, byte[] coverage, int nSupportingReads) {}

        public byte[] getSequence() { return null; }
        public int getNSupportingReads() { return 0; }
        public List<Connection> getConnections() { return null; }
        public void setConnections(List<Connection> connections) {}
        public Connection getSoleSuccessor() { return null; }
    }

    public static final class Connection {
        public Connection(Contig target, int overlapLen, boolean rc, boolean targetRC) {}

        public Contig getTarget() { return null; }
        public int getOverlapLen() { return 0; }
        public boolean isTargetRC() { return false; }
        public Connection rcConnection(Contig replacementTarget) { return null; }
    }
}
"#,
    )
    .unwrap();

    let disabled_config =
        DependencyModeConfig::for_primary_source_root(&app_root).without_builtin_external_packs();
    let disabled_report = benchmark_java_resolver_config(&disabled_config).unwrap();
    let disabled_rendered = render_resolver_benchmark_report(&disabled_report);
    assert!(
        disabled_report.external_constructor_guesses > 0
            || disabled_report.project_constructor_guesses > 0,
        "without the source-summary dependency, FermiLite-shaped source types should remain benchmark pressure: {disabled_rendered}"
    );

    let mut config = DependencyModeConfig::for_primary_source_root(&app_root);
    config.add_source_root(&summary_root, DependencyMode::SourceSummary);
    let report = benchmark_java_resolver_config(&config).unwrap();
    let rendered = render_resolver_benchmark_report(&report);

    assert_eq!(report.files_seen, 3);
    assert_eq!(report.files_resolved_without_errors, 1, "{rendered}");
    assert_eq!(report.unresolved_constructors, 0, "{rendered}");
    assert_eq!(report.unexplained_constructors, 0, "{rendered}");
    assert!(
        report.source_dependency_resolved_method_calls >= 8,
        "FermiLite-shaped dependency methods should resolve through source-summary declarations: {rendered}"
    );
    assert!(
        report.source_dependency_resolved_constructors >= 4,
        "FermiLite-shaped dependency constructors should resolve through source-summary declarations: {rendered}"
    );
    assert!(
        !report
            .top_external_api_guess_methods
            .iter()
            .any(|summary| matches!(
                summary.receiver_type.as_str(),
                "FermiLiteAssembler" | "FermiLiteAssembly" | "Connection" | "Contig"
            )),
        "source-summary FermiLite-shaped graph calls should not become external pack guesses: {rendered}"
    );
    assert!(
        rendered.contains("FermiLiteAssembler\t0\tSourceSummary")
            && rendered.contains("FermiLiteAssembly\t1\tSourceSummary")
            && rendered.contains("Connection\t4\tSourceSummary")
            && rendered.contains("Contig\t3\tSourceSummary"),
        "source dependency constructor table should retain the FermiLite-shaped targets: {rendered}"
    );
    assert!(
        rendered.contains("top_source_dependency_method_contexts")
            && rendered.contains(
                "FermiLiteAssembler\tcreate_assembly\tSourceSummary\t1\tUsesFermiAssembly\tassemble"
            )
            && rendered
                .contains("Connection\tget_target\tSourceSummary\t0\tUsesFermiAssembly\trewrite"),
        "source dependency method contexts should show where source-summary calls are consumed: {rendered}"
    );
    assert!(
        rendered.contains("top_source_dependency_constructor_contexts")
            && rendered
                .contains("FermiLiteAssembler\tSourceSummary\t0\tUsesFermiAssembly\tassemble")
            && rendered.contains("Connection\tSourceSummary\t4\tUsesFermiAssembly\trewrite"),
        "source dependency constructor contexts should show where source-summary constructors are consumed: {rendered}"
    );
}

#[test]
fn source_summary_roots_resolve_project_helper_fields_and_nested_exception_constructors() {
    let root = unique_temp_root("dependency-mode-source-summary-helper-fields-exceptions");
    let app_root = root.join("app");
    let helper_root = root.join("helpers");
    let exception_root = root.join("exceptions");
    fs::create_dir_all(app_root.join("app/evidence")).unwrap();
    fs::create_dir_all(helper_root.join("dep/sv")).unwrap();
    fs::create_dir_all(exception_root.join("dep/exceptions")).unwrap();

    fs::write(
        app_root.join("app/evidence/UsesSummaryHelpers.java"),
        r#"
package app.evidence;

import dep.exceptions.UserException.BadInput;
import dep.exceptions.UserException;
import dep.sv.FindArgs;
import dep.sv.SVLocation;

class UsesSummaryHelpers {
    int score(FindArgs args) {
        SVLocation location = new SVLocation(args.start, args.end);
        if (args.kSize < 0) {
            BadInput error = new BadInput("negative kmer size");
            return error.hashCode();
        }
        if (args.minEvidenceMapQ < 0) {
            UserException.ValidationFailure error =
                new UserException.ValidationFailure("negative evidence quality");
            return error.hashCode();
        }
        return location.span() + args.minEvidenceMapQ;
    }
}
"#,
    )
    .unwrap();
    fs::write(
        helper_root.join("dep/sv/FindArgs.java"),
        r#"
package dep.sv;

public final class FindArgs {
    public int start;
    public int end;
    public int kSize;
    public int minEvidenceMapQ;
}
"#,
    )
    .unwrap();
    fs::write(
        helper_root.join("dep/sv/SVLocation.java"),
        r#"
package dep.sv;

public final class SVLocation {
    public SVLocation(int start, int end) {}

    public int span() { return 0; }
}
"#,
    )
    .unwrap();
    fs::write(
        exception_root.join("dep/exceptions/UserException.java"),
        r#"
package dep.exceptions;

public class UserException extends RuntimeException {
    public UserException(String message) {}

    public static final class BadInput extends UserException {
        public BadInput(String message) {
            super(message);
        }
    }

    public static final class ValidationFailure extends UserException {
        public ValidationFailure(String message) {
            super(message);
        }
    }
}
"#,
    )
    .unwrap();

    let baseline_config =
        DependencyModeConfig::for_primary_source_root(&app_root).without_builtin_external_packs();
    let baseline = benchmark_java_resolver_config(&baseline_config).unwrap();
    let baseline_rendered = render_resolver_benchmark_report(&baseline);
    assert!(
        baseline.external_constructor_guesses > 0,
        "without source-summary roots, absent project helper constructors should remain external pressure: {baseline_rendered}"
    );
    assert!(
        baseline.external_field_guesses > 0,
        "without source-summary roots, absent project helper fields should remain external pressure: {baseline_rendered}"
    );

    let mut config = DependencyModeConfig::for_primary_source_root(&app_root);
    config.add_source_root(&helper_root, DependencyMode::SourceSummary);
    config.add_source_root(&exception_root, DependencyMode::SourceSummary);
    let report = benchmark_java_resolver_config(&config).unwrap();
    let rendered = render_resolver_benchmark_report(&report);

    assert_eq!(report.files_seen, 4);
    assert_eq!(report.files_resolved_without_errors, 1, "{rendered}");
    assert_eq!(
        report.external_constructor_guesses, 0,
        "project helper constructors covered by source summaries should not stay external guesses: {rendered}"
    );
    assert_eq!(
        report.external_field_guesses, 0,
        "project helper fields covered by source summaries should not stay external field guesses: {rendered}"
    );
    assert_eq!(
        report.project_field_guesses, 0,
        "project helper fields covered by source summaries should not stay project field guesses: {rendered}"
    );
    assert_eq!(
        report.source_field_guesses, 0,
        "project helper fields covered by source summaries should not stay source field guesses: {rendered}"
    );
    assert_eq!(
        report.source_constructor_guesses, 0,
        "qualified nested source-summary constructors should resolve through the same declaration facts: {rendered}"
    );
    assert!(
        report.source_dependency_resolved_constructors >= 2,
        "source-summary constructors should cover helper values and nested exceptions: {rendered}"
    );
    assert!(
        report.source_dependency_resolved_field_accesses >= 4,
        "source-summary fields should cover argument-collection-style public fields: {rendered}"
    );
    assert!(
        rendered.contains("SVLocation\t2\tSourceSummary")
            && rendered.contains("BadInput\t1\tSourceSummary")
            && rendered.contains("ValidationFailure\t1\tSourceSummary"),
        "source dependency constructor table should show helper and nested exception constructors: {rendered}"
    );
    assert!(
        rendered.contains("FindArgs\tk_size\tSourceSummary")
            && rendered.contains("FindArgs\tmin_evidence_map_q\tSourceSummary"),
        "source dependency field table should show argument-collection-style fields: {rendered}"
    );
    assert!(
        rendered.contains("top_source_dependency_field_contexts")
            && rendered
                .contains("FindArgs\tk_size\tFindArgs\tSourceSummary\tUsesSummaryHelpers\tscore"),
        "source dependency field contexts should show where summary fields are consumed: {rendered}"
    );
}

#[test]
fn source_summary_roots_resolve_nested_argument_collection_field_chains() {
    let root = unique_temp_root("dependency-mode-source-summary-argument-field-chains");
    let app_root = root.join("app");
    let caller_root = root.join("caller");
    let genotyper_root = root.join("genotyper");
    fs::create_dir_all(app_root.join("app/variant")).unwrap();
    fs::create_dir_all(caller_root.join("dep/caller")).unwrap();
    fs::create_dir_all(genotyper_root.join("dep/genotyper")).unwrap();

    fs::write(
        app_root.join("app/variant/UsesCallerArgs.java"),
        r#"
package app.variant;

import dep.caller.CallerArgs;
import dep.genotyper.ActiveArgs;

class UsesCallerArgs {
    ActiveArgs configure(CallerArgs caller) {
        return new ActiveArgs(caller.standardArgs.genotypeArgs.standardConfidenceForCalling,
            caller.standardArgs.annotateAllSitesWithPLs,
            caller.standardArgs.outputMode);
    }
}
"#,
    )
    .unwrap();
    fs::write(
        caller_root.join("dep/caller/CallerArgs.java"),
        r#"
package dep.caller;

import dep.genotyper.StandardArgs;

public class CallerArgs {
    public StandardArgs standardArgs = new StandardArgs();
}
"#,
    )
    .unwrap();
    fs::write(
        genotyper_root.join("dep/genotyper/StandardArgs.java"),
        r#"
package dep.genotyper;

public class StandardArgs {
    public GenotypeArgs genotypeArgs = new GenotypeArgs();
    public boolean annotateAllSitesWithPLs = false;
    public OutputMode outputMode = OutputMode.EMIT_VARIANTS_ONLY;
}

class GenotypeArgs {
    public double standardConfidenceForCalling = 30.0;
}

enum OutputMode {
    EMIT_VARIANTS_ONLY
}
"#,
    )
    .unwrap();
    fs::write(
        genotyper_root.join("dep/genotyper/ActiveArgs.java"),
        r#"
package dep.genotyper;

public class ActiveArgs {
    public ActiveArgs(double confidence, boolean annotate, OutputMode outputMode) {}
}
"#,
    )
    .unwrap();

    let baseline_config =
        DependencyModeConfig::for_primary_source_root(&app_root).without_builtin_external_packs();
    let baseline = benchmark_java_resolver_config(&baseline_config).unwrap();
    let baseline_rendered = render_resolver_benchmark_report(&baseline);
    assert!(
        baseline.external_field_guesses > 0,
        "without source-summary roots, nested argument fields should remain external pressure: {baseline_rendered}"
    );

    let mut config = DependencyModeConfig::for_primary_source_root(&app_root);
    config.add_source_root(&caller_root, DependencyMode::SourceSummary);
    config.add_source_root(&genotyper_root, DependencyMode::SourceSummary);
    let report = benchmark_java_resolver_config(&config).unwrap();
    let rendered = render_resolver_benchmark_report(&report);

    assert_eq!(report.files_seen, 4);
    assert_eq!(report.files_resolved_without_errors, 1, "{rendered}");
    assert_eq!(report.resolver_diagnostics, 0, "{rendered}");
    assert_eq!(report.external_field_guesses, 0, "{rendered}");
    assert_eq!(report.project_field_guesses, 0, "{rendered}");
    assert_eq!(report.source_field_guesses, 0, "{rendered}");
    assert_eq!(report.external_constructor_guesses, 0, "{rendered}");
    assert!(
        report.source_dependency_resolved_constructors >= 1,
        "constructor overload should consume nested source-summary field types: {rendered}"
    );
    assert!(
        report.source_dependency_resolved_field_accesses >= 5,
        "nested argument-collection fields should resolve through summary roots: {rendered}"
    );
    assert!(
        rendered.contains("ActiveArgs\t3\tSourceSummary"),
        "constructor should be attributed to the source-summary dependency: {rendered}"
    );
    assert!(
        rendered.contains("CallerArgs\tstandard_args\tSourceSummary")
            && rendered.contains("StandardArgs\tgenotype_args\tSourceSummary")
            && rendered.contains("GenotypeArgs\tstandard_confidence_for_calling\tSourceSummary")
            && rendered.contains("StandardArgs\tannotate_all_sites_with_p_ls\tSourceSummary")
            && rendered.contains("StandardArgs\toutput_mode\tSourceSummary"),
        "field table should retain the nested argument-collection chain: {rendered}"
    );
    assert!(
        rendered.contains("top_source_dependency_field_contexts")
            && rendered.contains(
                "GenotypeArgs\tstandard_confidence_for_calling\tGenotypeArgs\tSourceSummary\tUsesCallerArgs\tconfigure"
            ),
        "field contexts should show where the nested source-summary field is consumed: {rendered}"
    );
}

#[test]
fn source_summary_field_facts_resolve_feature_input_generic_overload() {
    let root = unique_temp_root("dependency-mode-source-summary-feature-input-overload");
    let app_root = root.join("app");
    let args_root = root.join("args");
    fs::create_dir_all(app_root.join("app/tools")).unwrap();
    fs::create_dir_all(args_root.join("dep/args")).unwrap();

    fs::write(
        app_root.join("app/tools/UsesDbsnpArgs.java"),
        r#"
package app.tools;

import dep.args.DbsnpArgumentCollection;
import java.util.Collection;
import java.util.List;

class FeatureInput<T> {}
class VariantContext {}

class UsesDbsnpArgs {
    DbsnpArgumentCollection dbsnp = new DbsnpArgumentCollection();

    <T> List<T> getValues(FeatureInput<T> input) {
        return null;
    }

    <T> List<T> getValues(Collection<FeatureInput<T>> inputs) {
        return null;
    }

    List<VariantContext> run() {
        return getValues(dbsnp.dbsnp);
    }
}
"#,
    )
    .unwrap();
    fs::write(
        args_root.join("dep/args/DbsnpArgumentCollection.java"),
        r#"
package dep.args;

import app.tools.FeatureInput;
import app.tools.VariantContext;

public class DbsnpArgumentCollection {
    public FeatureInput<VariantContext> dbsnp;
}
"#,
    )
    .unwrap();

    let baseline_config =
        DependencyModeConfig::for_primary_source_root(&app_root).without_builtin_external_packs();
    let baseline = benchmark_java_resolver_config(&baseline_config).unwrap();
    let baseline_rendered = render_resolver_benchmark_report(&baseline);
    assert!(
        baseline.resolver_diagnostics > 0,
        "without the argument-collection summary, the dbsnp field should remain ambiguous: {baseline_rendered}"
    );

    let mut config = DependencyModeConfig::for_primary_source_root(&app_root);
    config.add_source_root(&args_root, DependencyMode::SourceSummary);
    let report = benchmark_java_resolver_config(&config).unwrap();
    let rendered = render_resolver_benchmark_report(&report);

    assert_eq!(report.resolver_diagnostics, 0, "{rendered}");
    assert_eq!(report.external_field_guesses, 0, "{rendered}");
    assert!(
        report.source_dependency_resolved_field_accesses >= 1,
        "source-summary field facts should explain DbsnpArgumentCollection.dbsnp: {rendered}"
    );
    assert!(
        rendered.contains("DbsnpArgumentCollection\tdbsnp\tSourceSummary"),
        "field table should attribute dbsnp to the source-summary dependency: {rendered}"
    );
}

#[test]
fn overlapping_source_summary_root_does_not_duplicate_primary_files() {
    let root = unique_temp_root("dependency-mode-overlapping-source-roots");
    let parent_root = root.join("all");
    let app_root = parent_root.join("app/evidence");
    fs::create_dir_all(&app_root).unwrap();
    fs::create_dir_all(parent_root.join("dep/helper")).unwrap();

    fs::write(
        app_root.join("UseHelper.java"),
        r#"
package app.evidence;

import dep.helper.Helper;

class UseHelper {
    int run(Helper helper) {
        Helper created = new Helper(1);
        return helper.value + created.getValue();
    }
}
"#,
    )
    .unwrap();
    fs::write(
        parent_root.join("dep/helper/Helper.java"),
        r#"
package dep.helper;

public final class Helper {
    public int value;

    public Helper(int value) {}

    public int getValue() { return value; }
}
"#,
    )
    .unwrap();

    let mut config = DependencyModeConfig::default();
    config.add_source_root(&parent_root, DependencyMode::SourceSummary);
    config.add_source_root(&app_root, DependencyMode::PrimarySource);
    let report = benchmark_java_resolver_config(&config).unwrap();
    let rendered = render_resolver_benchmark_report(&report);

    assert_eq!(
        report.files_seen, 2,
        "overlapping source roots should assign each file to exactly one most-specific root: {rendered}"
    );
    assert!(
        report.source_roots.iter().any(|root| root.path == app_root
            && root.mode == DependencyMode::PrimarySource
            && root.files_seen == 1),
        "primary root should retain its file even when configured after a broader summary root: {rendered}"
    );
    assert!(
        report
            .source_roots
            .iter()
            .any(|root| root.path == parent_root
                && root.mode == DependencyMode::SourceSummary
                && root.files_seen == 1),
        "summary parent should see only dependency files outside the more-specific primary root: {rendered}"
    );
    assert_eq!(report.files_resolved_without_errors, 1, "{rendered}");
    assert_eq!(report.unresolved_method_calls, 0, "{rendered}");
    assert_eq!(report.unresolved_constructors, 0, "{rendered}");
    assert_eq!(report.unresolved_field_accesses, 0, "{rendered}");
    assert_eq!(report.external_field_guesses, 0, "{rendered}");
    assert_eq!(report.project_field_guesses, 0, "{rendered}");
    assert_eq!(report.source_field_guesses, 0, "{rendered}");
    assert!(
        rendered.contains("Helper\tget_value\tSourceSummary")
            && rendered.contains("Helper\t1\tSourceSummary")
            && rendered.contains("Helper\tvalue\tSourceSummary"),
        "dependency calls, constructors, and fields should be attributed to the summary root: {rendered}"
    );
}

#[test]
fn primary_root_resolves_imported_nested_graph_types_without_external_guesses() {
    let root = unique_temp_root("dependency-mode-primary-nested-graph");
    let app_root = root.join("app");
    fs::create_dir_all(app_root.join("app/evidence")).unwrap();
    fs::write(
        app_root.join("app/evidence/AssemblyGraph.java"),
        r#"
package app.evidence;

import java.util.List;

public final class AssemblyGraph {
    public AssemblyGraph(List<Contig> contigs) {}
    public AssemblyGraph(Contig contig) {}

    public List<Contig> getContigs() { return null; }

    public static final class Contig {
        public Contig(byte[] sequence, byte[] coverage, int readCount) {}

        public byte[] getSequence() { return null; }
        public int getReadCount() { return 0; }
        public List<Connection> getConnections() { return null; }
        public void setConnections(List<Connection> connections) {}
        public Connection getSoleSuccessor() { return null; }
    }

    public static final class Connection {
        public Connection(Contig target, int overlapLen, boolean sourceReverse, boolean targetReverse) {}

        public Contig getTarget() { return null; }
        public int getOverlapLen() { return 0; }
        public boolean isTargetReverse() { return false; }
        public Connection rcConnection(Contig replacementTarget) { return null; }
    }
}
"#,
    )
    .unwrap();
    fs::write(
        app_root.join("app/evidence/UsesAssemblyGraph.java"),
        r#"
package app.evidence;

import app.evidence.AssemblyGraph;
import app.evidence.AssemblyGraph.Contig;
import app.evidence.AssemblyGraph.Connection;

class UsesAssemblyGraph {
    AssemblyGraph rewrite(AssemblyGraph graph, Contig first) {
        Connection connection = first.getSoleSuccessor();
        Contig target = connection.getTarget();
        for (Connection conn : target.getConnections()) {
            Connection replacement =
                    new Connection(conn.getTarget(), conn.getOverlapLen(), false, conn.isTargetReverse());
            replacement.rcConnection(first);
        }
        first.setConnections(target.getConnections());
        Contig joined = new Contig(first.getSequence(), null, first.getReadCount());
        return new AssemblyGraph(joined);
    }

    int streamedWeight(Contig first) {
        first.getConnections().stream()
                .filter(conn -> conn.getTarget().getReadCount() > 0)
                .forEach(conn -> conn.rcConnection(first));
        return first.getConnections().stream()
                .mapToInt(conn -> conn.getTarget().getSequence().length - conn.getOverlapLen())
                .sum();
    }
}
"#,
    )
    .unwrap();

    let config = DependencyModeConfig::for_primary_source_root(&app_root);
    let report = benchmark_java_resolver_config(&config).unwrap();
    let rendered = render_resolver_benchmark_report(&report);

    assert_eq!(report.files_seen, 2);
    assert_eq!(report.unresolved_constructors, 0, "{rendered}");
    assert!(
        !report
            .top_external_api_guess_methods
            .iter()
            .any(|summary| matches!(summary.receiver_type.as_str(), "Connection" | "Contig")),
        "primary source graph methods should resolve as source calls, not external guesses: {rendered}"
    );
    assert!(
        rendered.contains("Stream\tforEach") || report.unresolved_method_calls == 0,
        "generic stream methods may remain external-boundary guesses, but source graph calls should not: {rendered}"
    );
    assert!(
        !report
            .top_external_constructor_guess_types
            .iter()
            .any(|summary| matches!(
                summary.type_name.as_str(),
                "AssemblyGraph" | "Connection" | "Contig"
            )),
        "primary source graph constructors should resolve as source constructors, not external guesses: {rendered}"
    );
}

#[test]
fn source_summary_root_does_not_report_body_object_lowering_pressure() {
    let root = unique_temp_root("dependency-mode-source-summary-object-pressure");
    let app_root = root.join("app");
    let summary_root = root.join("summary");
    fs::create_dir_all(app_root.join("app/usecase")).unwrap();
    fs::create_dir_all(summary_root.join("dep/summary")).unwrap();
    fs::write(
        app_root.join("app/usecase/UsesSummaryRunnable.java"),
        r#"
package app.usecase;

import dep.summary.SummaryRunnableFactory;

class UsesSummaryRunnable {
    Object create(SummaryRunnableFactory factory) {
        return factory.make();
    }
}
"#,
    )
    .unwrap();
    fs::write(
        summary_root.join("dep/summary/SummaryRunnableFactory.java"),
        r#"
package dep.summary;

public class SummaryRunnableFactory {
    public Runnable make() {
        return new Runnable() {
            public void run() {}
        };
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
    assert_eq!(report.files_resolved_without_errors, 1, "{rendered}");
    assert!(
        report.source_dependency_resolved_method_calls >= 1,
        "source-summary declarations should still resolve primary-source calls: {rendered}"
    );
    assert_eq!(
        report.object_representation_diagnostics, 0,
        "source-summary method bodies are not emitted and should not create object-lowering diagnostics: {rendered}"
    );
    assert!(
        !rendered.contains("unsupported_anonymous_class"),
        "anonymous classes inside source-summary bodies should not appear as benchmark object diagnostics: {rendered}"
    );
}

#[test]
fn primary_extends_source_summary_base_is_erased_boundary_not_inheritance_pressure() {
    let root = unique_temp_root("dependency-mode-source-summary-superclass-boundary");
    let app_root = root.join("app");
    let summary_root = root.join("summary");
    fs::create_dir_all(app_root.join("app/walkers")).unwrap();
    fs::create_dir_all(summary_root.join("dep/framework")).unwrap();
    fs::write(
        app_root.join("app/walkers/VariantLikeTool.java"),
        r#"
package app.walkers;

import dep.framework.SummaryWalker;

class VariantLikeTool extends SummaryWalker {
    int apply(int value) {
        return value + 1;
    }
}
"#,
    )
    .unwrap();
    fs::write(
        summary_root.join("dep/framework/SummaryWalker.java"),
        r#"
package dep.framework;

public class SummaryWalker {
    public void traverse() {}
}
"#,
    )
    .unwrap();

    let mut co_translated = DependencyModeConfig::for_primary_source_root(&app_root);
    co_translated.add_source_root(&summary_root, DependencyMode::CoTranslateSource);
    let co_translated_report = benchmark_java_resolver_config(&co_translated).unwrap();
    let co_translated_rendered = render_resolver_benchmark_report(&co_translated_report);
    assert!(
        co_translated_report.object_representation_diagnostics >= 1,
        "co-translated superclass bodies are emitted and should remain visible inheritance pressure: {co_translated_rendered}"
    );
    assert!(
        co_translated_rendered.contains("unsupported_inheritance\tPrimarySource")
            || co_translated_rendered.contains("unsupported_inheritance\tCoTranslateSource"),
        "{co_translated_rendered}"
    );

    let mut config = DependencyModeConfig::for_primary_source_root(&app_root);
    config.add_source_root(&summary_root, DependencyMode::SourceSummary);
    let report = benchmark_java_resolver_config(&config).unwrap();
    let rendered = render_resolver_benchmark_report(&report);

    assert_eq!(
        report.object_representation_diagnostics, 0,
        "source-summary superclasses are non-emitted framework boundaries, not primary-source inheritance lowering pressure: {rendered}"
    );
    assert!(
        report.object_erased_object_representations >= 1,
        "the source-summary superclass boundary should be represented explicitly as erased object: {rendered}"
    );
    assert!(
        !rendered.contains("unsupported_inheritance"),
        "source-summary superclass boundaries should not leave inheritance diagnostics: {rendered}"
    );
}

#[test]
fn source_summary_static_interface_members_do_not_create_object_safety_pressure() {
    let root = unique_temp_root("dependency-mode-source-summary-static-interface-members");
    let app_root = root.join("app");
    let summary_root = root.join("summary");
    fs::create_dir_all(app_root.join("app/usecase")).unwrap();
    fs::create_dir_all(summary_root.join("dep/summary")).unwrap();
    fs::write(
        app_root.join("app/usecase/UsesSummaryPredicate.java"),
        r#"
package app.usecase;

import dep.summary.SummaryPredicate;

class UsesSummaryPredicate {
    SummaryPredicate create() {
        return SummaryPredicate.alwaysTrue();
    }

    int threshold() {
        return SummaryPredicate.DEFAULT_THRESHOLD;
    }

    boolean apply(SummaryPredicate predicate) {
        return predicate.test(7);
    }
}
"#,
    )
    .unwrap();
    fs::write(
        summary_root.join("dep/summary/SummaryPredicate.java"),
        r#"
package dep.summary;

public interface SummaryPredicate {
    int DEFAULT_THRESHOLD = 10;

    static SummaryPredicate alwaysTrue() {
        return new SummaryPredicate() {
            public boolean test(int value) {
                return true;
            }
        };
    }

    boolean test(int value);
}
"#,
    )
    .unwrap();

    let mut config = DependencyModeConfig::for_primary_source_root(&app_root);
    config.add_source_root(&summary_root, DependencyMode::SourceSummary);
    let report = benchmark_java_resolver_config(&config).unwrap();
    let rendered = render_resolver_benchmark_report(&report);

    assert_eq!(report.parser_diagnostics, 0, "{rendered}");
    assert_eq!(report.resolver_diagnostics, 0, "{rendered}");
    assert_eq!(
        report.object_representation_diagnostics, 0,
        "source-summary static interface constants and factories should not make the interface non-object-safe: {rendered}"
    );
    assert!(
        report.source_dependency_resolved_method_calls >= 2,
        "static factory and instance predicate calls should both resolve through source-summary declarations: {rendered}"
    );
    assert!(
        report.source_dependency_resolved_field_accesses >= 1,
        "source-summary interface constants should resolve as dependency fields: {rendered}"
    );
    assert!(
        !rendered.contains("static interface method alwaysTrue"),
        "static source-summary interface methods should not appear as object-safety violations: {rendered}"
    );
    assert!(
        !rendered.contains("interface field DEFAULT_THRESHOLD"),
        "source-summary interface constants should not appear as instance object-safety violations: {rendered}"
    );
}

#[test]
fn source_summary_field_contexts_do_not_treat_lambda_params_as_static_receivers() {
    let root = unique_temp_root("dependency-mode-source-summary-lambda-field-shadow");
    let app_root = root.join("app");
    let summary_root = root.join("summary");
    fs::create_dir_all(app_root.join("app/usecase")).unwrap();
    fs::create_dir_all(summary_root.join("dep/summary")).unwrap();
    fs::write(
        app_root.join("app/usecase/UsesSummaryLambda.java"),
        r#"
package app.usecase;

import dep.summary.SummaryType;
import java.util.List;

class UsesSummaryLambda {
    int score(SummaryType summary, List<Object> values) {
        int direct = summary.length;
        values.stream().map(SummaryType -> SummaryType.length);
        return direct;
    }
}
"#,
    )
    .unwrap();
    fs::write(
        summary_root.join("dep/summary/SummaryType.java"),
        r#"
package dep.summary;

public class SummaryType {
    public int length;
}
"#,
    )
    .unwrap();

    let mut config = DependencyModeConfig::for_primary_source_root(&app_root);
    config.add_source_root(&summary_root, DependencyMode::SourceSummary);
    let report = benchmark_java_resolver_config(&config).unwrap();
    let rendered = render_resolver_benchmark_report(&report);

    assert_eq!(report.parser_diagnostics, 0, "{rendered}");
    assert_eq!(report.resolver_diagnostics, 0, "{rendered}");
    assert_eq!(
        report.source_dependency_resolved_field_accesses, 1,
        "only the real SummaryType parameter access should be source-summary evidence; the lambda parameter shadows the type-like name: {rendered}"
    );
    assert!(
        rendered.contains(
            "SummaryType\tlength\tSummaryType\tSourceSummary\tUsesSummaryLambda\tscore\t1\tapp/usecase/UsesSummaryLambda.java"
        ),
        "{rendered}"
    );
}

#[test]
fn source_summary_field_contexts_include_anonymous_class_bodies() {
    let root = unique_temp_root("dependency-mode-source-summary-anonymous-field-context");
    let app_root = root.join("app");
    let summary_root = root.join("summary");
    fs::create_dir_all(app_root.join("app/usecase")).unwrap();
    fs::create_dir_all(summary_root.join("dep/summary")).unwrap();
    fs::write(
        app_root.join("app/usecase/UsesSummaryAnonymous.java"),
        r#"
package app.usecase;

import dep.summary.SummaryType;

class UsesSummaryAnonymous {
    Object build(SummaryType summary) {
        return new Object() {
            int score() {
                return summary.length;
            }
        };
    }
}
"#,
    )
    .unwrap();
    fs::write(
        summary_root.join("dep/summary/SummaryType.java"),
        r#"
package dep.summary;

public class SummaryType {
    public int length;
}
"#,
    )
    .unwrap();

    let mut config = DependencyModeConfig::for_primary_source_root(&app_root);
    config.add_source_root(&summary_root, DependencyMode::SourceSummary);
    let report = benchmark_java_resolver_config(&config).unwrap();
    let rendered = render_resolver_benchmark_report(&report);

    assert_eq!(report.parser_diagnostics, 0, "{rendered}");
    assert_eq!(report.resolver_diagnostics, 0, "{rendered}");
    assert_eq!(
        report.source_dependency_resolved_field_accesses, 1,
        "source-summary fields inside anonymous class bodies should be attributed to the summary dependency: {rendered}"
    );
    assert!(
        rendered.contains(
            "SummaryType\tlength\tSummaryType\tSourceSummary\tUsesSummaryAnonymous\tscore\t1\tapp/usecase/UsesSummaryAnonymous.java"
        ),
        "{rendered}"
    );
}

#[test]
fn source_summary_method_contexts_include_anonymous_class_bodies() {
    let root = unique_temp_root("dependency-mode-source-summary-anonymous-method-context");
    let app_root = root.join("app");
    let summary_root = root.join("summary");
    fs::create_dir_all(app_root.join("app/usecase")).unwrap();
    fs::create_dir_all(summary_root.join("dep/summary")).unwrap();
    fs::write(
        app_root.join("app/usecase/UsesSummaryAnonymousMethod.java"),
        r#"
package app.usecase;

import dep.summary.SummaryType;

class UsesSummaryAnonymousMethod {
    Object build(SummaryType summary) {
        return new Object() {
            String render() {
                return summary.label();
            }
        };
    }
}
"#,
    )
    .unwrap();
    fs::write(
        summary_root.join("dep/summary/SummaryType.java"),
        r#"
package dep.summary;

public class SummaryType {
    public String label() {
        return "summary";
    }
}
"#,
    )
    .unwrap();

    let mut config = DependencyModeConfig::for_primary_source_root(&app_root);
    config.add_source_root(&summary_root, DependencyMode::SourceSummary);
    let report = benchmark_java_resolver_config(&config).unwrap();
    let rendered = render_resolver_benchmark_report(&report);

    assert_eq!(report.parser_diagnostics, 0, "{rendered}");
    assert_eq!(report.resolver_diagnostics, 0, "{rendered}");
    assert_eq!(
        report.source_dependency_resolved_method_calls, 1,
        "source-summary methods inside anonymous class bodies should be attributed to the summary dependency: {rendered}"
    );
    assert!(
        rendered.contains(
            "SummaryType\tlabel\tSourceSummary\t0\tUsesSummaryAnonymousMethod\trender\t1\tapp/usecase/UsesSummaryAnonymousMethod.java"
        ),
        "{rendered}"
    );
}

#[test]
fn source_summary_constructor_contexts_include_anonymous_class_bodies() {
    let root = unique_temp_root("dependency-mode-source-summary-anonymous-constructor-context");
    let app_root = root.join("app");
    let summary_root = root.join("summary");
    fs::create_dir_all(app_root.join("app/usecase")).unwrap();
    fs::create_dir_all(summary_root.join("dep/summary")).unwrap();
    fs::write(
        app_root.join("app/usecase/UsesSummaryAnonymousConstructor.java"),
        r#"
package app.usecase;

import dep.summary.SummaryType;

class UsesSummaryAnonymousConstructor {
    Object build(String label) {
        return new Object() {
            SummaryType create() {
                return new SummaryType(label);
            }
        };
    }
}
"#,
    )
    .unwrap();
    fs::write(
        summary_root.join("dep/summary/SummaryType.java"),
        r#"
package dep.summary;

public class SummaryType {
    public SummaryType(String label) {}
}
"#,
    )
    .unwrap();

    let mut config = DependencyModeConfig::for_primary_source_root(&app_root);
    config.add_source_root(&summary_root, DependencyMode::SourceSummary);
    let report = benchmark_java_resolver_config(&config).unwrap();
    let rendered = render_resolver_benchmark_report(&report);

    assert_eq!(report.parser_diagnostics, 0, "{rendered}");
    assert_eq!(report.resolver_diagnostics, 0, "{rendered}");
    assert_eq!(
        report.source_dependency_resolved_constructors, 1,
        "source-summary constructors inside anonymous class bodies should be attributed to the summary dependency: {rendered}"
    );
    assert!(
        rendered.contains(
            "SummaryType\tSourceSummary\t1\tUsesSummaryAnonymousConstructor\tcreate\t1\tapp/usecase/UsesSummaryAnonymousConstructor.java"
        ),
        "{rendered}"
    );
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
            arg_count: None,
            parameter_patterns: None,
            role: ExternalApiRole::Getter,
            receiver_mutability: ReceiverMutabilityFact::ReadOnly,
            return_type: None,
            return_nullability: NullabilityState::DefinitelyNonNull,
            thrown: Vec::new(),
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
        rendered.contains("external.summary\tSourceSummary\t2"),
        "{rendered}"
    );
}

#[test]
fn resolver_benchmark_attributes_configured_provider_members_with_type_aliases() {
    let root = unique_temp_root("dependency-mode-provider-type-aliases");
    let app_root = root.join("app");
    fs::create_dir_all(app_root.join("org/example/gatk")).unwrap();
    fs::write(
        app_root.join("org/example/gatk/UsesQualifiedProviderFact.java"),
        r#"
package org.example.gatk;

import external.summary.SummaryType;

class UsesQualifiedProviderFact {
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
            package_prefix: "external.summary".to_string(),
            mode: ExternalLibraryMode::ExternalFactModel,
            coverage: ExternalApiCoverage::Semantic,
            rust_target: Some("crate::external::summary".to_string()),
            provenance: FactProvenance::ExternalLibraryPack,
        });
    provider_facts
        .external_api_effect_facts
        .push(ExternalApiEffectFact {
            receiver_type: Some("external.summary.SummaryType".to_string()),
            method_name: "value".to_string(),
            arg_count: None,
            parameter_patterns: None,
            role: ExternalApiRole::Getter,
            receiver_mutability: ReceiverMutabilityFact::ReadOnly,
            return_type: None,
            return_nullability: NullabilityState::DefinitelyNonNull,
            thrown: Vec::new(),
            provenance: FactProvenance::ExternalLibraryPack,
        });

    let mut config = DependencyModeConfig::for_primary_source_root(&app_root);
    config.add_fact_provider(
        "external.summary",
        DependencyMode::ExternalFactModel,
        provider_facts,
    );
    let report = benchmark_java_resolver_config(&config).unwrap();
    let rendered = render_resolver_benchmark_report(&report);

    assert_eq!(report.external_pack_explained_method_calls, 1, "{rendered}");
    assert_eq!(
        report.external_fact_model_explained_method_calls, 1,
        "configured provider attribution should not depend on whether a fact uses qualified or imported receiver spelling: {rendered}"
    );
    assert!(
        rendered.contains("SummaryType\tvalue\tExternalFactModel"),
        "{rendered}"
    );
}

#[test]
fn resolver_benchmark_counts_configured_provider_analysis_facts() {
    let root = unique_temp_root("dependency-mode-provider-analysis-facts");
    let app_root = root.join("app");
    fs::create_dir_all(app_root.join("org/example/gatk")).unwrap();
    fs::write(
        app_root.join("org/example/gatk/AnalysisFactProbe.java"),
        r#"
package org.example.gatk;

class AnalysisFactProbe {
    int value() { return 1; }
}
"#,
    )
    .unwrap();

    let mut provider_facts = FactSet::default();
    let parsed_dispatch_facts = parse_external_facts(
        r#"
method-effect dep.AnalysisTarget summarize read-only definitely-non-null
external-argument summarize dep.AnalysisTarget 0 definitely-non-null read-only borrowed type=String
external-constructor dep.AnalysisTarget String
external-crate-api dep.AnalysisTarget summarize 1 summarize_record
external-crate-constructor dep.AnalysisTarget 1 AnalysisTargetFacade open
unsupported-api dep.analysis dep.AnalysisTarget rawSummary requires runtime summary
unsupported-constructor dep.analysis dep.AnalysisTarget 2 requires native handle
unsupported-field dep.analysis dep.AnalysisTarget RAW_HANDLE exposes native state
capture method dep.AnalysisTarget summarize 1 lambda capture * name local String true false
trivial-getter dep.AnalysisTarget summarize value
recursion dep.AnalysisTarget summarize dep.AnalysisTarget summarize direct
control-flow dep.AnalysisTarget summarize tail-recursive-self-call
dispatch-requirement dep.AnalysisTarget needs-subtype-dispatch
catch dep.AnalysisTarget summarize 0 dep.AnalysisException
"#,
    );
    assert!(
        parsed_dispatch_facts.diagnostics.is_empty(),
        "{:?}",
        parsed_dispatch_facts.diagnostics
    );
    provider_facts.extend_from(parsed_dispatch_facts.facts);

    let mut config = DependencyModeConfig::for_primary_source_root(&app_root);
    config.add_fact_provider(
        "analysis.summary",
        DependencyMode::SourceSummary,
        provider_facts,
    );
    let report = benchmark_java_resolver_config(&config).unwrap();
    let rendered = render_resolver_benchmark_report(&report);

    assert_eq!(report.configured_provider_capture_facts, 1, "{rendered}");
    assert_eq!(
        report.configured_provider_trivial_accessor_facts, 1,
        "{rendered}"
    );
    assert_eq!(
        report.configured_provider_method_effect_facts, 1,
        "{rendered}"
    );
    assert_eq!(
        report.configured_provider_external_api_argument_effect_facts, 1,
        "{rendered}"
    );
    assert_eq!(
        report.configured_provider_external_crate_api_mapping_facts, 1,
        "{rendered}"
    );
    assert_eq!(
        report.configured_provider_external_crate_constructor_mapping_facts, 1,
        "{rendered}"
    );
    assert_eq!(
        report.configured_provider_external_constructor_facts, 1,
        "{rendered}"
    );
    assert_eq!(
        report.configured_provider_unsupported_external_api_facts, 1,
        "{rendered}"
    );
    assert_eq!(
        report.configured_provider_unsupported_external_constructor_facts, 1,
        "{rendered}"
    );
    assert_eq!(
        report.configured_provider_unsupported_external_field_facts, 1,
        "{rendered}"
    );
    assert_eq!(report.configured_provider_recursion_facts, 1, "{rendered}");
    assert_eq!(
        report.configured_provider_control_flow_facts, 1,
        "{rendered}"
    );
    assert_eq!(
        report.configured_provider_dispatch_requirement_facts, 1,
        "{rendered}"
    );
    assert_eq!(report.configured_provider_catch_facts, 1, "{rendered}");
    assert_eq!(
        report.source_summary_provider_capture_facts, 1,
        "{rendered}"
    );
    assert_eq!(
        report.source_summary_provider_trivial_accessor_facts, 1,
        "{rendered}"
    );
    assert_eq!(
        report.source_summary_provider_method_effect_facts, 1,
        "{rendered}"
    );
    assert_eq!(
        report.source_summary_provider_external_api_argument_effect_facts, 1,
        "{rendered}"
    );
    assert_eq!(
        report.source_summary_provider_external_crate_api_mapping_facts, 1,
        "{rendered}"
    );
    assert_eq!(
        report.source_summary_provider_external_crate_constructor_mapping_facts, 1,
        "{rendered}"
    );
    assert_eq!(
        report.source_summary_provider_external_constructor_facts, 1,
        "{rendered}"
    );
    assert_eq!(
        report.source_summary_provider_unsupported_external_api_facts, 1,
        "{rendered}"
    );
    assert_eq!(
        report.source_summary_provider_unsupported_external_constructor_facts, 1,
        "{rendered}"
    );
    assert_eq!(
        report.source_summary_provider_unsupported_external_field_facts, 1,
        "{rendered}"
    );
    assert_eq!(
        report.source_summary_provider_recursion_facts, 1,
        "{rendered}"
    );
    assert_eq!(
        report.source_summary_provider_control_flow_facts, 1,
        "{rendered}"
    );
    assert_eq!(
        report.source_summary_provider_dispatch_requirement_facts, 1,
        "{rendered}"
    );
    assert_eq!(report.source_summary_provider_catch_facts, 1, "{rendered}");
    assert!(rendered.contains("configured_provider_external_api_argument_effect_facts\t1"));
    assert!(rendered.contains("configured_provider_external_crate_api_mapping_facts\t1"));
    assert!(rendered.contains("configured_provider_external_crate_constructor_mapping_facts\t1"));
    assert!(rendered.contains("configured_provider_external_constructor_facts\t1"));
    assert!(rendered.contains("configured_provider_unsupported_external_api_facts\t1"));
    assert!(rendered.contains("configured_provider_unsupported_external_constructor_facts\t1"));
    assert!(rendered.contains("configured_provider_unsupported_external_field_facts\t1"));
    assert!(rendered.contains("configured_provider_trivial_accessor_facts\t1"));
    assert!(rendered.contains("configured_provider_dispatch_requirement_facts\t1"));
    assert!(rendered.contains("source_summary_provider_control_flow_facts\t1"));
}

#[test]
fn resolver_benchmark_uses_source_summary_field_facts_for_static_constants() {
    let root = unique_temp_root("dependency-mode-source-summary-field-facts");
    let app_root = root.join("app");
    fs::create_dir_all(app_root.join("org/example/gatk")).unwrap();
    fs::write(
        app_root.join("org/example/gatk/UsesSummaryConstant.java"),
        r#"
package org.example.gatk;

class UsesSummaryConstant {
    void acceptInt(int value) {}
    void acceptInt(String value) {}

    void run() {
        acceptInt(SummaryConstants.MAX_COUNT);
    }
}
"#,
    )
    .unwrap();

    let baseline_config =
        DependencyModeConfig::for_primary_source_root(&app_root).without_builtin_external_packs();
    let baseline = benchmark_java_resolver_config(&baseline_config).unwrap();
    assert_eq!(
        baseline.unresolved_field_accesses,
        1,
        "{}",
        render_resolver_benchmark_report(&baseline)
    );

    let mut provider_facts = FactSet::default();
    provider_facts.external_field_facts.push(ExternalFieldFact {
        field: FieldKey {
            type_name: "SummaryConstants".to_string(),
            field_name: "MAX_COUNT".to_string(),
        },
        field_type: Some(JavaType::Primitive(PrimitiveType::Int)),
        field_kind: ExternalFieldKind::StaticFinal,
        provenance: FactProvenance::UserOverride,
    });

    let mut config =
        DependencyModeConfig::for_primary_source_root(&app_root).without_builtin_external_packs();
    config.add_fact_provider(
        "summary-constants",
        DependencyMode::SourceSummary,
        provider_facts,
    );
    let report = benchmark_java_resolver_config(&config).unwrap();
    let rendered = render_resolver_benchmark_report(&report);

    assert_eq!(report.unresolved_field_accesses, 0, "{rendered}");
    assert_eq!(report.unresolved_method_calls, 0, "{rendered}");
    assert_eq!(report.configured_provider_facts, 1, "{rendered}");
    assert_eq!(
        report.configured_provider_external_field_facts, 1,
        "{rendered}"
    );
    assert_eq!(report.source_summary_provider_facts, 1, "{rendered}");
    assert_eq!(
        report.source_summary_provider_external_field_facts, 1,
        "{rendered}"
    );
    assert_eq!(
        report.fact_providers[0].external_field_facts, 1,
        "{rendered}"
    );
    assert!(
        rendered.contains("configured_provider_external_field_facts\t1"),
        "{rendered}"
    );
    assert!(
        rendered.contains("source_summary_provider_external_field_facts\t1"),
        "{rendered}"
    );
    assert!(
        rendered.contains("summary-constants\tSourceSummary\t1\t0\t0\t0\t0\t0\t1"),
        "{rendered}"
    );
}

#[test]
fn resolver_benchmark_reports_external_pack_provider_boundary_diagnostics() {
    let root = unique_temp_root("dependency-mode-provider-boundary-audit");
    let app_root = root.join("app");
    fs::create_dir_all(app_root.join("org/example/gatk")).unwrap();
    fs::write(
        app_root.join("org/example/gatk/UsesBadProvider.java"),
        r#"
package org.example.gatk;

class UsesBadProvider {
    Object run(other.Reader reader) {
        return reader.read();
    }
}
"#,
    )
    .unwrap();

    let mut provider_facts = FactSet::default();
    provider_facts
        .external_library_packs
        .push(ExternalLibraryPackFact {
            pack_name: "dep.boundary".to_string(),
            package_prefix: "dep.boundary".to_string(),
            mode: ExternalLibraryMode::ExternalFactModel,
            coverage: ExternalApiCoverage::Semantic,
            rust_target: Some("crate::external::dep_boundary".to_string()),
            provenance: FactProvenance::ExternalLibraryPack,
        });
    provider_facts
        .external_api_effect_facts
        .push(ExternalApiEffectFact {
            receiver_type: Some("other.Reader".to_string()),
            method_name: "read".to_string(),
            arg_count: None,
            parameter_patterns: None,
            role: ExternalApiRole::Getter,
            receiver_mutability: ReceiverMutabilityFact::ReadOnly,
            return_type: None,
            return_nullability: NullabilityState::MaybeNull,
            thrown: Vec::new(),
            provenance: FactProvenance::ExternalLibraryPack,
        });
    let mut config =
        DependencyModeConfig::for_primary_source_root(&app_root).without_builtin_external_packs();
    config
        .add_external_pack_provider("bad-boundary", provider_facts)
        .unwrap();
    let report = benchmark_java_resolver_config(&config).unwrap();
    let rendered = render_resolver_benchmark_report(&report);

    assert_eq!(
        report.external_pack_manifest_coverage_diagnostics, 1,
        "{rendered}"
    );
    assert!(
        rendered.contains("top_external_pack_manifest_coverage_diagnostics"),
        "{rendered}"
    );
    assert!(
        rendered.contains("external_pack_manifest_unowned_api_fact"),
        "{rendered}"
    );
    assert!(
        rendered.contains("external_pack_manifest_unowned_api_fact\tExternalFactModel"),
        "{rendered}"
    );
    assert!(rendered.contains("bad-boundary"), "{rendered}");
}

#[test]
fn resolver_benchmark_audits_explicit_builtin_named_pack_providers() {
    let root = unique_temp_root("dependency-mode-builtin-named-provider-boundary-audit");
    let app_root = root.join("app");
    fs::create_dir_all(app_root.join("org/example/gatk")).unwrap();
    fs::write(
        app_root.join("org/example/gatk/UsesVariantContext.java"),
        r#"
package org.example.gatk;

import htsjdk.variant.variantcontext.VariantContext;

class UsesVariantContext {
    String contig(VariantContext variant) {
        return variant.getContig();
    }
}
"#,
    )
    .unwrap();

    let mut provider_facts = htsjdk_variantcontext_pack_facts();
    provider_facts
        .external_api_effect_facts
        .push(ExternalApiEffectFact {
            receiver_type: Some("other.Reader".to_string()),
            method_name: "read".to_string(),
            arg_count: None,
            parameter_patterns: None,
            role: ExternalApiRole::Getter,
            receiver_mutability: ReceiverMutabilityFact::ReadOnly,
            return_type: None,
            return_nullability: NullabilityState::MaybeNull,
            thrown: Vec::new(),
            provenance: FactProvenance::ExternalLibraryPack,
        });
    provider_facts
        .external_api_effect_facts
        .push(ExternalApiEffectFact {
            receiver_type: Some("VariantContext".to_string()),
            method_name: "unclassifiedProbe".to_string(),
            arg_count: None,
            parameter_patterns: None,
            role: ExternalApiRole::Getter,
            receiver_mutability: ReceiverMutabilityFact::ReadOnly,
            return_type: None,
            return_nullability: NullabilityState::MaybeNull,
            thrown: Vec::new(),
            provenance: FactProvenance::ExternalLibraryPack,
        });

    let mut config =
        DependencyModeConfig::for_primary_source_root(&app_root).without_builtin_external_packs();
    config
        .add_external_pack_provider("builtin-pack:htsjdk.variant.variantcontext", provider_facts)
        .unwrap();
    let report = benchmark_java_resolver_config(&config).unwrap();
    let rendered = render_resolver_benchmark_report(&report);

    assert_eq!(
        report.external_pack_manifest_coverage_diagnostics, 3,
        "{rendered}"
    );
    assert!(
        rendered.contains("external_pack_manifest_missing_api_coverage"),
        "{rendered}"
    );
    assert!(
        rendered.contains("external_pack_manifest_unowned_api_fact"),
        "{rendered}"
    );
    assert!(
        rendered.contains("external_pack_manifest_unowned_api_fact\tExternalFactModel"),
        "{rendered}"
    );
    assert!(
        rendered.contains("builtin-pack:htsjdk.variant.variantcontext"),
        "{rendered}"
    );
}

#[test]
fn parsed_external_pack_manifest_registers_as_dependency_fact_provider() {
    let root = unique_temp_root("dependency-mode-pack-manifest-provider");
    let app_root = root.join("app");
    fs::create_dir_all(app_root.join("org/example/gatk")).unwrap();
    fs::write(
        app_root.join("org/example/gatk/UsesRuntime.java"),
        r#"
package org.example.gatk;

import dep.runtime.Reader;
import dep.runtime.Handle;

class UsesRuntime {
    Object read(Reader reader) {
        return reader.read();
    }

    Object raw(Reader reader) {
        return reader.rawDecode();
    }

    Object cache(Reader reader) {
        return reader.cache;
    }

    Handle open(String path) {
        return new Handle(path);
    }

    Object openRaw(String path) {
        return new Reader(path);
    }

    Object rawHandle(Reader reader) {
        return reader.RAW_HANDLE;
    }
}
"#,
    )
    .unwrap();

    let parsed_manifest = parse_external_pack_manifest(
        r#"
        external-pack dep.runtime Reader rust-crate-mapping mapped target=dep_runtime
        external-pack dep.runtime Handle rust-crate-mapping mapped target=dep_runtime
        external-pack dep.runtime ReaderException rust-crate-mapping mapped target=dep_runtime
        external-pack-api dep.runtime Reader read mapped getter read-only maybe-null return=Object
        external-pack-constructor dep.runtime Handle 1 mapped
        external-pack-field dep.runtime Reader cache mapped
        external-pack-field-nullability dep.runtime Reader cache definitely-non-null
        external-pack-exception dep.runtime ReaderException checked
        external-pack-object dep.runtime Reader erased-object
        external-pack-capability dep.runtime Reader clone=true final=true
        external-pack-unsupported dep.runtime Reader rawDecode requires runtime state
        external-pack-constructor dep.runtime Reader 1 unsupported
        external-pack-field dep.runtime Reader RAW_HANDLE unsupported
        "#,
    );
    assert!(
        parsed_manifest.diagnostics.is_empty(),
        "manifest should parse cleanly: {:?}",
        parsed_manifest.diagnostics
    );

    let mut config = DependencyModeConfig::for_primary_source_root(&app_root);
    config
        .add_external_pack_provider("dep.runtime.manifest", parsed_manifest.facts)
        .expect("single-mode manifest should infer a dependency fact-provider mode");
    let report = benchmark_java_resolver_config(&config).unwrap();
    let rendered = render_resolver_benchmark_report(&report);

    assert_eq!(
        report.fact_providers[0].mode,
        DependencyMode::RustCrateMapping
    );
    assert_eq!(
        report.fact_providers[0].facts, 15,
        "fact provider count should include pack boundaries, modeled API/constructor, facade mappings, and unsupported entries"
    );
    assert_eq!(report.configured_provider_facts, 15, "{rendered}");
    assert_eq!(
        report.configured_provider_external_api_effect_facts, 1,
        "{rendered}"
    );
    assert_eq!(
        report.configured_provider_external_crate_api_mapping_facts, 1,
        "{rendered}"
    );
    assert_eq!(
        report.configured_provider_external_crate_constructor_mapping_facts, 1,
        "{rendered}"
    );
    assert_eq!(
        report.configured_provider_external_constructor_facts, 1,
        "{rendered}"
    );
    assert_eq!(
        report.configured_provider_external_field_facts, 1,
        "{rendered}"
    );
    assert_eq!(
        report.configured_provider_hierarchy_override_facts, 1,
        "{rendered}"
    );
    assert_eq!(
        report.configured_provider_type_capability_facts, 1,
        "{rendered}"
    );
    assert_eq!(
        report.configured_provider_field_nullability_facts, 1,
        "{rendered}"
    );
    assert_eq!(report.configured_provider_exception_facts, 1, "{rendered}");
    assert_eq!(
        report.configured_provider_unsupported_external_api_facts, 1,
        "{rendered}"
    );
    assert_eq!(
        report.configured_provider_unsupported_external_constructor_facts, 1,
        "{rendered}"
    );
    assert_eq!(
        report.configured_provider_unsupported_external_field_facts, 1,
        "{rendered}"
    );
    assert_eq!(
        report.fact_providers[0].external_crate_api_mapping_facts, 1,
        "{rendered}"
    );
    assert_eq!(
        report.fact_providers[0].external_crate_constructor_mapping_facts, 1,
        "{rendered}"
    );
    assert_eq!(
        report.fact_providers[0].hierarchy_override_facts, 1,
        "{rendered}"
    );
    assert_eq!(
        report.fact_providers[0].type_capability_facts, 1,
        "{rendered}"
    );
    assert_eq!(
        report.fact_providers[0].field_nullability_facts, 1,
        "{rendered}"
    );
    assert_eq!(report.fact_providers[0].exception_facts, 1, "{rendered}");
    assert_eq!(report.external_api_explained_method_calls, 2, "{rendered}");
    assert_eq!(
        report.external_api_unsupported_method_calls, 1,
        "{rendered}"
    );
    assert_eq!(
        report.external_api_unsupported_constructor_calls, 1,
        "{rendered}"
    );
    assert_eq!(
        report.external_api_unsupported_field_accesses, 1,
        "{rendered}"
    );
    assert_eq!(report.external_pack_explained_method_calls, 2, "{rendered}");
    assert_eq!(
        report.external_pack_api_facts, 0,
        "configured provider facts should not be counted again as generated per-file pack facts: {rendered}"
    );
    assert_eq!(
        report.rust_crate_mapping_explained_method_calls, 2,
        "{rendered}"
    );
    assert_eq!(report.external_pack_explained_constructors, 2, "{rendered}");
    assert_eq!(
        report.rust_crate_mapping_explained_constructors, 2,
        "{rendered}"
    );
    assert!(rendered.contains(
        "dep.runtime.manifest\tRustCrateMapping\t15\t1\t0\t1\t1\t1\t1\t1\t0\t0\t1\t1\t1\t0\t3\t1\t1\t1"
    ));
    assert!(rendered.contains("configured_provider_external_crate_api_mapping_facts\t1"));
    assert!(rendered.contains("configured_provider_external_crate_constructor_mapping_facts\t1"));
    assert!(rendered.contains("configured_provider_external_field_facts\t1"));
    assert!(rendered.contains("configured_provider_hierarchy_override_facts\t1"));
    assert!(rendered.contains("configured_provider_type_capability_facts\t1"));
    assert!(rendered.contains("configured_provider_field_nullability_facts\t1"));
    assert!(rendered.contains("configured_provider_exception_facts\t1"));
    assert!(rendered.contains("external_api_unsupported_method_calls\t1"));
    assert!(rendered.contains("external_api_unsupported_constructor_calls\t1"));
    assert!(rendered.contains("external_api_unsupported_field_accesses\t1"));
    assert!(rendered.contains("top_external_pack_api_method_contexts"));
    assert!(
        rendered.contains("Reader\tread\tRustCrateMapping\t0\tUsesRuntime\tread\t1"),
        "{rendered}"
    );
    assert!(rendered.contains("top_external_pack_constructor_contexts"));
    assert!(
        rendered.contains("Handle\tHandle\tRustCrateMapping\tString\tUsesRuntime\topen\t1"),
        "{rendered}"
    );
    assert!(rendered.contains("top_external_pack_field_accesses"));
    assert!(
        rendered.contains("Reader\tcache\tRustCrateMapping\t1"),
        "{rendered}"
    );
    assert!(rendered.contains("top_external_pack_field_contexts"));
    assert!(
        rendered.contains("cache\tReader\tRustCrateMapping\tUsesRuntime\tcache\t1"),
        "{rendered}"
    );
    assert!(rendered.contains("top_external_api_unsupported_methods"));
    assert!(
        rendered.contains("Reader\trawDecode\tRustCrateMapping\t1"),
        "{rendered}"
    );
    assert!(rendered.contains("top_external_api_unsupported_method_contexts"));
    assert!(
        rendered.contains("Reader\trawDecode\tRustCrateMapping\t0\tUsesRuntime\traw\t1"),
        "{rendered}"
    );
    assert!(rendered.contains("top_external_api_unsupported_constructors"));
    assert!(
        rendered.contains("Reader\t1\tRustCrateMapping\t1"),
        "{rendered}"
    );
    assert!(rendered.contains("top_external_api_unsupported_constructor_contexts"));
    assert!(
        rendered.contains("Reader\tReader\tRustCrateMapping\tString\tUsesRuntime\topenRaw\t1"),
        "{rendered}"
    );
    assert!(rendered.contains("top_external_api_unsupported_fields"));
    assert!(
        rendered.contains("Reader\tRAW_HANDLE\tRustCrateMapping\t1"),
        "{rendered}"
    );
    assert!(rendered.contains("top_external_api_unsupported_field_contexts"));
    assert!(
        rendered.contains("RAW_HANDLE\tReader\tRustCrateMapping\tUsesRuntime\trawHandle\t1"),
        "{rendered}"
    );
}

#[test]
fn parsed_ffi_and_conservative_manifests_keep_mode_specific_benchmark_attribution() {
    let root = unique_temp_root("dependency-mode-ffi-conservative-providers");
    let app_root = root.join("app");
    fs::create_dir_all(app_root.join("org/example/gatk")).unwrap();
    fs::write(
        app_root.join("org/example/gatk/UsesBoundaries.java"),
        r#"
package org.example.gatk;

import native.runtime.NativeHandle;
import legacy.runtime.LegacyReader;
import legacy.runtime.LegacyRecord;

class UsesBoundaries {
    NativeHandle openNative(String path) {
        return new NativeHandle(path);
    }

    int readNative(NativeHandle handle) {
        return handle.readInt();
    }

    LegacyReader openLegacy(String path) {
        return new LegacyReader(path);
    }

    LegacyRecord readLegacy(LegacyReader reader) {
        return reader.read();
    }
}
"#,
    )
    .unwrap();

    let ffi_manifest = parse_external_pack_manifest(
        r#"
        external-pack native.runtime NativeHandle ffi-boundary conservative target=native_runtime
        external-pack-api native.runtime NativeHandle readInt conservative getter unknown definitely-non-null return=int
        external-pack-constructor native.runtime NativeHandle 1 conservative
        "#,
    );
    assert!(
        ffi_manifest.diagnostics.is_empty(),
        "FFI manifest should parse cleanly: {:?}",
        ffi_manifest.diagnostics
    );

    let conservative_manifest = parse_external_pack_manifest(
        r#"
        external-pack legacy.runtime LegacyReader conservative-boundary conservative
        external-pack legacy.runtime LegacyRecord conservative-boundary declared
        external-pack-api legacy.runtime LegacyReader read conservative getter unknown unknown return=legacy.runtime.LegacyRecord
        external-pack-constructor legacy.runtime LegacyReader 1 conservative
        "#,
    );
    assert!(
        conservative_manifest.diagnostics.is_empty(),
        "conservative manifest should parse cleanly: {:?}",
        conservative_manifest.diagnostics
    );

    let mut config =
        DependencyModeConfig::for_primary_source_root(&app_root).without_builtin_external_packs();
    config
        .add_external_pack_provider("native.runtime.manifest", ffi_manifest.facts)
        .expect("single FFI pack mode should be inferred");
    config
        .add_external_pack_provider("legacy.runtime.manifest", conservative_manifest.facts)
        .expect("single conservative pack mode should be inferred");

    let report = benchmark_java_resolver_config(&config).unwrap();
    let rendered = render_resolver_benchmark_report(&report);

    assert_eq!(report.fact_providers.len(), 2, "{rendered}");
    assert!(
        report
            .fact_providers
            .iter()
            .any(|provider| provider.name == "native.runtime.manifest"
                && provider.mode == DependencyMode::FfiBoundary),
        "{rendered}"
    );
    assert!(
        report
            .fact_providers
            .iter()
            .any(|provider| provider.name == "legacy.runtime.manifest"
                && provider.mode == DependencyMode::ConservativeBoundary),
        "{rendered}"
    );
    assert_eq!(report.unexplained_method_calls, 0, "{rendered}");
    assert_eq!(report.unexplained_constructors, 0, "{rendered}");
    assert_eq!(
        report.external_pack_manifest_coverage_diagnostics, 0,
        "{rendered}"
    );
    assert_eq!(
        report.ffi_boundary_pack_api_facts, 0,
        "configured provider facts should not be counted again as generated per-file pack facts: {rendered}"
    );
    assert_eq!(
        report.ffi_boundary_pack_constructor_facts, 0,
        "configured provider facts should not be counted again as generated per-file pack facts: {rendered}"
    );
    assert_eq!(report.ffi_boundary_explained_method_calls, 0, "{rendered}");
    assert_eq!(report.ffi_boundary_explained_constructors, 1, "{rendered}");
    assert_eq!(
        report.conservative_boundary_pack_api_facts, 0,
        "configured provider facts should not be counted again as generated per-file pack facts: {rendered}"
    );
    assert_eq!(
        report.conservative_boundary_pack_constructor_facts, 0,
        "configured provider facts should not be counted again as generated per-file pack facts: {rendered}"
    );
    assert_eq!(
        report.conservative_boundary_explained_method_calls, 1,
        "{rendered}"
    );
    assert_eq!(
        report.conservative_boundary_explained_constructors, 1,
        "{rendered}"
    );
    assert!(
        rendered.contains("native.runtime.manifest\tFfiBoundary\t3"),
        "{rendered}"
    );
    assert!(
        rendered.contains("legacy.runtime.manifest\tConservativeBoundary\t4"),
        "{rendered}"
    );
    assert!(rendered.contains("top_external_api_guess_methods"));
    assert!(rendered.contains("NativeHandle\treadInt\t1"), "{rendered}");
    assert!(
        rendered.contains("LegacyReader\tread\tConservativeBoundary\t1"),
        "{rendered}"
    );
    assert!(rendered.contains("top_external_pack_api_method_contexts"));
    assert!(
        rendered
            .contains("LegacyReader\tread\tConservativeBoundary\t0\tUsesBoundaries\treadLegacy\t1"),
        "{rendered}"
    );
    assert!(rendered.contains("top_external_pack_constructor_contexts"));
    assert!(
        rendered.contains(
            "NativeHandle\tNativeHandle\tFfiBoundary\tString\tUsesBoundaries\topenNative\t1"
        ),
        "{rendered}"
    );
    assert!(
        rendered.contains(
            "LegacyReader\tLegacyReader\tConservativeBoundary\tString\tUsesBoundaries\topenLegacy\t1"
        ),
        "{rendered}"
    );
}

#[test]
fn parsed_external_pack_manifest_reports_missing_owned_members_as_pack_gaps() {
    let root = unique_temp_root("dependency-mode-missing-pack-members");
    let app_root = root.join("app");
    fs::create_dir_all(app_root.join("org/example/gatk")).unwrap();
    fs::write(
        app_root.join("org/example/gatk/UsesIncompleteBoundary.java"),
        r#"
package org.example.gatk;

import dep.incomplete.Reader;
import dep.incomplete.Handle;

class UsesIncompleteBoundary {
    Object modeled(Reader reader) {
        return reader.read();
    }

    Object missingMethod(Reader reader) {
        return reader.unmodeled();
    }

    Handle modeledConstructor(String path) {
        return new Handle(path);
    }

    Handle missingConstructor() {
        return new Handle();
    }

    Object missingField(Reader reader) {
        return reader.RAW_HANDLE;
    }
}
"#,
    )
    .unwrap();

    let parsed_manifest = parse_external_pack_manifest(
        r#"
        external-pack dep.incomplete Reader external-fact-model semantic
        external-pack dep.incomplete Handle external-fact-model semantic
        external-pack-api dep.incomplete Reader read semantic getter read-only maybe-null return=Object
        external-pack-constructor dep.incomplete Handle 1 semantic
        "#,
    );
    assert!(
        parsed_manifest.diagnostics.is_empty(),
        "manifest should parse cleanly: {:?}",
        parsed_manifest.diagnostics
    );

    let mut config =
        DependencyModeConfig::for_primary_source_root(&app_root).without_builtin_external_packs();
    config
        .add_external_pack_provider("dep.incomplete.manifest", parsed_manifest.facts)
        .unwrap();
    let report = benchmark_java_resolver_config(&config).unwrap();
    let rendered = render_resolver_benchmark_report(&report);

    assert_eq!(report.external_pack_explained_method_calls, 2, "{rendered}");
    assert_eq!(report.external_pack_explained_constructors, 2, "{rendered}");
    assert_eq!(report.external_api_missing_method_calls, 1, "{rendered}");
    assert_eq!(
        report.external_api_missing_constructor_calls, 1,
        "{rendered}"
    );
    assert_eq!(report.external_api_missing_field_accesses, 1, "{rendered}");
    assert!(
        report.semantic_diagnostics >= 3,
        "missing method/constructor/field should each produce pack-owned semantic diagnostics: {rendered}"
    );
    assert!(rendered.contains("external_pack_missing_api"), "{rendered}");
    assert!(
        rendered.contains("external_pack_missing_constructor"),
        "{rendered}"
    );
    assert!(
        rendered.contains("external_pack_missing_field"),
        "{rendered}"
    );
    assert!(
        rendered.contains("top_external_api_missing_methods"),
        "{rendered}"
    );
    assert!(
        rendered.contains("Reader\tunmodeled\tExternalFactModel\t1"),
        "{rendered}"
    );
    assert!(
        rendered.contains("top_external_api_missing_method_contexts"),
        "{rendered}"
    );
    assert!(
        rendered.contains(
            "Reader\tunmodeled\tExternalFactModel\t0\tUsesIncompleteBoundary\tmissingMethod\t1"
        ),
        "{rendered}"
    );
    assert!(
        rendered.contains("top_external_api_missing_constructors"),
        "{rendered}"
    );
    assert!(
        rendered.contains("Handle\t0\tExternalFactModel\t1"),
        "{rendered}"
    );
    assert!(
        rendered.contains("top_external_api_missing_constructor_contexts"),
        "{rendered}"
    );
    assert!(
        rendered.contains(
            "Handle\tHandle\tExternalFactModel\t\tUsesIncompleteBoundary\tmissingConstructor\t1"
        ),
        "{rendered}"
    );
    assert!(
        rendered.contains("top_external_api_missing_fields"),
        "{rendered}"
    );
    assert!(
        rendered.contains("Reader\tRAW_HANDLE\tExternalFactModel\t1"),
        "{rendered}"
    );
    assert!(
        rendered.contains("top_external_api_missing_field_contexts"),
        "{rendered}"
    );
    assert!(
        rendered.contains(
            "RAW_HANDLE\tReader\tExternalFactModel\tUsesIncompleteBoundary\tmissingField\t1"
        ),
        "{rendered}"
    );
    assert!(
        !rendered.contains("\tSourceDependency\t"),
        "external-pack gap summaries should not use the source-dependency label: {rendered}"
    );
    assert_eq!(
        report.unexplained_method_calls, 0,
        "missing pack methods should not be reported as generic unexplained method calls: {rendered}"
    );
    assert_eq!(
        report.unexplained_constructors, 0,
        "missing pack constructors should not be reported as generic unexplained constructors: {rendered}"
    );
    assert_eq!(
        report.unexplained_field_accesses, 0,
        "missing pack fields should not be reported as generic unexplained field accesses: {rendered}"
    );
}

#[test]
fn parsed_crate_mapping_pack_manifest_reports_missing_owned_members_as_pack_gaps() {
    let root = unique_temp_root("dependency-mode-missing-crate-pack-members");
    let app_root = root.join("app");
    fs::create_dir_all(app_root.join("org/example/gatk")).unwrap();
    fs::write(
        app_root.join("org/example/gatk/UsesIncompleteRuntimeBoundary.java"),
        r#"
package org.example.gatk;

import dep.runtime.Reader;
import dep.runtime.Handle;

class UsesIncompleteRuntimeBoundary {
    Object modeled(Reader reader) {
        return reader.read();
    }

    Object missingMethod(Reader reader) {
        return reader.unmodeledRuntimeCall();
    }

    Handle modeledConstructor(String path) {
        return new Handle(path);
    }

    Handle missingConstructor() {
        return new Handle();
    }

    Object missingField(Reader reader) {
        return reader.RAW_RUNTIME_HANDLE;
    }
}
"#,
    )
    .unwrap();

    let parsed_manifest = parse_external_pack_manifest(
        r#"
        external-pack dep.runtime Reader rust-crate-mapping mapped target=dep_runtime
        external-pack dep.runtime Handle rust-crate-mapping mapped target=dep_runtime
        external-pack-api dep.runtime Reader read mapped getter read-only maybe-null return=Object rust=read_record
        external-pack-constructor dep.runtime Handle 1 mapped rust-type=HandleFacade rust=open
        "#,
    );
    assert!(
        parsed_manifest.diagnostics.is_empty(),
        "manifest should parse cleanly: {:?}",
        parsed_manifest.diagnostics
    );

    let mut config =
        DependencyModeConfig::for_primary_source_root(&app_root).without_builtin_external_packs();
    config
        .add_external_pack_provider("dep.runtime.manifest", parsed_manifest.facts)
        .unwrap();
    let report = benchmark_java_resolver_config(&config).unwrap();
    let rendered = render_resolver_benchmark_report(&report);

    assert_eq!(
        report.fact_providers[0].mode,
        DependencyMode::RustCrateMapping
    );
    assert_eq!(report.external_pack_explained_method_calls, 2, "{rendered}");
    assert_eq!(
        report.rust_crate_mapping_explained_method_calls, 2,
        "{rendered}"
    );
    assert_eq!(report.external_pack_explained_constructors, 2, "{rendered}");
    assert_eq!(
        report.rust_crate_mapping_explained_constructors, 2,
        "{rendered}"
    );
    assert_eq!(report.external_api_missing_method_calls, 1, "{rendered}");
    assert_eq!(
        report.external_api_missing_constructor_calls, 1,
        "{rendered}"
    );
    assert_eq!(report.external_api_missing_field_accesses, 1, "{rendered}");
    assert!(rendered.contains("external_pack_missing_api"), "{rendered}");
    assert!(
        rendered.contains("external_pack_missing_constructor"),
        "{rendered}"
    );
    assert!(
        rendered.contains("external_pack_missing_field"),
        "{rendered}"
    );
    assert!(rendered.contains("top_external_api_missing_methods"));
    assert!(
        rendered.contains("Reader\tunmodeledRuntimeCall\tRustCrateMapping\t1"),
        "{rendered}"
    );
    assert!(rendered.contains("top_external_api_missing_method_contexts"));
    assert!(
        rendered.contains(
            "Reader\tunmodeledRuntimeCall\tRustCrateMapping\t0\tUsesIncompleteRuntimeBoundary\tmissingMethod\t1"
        ),
        "{rendered}"
    );
    assert!(rendered.contains("top_external_api_missing_constructors"));
    assert!(
        rendered.contains("Handle\t0\tRustCrateMapping\t1"),
        "{rendered}"
    );
    assert!(rendered.contains("top_external_api_missing_constructor_contexts"));
    assert!(
        rendered.contains(
            "Handle\tHandle\tRustCrateMapping\t\tUsesIncompleteRuntimeBoundary\tmissingConstructor\t1"
        ),
        "{rendered}"
    );
    assert!(rendered.contains("top_external_api_missing_fields"));
    assert!(
        rendered.contains("Reader\tRAW_RUNTIME_HANDLE\tRustCrateMapping\t1"),
        "{rendered}"
    );
    assert!(rendered.contains("top_external_api_missing_field_contexts"));
    assert!(
        rendered.contains(
            "RAW_RUNTIME_HANDLE\tReader\tRustCrateMapping\tUsesIncompleteRuntimeBoundary\tmissingField\t1"
        ),
        "{rendered}"
    );
    assert_eq!(report.unexplained_method_calls, 0, "{rendered}");
    assert_eq!(report.unexplained_constructors, 0, "{rendered}");
    assert_eq!(report.unexplained_field_accesses, 0, "{rendered}");
}

#[test]
fn parsed_ffi_and_conservative_pack_manifests_report_missing_owned_members_as_pack_gaps() {
    let root = unique_temp_root("dependency-mode-missing-ffi-conservative-pack-members");
    let app_root = root.join("app");
    fs::create_dir_all(app_root.join("org/example/gatk")).unwrap();
    fs::write(
        app_root.join("org/example/gatk/UsesIncompleteRuntimePacks.java"),
        r#"
package org.example.gatk;

import native.runtime.NativeHandle;
import legacy.runtime.LegacyReader;

class UsesIncompleteRuntimePacks {
    int modeledNative(NativeHandle handle) {
        return handle.readInt();
    }

    Object missingNativeMethod(NativeHandle handle) {
        return handle.unmodeledNativeCall();
    }

    NativeHandle missingNativeConstructor() {
        return new NativeHandle();
    }

    Object missingNativeField(NativeHandle handle) {
        return handle.RAW_HANDLE;
    }

    Object modeledLegacy(LegacyReader reader) {
        return reader.read();
    }

    Object missingLegacyMethod(LegacyReader reader) {
        return reader.unmodeledLegacyCall();
    }
}
"#,
    )
    .unwrap();

    let ffi_manifest = parse_external_pack_manifest(
        r#"
        external-pack native.runtime NativeHandle ffi-boundary conservative target=native_runtime
        external-pack-api native.runtime NativeHandle readInt conservative getter unknown definitely-non-null return=int
        external-pack-constructor native.runtime NativeHandle 1 conservative
        "#,
    );
    assert!(
        ffi_manifest.diagnostics.is_empty(),
        "FFI manifest should parse cleanly: {:?}",
        ffi_manifest.diagnostics
    );

    let conservative_manifest = parse_external_pack_manifest(
        r#"
        external-pack legacy.runtime LegacyReader conservative-boundary conservative
        external-pack-api legacy.runtime LegacyReader read conservative getter unknown unknown return=Object
        "#,
    );
    assert!(
        conservative_manifest.diagnostics.is_empty(),
        "conservative manifest should parse cleanly: {:?}",
        conservative_manifest.diagnostics
    );

    let mut config =
        DependencyModeConfig::for_primary_source_root(&app_root).without_builtin_external_packs();
    config
        .add_external_pack_provider("native.runtime.incomplete", ffi_manifest.facts)
        .unwrap();
    config
        .add_external_pack_provider("legacy.runtime.incomplete", conservative_manifest.facts)
        .unwrap();

    let report = benchmark_java_resolver_config(&config).unwrap();
    let rendered = render_resolver_benchmark_report(&report);

    assert_eq!(report.ffi_boundary_explained_method_calls, 1, "{rendered}");
    assert_eq!(report.ffi_boundary_explained_constructors, 1, "{rendered}");
    assert_eq!(
        report.conservative_boundary_explained_method_calls, 2,
        "{rendered}"
    );
    assert_eq!(report.external_api_missing_method_calls, 2, "{rendered}");
    assert_eq!(
        report.external_api_missing_constructor_calls, 1,
        "{rendered}"
    );
    assert_eq!(report.external_api_missing_field_accesses, 1, "{rendered}");
    assert_eq!(
        report.unexplained_method_calls, 0,
        "missing FFI/conservative pack methods should not fall into generic unexplained calls: {rendered}"
    );
    assert_eq!(
        report.unexplained_constructors, 0,
        "missing FFI pack constructors should not fall into generic unexplained constructors: {rendered}"
    );
    assert_eq!(
        report.unexplained_field_accesses, 0,
        "missing FFI pack fields should not fall into generic unexplained fields: {rendered}"
    );
    assert!(rendered.contains("NativeHandle\tunmodeledNativeCall\tFfiBoundary\t1"));
    assert!(rendered.contains("LegacyReader\tunmodeledLegacyCall\tConservativeBoundary\t1"));
    assert!(rendered.contains("NativeHandle\t0\tFfiBoundary\t1"));
    assert!(rendered.contains("NativeHandle\tRAW_HANDLE\tFfiBoundary\t1"));
    assert!(rendered.contains("top_external_api_missing_method_contexts"));
    assert!(
        rendered.contains(
            "NativeHandle\tunmodeledNativeCall\tFfiBoundary\t0\tUsesIncompleteRuntimePacks\tmissingNativeMethod\t1"
        ),
        "{rendered}"
    );
    assert!(
        rendered.contains(
            "LegacyReader\tunmodeledLegacyCall\tConservativeBoundary\t0\tUsesIncompleteRuntimePacks\tmissingLegacyMethod\t1"
        ),
        "{rendered}"
    );
    assert!(rendered.contains("top_external_api_missing_constructor_contexts"));
    assert!(
        rendered.contains(
            "NativeHandle\tNativeHandle\tFfiBoundary\t\tUsesIncompleteRuntimePacks\tmissingNativeConstructor\t1"
        ),
        "{rendered}"
    );
    assert!(rendered.contains("top_external_api_missing_field_contexts"));
    assert!(
        rendered.contains(
            "RAW_HANDLE\tNativeHandle\tFfiBoundary\tUsesIncompleteRuntimePacks\tmissingNativeField\t1"
        ),
        "{rendered}"
    );
}

#[test]
fn external_pack_provider_rejects_missing_or_mixed_manifest_modes() {
    let mut config = DependencyModeConfig::default();
    let missing = config
        .add_external_pack_provider("empty", FactSet::default())
        .unwrap_err();
    assert_eq!(
        missing,
        DependencyFactProviderError::MissingExternalPackBoundary {
            provider_name: "empty".to_string()
        }
    );

    let mixed_manifest = parse_external_pack_manifest(
        r#"
        external-pack dep.a dep.a external-fact-model semantic
        external-pack dep.b dep.b rust-crate-mapping mapped target=dep_b
        "#,
    );
    assert!(mixed_manifest.diagnostics.is_empty());
    let mixed = config
        .add_external_pack_provider("mixed", mixed_manifest.facts)
        .unwrap_err();
    assert_eq!(
        mixed,
        DependencyFactProviderError::MixedExternalPackModes {
            provider_name: "mixed".to_string(),
            modes: vec![
                DependencyMode::ExternalFactModel,
                DependencyMode::RustCrateMapping,
            ],
        }
    );
    assert_eq!(
        mixed.to_string(),
        "mixed declares multiple external-pack modes: external-fact-model, rust-crate-mapping; split mixed-mode manifests into separate providers"
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
    assert!(
        report.object_closed_enum_representations >= 1,
        "co-translated dependency source should contribute closed enum representation evidence to the benchmark: {rendered}"
    );
    assert!(
        rendered.contains("object_closed_enum_representations\t"),
        "{rendered}"
    );

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

    let mut override_facts = FactSet::default();
    override_facts.hierarchy_overrides.push(HierarchyOverride {
        type_name: "Pet".to_string(),
        strategy: HierarchyStrategy::ErasedObject,
        provenance: FactProvenance::UserOverride,
    });
    let mut override_config = DependencyModeConfig::for_primary_source_root(&app_root);
    override_config.add_source_root(&dep_root, DependencyMode::CoTranslateSource);
    override_config.add_fact_provider(
        "user.representation.override",
        DependencyMode::ExternalFactModel,
        override_facts.clone(),
    );
    let override_report = benchmark_java_resolver_config(&override_config).unwrap();
    let override_rendered = render_resolver_benchmark_report(&override_report);
    assert!(
        override_report.object_erased_object_representations >= 1,
        "user override should beat co-translated source representation facts in dependency-mode benchmarks: {override_rendered}"
    );

    let mut override_resolved_facts = resolved.facts.clone();
    override_resolved_facts.extend_from(override_facts);
    let override_representations =
        plan_representations(&resolved.program, &override_resolved_facts);
    assert_eq!(
        override_representations.representation_for(&JavaType::Named {
            name: "Pet".to_string(),
            args: Vec::new(),
        }),
        ObjectRepresentation::ErasedObject
    );
}

#[test]
fn source_summary_dependency_hierarchy_can_drive_closed_enum_representation() {
    let root = unique_temp_root("dependency-mode-summary-object-model");
    let app_root = root.join("app");
    let summary_root = root.join("summary");
    fs::create_dir_all(app_root.join("app/usecase")).unwrap();
    fs::create_dir_all(summary_root.join("dep/model")).unwrap();

    fs::write(
        app_root.join("app/usecase/ChoiceUse.java"),
        r#"
package app.usecase;

import dep.model.Choice;
import dep.model.Selected;

class ChoiceUse {
    Choice create() {
        return new Selected();
    }
}
"#,
    )
    .unwrap();
    fs::write(
        summary_root.join("dep/model/Choice.java"),
        r#"
package dep.model;

public sealed interface Choice permits Selected, Rejected {}

public final class Selected implements Choice {
    public Selected() {}
}

public final class Rejected implements Choice {
    public Rejected() {}
}
"#,
    )
    .unwrap();

    let mut config = DependencyModeConfig::for_primary_source_root(&app_root);
    config.add_source_root(&summary_root, DependencyMode::SourceSummary);
    let report = benchmark_java_resolver_config(&config).unwrap();
    let rendered = render_resolver_benchmark_report(&report);

    assert_eq!(report.files_seen, 2);
    assert_eq!(report.files_resolved_without_errors, 1, "{rendered}");
    assert_eq!(report.unresolved_constructors, 0, "{rendered}");
    assert!(
        report.source_summary_facts >= 4,
        "source-summary roots should emit declaration facts for object strategy consumers: {rendered}"
    );
    assert!(
        report.source_summary_root_closed_hierarchy_facts >= 1,
        "sealed source-summary dependencies should expose closed-hierarchy fact counts: {rendered}"
    );
    assert!(
        report.object_closed_enum_representations >= 1,
        "source-summary dependency declarations should contribute closed enum representation evidence to the benchmark: {rendered}"
    );

    let mut override_facts = FactSet::default();
    override_facts.hierarchy_overrides.push(HierarchyOverride {
        type_name: "Choice".to_string(),
        strategy: HierarchyStrategy::ErasedObject,
        provenance: FactProvenance::UserOverride,
    });
    let mut override_config = DependencyModeConfig::for_primary_source_root(&app_root);
    override_config.add_source_root(&summary_root, DependencyMode::SourceSummary);
    override_config.add_fact_provider(
        "user.summary.representation.override",
        DependencyMode::ExternalFactModel,
        override_facts,
    );
    let override_report = benchmark_java_resolver_config(&override_config).unwrap();
    let override_rendered = render_resolver_benchmark_report(&override_report);
    assert!(
        override_report.object_erased_object_representations >= 1,
        "user override should beat source-summary representation facts in dependency-mode benchmarks: {override_rendered}"
    );
}

#[test]
fn source_summary_object_safety_distinguishes_direct_and_nested_self_parameters() {
    let root = unique_temp_root("dependency-mode-source-summary-object-safety-self-params");
    let app_root = root.join("app");
    let summary_root = root.join("summary");
    fs::create_dir_all(app_root.join("app/usecase")).unwrap();
    fs::create_dir_all(summary_root.join("dep/model")).unwrap();

    fs::write(
        app_root.join("app/usecase/SelfParameterUse.java"),
        r#"
package app.usecase;

import dep.model.DirectSelf;
import dep.model.NestedSelf;
import java.util.List;

class SelfParameterUse {
    boolean direct(DirectSelf value, DirectSelf other) {
        return value.accept(other);
    }

    boolean nested(NestedSelf value, List<NestedSelf> others) {
        return value.acceptAll(others);
    }
}
"#,
    )
    .unwrap();
    fs::write(
        summary_root.join("dep/model/SelfParameters.java"),
        r#"
package dep.model;

import java.util.List;

public interface DirectSelf {
    boolean accept(DirectSelf other);
}

public interface NestedSelf {
    boolean acceptAll(List<NestedSelf> others);
}
"#,
    )
    .unwrap();

    let mut config = DependencyModeConfig::for_primary_source_root(&app_root);
    config.add_source_root(&summary_root, DependencyMode::SourceSummary);
    let report = benchmark_java_resolver_config(&config).unwrap();
    let rendered = render_resolver_benchmark_report(&report);

    assert_eq!(report.parser_diagnostics, 0, "{rendered}");
    assert_eq!(report.resolver_diagnostics, 0, "{rendered}");
    assert_eq!(report.unresolved_method_calls, 0, "{rendered}");
    assert!(
        report.source_summary_root_object_safety_facts >= 2,
        "source-summary object-safety facts should cover both interfaces: {rendered}"
    );
    assert!(
        report.object_dyn_trait_representations >= 1,
        "direct self-typed parameters should still allow dyn-trait representation: {rendered}"
    );
    assert!(
        report.object_erased_object_representations >= 1,
        "nested generic self parameters should remain conservative erased-object pressure: {rendered}"
    );
    assert!(
        rendered.contains("NestedSelf") && rendered.contains("acceptAll"),
        "nested generic self-parameter fallback should stay visible in object diagnostics: {rendered}"
    );
    assert!(
        !rendered.contains("method accept parameter other mentions the interface type"),
        "direct self-typed parameters should not be reported as object-safety violations: {rendered}"
    );
}

#[test]
fn primary_source_hierarchy_override_cleans_breakpoint_evidence_shape() {
    let root = unique_temp_root("dependency-mode-primary-hierarchy-override");
    let app_root = root.join("app");
    fs::create_dir_all(app_root.join("app/evidence")).unwrap();

    fs::write(
        app_root.join("app/evidence/BreakpointEvidence.java"),
        r#"
package app.evidence;

import java.util.List;

public class BreakpointEvidence {
    public int getWeight() { return 0; }

    public boolean hasDistalTargets() { return false; }

    public List<String> getDistalTargets() { return null; }

    public static final class SplitRead extends BreakpointEvidence {
        public int getSplitCount() { return 1; }
    }

    public static final class ExternalEvidence extends BreakpointEvidence {
        public String getSource() { return "external"; }
    }
}
"#,
    )
    .unwrap();
    fs::write(
        app_root.join("app/evidence/UseEvidence.java"),
        r#"
package app.evidence;

class UseEvidence {
    int weight(BreakpointEvidence evidence) {
        return evidence.getWeight();
    }
}
"#,
    )
    .unwrap();

    let baseline_config = DependencyModeConfig::for_primary_source_root(&app_root);
    let baseline_report = benchmark_java_resolver_config(&baseline_config).unwrap();
    let baseline_rendered = render_resolver_benchmark_report(&baseline_report);
    assert!(
        baseline_report.object_representation_diagnostics >= 2,
        "unoverridden primary-source extends hierarchy should stay visible as object-model pressure: {baseline_rendered}"
    );
    assert!(
        baseline_rendered.contains("unsupported_inheritance"),
        "{baseline_rendered}"
    );

    let mut override_facts = FactSet::default();
    override_facts.hierarchy_overrides.push(HierarchyOverride {
        type_name: "BreakpointEvidence".to_string(),
        strategy: HierarchyStrategy::ClosedEnum {
            enum_name: "BreakpointEvidenceValue".to_string(),
            variants: vec!["SplitRead".to_string(), "ExternalEvidence".to_string()],
        },
        provenance: FactProvenance::UserOverride,
    });
    let mut override_config = DependencyModeConfig::for_primary_source_root(&app_root);
    override_config.add_fact_provider(
        "user.breakpoint-evidence.representation",
        DependencyMode::ExternalFactModel,
        override_facts,
    );
    let override_report = benchmark_java_resolver_config(&override_config).unwrap();
    let override_rendered = render_resolver_benchmark_report(&override_report);

    assert_eq!(
        override_report.object_representation_diagnostics, 0,
        "primary-source hierarchy overrides should make represented extends edges non-diagnostic: {override_rendered}"
    );
    assert!(
        override_report.object_closed_enum_representations >= 1,
        "closed enum override should be counted in the benchmark: {override_rendered}"
    );
    assert!(
        !override_rendered.contains("unsupported_inheritance"),
        "{override_rendered}"
    );
}

#[test]
fn external_fact_hierarchy_override_cleans_breakpoint_evidence_shape() {
    let root = unique_temp_root("dependency-mode-external-fact-hierarchy-override");
    let app_root = root.join("app");
    fs::create_dir_all(app_root.join("app/evidence")).unwrap();

    fs::write(
        app_root.join("app/evidence/BreakpointEvidence.java"),
        r#"
package app.evidence;

public class BreakpointEvidence {
    public static final class SplitRead extends BreakpointEvidence {}
    public static final class ExternalEvidence extends BreakpointEvidence {}
}
"#,
    )
    .unwrap();
    fs::write(
        app_root.join("app/evidence/UseEvidence.java"),
        r#"
package app.evidence;

class UseEvidence {
    BreakpointEvidence evidence(boolean split) {
        if (split) {
            return new BreakpointEvidence.SplitRead();
        }
        return new BreakpointEvidence.ExternalEvidence();
    }
}
"#,
    )
    .unwrap();

    let baseline_config = DependencyModeConfig::for_primary_source_root(&app_root);
    let baseline_report = benchmark_java_resolver_config(&baseline_config).unwrap();
    let baseline_rendered = render_resolver_benchmark_report(&baseline_report);
    assert!(
        baseline_report.object_representation_diagnostics >= 2,
        "unoverridden primary-source hierarchy should remain object-model pressure: {baseline_rendered}"
    );

    let parsed_facts = parse_external_facts(
        "closed-enum BreakpointEvidence BreakpointEvidenceValue SplitRead ExternalEvidence\n",
    );
    assert!(
        parsed_facts.diagnostics.is_empty(),
        "external hierarchy override facts should parse cleanly: {:?}",
        parsed_facts.diagnostics
    );
    let mut config = DependencyModeConfig::for_primary_source_root(&app_root);
    config.add_fact_provider(
        "user.breakpoint-evidence.closed-enum",
        DependencyMode::ExternalFactModel,
        parsed_facts.facts,
    );
    let report = benchmark_java_resolver_config(&config).unwrap();
    let rendered = render_resolver_benchmark_report(&report);

    assert_eq!(
        report.object_representation_diagnostics, 0,
        "line-oriented user facts should make represented extends edges non-diagnostic: {rendered}"
    );
    assert_eq!(report.configured_provider_hierarchy_override_facts, 1);
    assert!(
        report.object_closed_enum_representations >= 1,
        "closed enum user fact should be reflected in benchmark object strategy counts: {rendered}"
    );
    assert!(
        rendered.contains("user.breakpoint-evidence.closed-enum\tExternalFactModel"),
        "{rendered}"
    );
    assert!(!rendered.contains("unsupported_inheritance"), "{rendered}");
}

#[test]
fn external_pack_manifest_erased_superclass_cleans_runtime_adapter_shape() {
    let root = unique_temp_root("dependency-mode-pack-erased-runtime-superclass");
    let app_root = root.join("app");
    fs::create_dir_all(app_root.join("app/adapter")).unwrap();

    fs::write(
        app_root.join("app/adapter/RuntimeAdapterUse.java"),
        r#"
package app.adapter;

import dep.runtime.CodecContext;
import dep.runtime.InputBuffer;
import dep.runtime.OutputBuffer;

class Payload {}

class PayloadSerializer extends dep.runtime.Serializer<Payload> {
    public void write(CodecContext context, OutputBuffer output, Payload payload) {}

    public Payload read(CodecContext context, InputBuffer input, Class<Payload> payloadClass) {
        return new Payload();
    }
}
"#,
    )
    .unwrap();

    let baseline_config =
        DependencyModeConfig::for_primary_source_root(&app_root).without_builtin_external_packs();
    let baseline_report = benchmark_java_resolver_config(&baseline_config).unwrap();
    let baseline_rendered = render_resolver_benchmark_report(&baseline_report);
    assert_eq!(
        baseline_report.configured_provider_hierarchy_override_facts, 0,
        "without a selected runtime pack, no explicit manifest object-strategy facts should be available: {baseline_rendered}"
    );
    assert_eq!(
        baseline_report.configured_provider_type_capability_facts, 0,
        "without a selected runtime pack, runtime type capabilities must not be invented as provider facts: {baseline_rendered}"
    );

    let parsed_manifest = parse_external_pack_manifest(
        r#"
        external-pack dep.runtime dep.runtime external-fact-model semantic
        external-pack-object dep.runtime dep.runtime.Serializer erased-object
        external-pack-capability dep.runtime dep.runtime.Serializer clone=false final=false
        external-pack-capability dep.runtime dep.runtime.CodecContext final=true
        external-pack-capability dep.runtime dep.runtime.InputBuffer final=true
        external-pack-capability dep.runtime dep.runtime.OutputBuffer final=true
        "#,
    );
    assert!(
        parsed_manifest.diagnostics.is_empty(),
        "runtime manifest should parse cleanly: {:?}",
        parsed_manifest.diagnostics
    );
    let mut config =
        DependencyModeConfig::for_primary_source_root(&app_root).without_builtin_external_packs();
    config
        .add_external_pack_provider("dep.runtime.manifest", parsed_manifest.facts)
        .expect("single-mode manifest should infer an external fact-model provider");
    let report = benchmark_java_resolver_config(&config).unwrap();
    let rendered = render_resolver_benchmark_report(&report);

    assert_eq!(report.parser_diagnostics, 0, "{rendered}");
    assert_eq!(report.resolver_diagnostics, 0, "{rendered}");
    assert_eq!(
        report.object_representation_diagnostics, 0,
        "external-pack object strategy facts should keep runtime adapter inheritance as an explicit erased boundary: {rendered}"
    );
    assert_eq!(
        report.configured_provider_hierarchy_override_facts, 1,
        "{rendered}"
    );
    assert!(
        report.configured_provider_type_capability_facts >= 4,
        "{rendered}"
    );
    assert!(
        report.object_erased_object_representations >= 1,
        "the selected external pack should contribute an erased runtime superclass representation: {rendered}"
    );
    assert!(
        rendered.contains("dep.runtime.manifest\tExternalFactModel"),
        "{rendered}"
    );
    assert!(!rendered.contains("unsupported_inheritance"), "{rendered}");
}

#[test]
fn external_pack_manifest_object_safety_facts_are_reported_by_dependency_mode() {
    let root = unique_temp_root("dependency-mode-pack-object-safety-fallback");
    let app_root = root.join("app");
    fs::create_dir_all(app_root.join("app/adapter")).unwrap();

    fs::write(
        app_root.join("app/adapter/GenericFactoryUse.java"),
        r#"
package app.adapter;

interface GenericFactory {
    Object create();
}

final class GenericFactoryImpl implements GenericFactory {
    public Object create() {
        return new Object();
    }
}
"#,
    )
    .unwrap();

    let parsed_manifest = parse_external_pack_manifest(
        r#"
        external-pack dep.runtime GenericFactory external-fact-model semantic
        external-pack-object dep.runtime GenericFactory dyn-trait GenericFactoryTrait
        external-pack-object-safety dep.runtime GenericFactory nested-self-return copyAll
        external-pack-object-safety dep.runtime GenericFactory nested-self-param acceptAll values
        "#,
    );
    assert!(
        parsed_manifest.diagnostics.is_empty(),
        "object-safety manifest should parse cleanly: {:?}",
        parsed_manifest.diagnostics
    );
    let mut config = DependencyModeConfig::for_primary_source_root(&app_root);
    config
        .add_external_pack_provider("dep.runtime.object-safety", parsed_manifest.facts)
        .expect("single-mode manifest should infer an external fact-model provider");
    let report = benchmark_java_resolver_config(&config).unwrap();
    let rendered = render_resolver_benchmark_report(&report);

    assert_eq!(report.parser_diagnostics, 0, "{rendered}");
    assert_eq!(report.resolver_diagnostics, 0, "{rendered}");
    assert_eq!(
        report.configured_provider_object_safety_facts, 1,
        "{rendered}"
    );
    assert!(
        report.configured_provider_type_capability_facts >= 2,
        "{rendered}"
    );
    assert!(
        report.object_dyn_trait_representations >= 1,
        "parsed source object-safety remains higher precedence than ordinary external-pack facts for emitted source types: {rendered}"
    );
    assert!(
        rendered.contains("dep.runtime.object-safety\tExternalFactModel"),
        "{rendered}"
    );
}

#[test]
fn external_fact_closed_enum_override_cleans_multilevel_hierarchy_shape() {
    let root = unique_temp_root("dependency-mode-external-fact-multilevel-closed-enum");
    let app_root = root.join("app");
    fs::create_dir_all(app_root.join("app/evidence")).unwrap();

    fs::write(
        app_root.join("app/evidence/SignalEvidence.java"),
        r#"
package app.evidence;

public class SignalEvidence {
    public int weight() { return 0; }

    public static class ReadEvidence extends SignalEvidence {}

    public static abstract class PairEvidence extends ReadEvidence {}

    public static final class SplitRead extends ReadEvidence {}

    public static final class MateUnmapped extends ReadEvidence {}

    public static final class InterContigPair extends PairEvidence {}

    public static final class SameStrandPair extends PairEvidence {}
}
"#,
    )
    .unwrap();
    fs::write(
        app_root.join("app/evidence/UseSignalEvidence.java"),
        r#"
package app.evidence;

class UseSignalEvidence {
    SignalEvidence evidence(boolean split) {
        if (split) {
            return new SignalEvidence.SplitRead();
        }
        return new SignalEvidence.SameStrandPair();
    }
}
"#,
    )
    .unwrap();

    let baseline_config = DependencyModeConfig::for_primary_source_root(&app_root);
    let baseline_report = benchmark_java_resolver_config(&baseline_config).unwrap();
    let baseline_rendered = render_resolver_benchmark_report(&baseline_report);
    assert!(
        baseline_report.object_representation_diagnostics >= 6,
        "unoverridden multi-level hierarchy should remain visible as object-model pressure: {baseline_rendered}"
    );

    let parsed_facts = parse_external_facts(
        "closed-enum SignalEvidence SignalEvidenceValue SplitRead MateUnmapped InterContigPair SameStrandPair\n",
    );
    assert!(
        parsed_facts.diagnostics.is_empty(),
        "multi-level hierarchy override facts should parse cleanly: {:?}",
        parsed_facts.diagnostics
    );

    let mut config = DependencyModeConfig::for_primary_source_root(&app_root);
    config.add_fact_provider(
        "user.signal-evidence.closed-enum",
        DependencyMode::ExternalFactModel,
        parsed_facts.facts,
    );
    let report = benchmark_java_resolver_config(&config).unwrap();
    let rendered = render_resolver_benchmark_report(&report);

    assert_eq!(
        report.object_representation_diagnostics, 0,
        "closed-enum overrides should clean represented edges across intermediate subclasses: {rendered}"
    );
    assert!(
        report.object_closed_enum_representations >= 1,
        "multi-level closed enum user fact should be reflected in benchmark object strategy counts: {rendered}"
    );
    assert!(!rendered.contains("unsupported_inheritance"), "{rendered}");
}

#[test]
fn resolver_benchmark_reports_top_object_representation_diagnostics() {
    let root = unique_temp_root("dependency-mode-object-diagnostic-summary");
    let app_root = root.join("app");
    fs::create_dir_all(app_root.join("app/usecase")).unwrap();

    fs::write(
        app_root.join("app/usecase/InheritanceUse.java"),
        r#"
package app.usecase;

class Base {}

class Child extends Base {}
"#,
    )
    .unwrap();

    let config = DependencyModeConfig::for_primary_source_root(&app_root);
    let report = benchmark_java_resolver_config(&config).unwrap();
    let rendered = render_resolver_benchmark_report(&report);

    assert!(
        report.object_representation_diagnostics >= 1,
        "inheritance representation pressure should be counted: {rendered}"
    );
    assert!(
        report
            .top_object_representation_diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "unsupported_inheritance"
                && diagnostic.mode == Some(DependencyMode::PrimarySource)
                && diagnostic
                    .message
                    .contains("participates in an extends hierarchy")),
        "top representation diagnostics should retain code/message detail: {rendered}"
    );
    assert!(
        rendered.contains("top_object_representation_diagnostics"),
        "{rendered}"
    );
    assert!(rendered.contains("unsupported_inheritance"), "{rendered}");
    assert!(
        rendered.contains("unsupported_inheritance\tPrimarySource"),
        "{rendered}"
    );
    assert!(
        rendered.contains("app/usecase/InheritanceUse.java"),
        "{rendered}"
    );
}

#[test]
fn resolver_benchmark_labels_co_translated_object_representation_diagnostics() {
    let root = unique_temp_root("dependency-mode-co-translated-object-diagnostic-mode");
    let app_root = root.join("app");
    let dep_root = root.join("dep");
    fs::create_dir_all(app_root.join("app/usecase")).unwrap();
    fs::create_dir_all(dep_root.join("dep/model")).unwrap();

    fs::write(
        app_root.join("app/usecase/UseDependency.java"),
        r#"
package app.usecase;

import dep.model.Base;

class UseDependency {
    Base use(Base base) {
        return base;
    }
}
"#,
    )
    .unwrap();
    fs::write(
        dep_root.join("dep/model/DependencyHierarchy.java"),
        r#"
package dep.model;

public class Base {}

public class Child extends Base {}
"#,
    )
    .unwrap();

    let mut config = DependencyModeConfig::for_primary_source_root(&app_root);
    config.add_source_root(&dep_root, DependencyMode::CoTranslateSource);
    let report = benchmark_java_resolver_config(&config).unwrap();
    let rendered = render_resolver_benchmark_report(&report);

    assert!(
        report
            .top_object_representation_diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "unsupported_inheritance"
                && diagnostic.mode == Some(DependencyMode::CoTranslateSource)
                && diagnostic
                    .sample_files
                    .iter()
                    .any(|file| file == "dep/model/DependencyHierarchy.java")),
        "co-translated dependency object pressure should be labeled by source-root mode: {rendered}"
    );
    assert!(
        rendered.contains("unsupported_inheritance\tCoTranslateSource"),
        "{rendered}"
    );
    assert!(
        !rendered.contains("unsupported_inheritance\tSourceSummary"),
        "source-summary bodies should not be reported as emitted object pressure: {rendered}"
    );
}

#[test]
fn htsjdk_sam_record_filter_anonymous_class_is_erased_external_boundary() {
    let root = unique_temp_root("dependency-mode-sam-record-filter-anonymous");
    let app_root = root.join("app");
    fs::create_dir_all(app_root.join("app/usecase")).unwrap();

    fs::write(
        app_root.join("app/usecase/SamRecordFilterUse.java"),
        r#"
package app.usecase;

import htsjdk.samtools.SAMRecord;
import htsjdk.samtools.filter.SamRecordFilter;

class SamRecordFilterUse {
    SamRecordFilter filter() {
        return new SamRecordFilter() {
            public boolean filterOut(SAMRecord record) {
                return false;
            }

            public boolean filterOut(SAMRecord first, SAMRecord second) {
                return false;
            }
        };
    }
}
"#,
    )
    .unwrap();

    let config = DependencyModeConfig::for_primary_source_root(&app_root);
    let report = benchmark_java_resolver_config(&config).unwrap();
    let rendered = render_resolver_benchmark_report(&report);

    assert_eq!(report.parser_diagnostics, 0, "{rendered}");
    assert_eq!(report.resolver_diagnostics, 0, "{rendered}");
    assert_eq!(
        report.object_representation_diagnostics, 0,
        "anonymous implementations of pack-erased SamRecordFilter should stay at the HTSJDK boundary: {rendered}"
    );
    assert!(
        report.object_erased_object_representations >= 1,
        "SamRecordFilter should contribute an erased-object representation: {rendered}"
    );
    assert!(
        !rendered.contains("unsupported_anonymous_class"),
        "{rendered}"
    );
}

#[test]
fn htsjdk_delegating_iterator_anonymous_class_is_erased_external_boundary() {
    let root = unique_temp_root("dependency-mode-delegating-iterator-anonymous");
    let app_root = root.join("app");
    fs::create_dir_all(app_root.join("app/usecase")).unwrap();

    fs::write(
        app_root.join("app/usecase/DelegatingIteratorUse.java"),
        r#"
package app.usecase;

import htsjdk.samtools.util.DelegatingIterator;
import java.util.List;

class DelegatingIteratorUse {
    DelegatingIterator<String> wrap(List<String> names) {
        return new DelegatingIterator<String>(names.iterator()) {
            public void close() {
                super.close();
            }
        };
    }
}
"#,
    )
    .unwrap();

    let config = DependencyModeConfig::for_primary_source_root(&app_root);
    let report = benchmark_java_resolver_config(&config).unwrap();
    let rendered = render_resolver_benchmark_report(&report);

    assert_eq!(report.parser_diagnostics, 0, "{rendered}");
    assert_eq!(report.resolver_diagnostics, 0, "{rendered}");
    assert_eq!(
        report.object_representation_diagnostics, 0,
        "anonymous subclasses of pack-erased DelegatingIterator should stay at the HTSJDK util boundary: {rendered}"
    );
    assert!(
        report.object_erased_object_representations >= 1,
        "DelegatingIterator should contribute an erased-object representation: {rendered}"
    );
    assert!(
        !rendered.contains("unsupported_anonymous_class"),
        "{rendered}"
    );
}

#[test]
fn source_subclass_of_qualified_kryo_serializer_uses_erased_external_boundary() {
    let root = unique_temp_root("dependency-mode-qualified-kryo-serializer");
    let app_root = root.join("app");
    fs::create_dir_all(app_root.join("app/usecase")).unwrap();

    fs::write(
        app_root.join("app/usecase/KryoSerializerUse.java"),
        r#"
package app.usecase;

import com.esotericsoftware.kryo.Kryo;
import com.esotericsoftware.kryo.io.Input;
import com.esotericsoftware.kryo.io.Output;

class Payload {}

class PayloadSerializer extends com.esotericsoftware.kryo.Serializer<Payload> {
    public void write(Kryo kryo, Output output, Payload payload) {}
    public Payload read(Kryo kryo, Input input, Class<Payload> klass) {
        return new Payload();
    }
}
"#,
    )
    .unwrap();

    let config = DependencyModeConfig::for_primary_source_root(&app_root);
    let report = benchmark_java_resolver_config(&config).unwrap();
    let rendered = render_resolver_benchmark_report(&report);

    assert_eq!(report.parser_diagnostics, 0, "{rendered}");
    assert_eq!(report.resolver_diagnostics, 0, "{rendered}");
    assert_eq!(
        report.object_representation_diagnostics, 0,
        "source classes extending a qualified erased Kryo runtime superclass should not require source hierarchy lowering: {rendered}"
    );
    assert!(
        report.object_erased_object_representations >= 1,
        "qualified Kryo Serializer should contribute an erased-object representation: {rendered}"
    );
    assert!(!rendered.contains("unsupported_inheritance"), "{rendered}");
}

#[test]
fn nested_source_serializer_subclass_of_qualified_kryo_serializer_uses_external_boundary() {
    let root = unique_temp_root("dependency-mode-nested-qualified-kryo-serializer");
    let app_root = root.join("app");
    fs::create_dir_all(app_root.join("app/usecase")).unwrap();

    fs::write(
        app_root.join("app/usecase/PairLike.java"),
        r#"
package app.usecase;

import com.esotericsoftware.kryo.Kryo;
import com.esotericsoftware.kryo.io.Input;
import com.esotericsoftware.kryo.io.Output;

final class PairLike {
    static final class Serializer extends com.esotericsoftware.kryo.Serializer<PairLike> {
        public void write(Kryo kryo, Output output, PairLike pair) {}
        public PairLike read(Kryo kryo, Input input, Class<PairLike> klass) {
            return new PairLike();
        }
    }
}
"#,
    )
    .unwrap();

    let config = DependencyModeConfig::for_primary_source_root(&app_root);
    let report = benchmark_java_resolver_config(&config).unwrap();
    let rendered = render_resolver_benchmark_report(&report);

    assert_eq!(report.parser_diagnostics, 0, "{rendered}");
    assert_eq!(report.resolver_diagnostics, 0, "{rendered}");
    assert_eq!(
        report.object_representation_diagnostics, 0,
        "nested source serializers extending a qualified erased Kryo runtime superclass should not require source hierarchy lowering: {rendered}"
    );
    assert!(
        report.object_erased_object_representations >= 1,
        "qualified Kryo Serializer should contribute an erased-object representation: {rendered}"
    );
    assert!(!rendered.contains("unsupported_inheritance"), "{rendered}");
}

#[test]
fn resolver_benchmark_uses_apache_http_pack_for_request_header_mutation() {
    let root = unique_temp_root("dependency-mode-apache-http-request-header");
    let app_root = root.join("app");
    fs::create_dir_all(app_root.join("app/usecase")).unwrap();

    fs::write(
        app_root.join("app/usecase/ApacheHttpUse.java"),
        r#"
package app.usecase;

import java.net.URI;
import org.apache.http.client.methods.HttpGet;

class ApacheHttpUse {
    HttpGet request(URI uri, String token) {
        HttpGet get = new HttpGet(uri);
        get.addHeader("Authorization", token);
        return get;
    }
}
"#,
    )
    .unwrap();

    let config = DependencyModeConfig::for_primary_source_root(&app_root);
    let report = benchmark_java_resolver_config(&config).unwrap();
    let rendered = render_resolver_benchmark_report(&report);

    assert_eq!(report.semantic_diagnostics, 0, "{rendered}");
    assert!(
        rendered.contains("external_pack_explained_method_calls\t1"),
        "HttpGet.addHeader should be explained by the Apache HTTP pack: {rendered}"
    );
    assert!(
        !rendered.contains("external_pack_missing_api"),
        "Apache HTTP modeled header mutation should not produce missing-pack diagnostics: {rendered}"
    );
}

#[test]
fn source_summary_root_package_prefix_filters_summary_files() {
    let root = unique_temp_root("dependency-mode-source-summary-package-prefix");
    let app_root = root.join("app");
    let deps_root = root.join("deps");
    fs::create_dir_all(app_root.join("app/usecase")).unwrap();
    fs::create_dir_all(deps_root.join("dep/keep")).unwrap();
    fs::create_dir_all(deps_root.join("dep/skip")).unwrap();

    fs::write(
        app_root.join("app/usecase/UsesHelper.java"),
        r#"
package app.usecase;

import dep.keep.Helper;

class UsesHelper {
    int read() {
        return new Helper().value();
    }
}
"#,
    )
    .unwrap();
    fs::write(
        deps_root.join("dep/keep/Helper.java"),
        r#"
package dep.keep;

public class Helper {
    public int value() {
        return 7;
    }
}
"#,
    )
    .unwrap();
    fs::write(
        deps_root.join("dep/skip/Noise.java"),
        r#"
package dep.skip;

public class Noise {
    public int noise() {
        return 1;
    }
}
"#,
    )
    .unwrap();

    let mut config = DependencyModeConfig::for_primary_source_root(&app_root);
    config.add_source_root_with_package_prefixes(
        &deps_root,
        DependencyMode::SourceSummary,
        ["dep.keep"],
    );

    let report = benchmark_java_resolver_config(&config).unwrap();
    let rendered = render_resolver_benchmark_report(&report);
    let summary_root = report
        .source_roots
        .iter()
        .find(|root| root.mode == DependencyMode::SourceSummary)
        .unwrap();

    assert_eq!(report.files_seen, 2, "{rendered}");
    assert_eq!(summary_root.files_seen, 1, "{rendered}");
    assert_eq!(summary_root.package_prefixes, vec!["dep.keep".to_string()]);
    assert!(
        rendered.contains("SourceSummary\t1\tdep.keep"),
        "{rendered}"
    );
    assert_eq!(report.unresolved_method_calls, 0, "{rendered}");
}

#[test]
fn source_summary_neutral_envelope_resolves_core_declaration_shapes() {
    let root = unique_temp_root("dependency-mode-source-summary-neutral-envelope");
    let app_root = root.join("app");
    let summary_root = root.join("summary");
    fs::create_dir_all(app_root.join("app/envelope")).unwrap();
    fs::create_dir_all(summary_root.join("dep/envelope")).unwrap();

    fs::write(
        app_root.join("app/envelope/UsesEnvelope.java"),
        r#"
package app.envelope;

import dep.envelope.Args;
import dep.envelope.Box.Inner;
import dep.envelope.EnvelopeException.BadInput;
import dep.envelope.Flags;

class UsesEnvelope {
    int score(Args args) {
        Args copy = new Args(args.size, args.offset);
        Inner inner = new Inner(copy.size);
        if (copy.offset < 0) {
            BadInput error = new BadInput("negative offset");
            return -1;
        }
        return inner.value() + copy.offset + Flags.DEFAULT_OFFSET;
    }
}
"#,
    )
    .unwrap();
    fs::write(
        summary_root.join("dep/envelope/Args.java"),
        r#"
package dep.envelope;

public final class Args {
    public int size;
    public int offset;

    public Args(int size, int offset) {}
}
"#,
    )
    .unwrap();
    fs::write(
        summary_root.join("dep/envelope/Box.java"),
        r#"
package dep.envelope;

public final class Box {
    public static final class Inner {
        public Inner(int value) {}

        public int value() {
            return 1;
        }
    }
}
"#,
    )
    .unwrap();
    fs::write(
        summary_root.join("dep/envelope/EnvelopeException.java"),
        r#"
package dep.envelope;

public class EnvelopeException extends RuntimeException {
    public EnvelopeException(String message) {}

    public static final class BadInput extends EnvelopeException {
        public BadInput(String message) {
            super(message);
        }
    }
}
"#,
    )
    .unwrap();
    fs::write(
        summary_root.join("dep/envelope/Flags.java"),
        r#"
package dep.envelope;

public interface Flags {
    int DEFAULT_OFFSET = 3;
}
"#,
    )
    .unwrap();

    let mut config = DependencyModeConfig::for_primary_source_root(&app_root);
    config.add_source_root(&summary_root, DependencyMode::SourceSummary);
    let report = benchmark_java_resolver_config(&config).unwrap();
    let rendered = render_resolver_benchmark_report(&report);

    assert_eq!(report.parser_diagnostics, 0, "{rendered}");
    assert_eq!(report.resolver_diagnostics, 0, "{rendered}");
    assert_eq!(report.unresolved_constructors, 0, "{rendered}");
    assert_eq!(report.unresolved_field_accesses, 0, "{rendered}");
    assert!(
        report.source_dependency_resolved_constructors >= 3,
        "constructors for Args, nested Inner, and nested BadInput should resolve through summaries: {rendered}"
    );
    assert!(
        report.source_dependency_resolved_field_accesses >= 4,
        "public fields and interface constants should resolve through summaries: {rendered}"
    );
    assert!(
        report.source_dependency_resolved_method_calls >= 1,
        "nested class methods should resolve through summaries: {rendered}"
    );
    assert!(
        rendered.contains("Args\t2\tSourceSummary")
            && rendered.contains("Inner\t1\tSourceSummary")
            && rendered.contains("BadInput\t1\tSourceSummary"),
        "source dependency constructor table should show all envelope constructors: {rendered}"
    );
    assert!(
        rendered.contains("Args\tsize\tSourceSummary")
            && rendered.contains("Args\toffset\tSourceSummary")
            && rendered.contains("Flags\t"),
        "source dependency field table should show envelope fields and constants: {rendered}"
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
