use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

fn unique_temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "new_java2rust_{name}_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn write_batch_sources(root: &Path) {
    fs::create_dir_all(root).unwrap();
    fs::write(
        root.join("A.java"),
        r#"
package demo;

public class A {
    public int inc(int value) {
        return value + 1;
    }

    public int callB() {
        return new B().two();
    }
}
"#,
    )
    .unwrap();
    fs::write(
        root.join("B.java"),
        r#"
package demo;

public class B {
    public int two() {
        return 2;
    }
}
"#,
    )
    .unwrap();
}

#[test]
fn batch_parallel_emission_matches_serial_output_order() {
    let primary_sources = vec![
        new_java2rust::BatchSource {
            path: "A.java".to_string(),
            source: r#"
public class A {
    public int inc(int value) {
        return value + 1;
    }
}
"#
            .to_string(),
        },
        new_java2rust::BatchSource {
            path: "B.java".to_string(),
            source: r#"
public class B {
    public int two() {
        return 2;
    }
}
"#
            .to_string(),
        },
    ];

    let serial =
        new_java2rust::translate_batch_sources_with_context_sources_summary_sources_and_options(
            &primary_sources,
            &[],
            &[],
            true,
            &new_java2rust::facts::FactSet::default(),
            &new_java2rust::BatchTranslationOptions {
                emission_jobs: 1,
                ..Default::default()
            },
        );
    let parallel =
        new_java2rust::translate_batch_sources_with_context_sources_summary_sources_and_options(
            &primary_sources,
            &[],
            &[],
            true,
            &new_java2rust::facts::FactSet::default(),
            &new_java2rust::BatchTranslationOptions {
                emission_jobs: 2,
                ..Default::default()
            },
        );

    assert_eq!(serial.translations.len(), parallel.translations.len());
    assert_eq!(serial.events.len(), parallel.events.len());
    for (serial, parallel) in serial.translations.iter().zip(&parallel.translations) {
        assert_eq!(serial.path, parallel.path);
        assert_eq!(serial.rust, parallel.rust);
        assert_eq!(serial.diagnostics.len(), parallel.diagnostics.len());
    }
    assert_eq!(
        parallel
            .events
            .iter()
            .map(|event| (event.phase, event.index, event.status))
            .collect::<Vec<_>>(),
        vec![
            ("emit_start", 1, "running"),
            ("emit_end", 1, "ok"),
            ("emit_start", 2, "running"),
            ("emit_end", 2, "ok")
        ]
    );
}

#[test]
fn batch_translation_reports_inner_class_constructor_panics() {
    let translations =
        new_java2rust::translate_batch_sources_with_context_sources_and_summary_sources(
            &[new_java2rust::BatchSource {
                path: "MarkerInnerCapture.java".to_string(),
                source: r#"
public class MarkerInnerCapture {
    private int base = 10;
    class Adder {
        int add(int x) { return base + x; }
    }
    public int run() {
        Adder a = new Adder();
        base = 20;
        return a.add(1);
    }
}
"#
                .to_string(),
            }],
            &[],
            &[],
            true,
            &new_java2rust::facts::FactSet::default(),
        );

    assert_eq!(translations.len(), 1);
    assert!(
        translations[0]
            .diagnostics
            .has_code("inner_class_construction_panics"),
        "batch translation should report the runtime panic marker as a \
         diagnostic:\n{:?}",
        translations[0].diagnostics.iter().collect::<Vec<_>>()
    );
}

#[test]
fn batch_duplicate_simple_type_names_keep_distinct_signature_keys() {
    let primary_sources = vec![
        new_java2rust::BatchSource {
            path: "a/DuplicateOwner.java".to_string(),
            source: r#"
package a;

public class DuplicateOwner {
    private int first;

    public int firstValue() {
        return first;
    }
}
"#
            .to_string(),
        },
        new_java2rust::BatchSource {
            path: "b/DuplicateOwner.java".to_string(),
            source: r#"
package b;

public class DuplicateOwner {
    private int second;

    public int secondValue() {
        return second;
    }
}
"#
            .to_string(),
        },
    ];

    let report =
        new_java2rust::translate_batch_sources_with_context_sources_summary_sources_and_options(
            &primary_sources,
            &[],
            &[],
            true,
            &new_java2rust::facts::FactSet::default(),
            &new_java2rust::BatchTranslationOptions {
                emission_jobs: 2,
                ..Default::default()
            },
        );

    assert_eq!(report.translations.len(), 2);
    assert!(
        report.translations[0].rust.contains("first_value")
            && report.translations[1].rust.contains("second_value"),
        "{:#?}",
        report
            .translations
            .iter()
            .map(|translation| (&translation.path, &translation.rust))
            .collect::<Vec<_>>()
    );
}

