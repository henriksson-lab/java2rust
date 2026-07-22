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

    let module = fs::read_to_string(generated_dir.join("00001_Outer.rs")).unwrap();
    assert_eq!(
        module.matches("pub struct OuterNested").count(),
        1,
        "{module}"
    );
    assert!(module.contains("impl OuterNested"), "{module}");
    assert!(module.contains("pub fn new(value: i32)"), "{module}");
    assert!(!module.contains("pub fn weak"), "{module}");
    assert!(!module.contains("pub struct SharedFacade"), "{module}");
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
