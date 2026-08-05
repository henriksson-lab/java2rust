use std::collections::HashMap;
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

fn run_idiom_sweep(root: &Path) -> HashMap<String, usize> {
    let out_tsv = root.join("idiom-sweep.tsv");
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/sweep-generated-idioms.py");
    let output = Command::new("python3")
        .arg(script)
        .arg("--generated-root")
        .arg(root)
        .arg("--out-tsv")
        .arg(&out_tsv)
        .output()
        .expect("run idiom sweep");
    assert!(
        output.status.success(),
        "idiom sweep failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    fs::read_to_string(out_tsv)
        .unwrap()
        .lines()
        .skip(1)
        .filter_map(|line| {
            let fields = line.split('\t').collect::<Vec<_>>();
            Some((fields.first()?.to_string(), fields.get(1)?.parse().ok()?))
        })
        .collect()
}

#[test]
fn focused_parse_failure_helper_counts_parse_errors_and_invalidates_type_counts() {
    let dir = unique_temp_dir("focused_parse_failures");
    let compile_log = dir.join("compile.log");
    let report = dir.join("parse-failures.tsv");
    fs::write(
        &compile_log,
        r#"
error: expected expression, found `}`
error[E0308]: mismatched types
error: unknown start of token: \
error: aborting due to 3 previous errors
"#,
    )
    .unwrap();

    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/focused-parse-failures.sh");
    let output = Command::new("bash")
        .arg(&script)
        .arg("--compile-log")
        .arg(&compile_log)
        .arg("--out")
        .arg(&report)
        .output()
        .expect("run parse failure helper");
    assert!(
        output.status.success(),
        "parse helper failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("parse_failures\t2"), "{stdout}");
    assert!(stdout.contains("cargo_type_errors_valid\t0"), "{stdout}");
    let report = fs::read_to_string(report).unwrap();
    assert!(report.contains("error: expected expression"), "{report}");
    assert!(report.contains("error: unknown start of token"), "{report}");
    assert!(!report.contains("mismatched types"), "{report}");
}

#[test]
fn focused_package_support_emits_uri_parse_and_path_helpers() {
    let script_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/translate-focused-package.sh");
    let script = fs::read_to_string(script_path).unwrap();

    assert!(
        script.contains("pub fn parse<P: JavaPathArg>(value: P) -> Result<URI, JavaException>"),
        "{script}"
    );
    assert!(
        script.contains("pub fn get_path(&self) -> String"),
        "{script}"
    );
    for shared_type in ["Path", "URI", "File", "JavaPathArg", "IOUtils"] {
        assert!(script.contains(shared_type), "{script}");
    }
    assert!(script.contains("shared[impl_trait]"), "{script}");
}

#[test]
fn focused_package_source_type_stub_stripping_is_separate_from_shared_support() {
    let script_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/translate-focused-package.sh");
    let script = fs::read_to_string(script_path).unwrap();

    assert!(
        script.contains("strip_source_type_facade_stubs()"),
        "{script}"
    );
    assert!(
        script.contains("strip_shared_support_items \"$rust_path\" \"$shared_support_types_path\""),
        "{script}"
    );
    assert!(
        script.contains("--merge-input-dir \"$raw_generated_backup_dir\""),
        "{script}"
    );
    assert!(
        script.contains("strip_source_type_facade_stubs \"$rust_path\" \"$source_types_path\""),
        "{script}"
    );
    assert!(
        !script.contains("strip_shared_support_items \"$rust_path\" \"$source_types_path\""),
        "{script}"
    );
    assert!(
        script.contains("restore_missing_prelude_support_items()"),
        "{script}"
    );
    assert!(
        script.contains("restore_missing_prelude_support_items \"$rust_path\" \"$raw_generated_backup_dir/$(basename \"$rust_path\")\" \"$source_types_path\" \"$shared_support_types_path\""),
        "{script}"
    );
}

#[test]
fn focused_package_prelude_restore_keeps_top_level_helper_dependencies() {
    let script_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/translate-focused-package.sh");
    let script = fs::read_to_string(script_path).unwrap();

    assert!(script.contains("top_level_function_items"), "{script}");
    assert!(script.contains("referenced_function_names"), "{script}");
    assert!(
        script.contains("pending_texts = [current] + list(chunks)")
            && script.contains("chunks.append(variants[0])"),
        "{script}"
    );
}

#[test]
fn focused_package_shared_support_stripper_keeps_dyn_trait_inherent_impls() {
    let script_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/translate-focused-package.sh");
    let script = fs::read_to_string(script_path).unwrap();

    assert!(
        script.contains(
            "(dyn[[:space:]]+)?[A-Za-z_][A-Za-z0-9_]*(<[^{}]+>)?[[:space:]]*(\\{|where([[:space:]]|$))"
        ),
        "{script}"
    );
    assert!(
        script.contains("(dyn[[:space:]]+)?/, \"\", impl_name)"),
        "{script}"
    );
}

#[test]
fn focused_package_shared_support_stripper_matches_generic_inherent_impl_targets() {
    let script_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/translate-focused-package.sh");
    let script = fs::read_to_string(script_path).unwrap();

    assert!(
        script.contains("[A-Za-z_][A-Za-z0-9_]*(<[^{}]+>)?[[:space:]]*(\\{|where([[:space:]]|$))"),
        "{script}"
    );
    assert!(
        script.contains(
            "sub(/(<[^{}]+>)?[[:space:]]*(\\{|where([[:space:]]|$)).*$/, \"\", impl_name)"
        ),
        "{script}"
    );
}

#[test]
fn focused_package_shared_support_stripper_keeps_from_impls_from_module_local_types() {
    let script_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/translate-focused-package.sh");
    let script = fs::read_to_string(script_path).unwrap();

    assert!(
        script.contains("from_trait_source = impl_trait_raw"),
        "{script}"
    );
    assert!(
        script.contains("keep_module_from_impl_to_shared = target_is_shared && from_trait_source != \"\" && !shared[from_trait_source]"),
        "{script}"
    );
    assert!(
        script.contains("(target_is_shared && !keep_module_from_impl_to_shared)"),
        "{script}"
    );
}

#[test]
fn focused_package_shared_support_stripper_keeps_reference_trait_impl_targets() {
    let script_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/translate-focused-package.sh");
    let script = fs::read_to_string(script_path).unwrap();

    assert!(
        script.contains("for[[:space:]]+&?([[:space:]]*)?[A-Za-z_]"),
        "{script}"
    );
}

#[test]
fn focused_package_module_type_extractor_strips_trait_bounds() {
    let script_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/translate-focused-package.sh");
    let script = fs::read_to_string(script_path).unwrap();

    assert!(script.contains("sub(/[=;{(<:].*/, \"\", name)"), "{script}");
}

#[test]
fn focused_package_source_type_stub_stripper_removes_earlier_same_module_duplicate() {
    let script_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/translate-focused-package.sh");
    let script = fs::read_to_string(script_path).unwrap();

    assert!(script.contains("current_source_decl_positions"), "{script}");
    assert!(
        script.contains("if any(position > index for position in current_source_decl_positions):"),
        "{script}"
    );
}

#[test]
fn focused_package_source_type_stub_stripping_does_not_treat_lifetimes_as_chars() {
    let script_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/translate-focused-package.sh");
    let script = fs::read_to_string(script_path).unwrap();

    assert!(
        script.contains("if ch == \"'\":\n                next_index = index + 1"),
        "{script}"
    );
    assert!(
        script.contains("is_char_literal = index + 2 < len(line) and line[index + 2] == \"'\""),
        "{script}"
    );
    assert!(
        script.contains("if ch == \"'\" and is_char_literal:"),
        "{script}"
    );
}

#[test]
fn external_pack_dependency_extractor_infers_sibling_facade_crates() {
    let dir = unique_temp_dir("external_pack_deps");
    let manifests_dir = dir.join("manifests");
    let crates_dir = dir.join("crates");
    fs::create_dir_all(&manifests_dir).unwrap();
    fs::create_dir_all(crates_dir.join("hdf5-facade")).unwrap();
    fs::create_dir_all(crates_dir.join("sqlite-facade")).unwrap();
    fs::write(
        manifests_dir.join("native.external-pack"),
        r#"
external-pack hdf5 HDF5Library rust-crate-mapping mapped target=hdf5_facade
external-pack hdf5 HDF5File rust-crate-mapping mapped target=hdf5_facade
external-pack sqlite java.sql.Connection rust-crate-mapping mapped target=sqlite_facade
"#,
    )
    .unwrap();

    let script =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/extract-external-pack-cargo-deps.py");
    let output = Command::new("python3")
        .arg(script)
        .arg(manifests_dir.join("native.external-pack"))
        .output()
        .expect("run dependency extractor");
    assert!(
        output.status.success(),
        "dependency extractor failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains(&format!(
            "hdf5-facade = {{ path = \"{}\" }}",
            crates_dir.join("hdf5-facade").display()
        )),
        "{stdout}"
    );
    assert!(
        stdout.contains(&format!(
            "sqlite-facade = {{ path = \"{}\" }}",
            crates_dir.join("sqlite-facade").display()
        )),
        "{stdout}"
    );
    assert_eq!(
        stdout.matches("hdf5-facade = { path").count(),
        1,
        "{stdout}"
    );
}

#[test]
fn external_pack_dependency_extractor_honors_explicit_package_and_path() {
    let dir = unique_temp_dir("external_pack_explicit_deps");
    let manifests_dir = dir.join("manifests");
    fs::create_dir_all(&manifests_dir).unwrap();
    fs::create_dir_all(dir.join("vendor/native-assertions")).unwrap();
    fs::write(
        manifests_dir.join("test.external-pack"),
        r#"
external-pack test Assertions rust-crate-mapping mapped target=test_facade package=native-assertions path=../vendor/native-assertions
"#,
    )
    .unwrap();

    let script =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/extract-external-pack-cargo-deps.py");
    let output = Command::new("python3")
        .arg(script)
        .arg(manifests_dir.join("test.external-pack"))
        .output()
        .expect("run dependency extractor");
    assert!(
        output.status.success(),
        "dependency extractor failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains(&format!(
            "native-assertions = {{ path = \"{}\" }}",
            dir.join("vendor/native-assertions").display()
        )),
        "{stdout}"
    );
}