#[test]
fn external_pack_object_facts_do_not_erase_primary_source_type_emission() {
    let primary_sources = vec![new_java2rust::BatchSource {
        path: "modeled/ModeledPrimary.java".to_string(),
        source: r#"
package modeled;

public class ModeledPrimary {
    private int value;

    public int getValue() {
        return value;
    }
}
"#
        .to_string(),
    }];
    let facts = new_java2rust::external_packs::parse_external_pack_manifest(
        r#"
        external-pack modeled modeled external-fact-model semantic
        external-pack-object modeled modeled.ModeledPrimary erased-object
        external-pack-object modeled ModeledPrimary erased-object
        "#,
    )
    .facts;

    let report =
        new_java2rust::translate_batch_sources_with_context_sources_summary_sources_and_options(
            &primary_sources,
            &[],
            &[],
            true,
            &facts,
            &new_java2rust::BatchTranslationOptions::default(),
        );

    assert_eq!(report.translations.len(), 1);
    let rust = &report.translations[0].rust;
    assert!(rust.contains("pub struct ModeledPrimary"), "{rust}");
    assert!(rust.contains("pub fn get_value(&self) -> i32"), "{rust}");
    assert!(
        !rust.contains("pub type ModeledPrimary = Box<dyn std::any::Any>;"),
        "{rust}"
    );
}

