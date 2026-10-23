use std::{fs, path::Path, process::Command};

#[test]
fn runtime_support_snapshot_preserves_producer_authority_on_resume() {
    let dir = std::env::temp_dir().join(format!(
        "new_java2rust_runtime_provenance_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&dir).unwrap();
    let helper = Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/focused-provenance.sh");
    let result = Command::new("bash").arg("-c").arg(r#"
set -euo pipefail
source "$1"
focused_record_translation_producer "$2" "$3/producer.tsv" debug test
focused_load_translation_producer "$3/producer.tsv"
focused_emit_runtime_support "$2" "$translation_producer_sha256" floating-point-display "$3/runtime.rs" > "$3/first.rs"
# Reassembly can use the verified snapshot without the producer executable.
focused_emit_runtime_support "$3/missing" "$translation_producer_sha256" floating-point-display "$3/runtime.rs" > "$3/resume.rs"
cmp "$3/first.rs" "$3/resume.rs"
if focused_emit_runtime_support "$2" "$translation_producer_sha256" another-kind "$3/runtime.rs"; then exit 1; fi
if focused_emit_runtime_support "$2" "$(printf '%064d' 0)" floating-point-display "$3/runtime.rs"; then exit 1; fi
cp "$3/producer.tsv" "$3/runtime.rs"
if focused_emit_runtime_support "$2" "$translation_producer_sha256" floating-point-display "$3/runtime.rs"; then exit 1; fi
"#).arg("runtime-provenance-test").arg(helper)
        .arg(env!("CARGO_BIN_EXE_new-java2rust")).arg(&dir).output().unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let support = fs::read_to_string(dir.join("first.rs")).unwrap();
    assert_eq!(
        support,
        new_java2rust::RuntimeSupport::FloatingPointDisplay.emit_rust()
    );
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn focused_error_count_excludes_source_quotes_and_summary_footers() {
    let dir = std::env::temp_dir().join(format!(
        "new_java2rust_error_count_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
    ));
    fs::create_dir_all(&dir).unwrap();
    let log = dir.join("compile.log");
    fs::write(
        &log,
        concat!(
            "error: unsupported operation\n",
            "error[E0308]: mismatched types\n",
            "\x1b[1;31merror[E0599]\x1b[0m: missing method\n",
            "  4 | println!(\"error: source text\");\n",
            "warning: mentions error[E0308]: in a message\n",
            "error: aborting due to 3 previous errors\n",
            "error: could not compile `example` (lib) due to 3 previous errors\n",
        ),
    )
    .unwrap();
    let helper = Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/focused-parse-failures.sh");
    let count = |path: &Path| {
        let output = Command::new("bash")
            .arg("-c")
            .arg("source \"$1\"; compiler_diagnostic_error_count \"$2\"")
            .arg("error-count-test")
            .arg(&helper)
            .arg(path)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap()
    };
    assert_eq!(count(&log), "3\n");
    let headers = Command::new("bash")
        .arg("-c")
        .arg("source \"$1\"; compiler_diagnostic_error_headers \"$2\"")
        .arg("header-test")
        .arg(&helper)
        .arg(&log)
        .output()
        .unwrap();
    assert!(headers.status.success());
    assert_eq!(
        String::from_utf8(headers.stdout).unwrap(),
        "error: unsupported operation\nerror[E0308]: mismatched types\nerror[E0599]: missing method\n"
    );
    let empty = dir.join("empty.log");
    fs::write(&empty, "warning: no errors\n").unwrap();
    assert_eq!(count(&empty), "0\n");
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn focused_producer_snapshot_survives_binary_replacement_and_legacy_resume_is_unknown() {
    let dir = std::env::temp_dir().join(format!(
        "new_java2rust_producer_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
    ));
    fs::create_dir_all(&dir).unwrap();
    let producer = dir.join("translator");
    fs::write(&producer, b"original release artifact").unwrap();
    let helper = Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/focused-provenance.sh");
    let output = Command::new("bash").arg("-c").arg(r#"
set -euo pipefail
source "$1"
focused_record_translation_producer "$2/translator" "$2/producer.tsv" release caller-declared
focused_load_translation_producer "$2/producer.tsv"
[[ "$translation_producer_sha256" =~ ^[0-9a-f]{64}$ ]]
[[ "$translation_producer_bytes" == 25 ]]
[[ "$translation_producer_build_profile" == release ]]
[[ "$translation_producer_profile_source" == caller-declared ]]
original_digest="$translation_producer_sha256"
focused_verify_translation_producer "$2/translator" "$original_digest"
ln -s translator "$2/link with \\ name"
focused_record_translation_producer "$2/link with \\ name" "$2/link.tsv" unknown-prebuilt unknown
focused_load_translation_producer "$2/link.tsv"
[[ "$translation_producer_sha256" == "$original_digest" ]]
[[ "$translation_producer_bytes" == 25 ]]
[[ "$translation_producer_build_profile" == unknown-prebuilt ]]
# A validator/replacement executable must not become the recorded producer.
cp "$2/producer.tsv" "$2/translator"
if focused_verify_translation_producer "$2/translator" "$original_digest"; then exit 1; fi
focused_load_translation_producer "$2/producer.tsv"
[[ "$translation_producer_sha256" == "$original_digest" ]]
[[ "$translation_producer_bytes" == 25 ]]
focused_load_translation_producer "$2/missing-legacy.tsv"
[[ "$translation_producer_binary" == unknown ]]
[[ "$translation_producer_sha256" == unknown ]]
[[ "$translation_producer_build_profile" == unknown ]]
[[ "$translation_producer_profile_source" == unknown-legacy ]]
if focused_record_translation_producer "$2/translator" "$2/bad.tsv" $'bad\tprofile' caller-declared; then exit 1; fi
[[ ! -f "$2/bad.tsv" ]]
cp "$1" "$2/broken.tsv"
if focused_load_translation_producer "$2/broken.tsv"; then exit 1; fi
"#).arg("producer-test").arg(helper).arg(&dir).output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    fs::remove_dir_all(&dir).unwrap();
}