#[test]
fn external_pack_exception_conversion_extractor_uses_crate_mapped_exception_facts() {
    let dir = unique_temp_dir("external_pack_exception_conversions");
    let manifest = dir.join("native.external-pack");
    fs::write(
        &manifest,
        r#"
external-pack hdf5 HDF5Error rust-crate-mapping mapped target=hdf5_facade
external-pack java.sql java.sql.SQLException rust-crate-mapping mapped target=sqlite_facade
external-pack htsjdk htsjdk.samtools.util.RuntimeIOException rust-crate-mapping mapped target=htsjdk_facade
external-pack-exception hdf5 HDF5Error checked
external-pack-exception java.sql java.sql.SQLException checked
external-pack-exception htsjdk htsjdk.samtools.util.RuntimeIOException unchecked
"#,
    )
    .unwrap();

    let script = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("scripts/extract-external-pack-exception-conversions.py");
    let output = Command::new("python3")
        .arg(script)
        .arg(manifest)
        .output()
        .expect("run exception conversion extractor");
    assert!(
        output.status.success(),
        "exception conversion extractor failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("impl From<hdf5_facade::HDF5Error> for JavaException"),
        "{stdout}"
    );
    assert!(
        stdout.contains("JavaException::from_display(\"HDF5Error\", err)"),
        "{stdout}"
    );
    assert!(
        stdout.contains("impl From<sqlite_facade::SQLException> for JavaException"),
        "{stdout}"
    );
    assert!(
        stdout.contains("impl From<htsjdk_facade::RuntimeIOException> for JavaException"),
        "{stdout}"
    );
}

