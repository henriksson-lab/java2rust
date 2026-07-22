use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

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
    assert!(status.contains("\tok\t"), "{status}");

    let manifest = fs::read_to_string(out_dir.join("batch-manifest.tsv")).unwrap();
    assert_eq!(manifest.lines().count(), 3, "{manifest}");
    assert!(
        manifest.lines().skip(1).all(|line| {
            let fields = line.split('\t').collect::<Vec<_>>();
            fields.len() == 6 && fields[3].len() == 16
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

fn write_comparable_batch_output(root: &Path, hash: &str) {
    fs::create_dir_all(root).unwrap();
    fs::write(
        root.join("batch-manifest.tsv"),
        format!(
            "index\tsource\trust_path\trust_hash\tdiagnostics\tinvalid_patterns\n1\tA.java\t{}/generated/00001_A.rs\t{}\t0\t0\n",
            root.display(),
            hash
        ),
    )
    .unwrap();
    fs::write(
        root.join("status.tsv"),
        format!(
            "source\trust_path\ttranslate_status\tdiagnostics\tinvalid_patterns\nA.java\t{}/generated/00001_A.rs\tok\t0\t0\n",
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
