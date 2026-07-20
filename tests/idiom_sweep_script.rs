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
        !fs::read_to_string(generated_dir.join("00001_UseA.rs"))
            .unwrap()
            .contains("impl VCFConstants"),
        "merged support impl should be removed from source module"
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