#[test]
fn translate_batch_cli_writes_phase_and_worker_events() {
    let dir = unique_temp_dir("translate_batch_events");
    let source_dir = dir.join("java");
    let out_dir = dir.join("out");
    let generated_dir = out_dir.join("generated");
    let phase_events = out_dir.join("phase-events.tsv");
    let worker_events = out_dir.join("worker-events.tsv");
    write_batch_sources(&source_dir);
    fs::create_dir_all(&out_dir).unwrap();
    fs::write(&phase_events, "phase\tstatus\telapsed_ms\tdetail\n").unwrap();
    fs::write(
        &worker_events,
        "event\tpid\tindex\tsource\telapsed_ms\tstatus\trust_path\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_new-java2rust"))
        .arg("translate-batch")
        .arg("--primary")
        .arg(&source_dir)
        .arg("--out-dir")
        .arg(&out_dir)
        .arg("--generated-dir")
        .arg(&generated_dir)
        .arg("--phase-events")
        .arg(&phase_events)
        .arg("--worker-events")
        .arg(&worker_events)
        .output()
        .expect("run translate-batch");
    assert!(
        output.status.success(),
        "translate-batch failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let status = fs::read_to_string(out_dir.join("status.tsv")).unwrap();
    assert_eq!(status.lines().count(), 3, "{status}");
    assert!(
        status.starts_with(
            "source\trust_path\ttranslate_status\tdiagnostics\tinvalid_patterns\tunsupported_members\tstubbed_constants\n"
        ),
        "{status}"
    );
    assert!(
        status.lines().skip(1).all(|line| {
            let fields = line.split('\t').collect::<Vec<_>>();
            fields.len() == 7
        }),
        "{status}"
    );
    assert!(status.contains("\tok\t"), "{status}");

    let manifest = fs::read_to_string(out_dir.join("batch-manifest.tsv")).unwrap();
    assert_eq!(manifest.lines().count(), 3, "{manifest}");
    assert!(
        manifest.lines().skip(1).all(|line| {
            let fields = line.split('\t').collect::<Vec<_>>();
            fields.len() == 7 && fields[3].len() == 16
        }),
        "{manifest}"
    );

    let phase_events = fs::read_to_string(phase_events).unwrap();
    assert!(
        phase_events.contains("batch_primary_parse\tok\t"),
        "{phase_events}"
    );
    assert!(
        phase_events.contains("batch_global_analysis\tok\t"),
        "{phase_events}"
    );
    assert!(
        phase_events.contains("batch_global_resolve_context\tok\t"),
        "{phase_events}"
    );
    assert!(
        phase_events.contains("batch_global_semantic\tok\t"),
        "{phase_events}"
    );
    assert!(
        phase_events.contains("batch_global_representation\tok\t"),
        "{phase_events}"
    );
    assert!(
        phase_events.contains("batch_global_signature\tok\t"),
        "{phase_events}"
    );
    assert!(
        phase_events.contains("batch_global_signature_fixpoint\tok\t"),
        "{phase_events}"
    );
    assert!(
        phase_events.contains("batch_emission\tok\t"),
        "{phase_events}"
    );
    assert!(
        phase_events.contains("\tjobs=10"),
        "translate-batch should default to 10 jobs\n{phase_events}"
    );

    let analysis_profile = fs::read_to_string(out_dir.join("analysis-profile.tsv")).unwrap();
    assert!(
        analysis_profile.contains("metric\tvalue\n"),
        "{analysis_profile}"
    );
    assert!(
        analysis_profile.contains("primary_files\t2\n"),
        "{analysis_profile}"
    );
    assert!(
        analysis_profile.contains("program_types\t2\n"),
        "{analysis_profile}"
    );
    assert!(
        analysis_profile.contains("signature_slot_count\t"),
        "{analysis_profile}"
    );
    assert!(
        analysis_profile.contains("signature_fixpoint_iterations\t"),
        "{analysis_profile}"
    );
    assert!(
        analysis_profile.contains("signature_component_sweeps\t"),
        "{analysis_profile}"
    );
    assert!(
        analysis_profile.contains("signature_state_clones\t"),
        "{analysis_profile}"
    );
    assert!(
        analysis_profile.contains("signature_propagation_arm_call_site_sweeps\t"),
        "{analysis_profile}"
    );

    let worker_events = fs::read_to_string(worker_events).unwrap();
    assert!(
        worker_events
            .lines()
            .any(|line| line.starts_with("batch_global_signature\t") && line.contains("\t0\t")),
        "{worker_events}"
    );
    assert!(
        worker_events.lines().any(|line| {
            line.starts_with("batch_global_signature_fixpoint_iteration_live\t")
                && line.contains("\t0\t")
        }),
        "{worker_events}"
    );
    assert!(
        worker_events
            .lines()
            .any(|line| line.starts_with("batch_emission\t") && line.contains("\t0\t")),
        "{worker_events}"
    );
    assert!(
        worker_events
            .lines()
            .any(|line| line.starts_with("emit_start\t") && line.contains("\t1\t")),
        "{worker_events}"
    );
    assert!(
        worker_events
            .lines()
            .any(|line| line.starts_with("emit_end\t") && line.contains("\t2\t")),
        "{worker_events}"
    );
}

#[test]
fn translate_batch_cli_counts_unsupported_member_diagnostics_in_status() {
    let dir = unique_temp_dir("translate_batch_unsupported_member_status");
    let source_dir = dir.join("java");
    let out_dir = dir.join("out");
    let generated_dir = out_dir.join("generated");
    fs::create_dir_all(&source_dir).unwrap();
    fs::write(
        source_dir.join("InstanceInit.java"),
        r#"
public class InstanceInit {
    {
        int value = 1;
    }
}
"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_new-java2rust"))
        .arg("translate-batch")
        .arg("--primary")
        .arg(&source_dir)
        .arg("--out-dir")
        .arg(&out_dir)
        .arg("--generated-dir")
        .arg(&generated_dir)
        .output()
        .expect("run translate-batch");
    assert!(
        output.status.success(),
        "translate-batch failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let status = fs::read_to_string(out_dir.join("status.tsv")).unwrap();
    let row = status
        .lines()
        .nth(1)
        .unwrap_or_else(|| panic!("missing status row\n{status}"));
    let fields = row.split('\t').collect::<Vec<_>>();
    assert_eq!(fields.len(), 7, "{status}");
    assert!(fields[5].parse::<usize>().unwrap_or(0) > 0, "{status}");
}