#[test]
fn shared_support_merger_preserves_associated_constants() {
    let dir = unique_temp_dir("shared_support_consts");
    let generated_dir = dir.join("generated");
    fs::create_dir_all(&generated_dir).unwrap();
    fs::write(
        generated_dir.join("00001_UseA.rs"),
        r#"
#[derive(Clone, Debug, Default)]
pub struct VCFConstants;

impl VCFConstants {
    pub const DEPTH_KEY: &'static str = "DEPTH_KEY";

    pub fn get_name(&self) -> String {
        String::new()
    }
}

pub struct UseA;
"#,
    )
    .unwrap();
    fs::write(
        generated_dir.join("00002_UseB.rs"),
        r#"
#[derive(Clone, Debug, Default)]
pub struct VCFConstants;

impl VCFConstants {
    pub const DEPTH_KEY: &'static str = "DEPTH_KEY";

    pub fn get_name(&self) -> String {
        String::new()
    }
}

pub struct UseB;
"#,
    )
    .unwrap();
    fs::write(
        generated_dir.join("00003_UseColorAlias.rs"),
        r#"
#[derive(Clone, Debug, Default)]
pub struct JavaAwtColor;

impl JavaAwtColor {
    pub const BLACK: Color = Color;
}

pub type Color = JavaAwtColor;

pub struct UseColorAlias;
"#,
    )
    .unwrap();
    fs::write(
        generated_dir.join("00004_UseColorTarget.rs"),
        r#"
#[derive(Clone, Debug, Default)]
pub struct JavaAwtColor;

impl JavaAwtColor {
    pub const BLACK: JavaAwtColor = JavaAwtColor;
}

pub type Color = JavaAwtColor;

pub struct UseColorTarget;
"#,
    )
    .unwrap();

    let shared_support = dir.join("shared.rs");
    let shared_types = dir.join("shared-types.tsv");
    let conflicts = dir.join("conflicts.tsv");
    let script =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/merge-shared-support-facades.py");
    let output = Command::new("python3")
        .arg(script)
        .arg("--generated-dir")
        .arg(&generated_dir)
        .arg("--shared-support")
        .arg(&shared_support)
        .arg("--shared-types")
        .arg(&shared_types)
        .arg("--conflicts")
        .arg(&conflicts)
        .output()
        .expect("run shared support merger");
    assert!(
        output.status.success(),
        "shared support merger failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let shared = fs::read_to_string(shared_support).unwrap();
    assert!(
        shared.contains("pub const DEPTH_KEY: &'static str = \"DEPTH_KEY\";"),
        "{shared}"
    );
    assert!(
        shared.contains("pub fn get_name(&self) -> String"),
        "{shared}"
    );
    assert!(
        shared.contains("pub const BLACK: Color = JavaAwtColor;")
            && !shared.contains("pub const BLACK: Color = Color;")
            && shared.matches("pub const BLACK:").count() == 1,
        "{shared}"
    );
    assert!(
        !fs::read_to_string(generated_dir.join("00001_UseA.rs"))
            .unwrap()
            .contains("impl VCFConstants"),
        "merged support impl should be removed from source module"
    );
}

#[test]
fn shared_support_merger_prefers_source_enum_over_opaque_stub() {
    let dir = unique_temp_dir("shared_support_prefers_enum");
    let generated_dir = dir.join("generated");
    fs::create_dir_all(&generated_dir).unwrap();
    fs::write(
        generated_dir.join("00001_UsesHeader.rs"),
        r#"
#[derive(Clone, Debug, Default)]
pub struct HeaderFIELDS;

impl std::fmt::Display for HeaderFIELDS {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { write!(f, "") }
}

pub struct UsesHeader;
"#,
    )
    .unwrap();
    fs::write(
        generated_dir.join("00002_UsesHeaderAgain.rs"),
        r#"
#[derive(Clone, Debug, Default)]
pub struct HeaderFIELDS;

impl std::fmt::Display for HeaderFIELDS {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { write!(f, "") }
}

pub struct UsesHeaderAgain;
"#,
    )
    .unwrap();
    fs::write(
        generated_dir.join("00003_Header.rs"),
        r#"
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HeaderFIELDS {
    CHROM,
    POS,
}

impl HeaderFIELDS {
    pub fn values() -> Vec<Self> { vec![Self::CHROM, Self::POS] }
    pub fn value_of<T: AsRef<str>>(name: T) -> Self {
        match name.as_ref() {
            "CHROM" => Self::CHROM,
            "POS" => Self::POS,
            other => panic!("unknown Java enum constant Header.FIELDS.{}", other),
        }
    }
}

impl std::fmt::Display for HeaderFIELDS {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", match self { Self::CHROM => "CHROM", Self::POS => "POS" })
    }
}

pub struct Header;
"#,
    )
    .unwrap();

    let shared_support = dir.join("shared.rs");
    let shared_types = dir.join("shared-types.tsv");
    let conflicts = dir.join("conflicts.tsv");
    let script =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/merge-shared-support-facades.py");
    let output = Command::new("python3")
        .arg(script)
        .arg("--generated-dir")
        .arg(&generated_dir)
        .arg("--shared-support")
        .arg(&shared_support)
        .arg("--shared-types")
        .arg(&shared_types)
        .arg("--conflicts")
        .arg(&conflicts)
        .output()
        .expect("run shared support merger");
    assert!(
        output.status.success(),
        "shared support merger failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let shared = fs::read_to_string(shared_support).unwrap();
    assert!(shared.contains("pub enum HeaderFIELDS"), "{shared}");
    assert!(shared.contains("pub fn values() -> Vec<Self>"), "{shared}");
    assert!(shared.contains("Self::CHROM"), "{shared}");
    assert!(
        !shared.contains("pub struct HeaderFIELDS;") && !shared.contains("write!(f, \"\")"),
        "{shared}"
    );
    let conflicts = fs::read_to_string(conflicts).unwrap();
    assert!(
        conflicts.contains("HeaderFIELDS\tdeclaration\tkept enum"),
        "{conflicts}"
    );

    let source_module = fs::read_to_string(generated_dir.join("00003_Header.rs")).unwrap();
    assert!(
        !source_module.contains("pub enum HeaderFIELDS")
            && !source_module.contains("impl HeaderFIELDS"),
        "{source_module}"
    );
}

#[test]
fn shared_support_merger_adds_level_constants_for_promoted_logging_facade() {
    let dir = unique_temp_dir("shared_support_level_consts");
    let generated_dir = dir.join("generated");
    fs::create_dir_all(&generated_dir).unwrap();
    for (file, primary, field) in [
        ("00001_UseWarning.rs", "UseWarning", "WARNING"),
        ("00002_UseSevere.rs", "UseSevere", "SEVERE"),
    ] {
        fs::write(
            generated_dir.join(file),
            format!(
                r#"
#[derive(Clone, Debug, Default)]
pub struct Level;

pub struct {primary};

impl {primary} {{
    pub fn value() -> Level {{
        Level::{field}
    }}
}}
"#
            ),
        )
        .unwrap();
    }

    let shared_support = dir.join("shared.rs");
    let shared_types = dir.join("shared-types.tsv");
    let conflicts = dir.join("conflicts.tsv");
    let script =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/merge-shared-support-facades.py");
    let output = Command::new("python3")
        .arg(script)
        .arg("--generated-dir")
        .arg(&generated_dir)
        .arg("--shared-support")
        .arg(&shared_support)
        .arg("--shared-types")
        .arg(&shared_types)
        .arg("--conflicts")
        .arg(&conflicts)
        .output()
        .expect("run shared support merger");
    assert!(
        output.status.success(),
        "shared support merger failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let shared = fs::read_to_string(shared_support).unwrap();
    assert!(
        shared.contains("pub struct Level;")
            && shared.contains("pub const WARNING: Level = Level;")
            && shared.contains("pub const SEVERE: Level = Level;"),
        "{shared}"
    );
}

#[test]
fn shared_support_merger_adds_input_stream_upcast_for_promoted_jdk_facades() {
    let dir = unique_temp_dir("shared_support_input_stream_upcast");
    let generated_dir = dir.join("generated");
    fs::create_dir_all(&generated_dir).unwrap();
    for (file, primary) in [
        ("00001_UseFileInput.rs", "UseFileInput"),
        ("00002_UseInput.rs", "UseInput"),
    ] {
        fs::write(
            generated_dir.join(file),
            format!(
                r#"
#[derive(Clone, Debug, Default)]
pub struct InputStream;

#[derive(Clone, Debug, Default)]
pub struct FileInputStream;

pub struct {primary};
"#
            ),
        )
        .unwrap();
    }

    let shared_support = dir.join("shared.rs");
    let shared_types = dir.join("shared-types.tsv");
    let conflicts = dir.join("conflicts.tsv");
    fs::write(
        &shared_support,
        "#[derive(Clone, Debug, Default)]\npub struct InputStream;\n",
    )
    .unwrap();
    fs::write(&shared_types, "InputStream\n").unwrap();
    let script =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/merge-shared-support-facades.py");
    let output = Command::new("python3")
        .arg(script)
        .arg("--generated-dir")
        .arg(&generated_dir)
        .arg("--shared-support")
        .arg(&shared_support)
        .arg("--shared-types")
        .arg(&shared_types)
        .arg("--conflicts")
        .arg(&conflicts)
        .output()
        .expect("run shared support merger");
    assert!(
        output.status.success(),
        "shared support merger failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let shared = fs::read_to_string(shared_support).unwrap();
    assert!(
        shared.contains("impl From<FileInputStream> for InputStream"),
        "{shared}"
    );
}

#[test]
fn shared_support_merger_adds_input_stream_upcast_for_existing_jdk_facades() {
    let dir = unique_temp_dir("shared_support_existing_input_stream_upcast");
    let generated_dir = dir.join("generated");
    fs::create_dir_all(&generated_dir).unwrap();
    fs::write(
        generated_dir.join("00001_UseFileInput.rs"),
        r#"
pub struct UseFileInput;
"#,
    )
    .unwrap();

    let shared_support = dir.join("shared.rs");
    let shared_types = dir.join("shared-types.tsv");
    let conflicts = dir.join("conflicts.tsv");
    fs::write(
        &shared_support,
        r#"
#[derive(Clone, Debug, Default)]
pub struct InputStream;

#[derive(Clone, Debug, Default)]
pub struct FileInputStream;
"#,
    )
    .unwrap();
    fs::write(&shared_types, "InputStream\nFileInputStream\n").unwrap();
    let script =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/merge-shared-support-facades.py");
    let output = Command::new("python3")
        .arg(script)
        .arg("--generated-dir")
        .arg(&generated_dir)
        .arg("--shared-support")
        .arg(&shared_support)
        .arg("--shared-types")
        .arg(&shared_types)
        .arg("--conflicts")
        .arg(&conflicts)
        .output()
        .expect("run shared support merger");
    assert!(
        output.status.success(),
        "shared support merger failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let shared = fs::read_to_string(shared_support).unwrap();
    assert!(
        shared.contains("impl From<FileInputStream> for InputStream"),
        "{shared}"
    );
}

#[test]
fn shared_support_merger_removes_repeated_module_from_impls_to_existing_shared_type() {
    let dir = unique_temp_dir("shared_support_existing_target_from_impl_dedupe");
    let generated_dir = dir.join("generated");
    let raw_generated_dir = dir.join("raw-generated");
    fs::create_dir_all(&generated_dir).unwrap();
    fs::create_dir_all(&raw_generated_dir).unwrap();
    for (file, primary) in [("00001_UseA.rs", "UseA"), ("00002_UseB.rs", "UseB")] {
        let text = format!(
            r#"
#[derive(Clone, Debug, Default)]
pub struct BufferedInputStream;

impl From<BufferedInputStream> for InputStream {{
    fn from(_value: BufferedInputStream) -> Self {{ Default::default() }}
}}

pub struct {primary};
"#
        );
        fs::write(generated_dir.join(file), &text).unwrap();
        fs::write(raw_generated_dir.join(file), text).unwrap();
    }

    let shared_support = dir.join("shared.rs");
    let shared_types = dir.join("shared-types.tsv");
    let conflicts = dir.join("conflicts.tsv");
    fs::write(
        &shared_support,
        "#[derive(Clone, Debug, Default)]\npub struct InputStream;\n",
    )
    .unwrap();
    fs::write(&shared_types, "InputStream\n").unwrap();

    let script =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/merge-shared-support-facades.py");
    let output = Command::new("python3")
        .arg(script)
        .arg("--generated-dir")
        .arg(&generated_dir)
        .arg("--merge-input-dir")
        .arg(&raw_generated_dir)
        .arg("--shared-support")
        .arg(&shared_support)
        .arg("--shared-types")
        .arg(&shared_types)
        .arg("--conflicts")
        .arg(&conflicts)
        .output()
        .expect("run shared support merger");
    assert!(
        output.status.success(),
        "shared support merger failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let shared = fs::read_to_string(&shared_support).unwrap();
    assert_eq!(
        shared
            .matches("impl From<BufferedInputStream> for InputStream")
            .count(),
        1,
        "{shared}"
    );
    for file in ["00001_UseA.rs", "00002_UseB.rs"] {
        let module = fs::read_to_string(generated_dir.join(file)).unwrap();
        assert!(
            !module.contains("impl From<BufferedInputStream> for InputStream"),
            "{module}"
        );
    }
}

#[test]
fn shared_support_merger_does_not_let_local_alias_override_existing_shared_type() {
    let dir = unique_temp_dir("shared_support_alias_collision");
    let generated_dir = dir.join("generated");
    fs::create_dir_all(&generated_dir).unwrap();
    fs::write(
        generated_dir.join("00001_Base64.rs"),
        r#"
#[derive(Clone, Debug, Default)]
pub struct Base64OutputStream;

pub type OutputStream = Base64OutputStream;

impl Base64OutputStream {
    pub fn new_output_stream(_arg0: OutputStream) -> Self { Default::default() }
    pub fn source_owned_stateful_method(&mut self) {
        self.position = 0;
    }
}

pub struct Base64;
"#,
    )
    .unwrap();
    fs::write(
        generated_dir.join("00002_ZipUser.rs"),
        r#"
#[derive(Clone, Debug, Default)]
pub struct ZipOutputStream;

#[derive(Clone, Debug, Default)]
pub struct JavaIoInputStream;

pub struct ZipUser;
"#,
    )
    .unwrap();

    let shared_support = dir.join("shared.rs");
    let shared_types = dir.join("shared-types.tsv");
    let conflicts = dir.join("conflicts.tsv");
    fs::write(
        &shared_support,
        "#[derive(Clone, Debug, Default)]\npub struct Base64OutputStream;\n#[derive(Clone, Debug, Default)]\npub struct InputStream;\n#[derive(Clone, Debug, Default)]\npub struct OutputStream;\n#[derive(Clone, Debug, Default)]\npub struct ZipOutputStream;\npub type OutputStream = Base64OutputStream;\n",
    )
    .unwrap();
    fs::write(
        &shared_types,
        "Base64OutputStream\nInputStream\nJavaIoInputStream\nOutputStream\nZipOutputStream\n",
    )
    .unwrap();
    let script =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/merge-shared-support-facades.py");
    let output = Command::new("python3")
        .arg(script)
        .arg("--generated-dir")
        .arg(&generated_dir)
        .arg("--shared-support")
        .arg(&shared_support)
        .arg("--shared-types")
        .arg(&shared_types)
        .arg("--conflicts")
        .arg(&conflicts)
        .output()
        .expect("run shared support merger");
    assert!(
        output.status.success(),
        "shared support merger failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let shared = fs::read_to_string(shared_support).unwrap();
    let base64 = fs::read_to_string(generated_dir.join("00001_Base64.rs")).unwrap();
    assert!(shared.contains("pub struct OutputStream;"), "{shared}");
    assert!(
        shared.contains("impl From<ZipOutputStream> for OutputStream"),
        "{shared}"
    );
    assert!(
        shared.contains("impl From<JavaIoInputStream> for InputStream"),
        "{shared}"
    );
    assert!(
        !shared.contains("pub type OutputStream = Base64OutputStream;"),
        "{shared}"
    );
    assert!(!shared.contains("source_owned_stateful_method"), "{shared}");
    assert!(
        !base64.contains("pub type OutputStream = Base64OutputStream;"),
        "{base64}"
    );
    assert!(base64.contains("source_owned_stateful_method"), "{base64}");
}

#[test]
fn shared_support_merger_protects_declared_shared_type_without_type_index_row() {
    let dir = unique_temp_dir("shared_support_declared_type_collision");
    let generated_dir = dir.join("generated");
    fs::create_dir_all(&generated_dir).unwrap();
    fs::write(
        generated_dir.join("00001_Base64.rs"),
        r##"
#[derive(Clone, Debug)]
pub struct OutputStream;

#[derive(Clone, Debug)]
pub struct Base64OutputStream {
    pub position: i32,
}

impl Base64OutputStream {
    pub fn source_owned_stateful_method(&mut self) {
        self.position = 0;
    }
}

pub type OutputStream = Base64OutputStream;

pub struct Base64;
"##,
    )
    .unwrap();

    let shared_support = dir.join("shared.rs");
    let shared_types = dir.join("shared-types.tsv");
    let conflicts = dir.join("conflicts.tsv");
    fs::write(
        &shared_support,
        "#[derive(Clone, Debug, Default)]\npub struct OutputStream;\n",
    )
    .unwrap();
    fs::write(&shared_types, "").unwrap();
    let script =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/merge-shared-support-facades.py");
    let output = Command::new("python3")
        .arg(script)
        .arg("--generated-dir")
        .arg(&generated_dir)
        .arg("--shared-support")
        .arg(&shared_support)
        .arg("--shared-types")
        .arg(&shared_types)
        .arg("--conflicts")
        .arg(&conflicts)
        .output()
        .expect("run shared support merger");
    assert!(
        output.status.success(),
        "shared support merger failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let shared = fs::read_to_string(shared_support).unwrap();
    let base64 = fs::read_to_string(generated_dir.join("00001_Base64.rs")).unwrap();
    assert!(shared.contains("pub struct OutputStream;"), "{shared}");
    assert!(
        !shared.contains("pub type OutputStream = Base64OutputStream;"),
        "{shared}"
    );
    assert!(
        !base64.contains("pub type OutputStream = Base64OutputStream;"),
        "{base64}"
    );
    assert!(
        base64.contains("pub struct Base64OutputStream")
            && base64.contains("source_owned_stateful_method"),
        "{base64}"
    );
}

#[test]
fn shared_support_merger_promotes_from_impl_dependency_closure() {
    let dir = unique_temp_dir("shared_support_from_impl_closure");
    let generated_dir = dir.join("generated");
    fs::create_dir_all(&generated_dir).unwrap();
    fs::write(
        generated_dir.join("00001_UseA.rs"),
        r#"
#[derive(Clone, Debug, Default)]
pub struct ReadsFormatWriteOption;

#[derive(Clone, Debug, Default)]
pub struct WriteOption;

impl From<ReadsFormatWriteOption> for WriteOption {
    fn from(_value: ReadsFormatWriteOption) -> Self { Default::default() }
}

pub struct UseA;
"#,
    )
    .unwrap();
    fs::write(
        generated_dir.join("00002_UseB.rs"),
        r#"
#[derive(Clone, Debug, Default)]
pub struct ReadsFormatWriteOption;

pub struct UseB;
"#,
    )
    .unwrap();

    let shared_support = dir.join("shared.rs");
    let shared_types = dir.join("shared-types.tsv");
    let conflicts = dir.join("conflicts.tsv");
    let script =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/merge-shared-support-facades.py");
    let output = Command::new("python3")
        .arg(script)
        .arg("--generated-dir")
        .arg(&generated_dir)
        .arg("--shared-support")
        .arg(&shared_support)
        .arg("--shared-types")
        .arg(&shared_types)
        .arg("--conflicts")
        .arg(&conflicts)
        .output()
        .expect("run shared support merger");
    assert!(
        output.status.success(),
        "shared support merger failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let shared = fs::read_to_string(shared_support).unwrap();
    assert!(
        shared.contains("pub struct ReadsFormatWriteOption"),
        "{shared}"
    );
    assert!(shared.contains("pub struct WriteOption"), "{shared}");
    assert!(
        shared.contains("impl From<ReadsFormatWriteOption> for WriteOption"),
        "{shared}"
    );
    assert!(
        !fs::read_to_string(generated_dir.join("00001_UseA.rs"))
            .unwrap()
            .contains("impl From<ReadsFormatWriteOption> for WriteOption")
    );
}

#[test]
fn shared_support_merger_promotes_top_level_helper_dependency_closure() {
    let dir = unique_temp_dir("shared_support_top_level_helper_closure");
    let generated_dir = dir.join("generated");
    fs::create_dir_all(&generated_dir).unwrap();
    let facade_support = r#"
#[derive(Clone, Debug, Default)]
pub struct StringBuffer {
    value: String,
}

impl StringBuffer {
    pub fn set_length(&mut self, new_len: i32) {
        java_string_buffer_set_length(&mut self.value, new_len);
        java_string_buffer_multiline_helper(
            &self.value,
            new_len,
        );
    }
}

fn java_string_buffer_byte_index(value: &str, char_index: i32) -> usize {
    value.char_indices().nth(char_index as usize).map(|(index, _)| index).unwrap_or(value.len())
}

fn java_string_buffer_set_length(value: &mut String, new_len: i32) {
    let byte_index = java_string_buffer_byte_index(value, new_len);
    value.truncate(byte_index);
}

fn java_string_buffer_multiline_helper(
    value: &str,
    char_index: i32,
) -> usize {
    java_string_buffer_byte_index(value, char_index)
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Pattern {
    pattern: &'static str,
}

impl Pattern {
    pub fn matches(&self, input: &str) -> bool {
        java_pattern_matches(self.pattern, input)
    }
}

fn java_pattern_matches(pattern: &str, input: &str) -> bool {
    input == java_pattern_simple_regex_literal(pattern)
}

fn java_pattern_simple_regex_literal(regex: &str) -> &str {
    regex
}

fn java_existing_shared_helper() -> i32 {
    7
}

fn java_already_shared_helper() -> i32 {
    11
}
"#;
    fs::write(
        generated_dir.join("00001_UseA.rs"),
        format!("{facade_support}\npub struct UseA;\n"),
    )
    .unwrap();
    fs::write(
        generated_dir.join("00002_UseB.rs"),
        format!("{facade_support}\npub struct UseB;\n"),
    )
    .unwrap();

    let shared_support = dir.join("shared.rs");
    let shared_types = dir.join("shared-types.tsv");
    let conflicts = dir.join("conflicts.tsv");
    fs::write(
        &shared_support,
        r#"
pub struct ExistingSharedFacade;

impl ExistingSharedFacade {
    pub fn existing_value(&self) -> i32 {
        java_existing_shared_helper() + java_already_shared_helper()
    }
}

fn java_already_shared_helper() -> i32 {
    11
}
"#,
    )
    .unwrap();
    let script =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/merge-shared-support-facades.py");
    let output = Command::new("python3")
        .arg(script)
        .arg("--generated-dir")
        .arg(&generated_dir)
        .arg("--shared-support")
        .arg(&shared_support)
        .arg("--shared-types")
        .arg(&shared_types)
        .arg("--conflicts")
        .arg(&conflicts)
        .output()
        .expect("run shared support merger");
    assert!(
        output.status.success(),
        "shared support merger failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let shared = fs::read_to_string(&shared_support).unwrap();
    assert!(shared.contains("pub struct StringBuffer"), "{shared}");
    assert!(shared.contains("pub struct Pattern"), "{shared}");
    assert!(
        shared.contains("fn java_string_buffer_set_length"),
        "{shared}"
    );
    assert!(
        shared.contains("fn java_string_buffer_byte_index"),
        "{shared}"
    );
    assert!(
        shared.contains("fn java_string_buffer_multiline_helper"),
        "{shared}"
    );
    assert!(
        shared.contains("java_string_buffer_byte_index(value, char_index)"),
        "{shared}"
    );
    assert!(
        shared.contains("fn java_existing_shared_helper"),
        "{shared}"
    );
    assert_eq!(
        shared.matches("fn java_already_shared_helper").count(),
        1,
        "{shared}"
    );
    assert!(shared.contains("fn java_pattern_matches"), "{shared}");
    assert!(
        shared.contains("fn java_pattern_simple_regex_literal"),
        "{shared}"
    );

    for file in ["00001_UseA.rs", "00002_UseB.rs"] {
        let source = fs::read_to_string(generated_dir.join(file)).unwrap();
        assert!(!source.contains("pub struct StringBuffer"), "{source}");
        assert!(!source.contains("pub struct Pattern"), "{source}");
        assert!(
            !source.contains("fn java_string_buffer_set_length"),
            "{source}"
        );
        assert!(!source.contains("fn java_pattern_matches"), "{source}");
    }
}

#[test]
fn shared_support_merger_promotes_trait_dependency_for_shared_facade_impl() {
    let dir = unique_temp_dir("shared_support_trait_impl_dependency");
    let generated_dir = dir.join("generated");
    fs::create_dir_all(&generated_dir).unwrap();
    fs::write(
        generated_dir.join("00001_UseA.rs"),
        r#"
pub trait Localizable: std::fmt::Display {}

impl dyn Localizable {
    pub fn anonymous() -> Box<dyn Localizable> { panic!("anonymous") }
}

#[derive(Clone, Debug, Default)]
pub struct LocalizedFormats;

impl Localizable for LocalizedFormats {}

impl Eq for LocalizedFormats {}

pub trait JavaErasedObjectFacade {
    fn equals<T0>(&self, _arg0: T0) -> bool;
}

impl JavaErasedObjectFacade for Box<dyn std::any::Any> {
    fn equals<T0>(&self, _arg0: T0) -> bool { Default::default() }
}

pub struct UseA;
"#,
    )
    .unwrap();
    fs::write(
        generated_dir.join("00002_UseB.rs"),
        r#"
pub trait Localizable: std::fmt::Display {}

#[derive(Clone, Debug, Default)]
pub struct LocalizedFormats;

pub struct UseB;
"#,
    )
    .unwrap();

    let shared_support = dir.join("shared.rs");
    let shared_types = dir.join("shared-types.tsv");
    let conflicts = dir.join("conflicts.tsv");
    let script =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/merge-shared-support-facades.py");
    let output = Command::new("python3")
        .arg(script)
        .arg("--generated-dir")
        .arg(&generated_dir)
        .arg("--shared-support")
        .arg(&shared_support)
        .arg("--shared-types")
        .arg(&shared_types)
        .arg("--conflicts")
        .arg(&conflicts)
        .output()
        .expect("run shared support merger");
    assert!(
        output.status.success(),
        "shared support merger failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let shared = fs::read_to_string(&shared_support).unwrap();
    assert!(shared.contains("pub trait Localizable"), "{shared}");
    assert!(shared.contains("impl dyn Localizable"), "{shared}");
    assert!(
        shared.contains("pub fn anonymous() -> Box<dyn Localizable>"),
        "{shared}"
    );
    assert!(
        shared.contains("impl Localizable for LocalizedFormats"),
        "{shared}"
    );
    assert!(
        !shared.contains("pub trait JavaErasedObjectFacade"),
        "unrelated traits should not be promoted by trait-dependency closure:\n{shared}"
    );
    assert!(
        !shared.contains("pub trait Eq") && !shared.contains("pub trait PartialEq"),
        "standard traits should not be promoted as crate-local support traits:\n{shared}"
    );
    assert!(
        !fs::read_to_string(generated_dir.join("00001_UseA.rs"))
            .unwrap()
            .contains("impl Localizable for LocalizedFormats"),
        "promoted trait impl should be removed from source module"
    );
    assert!(
        !fs::read_to_string(generated_dir.join("00002_UseB.rs"))
            .unwrap()
            .contains("pub trait Localizable"),
        "promoted trait declaration should be removed from sibling module"
    );
    assert!(
        !fs::read_to_string(generated_dir.join("00001_UseA.rs"))
            .unwrap()
            .contains("impl dyn Localizable"),
        "promoted dyn-trait inherent impl should be removed from source module"
    );
}

#[test]
fn shared_support_merger_removes_orphan_trait_impl_for_missing_external_target() {
    let dir = unique_temp_dir("shared_support_orphan_trait_impl");
    let generated_dir = dir.join("generated");
    fs::create_dir_all(&generated_dir).unwrap();
    fs::write(
        generated_dir.join("00001_UseA.rs"),
        r#"
pub trait Localizable: std::fmt::Display {}
impl Localizable for LocalizedFormats {}
pub struct UseA;
"#,
    )
    .unwrap();
    fs::write(
        generated_dir.join("00002_UseB.rs"),
        r#"
pub trait Localizable: std::fmt::Display {}
impl Localizable for LocalizedFormats {}
pub struct UseB;
"#,
    )
    .unwrap();

    let shared_support = dir.join("shared.rs");
    let shared_types = dir.join("shared-types.tsv");
    let conflicts = dir.join("conflicts.tsv");
    let script =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/merge-shared-support-facades.py");
    let output = Command::new("python3")
        .arg(script)
        .arg("--generated-dir")
        .arg(&generated_dir)
        .arg("--shared-support")
        .arg(&shared_support)
        .arg("--shared-types")
        .arg(&shared_types)
        .arg("--conflicts")
        .arg(&conflicts)
        .output()
        .expect("run shared support merger");
    assert!(
        output.status.success(),
        "shared support merger failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let shared = fs::read_to_string(shared_support).unwrap();
    assert!(shared.contains("pub trait Localizable"), "{shared}");
    assert!(
        !shared.contains("impl Localizable for LocalizedFormats"),
        "orphan impl target should not be promoted without a LocalizedFormats declaration:\n{shared}"
    );
    for file in ["00001_UseA.rs", "00002_UseB.rs"] {
        let source = fs::read_to_string(generated_dir.join(file)).unwrap();
        assert!(
            !source.contains("impl Localizable for LocalizedFormats"),
            "orphan impl should be removed from {file}:\n{source}"
        );
        assert!(
            !source.contains("pub trait Localizable"),
            "promoted trait should be removed from {file}:\n{source}"
        );
    }
}

#[test]
fn shared_support_merger_merges_erased_object_facade_trait_and_complex_impl() {
    let dir = unique_temp_dir("shared_support_erased_object_facade_impl");
    let generated_dir = dir.join("generated");
    fs::create_dir_all(&generated_dir).unwrap();
    fs::write(
        generated_dir.join("00001_UseA.rs"),
        r#"
pub trait JavaErasedObjectFacade {
    fn close(&self) -> Result<(), JavaException>;
}

impl JavaErasedObjectFacade for Box<dyn std::any::Any> {
    fn close(&self) -> Result<(), JavaException> { Ok(()) }
}

pub struct UseA;
"#,
    )
    .unwrap();
    fs::write(
        generated_dir.join("00002_UseB.rs"),
        r#"
pub trait JavaErasedObjectFacade {
    fn length(&self) -> ();
}

impl JavaErasedObjectFacade for Box<dyn std::any::Any> {
    fn length(&self) -> () { () }
}

pub struct UseB;
"#,
    )
    .unwrap();

    let shared_support = dir.join("shared.rs");
    let shared_types = dir.join("shared-types.tsv");
    let conflicts = dir.join("conflicts.tsv");
    let script =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/merge-shared-support-facades.py");
    let output = Command::new("python3")
        .arg(script)
        .arg("--generated-dir")
        .arg(&generated_dir)
        .arg("--shared-support")
        .arg(&shared_support)
        .arg("--shared-types")
        .arg(&shared_types)
        .arg("--conflicts")
        .arg(&conflicts)
        .output()
        .expect("run shared support merger");
    assert!(
        output.status.success(),
        "shared support merger failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let shared = fs::read_to_string(&shared_support).unwrap();
    assert!(
        shared.contains("pub trait JavaErasedObjectFacade"),
        "{shared}"
    );
    assert!(
        shared.contains("fn close(&self) -> Result<(), JavaException>;"),
        "{shared}"
    );
    assert!(shared.contains("fn length(&self) -> ();"), "{shared}");
    assert!(
        shared.contains("impl JavaErasedObjectFacade for Box<dyn std::any::Any>"),
        "{shared}"
    );
    assert!(
        shared.contains("fn close(&self) -> Result<(), JavaException> { Ok(()) }"),
        "{shared}"
    );
    assert!(shared.contains("fn length(&self) -> () { () }"), "{shared}");
    for file in ["00001_UseA.rs", "00002_UseB.rs"] {
        let source = fs::read_to_string(generated_dir.join(file)).unwrap();
        assert!(
            !source.contains("impl JavaErasedObjectFacade for Box<dyn std::any::Any>"),
            "{source}"
        );
        assert!(
            !source.contains("pub trait JavaErasedObjectFacade"),
            "{source}"
        );
    }
}

#[test]
fn shared_support_merger_uses_raw_input_for_erased_object_facade_impl() {
    let dir = unique_temp_dir("shared_support_existing_erased_object_facade_impl");
    let generated_dir = dir.join("generated");
    let raw_generated_dir = dir.join("raw-generated");
    fs::create_dir_all(&generated_dir).unwrap();
    fs::create_dir_all(&raw_generated_dir).unwrap();
    fs::write(
        raw_generated_dir.join("00001_UseA.rs"),
        r#"
pub trait JavaErasedObjectFacade {
    fn clear(&self) -> ();
    fn read_3<T0, T1, T2>(&self, _arg0: T0, _arg1: T1, _arg2: T2) -> Result<i32, JavaException>;
}

impl JavaErasedObjectFacade for Box<dyn std::any::Any> {
    fn clear(&self) -> () { () }
    fn read_3<T0, T1, T2>(&self, _arg0: T0, _arg1: T1, _arg2: T2) -> Result<i32, JavaException> { Ok(Default::default()) }
}

pub struct UseA;
"#,
    )
    .unwrap();
    fs::write(
        raw_generated_dir.join("00002_UseB.rs"),
        r#"
pub trait JavaErasedObjectFacade {
    fn remove_all<T0>(&self, _arg0: T0) -> ();
    fn write_header<T0>(&self, _arg0: T0) -> ();
}

impl JavaErasedObjectFacade for Box<dyn std::any::Any> {
    fn remove_all<T0>(&self, _arg0: T0) -> () { () }
    fn write_header<T0>(&self, _arg0: T0) -> () { () }
}

pub struct UseB;
"#,
    )
    .unwrap();
    fs::write(
        generated_dir.join("00001_UseA.rs"),
        r#"
pub trait JavaErasedObjectFacade {
    fn clear(&self) -> ();
}

pub struct UseA;
"#,
    )
    .unwrap();
    fs::write(
        generated_dir.join("00002_UseB.rs"),
        r#"
pub trait JavaErasedObjectFacade {
    fn remove_all<T0>(&self, _arg0: T0) -> ();
}

pub struct UseB;
"#,
    )
    .unwrap();

    let shared_support = dir.join("shared.rs");
    let shared_types = dir.join("shared-types.tsv");
    let conflicts = dir.join("conflicts.tsv");
    fs::write(
        &shared_support,
        r#"
#[derive(Clone, Debug, Default)]
pub struct ExistingSharedSupport;
"#,
    )
    .unwrap();
    fs::write(&shared_types, "ExistingSharedSupport\n").unwrap();

    let script =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/merge-shared-support-facades.py");
    let output = Command::new("python3")
        .arg(script)
        .arg("--generated-dir")
        .arg(&generated_dir)
        .arg("--merge-input-dir")
        .arg(&raw_generated_dir)
        .arg("--shared-support")
        .arg(&shared_support)
        .arg("--shared-types")
        .arg(&shared_types)
        .arg("--conflicts")
        .arg(&conflicts)
        .output()
        .expect("run shared support merger");
    assert!(
        output.status.success(),
        "shared support merger failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let shared = fs::read_to_string(&shared_support).unwrap();
    assert_eq!(
        shared.matches("pub trait JavaErasedObjectFacade").count(),
        1,
        "{shared}"
    );
    assert_eq!(
        shared
            .matches("impl JavaErasedObjectFacade for Box<dyn std::any::Any>")
            .count(),
        1,
        "{shared}"
    );
    for required in [
        "fn clear(&self) -> ();",
        "fn read_3<T0, T1, T2>(&self, _arg0: T0, _arg1: T1, _arg2: T2) -> Result<i32, JavaException>;",
        "fn remove_all<T0>(&self, _arg0: T0) -> ();",
        "fn write_header<T0>(&self, _arg0: T0) -> ();",
        "fn clear(&self) -> () { () }",
        "fn read_3<T0, T1, T2>(&self, _arg0: T0, _arg1: T1, _arg2: T2) -> Result<i32, JavaException> { Ok(Default::default()) }",
        "fn remove_all<T0>(&self, _arg0: T0) -> () { () }",
        "fn write_header<T0>(&self, _arg0: T0) -> () { () }",
    ] {
        assert!(
            shared.contains(required),
            "missing {required} in:\n{shared}"
        );
    }
}

#[test]
fn shared_support_merger_prefers_static_to_string_variant_when_static_call_is_used() {
    let dir = unique_temp_dir("shared_support_static_to_string_preference");
    let generated_dir = dir.join("generated");
    fs::create_dir_all(&generated_dir).unwrap();
    fs::write(
        generated_dir.join("00001_UseA.rs"),
        r#"
#[derive(Clone, Debug, Default)]
pub struct Integer;

impl Integer {
    pub fn to_string(&self) -> String { Default::default() }
}

pub struct UseA;
"#,
    )
    .unwrap();
    fs::write(
        generated_dir.join("00002_UseB.rs"),
        r#"
#[derive(Clone, Debug, Default)]
pub struct Integer;

impl Integer {
    pub fn to_string(_arg0: impl std::fmt::Display) -> String { _arg0.to_string() }
}

pub struct UseB;

impl UseB {
    pub fn width(&self, value: i32) -> i32 {
        Integer::to_string(value).len() as i32
    }
}
"#,
    )
    .unwrap();

    let shared_support = dir.join("shared.rs");
    let shared_types = dir.join("shared-types.tsv");
    let conflicts = dir.join("conflicts.tsv");
    let script =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/merge-shared-support-facades.py");
    let output = Command::new("python3")
        .arg(script)
        .arg("--generated-dir")
        .arg(&generated_dir)
        .arg("--shared-support")
        .arg(&shared_support)
        .arg("--shared-types")
        .arg(&shared_types)
        .arg("--conflicts")
        .arg(&conflicts)
        .output()
        .expect("run shared support merger");
    assert!(
        output.status.success(),
        "shared support merger failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let shared = fs::read_to_string(&shared_support).unwrap();
    assert!(
        shared.contains(
            "pub fn to_string(_arg0: impl std::fmt::Display) -> String { _arg0.to_string() }"
        ),
        "{shared}"
    );
    assert!(
        !shared.contains("pub fn to_string(&self) -> String { Default::default() }"),
        "{shared}"
    );
}

#[test]
fn shared_support_merger_ignores_braces_inside_format_strings() {
    let dir = unique_temp_dir("shared_support_format_braces");
    let generated_dir = dir.join("generated");
    fs::create_dir_all(&generated_dir).unwrap();
    let source = r#"
pub struct SharedFacade {
    value: i32,
}

impl SharedFacade {
    pub fn label(&self) -> String {
        return format!("SharedFacade{{value={}{}", (self.value).to_string(), ('}').to_string());
    }
}

pub struct UseShared;

impl UseShared {
    pub fn still_here(&self) -> bool {
        return true;
    }
}
"#;
    fs::write(generated_dir.join("00001_UseA.rs"), source).unwrap();
    fs::write(
        generated_dir.join("00002_UseB.rs"),
        source.replace("UseShared", "UseB"),
    )
    .unwrap();

    let shared_support = dir.join("shared.rs");
    let shared_types = dir.join("shared-types.tsv");
    let conflicts = dir.join("conflicts.tsv");
    let script =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/merge-shared-support-facades.py");
    let output = Command::new("python3")
        .arg(script)
        .arg("--generated-dir")
        .arg(&generated_dir)
        .arg("--shared-support")
        .arg(&shared_support)
        .arg("--shared-types")
        .arg(&shared_types)
        .arg("--conflicts")
        .arg(&conflicts)
        .output()
        .expect("run shared support merger");
    assert!(
        output.status.success(),
        "shared support merger failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let shared = fs::read_to_string(shared_support).unwrap();
    assert!(shared.contains("pub fn label(&self) -> String"), "{shared}");
    assert!(
        !shared.contains("pub fn still_here(&self) -> bool"),
        "{shared}"
    );
    assert!(
        fs::read_to_string(generated_dir.join("00001_UseA.rs"))
            .unwrap()
            .contains("pub fn still_here(&self) -> bool")
    );
}

#[test]
fn shared_support_merger_preserves_display_impl_with_formatter_lifetime() {
    let dir = unique_temp_dir("shared_support_display_lifetime");
    let generated_dir = dir.join("generated");
    fs::create_dir_all(&generated_dir).unwrap();
    let source = r#"
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SharedKind {
    INFO,
    FORMAT,
}

impl std::fmt::Display for SharedKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl Eq for SharedKind {}

impl PartialEq for SharedKind {
    fn eq(&self, _other: &Self) -> bool { true }
}

impl std::hash::Hash for SharedKind {
    fn hash<H: std::hash::Hasher>(&self, _state: &mut H) {}
}

pub struct UseShared;

impl UseShared {
    pub fn kind(&self) -> SharedKind {
        return SharedKind::INFO;
    }
}
"#;
    fs::write(generated_dir.join("00001_UseA.rs"), source).unwrap();
    fs::write(
        generated_dir.join("00002_UseB.rs"),
        source.replace("UseShared", "UseB"),
    )
    .unwrap();

    let shared_support = dir.join("shared.rs");
    let shared_types = dir.join("shared-types.tsv");
    let conflicts = dir.join("conflicts.tsv");
    let script =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/merge-shared-support-facades.py");
    let output = Command::new("python3")
        .arg(script)
        .arg("--generated-dir")
        .arg(&generated_dir)
        .arg("--shared-support")
        .arg(&shared_support)
        .arg("--shared-types")
        .arg(&shared_types)
        .arg("--conflicts")
        .arg(&conflicts)
        .output()
        .expect("run shared support merger");
    assert!(
        output.status.success(),
        "shared support merger failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let shared = fs::read_to_string(shared_support).unwrap();
    assert!(
        shared.contains("impl std::fmt::Display for SharedKind"),
        "{shared}"
    );
    assert!(
        shared.contains("    }\n}"),
        "Display impl should retain both function and impl closing braces:\n{shared}"
    );
    assert!(
        !fs::read_to_string(generated_dir.join("00001_UseA.rs"))
            .unwrap()
            .contains("impl std::fmt::Display for SharedKind")
    );
    assert!(!shared.contains("impl Eq for SharedKind"), "{shared}");
    assert!(
        !shared.contains("impl PartialEq for SharedKind"),
        "{shared}"
    );
    assert!(
        !shared.contains("impl std::hash::Hash for SharedKind"),
        "{shared}"
    );
}

#[test]
fn shared_support_merger_does_not_promote_primary_output_types() {
    let dir = unique_temp_dir("shared_support_primary_outputs");
    let generated_dir = dir.join("generated");
    fs::create_dir_all(&generated_dir).unwrap();
    fs::write(
        generated_dir.join("00001_PrimaryA.rs"),
        r#"
pub struct PrimaryA {
    value: i32,
}

impl PrimaryA {
    pub fn value(&self) -> i32 {
        return self.value;
    }
}
"#,
    )
    .unwrap();
    let context_then_source = |source_type: &str| {
        format!(
            r#"
pub struct PrimaryA {{
    value: i32,
}}

impl PrimaryA {{
    pub fn value(&self) -> i32 {{
        return self.value;
    }}
}}

pub struct {source_type};
"#
        )
    };
    fs::write(
        generated_dir.join("00002_UseB.rs"),
        context_then_source("UseB"),
    )
    .unwrap();
    fs::write(
        generated_dir.join("00003_UseC.rs"),
        context_then_source("UseC"),
    )
    .unwrap();

    let shared_support = dir.join("shared.rs");
    let shared_types = dir.join("shared-types.tsv");
    let conflicts = dir.join("conflicts.tsv");
    let script =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/merge-shared-support-facades.py");
    let output = Command::new("python3")
        .arg(script)
        .arg("--generated-dir")
        .arg(&generated_dir)
        .arg("--shared-support")
        .arg(&shared_support)
        .arg("--shared-types")
        .arg(&shared_types)
        .arg("--conflicts")
        .arg(&conflicts)
        .output()
        .expect("run shared support merger");
    assert!(
        output.status.success(),
        "shared support merger failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let shared = fs::read_to_string(shared_support).unwrap_or_default();
    assert!(
        !shared.contains("pub struct PrimaryA"),
        "primary output type should stay in its source module:\n{shared}"
    );
    assert!(
        fs::read_to_string(generated_dir.join("00001_PrimaryA.rs"))
            .unwrap()
            .contains("pub struct PrimaryA")
    );
}

#[test]
fn shared_support_merger_boxes_direct_self_recursive_fields() {
    let dir = unique_temp_dir("shared_support_recursive_fields");
    let generated_dir = dir.join("generated");
    fs::create_dir_all(&generated_dir).unwrap();
    let source = r#"
#[derive(Clone, Debug)]
pub struct TreeNode {
    pub value: i32,
    pub left: TreeNode,
    pub right: TreeNode,
}

impl Default for TreeNode {
    fn default() -> Self {
        panic!("external facade placeholder TreeNode has no default value")
    }
}

pub struct UseShared;
"#;
    fs::write(generated_dir.join("00001_UseA.rs"), source).unwrap();
    fs::write(
        generated_dir.join("00002_UseB.rs"),
        source.replace("UseShared", "UseB"),
    )
    .unwrap();

    let shared_support = dir.join("shared.rs");
    let shared_types = dir.join("shared-types.tsv");
    let conflicts = dir.join("conflicts.tsv");
    let script =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/merge-shared-support-facades.py");
    let output = Command::new("python3")
        .arg(script)
        .arg("--generated-dir")
        .arg(&generated_dir)
        .arg("--shared-support")
        .arg(&shared_support)
        .arg("--shared-types")
        .arg(&shared_types)
        .arg("--conflicts")
        .arg(&conflicts)
        .output()
        .expect("run shared support merger");
    assert!(
        output.status.success(),
        "shared support merger failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let shared = fs::read_to_string(shared_support).unwrap();
    assert!(shared.contains("pub left: Box<TreeNode>,"), "{shared}");
    assert!(shared.contains("pub right: Box<TreeNode>,"), "{shared}");
    assert!(!shared.contains("pub left: TreeNode,"), "{shared}");
}

#[test]
fn shared_support_merger_removes_module_declarations_for_existing_shared_aliases() {
    let dir = unique_temp_dir("shared_support_existing_alias");
    let generated_dir = dir.join("generated");
    fs::create_dir_all(&generated_dir).unwrap();
    fs::write(
        generated_dir.join("00001_UseAlias.rs"),
        r#"
#[derive(Clone, Debug, Default)]
pub struct SharedAlias;

impl SharedAlias {
    pub fn local_only(&self) -> i32 { 1 }
}

#[derive(Clone, Debug, Default)]
pub struct KeptSupport;

impl KeptSupport {
    pub fn keep(&self) -> i32 { 2 }
}

pub struct UseAlias;
"#,
    )
    .unwrap();

    let shared_support = dir.join("shared.rs");
    let shared_types = dir.join("shared-types.tsv");
    let conflicts = dir.join("conflicts.tsv");
    fs::write(
        &shared_support,
        "pub struct SharedTarget;\npub type SharedAlias = SharedTarget;\n",
    )
    .unwrap();
    fs::write(&shared_types, "SharedTarget\nSharedAlias\n").unwrap();

    let script =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/merge-shared-support-facades.py");
    let output = Command::new("python3")
        .arg(script)
        .arg("--generated-dir")
        .arg(&generated_dir)
        .arg("--shared-support")
        .arg(&shared_support)
        .arg("--shared-types")
        .arg(&shared_types)
        .arg("--conflicts")
        .arg(&conflicts)
        .output()
        .expect("run shared support merger");
    assert!(
        output.status.success(),
        "shared support merger failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let module = fs::read_to_string(generated_dir.join("00001_UseAlias.rs")).unwrap();
    assert!(!module.contains("pub struct SharedAlias"), "{module}");
    assert!(!module.contains("impl SharedAlias"), "{module}");
    assert!(module.contains("pub struct KeptSupport"), "{module}");
    assert!(module.contains("impl KeptSupport"), "{module}");

    let shared = fs::read_to_string(shared_support).unwrap();
    assert_eq!(
        shared
            .matches("pub type SharedAlias = SharedTarget;")
            .count(),
        1
    );
    assert!(!shared.contains("pub struct SharedAlias"), "{shared}");
}

#[test]
fn shared_support_merger_folds_promoted_alias_impls_into_target() {
    let dir = unique_temp_dir("shared_support_alias_impls");
    let generated_dir = dir.join("generated");
    fs::create_dir_all(&generated_dir).unwrap();
    fs::write(
        generated_dir.join("00001_First.rs"),
        r#"
#[derive(Clone, Debug, Default)]
pub struct SharedTarget;

impl SharedTarget {
    pub fn first(&self) -> i32 { 1 }
}

pub type SharedAlias = SharedTarget;

pub struct First;
"#,
    )
    .unwrap();
    fs::write(
        generated_dir.join("00002_Second.rs"),
        r#"
#[derive(Clone, Debug, Default)]
pub struct SharedTarget;

pub type SharedAlias = SharedTarget;

impl SharedAlias {
    pub const LABEL: &'static str = "alias";
    pub fn first(&self) -> i32 { 10 }
    pub fn second(&self) -> i32 { 2 }
}

pub struct Second;
"#,
    )
    .unwrap();

    let shared_support = dir.join("shared.rs");
    let shared_types = dir.join("shared-types.tsv");
    let conflicts = dir.join("conflicts.tsv");
    fs::write(&shared_support, "").unwrap();
    fs::write(&shared_types, "").unwrap();

    let script =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/merge-shared-support-facades.py");
    let output = Command::new("python3")
        .arg(script)
        .arg("--generated-dir")
        .arg(&generated_dir)
        .arg("--shared-support")
        .arg(&shared_support)
        .arg("--shared-types")
        .arg(&shared_types)
        .arg("--conflicts")
        .arg(&conflicts)
        .output()
        .expect("run shared support merger");
    assert!(
        output.status.success(),
        "shared support merger failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let shared = fs::read_to_string(shared_support).unwrap();
    assert!(shared.contains("pub struct SharedTarget"), "{shared}");
    assert!(
        shared.contains("pub type SharedAlias = SharedTarget;"),
        "{shared}"
    );
    assert!(shared.contains("impl SharedTarget"), "{shared}");
    assert_eq!(
        shared.matches("pub fn first(&self) -> i32").count(),
        1,
        "{shared}"
    );
    assert!(shared.contains("pub const LABEL: &'static str"), "{shared}");
    assert!(shared.contains("pub fn second(&self) -> i32"), "{shared}");
    assert!(!shared.contains("impl SharedAlias"), "{shared}");

    let first = fs::read_to_string(generated_dir.join("00001_First.rs")).unwrap();
    let second = fs::read_to_string(generated_dir.join("00002_Second.rs")).unwrap();
    assert!(!first.contains("pub struct SharedTarget"), "{first}");
    assert!(!first.contains("impl SharedTarget"), "{first}");
    assert!(!second.contains("pub struct SharedTarget"), "{second}");
    assert!(!second.contains("impl SharedAlias"), "{second}");
}

#[test]
fn shared_support_merger_removes_prelude_stub_redeclared_after_primary_type() {
    let dir = unique_temp_dir("shared_support_local_redeclared_stub");
    let generated_dir = dir.join("generated");
    fs::create_dir_all(&generated_dir).unwrap();
    fs::write(
        generated_dir.join("00001_Primary.rs"),
        r#"
#[derive(Clone, Debug, Default)]
pub struct Inner;

#[derive(Clone, Debug, Default)]
pub struct OtherPrelude;

pub struct Primary;

pub struct Inner {
    value: i32,
}
"#,
    )
    .unwrap();

    let shared_support = dir.join("shared.rs");
    let shared_types = dir.join("shared-types.tsv");
    let conflicts = dir.join("conflicts.tsv");
    fs::write(&shared_support, "").unwrap();
    fs::write(&shared_types, "").unwrap();

    let script =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/merge-shared-support-facades.py");
    let output = Command::new("python3")
        .arg(script)
        .arg("--generated-dir")
        .arg(&generated_dir)
        .arg("--shared-support")
        .arg(&shared_support)
        .arg("--shared-types")
        .arg(&shared_types)
        .arg("--conflicts")
        .arg(&conflicts)
        .output()
        .expect("run shared support merger");
    assert!(
        output.status.success(),
        "shared support merger failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let module = fs::read_to_string(generated_dir.join("00001_Primary.rs")).unwrap();
    assert_eq!(module.matches("pub struct Inner").count(), 1, "{module}");
    assert!(module.contains("pub struct OtherPrelude"), "{module}");
    assert!(module.contains("value: i32"), "{module}");
}

#[test]
fn shared_support_merger_keeps_source_owned_nested_types_before_primary() {
    let dir = unique_temp_dir("shared_support_source_owned_nested");
    let generated_dir = dir.join("generated");
    fs::create_dir_all(&generated_dir).unwrap();
    fs::write(
        generated_dir.join("00001_Outer.rs"),
        r#"
pub struct SharedFacade;

impl SharedFacade {
    pub fn shared(&self) -> i32 { 1 }
}

#[derive(Clone, Debug, Default)]
pub struct OuterNested;

impl OuterNested {
    pub fn new<T0>(_arg0: T0) -> Self { Default::default() }
    pub fn weak(&self) -> i32 { 0 }
}

pub struct OuterNested {
    value: i32,
}

impl OuterNested {
    pub fn new(value: i32) -> OuterNested { OuterNested { value } }
    pub fn value(&self) -> i32 { self.value }
}

impl From<OuterNested> for SharedFacade {
    fn from(_value: OuterNested) -> Self { SharedFacade }
}

pub struct Outer;
"#,
    )
    .unwrap();
    fs::write(
        generated_dir.join("00002_UseShared.rs"),
        r#"
pub struct SharedFacade;

impl SharedFacade {
    pub fn shared(&self) -> i32 { 1 }
}

pub struct UseShared;
"#,
    )
    .unwrap();

    let shared_support = dir.join("shared.rs");
    let shared_types = dir.join("shared-types.tsv");
    let conflicts = dir.join("conflicts.tsv");
    fs::write(&shared_support, "").unwrap();
    fs::write(&shared_types, "").unwrap();

    let script =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/merge-shared-support-facades.py");
    let output = Command::new("python3")
        .arg(script)
        .arg("--generated-dir")
        .arg(&generated_dir)
        .arg("--shared-support")
        .arg(&shared_support)
        .arg("--shared-types")
        .arg(&shared_types)
        .arg("--conflicts")
        .arg(&conflicts)
        .output()
        .expect("run shared support merger");
    assert!(
        output.status.success(),
        "shared support merger failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let shared = fs::read_to_string(shared_support).unwrap();
    assert!(shared.contains("pub struct SharedFacade"), "{shared}");
    assert!(!shared.contains("pub struct OuterNested"), "{shared}");
    assert!(!shared.contains("impl From<OuterNested>"), "{shared}");

    let module = fs::read_to_string(generated_dir.join("00001_Outer.rs")).unwrap();
    assert_eq!(
        module.matches("pub struct OuterNested").count(),
        1,
        "{module}"
    );
    assert!(module.contains("impl OuterNested"), "{module}");
    assert!(
        module.contains("impl From<OuterNested> for SharedFacade"),
        "{module}"
    );
    assert!(module.contains("pub fn new(value: i32)"), "{module}");
    assert!(!module.contains("pub fn weak"), "{module}");
    assert!(!module.contains("pub struct SharedFacade"), "{module}");
}

#[test]
fn shared_support_merger_preserves_generic_inherent_impl_methods() {
    let dir = unique_temp_dir("shared_support_generic_impl_methods");
    let generated_dir = dir.join("generated");
    fs::create_dir_all(&generated_dir).unwrap();
    let source = r#"
#[derive(Clone, Debug, Default)]
pub struct SharedBox<T> {
    value: Option<T>,
}

impl<T> SharedBox<T> {
    pub fn new(value: T) -> SharedBox<T> {
        SharedBox { value: Some(value) }
    }

    pub fn take(&mut self) -> Option<T> {
        self.value.take()
    }
}

pub struct UseShared;
"#;
    fs::write(generated_dir.join("00001_UseA.rs"), source).unwrap();
    fs::write(
        generated_dir.join("00002_UseB.rs"),
        source.replace("UseShared", "UseB"),
    )
    .unwrap();

    let shared_support = dir.join("shared.rs");
    let shared_types = dir.join("shared-types.tsv");
    let conflicts = dir.join("conflicts.tsv");
    fs::write(&shared_support, "").unwrap();
    fs::write(&shared_types, "").unwrap();

    let script =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/merge-shared-support-facades.py");
    let output = Command::new("python3")
        .arg(script)
        .arg("--generated-dir")
        .arg(&generated_dir)
        .arg("--shared-support")
        .arg(&shared_support)
        .arg("--shared-types")
        .arg(&shared_types)
        .arg("--conflicts")
        .arg(&conflicts)
        .output()
        .expect("run shared support merger");
    assert!(
        output.status.success(),
        "shared support merger failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let shared = fs::read_to_string(shared_support).unwrap();
    assert!(shared.contains("pub struct SharedBox<T>"), "{shared}");
    assert!(shared.contains("impl<T> SharedBox<T> {"), "{shared}");
    assert!(
        shared.contains("pub fn new(value: T) -> SharedBox<T>"),
        "{shared}"
    );
    assert!(
        shared.contains("pub fn take(&mut self) -> Option<T>"),
        "{shared}"
    );
}

#[test]
fn shared_support_merger_prefers_concrete_method_over_generic_fallback() {
    let dir = unique_temp_dir("shared_support_concrete_method_preference");
    let generated_dir = dir.join("generated");
    fs::create_dir_all(&generated_dir).unwrap();
    fs::write(
        generated_dir.join("00001_UseGeneric.rs"),
        r#"
pub struct ExternalUtils;

impl ExternalUtils {
    pub fn read_entire_file<R: Default, T0>(_arg0: T0) -> R { Default::default() }
}

pub struct UseGeneric;
"#,
    )
    .unwrap();
    fs::write(
        generated_dir.join("00002_UseConcrete.rs"),
        r#"
pub struct ExternalUtils;

impl ExternalUtils {
    pub fn read_entire_file(_arg0: &str) -> String { Default::default() }
}

pub struct UseConcrete;
"#,
    )
    .unwrap();

    let shared_support = dir.join("shared.rs");
    let shared_types = dir.join("shared-types.tsv");
    let conflicts = dir.join("conflicts.tsv");
    fs::write(&shared_support, "").unwrap();
    fs::write(&shared_types, "").unwrap();

    let script =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/merge-shared-support-facades.py");
    let output = Command::new("python3")
        .arg(script)
        .arg("--generated-dir")
        .arg(&generated_dir)
        .arg("--shared-support")
        .arg(&shared_support)
        .arg("--shared-types")
        .arg(&shared_types)
        .arg("--conflicts")
        .arg(&conflicts)
        .output()
        .expect("run shared support merger");
    assert!(
        output.status.success(),
        "shared support merger failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let shared = fs::read_to_string(shared_support).unwrap();
    assert!(
        shared.contains("pub fn read_entire_file(_arg0: &str) -> String"),
        "{shared}"
    );
    assert!(
        !shared.contains("pub fn read_entire_file<R: Default"),
        "{shared}"
    );
}

#[test]
fn shared_support_merger_prefers_observed_static_factory_receiver_shape() {
    let dir = unique_temp_dir("shared_support_static_factory_receiver_shape");
    let generated_dir = dir.join("generated");
    fs::create_dir_all(&generated_dir).unwrap();
    fs::write(
        generated_dir.join("00001_StaticUse.rs"),
        r#"
pub struct VidMappingPB;
pub struct VidMappingPBBuilder;

impl VidMappingPB {
    pub fn new_builder() -> VidMappingPBBuilder { Default::default() }
}

pub struct StaticUse;

impl StaticUse {
    pub fn make(&self) -> VidMappingPBBuilder {
        return VidMappingPB::new_builder();
    }
}
"#,
    )
    .unwrap();
    fs::write(
        generated_dir.join("00002_InstanceStub.rs"),
        r#"
pub struct VidMappingPB;
pub struct VidMappingPBBuilder;

impl VidMappingPB {
    pub fn new_builder(&self) -> VidMappingPBBuilder { Default::default() }
}

pub struct InstanceStub;
"#,
    )
    .unwrap();

    let shared_support = dir.join("shared.rs");
    let shared_types = dir.join("shared-types.tsv");
    let conflicts = dir.join("conflicts.tsv");
    fs::write(&shared_support, "").unwrap();
    fs::write(&shared_types, "").unwrap();

    let script =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/merge-shared-support-facades.py");
    let output = Command::new("python3")
        .arg(script)
        .arg("--generated-dir")
        .arg(&generated_dir)
        .arg("--shared-support")
        .arg(&shared_support)
        .arg("--shared-types")
        .arg(&shared_types)
        .arg("--conflicts")
        .arg(&conflicts)
        .output()
        .expect("run shared support merger");
    assert!(
        output.status.success(),
        "shared support merger failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let shared = fs::read_to_string(shared_support).unwrap();
    assert!(
        shared.contains("pub fn new_builder() -> VidMappingPBBuilder"),
        "{shared}"
    );
    assert!(!shared.contains("pub fn new_builder(&self)"), "{shared}");
    let module = fs::read_to_string(generated_dir.join("00001_StaticUse.rs")).unwrap();
    assert!(
        module.contains("return VidMappingPB::new_builder();"),
        "{module}"
    );
}

#[test]
fn shared_support_merger_prefers_observed_instance_receiver_shape() {
    let dir = unique_temp_dir("shared_support_instance_receiver_shape");
    let generated_dir = dir.join("generated");
    fs::create_dir_all(&generated_dir).unwrap();
    fs::write(
        generated_dir.join("00001_InstanceUse.rs"),
        r#"
pub struct ProgressMeter;

impl ProgressMeter {
    pub fn set_records_between_time_checks<T0>(&self, _arg0: T0) -> Self { self.clone() }
}

pub struct InstanceUse {
    progress_meter: ProgressMeter,
}

impl InstanceUse {
    pub fn configure(&self) {
        self.progress_meter.set_records_between_time_checks(1i64);
    }
}
"#,
    )
    .unwrap();
    fs::write(
        generated_dir.join("00002_StaticStub.rs"),
        r#"
pub struct ProgressMeter;

impl ProgressMeter {
    pub fn set_records_between_time_checks<T0>(_arg0: T0) -> () { () }
}

pub struct StaticStub;
"#,
    )
    .unwrap();

    let shared_support = dir.join("shared.rs");
    let shared_types = dir.join("shared-types.tsv");
    let conflicts = dir.join("conflicts.tsv");
    fs::write(&shared_support, "").unwrap();
    fs::write(&shared_types, "").unwrap();

    let script =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/merge-shared-support-facades.py");
    let output = Command::new("python3")
        .arg(script)
        .arg("--generated-dir")
        .arg(&generated_dir)
        .arg("--shared-support")
        .arg(&shared_support)
        .arg("--shared-types")
        .arg(&shared_types)
        .arg("--conflicts")
        .arg(&conflicts)
        .output()
        .expect("run shared support merger");
    assert!(
        output.status.success(),
        "shared support merger failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let shared = fs::read_to_string(shared_support).unwrap();
    assert!(
        shared.contains("pub fn set_records_between_time_checks<T0>(&self, _arg0: T0) -> Self"),
        "{shared}"
    );
    assert!(
        !shared.contains("pub fn set_records_between_time_checks<T0>(_arg0: T0) -> ()"),
        "{shared}"
    );
}

#[test]
fn shared_support_merger_removes_duplicate_methods_inside_single_impl() {
    let dir = unique_temp_dir("shared_support_single_impl_method_dedupe");
    let generated_dir = dir.join("generated");
    fs::create_dir_all(&generated_dir).unwrap();
    fs::write(
        generated_dir.join("00001_UseBuilder.rs"),
        r#"
pub struct Builder;

impl Builder {
    pub fn get_fields_builder<T0>(&self, _arg0: T0) -> ConcreteBuilder { Default::default() }
    pub fn get_fields_builder<T: Default, T0>(&self, _arg0: T0) -> T { Default::default() }
}

pub struct UseBuilder;
"#,
    )
    .unwrap();

    let shared_support = dir.join("shared.rs");
    let shared_types = dir.join("shared-types.tsv");
    let conflicts = dir.join("conflicts.tsv");
    fs::write(&shared_support, "").unwrap();
    fs::write(&shared_types, "").unwrap();

    let script =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/merge-shared-support-facades.py");
    let output = Command::new("python3")
        .arg(script)
        .arg("--generated-dir")
        .arg(&generated_dir)
        .arg("--shared-support")
        .arg(&shared_support)
        .arg("--shared-types")
        .arg(&shared_types)
        .arg("--conflicts")
        .arg(&conflicts)
        .output()
        .expect("run shared support merger");
    assert!(
        output.status.success(),
        "shared support merger failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let module = fs::read_to_string(generated_dir.join("00001_UseBuilder.rs")).unwrap();
    assert_eq!(
        module.matches("pub fn get_fields_builder").count(),
        1,
        "{module}"
    );
    assert!(module.contains("-> ConcreteBuilder"), "{module}");
    assert!(!module.contains("-> T"), "{module}");
}

#[test]
fn idiom_sweep_ignores_valid_option_none_carriers() {
    let dir = unique_temp_dir("idiom_sweep_option_none");
    fs::write(
        dir.join("generated.rs"),
        r#"
pub fn nullable_return() -> Option<Void> {
    return None;
}

pub fn nullable_local() {
    let mut value: Option<String> = None;
}

pub fn concrete_assignment() {
    value = None;
}
"#,
    )
    .unwrap();

    let counts = run_idiom_sweep(&dir);
    assert_eq!(counts.get("concrete_none").copied(), Some(1));
}

#[test]
fn idiom_sweep_counts_only_strong_java_collection_spellings() {
    let dir = unique_temp_dir("idiom_sweep_collection");
    fs::write(
        dir.join("generated.rs"),
        r#"
pub fn collection_examples() {
    if values.contains_key(&name) {}
    interval_list.add(interval);
    values.add_all(other);
    values.size();
    values.put(key, value);
    values.entry_set();
}
"#,
    )
    .unwrap();

    let counts = run_idiom_sweep(&dir);
    assert_eq!(counts.get("java_collection_api").copied(), Some(4));
}

#[test]
fn idiom_sweep_counts_unsupported_and_facade_coverage_signals() {
    let dir = unique_temp_dir("idiom_sweep_coverage_signals");
    fs::write(
        dir.join("generated.rs"),
        r#"
pub type OpaqueThing = Box<dyn std::any::Any>;

pub fn unsupported_examples() {
    compile_error!("unsupported Java statement: assert_statement");
    compile_error!("unsupported Java member: unsupported member node: static_initializer");
    compile_error!("unsupported Java expression: unsupported expression node: switch_expression");
}

impl GraphicsFacade for Graphics {
    fn draw_line(&self) { unimplemented!("Graphics.drawLine is not modelled"); }
    /// A modelled surface with no behaviour behind it.
    fn get_font(&self) -> Font { Default::default() }
}

impl From<ZipOutputStream> for OutputStream {
    /// Unmodelled Java upcast: ZipOutputStream -> OutputStream.
    fn from(_value: ZipOutputStream) -> Self { Default::default() }
}

pub const UNKNOWN_LIMIT: i32 = { compile_error!("external static constant SomeLib.UNKNOWN_LIMIT has no modeled value"); 0 };
"#,
    )
    .unwrap();

    let counts = run_idiom_sweep(&dir);
    assert_eq!(counts.get("unsupported_statements").copied(), Some(1));
    assert_eq!(counts.get("unsupported_members").copied(), Some(1));
    assert_eq!(counts.get("unsupported_expressions").copied(), Some(1));
    assert_eq!(counts.get("unmodelled_types").copied(), Some(1));
    assert_eq!(counts.get("unmodelled_call_sites").copied(), Some(1));
    // The no-op *method* counts; the synthesised upcast conversion does not,
    // even though both bodies are `Default::default()`. They are different
    // phenomena with different fixes, and lumping them together made this
    // counter 97% upcasts on real output (claude_audits/0018).
    assert_eq!(counts.get("noop_facade_methods").copied(), Some(1));
    assert_eq!(
        counts.get("unmodelled_upcast_conversions").copied(),
        Some(1)
    );
    assert_eq!(counts.get("stubbed_constants").copied(), Some(1));
}

/// The Python sweep and the Rust counters must agree on the signals they share.
///
/// They are two independent implementations of the same named metric, on
/// opposite sides of a language boundary, and they *did* disagree:
/// `sweep-generated-idioms.py` matched every `Default::default()` anywhere in
/// the file while `output_markers::count_noop_facade_methods` matched only a
/// method body, and neither excluded the synthesised upcast conversions that
/// made up 78 of 80 matches on a trivial program.
///
/// A shared constant cannot fix this — one side is Python — so the agreement is
/// asserted instead. See
/// `claude_audits/0018-the-compile-error-metric-cannot-see-silent-failures.md`.
#[test]
fn the_python_sweep_and_the_rust_counters_agree_on_shared_signals() {
    let rust = new_java2rust::translate_source(
        "public class SweepAgreement { public String m() { StringBuilder b = new StringBuilder(); \
         b.append(\"x\"); return b.toString(); } }",
    )
    .rust;

    let dir = unique_temp_dir("sweep_agreement");
    fs::write(dir.join("generated.rs"), &rust).unwrap();
    let swept = run_idiom_sweep(&dir);

    for (signal, counted) in [
        (
            "noop_facade_methods",
            new_java2rust::output_markers::count_noop_facade_methods(&rust),
        ),
        (
            "unmodelled_upcast_conversions",
            new_java2rust::output_markers::count_unmodelled_upcast_conversions(&rust),
        ),
        (
            "stubbed_constants",
            new_java2rust::output_markers::count_stubbed_constants(&rust),
        ),
    ] {
        assert_eq!(
            swept.get(signal).copied().unwrap_or(0),
            counted,
            "the sweep and the Rust counter disagree on {signal}"
        );
    }
}