#[test]
fn translate_batch_cli_counts_unmodelled_external_constants_in_status() {
    let dir = unique_temp_dir("translate_batch_stubbed_constant_status");
    let source_dir = dir.join("java");
    let out_dir = dir.join("out");
    let generated_dir = out_dir.join("generated");
    fs::create_dir_all(&source_dir).unwrap();
    fs::write(
        source_dir.join("UseExternalLimit.java"),
        r#"
public class UseExternalLimit {
    int limit() {
        return ExternalLimits.MAX_COUNT;
    }
}
"#,
    )
    .unwrap();
    let manifest = dir.join("pack.txt");
    fs::write(
        &manifest,
        "external-pack external external semantic\n\
         external-pack-field external ExternalLimits MAX_COUNT semantic type=int\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_new-java2rust"))
        .arg("translate-batch")
        .arg("--primary")
        .arg(&source_dir)
        .arg("--out-dir")
        .arg(&out_dir)
        .arg("--generated-dir")
        .arg(&generated_dir)
        .arg("--external-pack-manifest")
        .arg(&manifest)
        .output()
        .expect("run translate-batch");
    assert!(
        output.status.success(),
        "translate-batch failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let status = fs::read_to_string(out_dir.join("status.tsv")).unwrap();
    let row = status
        .lines()
        .nth(1)
        .unwrap_or_else(|| panic!("missing status row\n{status}"));
    let fields = row.split('\t').collect::<Vec<_>>();
    assert_eq!(fields.len(), 7, "{status}");
    assert_eq!(fields[6], "1", "{status}");

    let manifest = fs::read_to_string(out_dir.join("batch-manifest.tsv")).unwrap();
    let row = manifest
        .lines()
        .nth(1)
        .unwrap_or_else(|| panic!("missing manifest row\n{manifest}"));
    let fields = row.split('\t').collect::<Vec<_>>();
    assert_eq!(fields.len(), 7, "{manifest}");
    assert_eq!(fields[6], "1", "{manifest}");
}

fn write_comparable_batch_output(root: &Path, hash: &str) {
    fs::create_dir_all(root).unwrap();
    fs::write(
        root.join("batch-manifest.tsv"),
        format!(
            "index\tsource\trust_path\trust_hash\tdiagnostics\tinvalid_patterns\tstubbed_constants\n1\tA.java\t{}/generated/00001_A.rs\t{}\t0\t0\t0\n",
            root.display(),
            hash
        ),
    )
    .unwrap();
    fs::write(
        root.join("status.tsv"),
        format!(
            "source\trust_path\ttranslate_status\tdiagnostics\tinvalid_patterns\tunsupported_members\tstubbed_constants\nA.java\t{}/generated/00001_A.rs\tok\t0\t0\t0\t0\n",
            root.display()
        ),
    )
    .unwrap();
    fs::write(
        root.join("invalid-patterns.tsv"),
        "pattern\tcount\tfiles\ncompile_error_placeholder\t0\t0\n",
    )
    .unwrap();
    fs::write(root.join("top-compile-errors.tsv"), "count\terror\n").unwrap();
    fs::write(
        root.join("summary.tsv"),
        "files\t1\ntranslate_failed_files\t0\ndiagnostic_files\t0\ninvalid_pattern_files\t0\ncargo_check\tok\ncargo_check_errors\t0\n",
    )
    .unwrap();
}

#[test]
fn focused_batch_artifact_comparator_ignores_output_prefixes_and_checks_hashes() {
    let dir = unique_temp_dir("batch_artifact_compare");
    let serial = dir.join("serial");
    let parallel = dir.join("parallel");
    write_comparable_batch_output(&serial, "0123456789abcdef");
    write_comparable_batch_output(&parallel, "0123456789abcdef");

    let script =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/compare-focused-batch-artifacts.py");
    let output = Command::new("python3")
        .arg(&script)
        .arg("--serial-dir")
        .arg(&serial)
        .arg("--parallel-dir")
        .arg(&parallel)
        .output()
        .expect("run comparator");
    assert!(
        output.status.success(),
        "comparator failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    write_comparable_batch_output(&parallel, "fedcba9876543210");
    let output = Command::new("python3")
        .arg(script)
        .arg("--serial-dir")
        .arg(&serial)
        .arg("--parallel-dir")
        .arg(&parallel)
        .output()
        .expect("run comparator");
    assert!(
        !output.status.success(),
        "comparator should reject changed hash\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("batch-manifest differs"),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
}

#[test]
fn focused_package_offline_cargo_uses_local_ndarray_shim() {
    let dir = unique_temp_dir("focused_ndarray_shim");
    let primary = dir.join("java");
    let out = dir.join("out");
    fs::create_dir_all(&primary).unwrap();
    fs::write(
        primary.join("MatrixSource.java"),
        r#"
public class MatrixSource {
    public double[][] literal() {
        return new double[][] {{1.0, 2.0}, {3.0, 4.0}};
    }
}
"#,
    )
    .unwrap();

    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/translate-focused-package.sh");
    let mut command = Command::new(&script);
    command
        .arg("--skip-resolver-bench")
        .arg("--offline-cargo")
        .arg("--jobs")
        .arg("1")
        .arg("--primary")
        .arg(&primary)
        .arg("--out")
        .arg(&out);
    if let Ok(binary) = std::env::var("CARGO_BIN_EXE_new-java2rust") {
        command.env("NEW_JAVA2RUST_BIN", binary);
    }
    let output = command.output().expect("run focused package wrapper");
    assert!(
        output.status.success(),
        "focused wrapper failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let cargo_toml = fs::read_to_string(out.join("generated-crate/Cargo.toml")).unwrap();
    assert!(
        cargo_toml.contains("ndarray = { path = \"ndarray-shim\" }"),
        "{cargo_toml}"
    );
    assert!(
        out.join("generated-crate/ndarray-shim/src/lib.rs").exists(),
        "missing local ndarray shim"
    );
    let ndarray_shim =
        fs::read_to_string(out.join("generated-crate/ndarray-shim/src/lib.rs")).unwrap();
    assert!(
        ndarray_shim.contains("pub fn from_rows(rows: &[&[T]]) -> Self")
            && ndarray_shim.contains("pub fn iter(&self) -> Rows<'_, T>"),
        "{ndarray_shim}"
    );
    let summary = fs::read_to_string(out.join("summary.tsv")).unwrap();
    assert!(summary.contains("cargo_check\tok"), "{summary}");
    assert!(summary.contains("cargo_failure_kind\tnone"), "{summary}");
    assert!(summary.contains("parse_failures\t0"), "{summary}");
    assert!(summary.contains("cargo_type_errors_valid\t1"), "{summary}");
    assert!(summary.contains("unsupported_statements\t0"), "{summary}");
    assert!(summary.contains("unsupported_members\t0"), "{summary}");
    assert!(summary.contains("unsupported_expressions\t0"), "{summary}");
    assert!(
        summary.lines().any(|line| line
            .strip_prefix("unmodelled_types\t")
            .is_some_and(|count| count.parse::<usize>().is_ok())),
        "{summary}"
    );
    assert!(summary.contains("unmodelled_call_sites\t0"), "{summary}");
    assert!(summary.contains("noop_facade_methods\t"), "{summary}");
    assert!(summary.contains("noop_facade_call_sites\t"), "{summary}");
    assert!(
        summary.lines().any(|line| line
            .strip_prefix("unmodelled_upcast_conversions\t")
            .is_some_and(|count| count.parse::<usize>().is_ok_and(|count| count > 0))),
        "{summary}"
    );
    assert!(summary.contains("stubbed_constants\t0"), "{summary}");
    assert!(
        out.join("parse-failures.tsv").exists(),
        "missing parse failure report"
    );
}

#[cfg(unix)]
#[test]
fn focused_package_generated_rust_parse_error_invalidates_type_error_count() {
    let dir = unique_temp_dir("focused_parse_gate");
    let primary = dir.join("java");
    let out = dir.join("out");
    fs::create_dir_all(&primary).unwrap();
    fs::write(
        primary.join("Broken.java"),
        r#"
public class Broken {
    public int value() {
        return 1;
    }
}
"#,
    )
    .unwrap();

    let fake_translator = dir.join("fake-translator.sh");
    fs::write(
        &fake_translator,
        r#"#!/usr/bin/env bash
set -u
set -o pipefail
printf 'Warning unsupported_member: static initializer\n' >&2
printf 'pub struct Broken {\n'
"#,
    )
    .unwrap();
    let mut permissions = fs::metadata(&fake_translator).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&fake_translator, permissions).unwrap();

    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/translate-focused-package.sh");
    let output = Command::new(&script)
        .arg("--skip-resolver-bench")
        .arg("--no-batch-translate")
        .arg("--jobs")
        .arg("1")
        .arg("--primary")
        .arg(&primary)
        .arg("--out")
        .arg(&out)
        .env("NEW_JAVA2RUST_BIN", &fake_translator)
        .output()
        .expect("run focused package wrapper");
    assert!(
        !output.status.success(),
        "focused wrapper should fail the gate when generated Rust does not parse\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let summary = fs::read_to_string(out.join("summary.tsv")).unwrap();
    assert!(summary.contains("cargo_check\tfailed"), "{summary}");
    assert!(
        summary.contains("cargo_failure_kind\tgenerated_rust"),
        "{summary}"
    );
    assert!(summary.contains("cargo_type_errors_valid\t0"), "{summary}");
    assert!(summary.contains("unsupported_members\t1"), "{summary}");
    assert!(!summary.contains("parse_failures\t0"), "{summary}");
    let parse_failures = fs::read_to_string(out.join("parse-failures.tsv")).unwrap();
    assert!(
        parse_failures.contains("this file contains an unclosed delimiter"),
        "{parse_failures}"
    );
}

#[test]
fn focused_package_wrapper_preserves_primary_source_types_covered_by_external_pack() {
    let dir = unique_temp_dir("focused_primary_pack_overlap");
    let primary = dir.join("java");
    let out = dir.join("out");
    fs::create_dir_all(&primary).unwrap();
    fs::write(
        primary.join("ModeledPrimary.java"),
        r#"
package modeled;

public class ModeledPrimary {
    public enum Kind {
        ONE,
        TWO
    }

    public int value() {
        return 1;
    }

    public ModeledPeer peer() {
        return new ModeledPeer();
    }
}
"#,
    )
    .unwrap();
    fs::write(
        primary.join("ModeledPeer.java"),
        r#"
package modeled;

public class ModeledPeer {
    public int two() {
        return 2;
    }

    public int kinds() {
        return ModeledPrimary.Kind.values().length;
    }
}
"#,
    )
    .unwrap();
    let manifest = dir.join("pack.txt");
    fs::write(
        &manifest,
        "external-pack modeled ModeledPrimary semantic\n\
         external-pack modeled ModeledPeer semantic\n\
         external-pack-object modeled ModeledPrimary erased-object\n\
         external-pack-object modeled ModeledPeer erased-object\n",
    )
    .unwrap();

    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/translate-focused-package.sh");
    let mut command = Command::new(&script);
    command
        .arg("--skip-resolver-bench")
        .arg("--jobs")
        .arg("2")
        .arg("--primary")
        .arg(&primary)
        .arg("--out")
        .arg(&out)
        .arg("--external-pack-manifest")
        .arg(&manifest);
    if let Ok(binary) = std::env::var("CARGO_BIN_EXE_new-java2rust") {
        command.env("NEW_JAVA2RUST_BIN", binary);
    }
    let output = command.output().expect("run focused package wrapper");
    assert!(
        output.status.success(),
        "focused wrapper failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let mut generated = String::new();
    for entry in fs::read_dir(out.join("generated-crate/src/generated")).unwrap() {
        let entry = entry.unwrap();
        if entry.path().extension().and_then(|ext| ext.to_str()) == Some("rs") {
            generated.push_str(&fs::read_to_string(entry.path()).unwrap());
        }
    }
    assert!(
        generated.contains("pub struct ModeledPrimary"),
        "{generated}"
    );
    assert!(
        generated.contains("pub enum ModeledPrimaryKind"),
        "{generated}"
    );
    assert!(
        generated.contains("ModeledPrimaryKind::values()"),
        "{generated}"
    );
    assert!(
        !generated.contains("pub struct ModeledPrimaryKind;"),
        "{generated}"
    );
    assert!(generated.contains("pub struct ModeledPeer"), "{generated}");
}
