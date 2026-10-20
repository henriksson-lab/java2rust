use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[test]
fn shared_runtime_registries_merge_and_execute_across_modules() {
    let output = Command::new("python3")
        .arg(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/test_shared_runtime_registries.py"),
        )
        .output()
        .expect("run shared runtime registry controls");
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn gate_uses_the_terminal_outer_test_result_not_a_nested_result() {
    let gate =
        fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/gate.sh")).unwrap();

    assert_eq!(gate.matches("grep '^test result'").count(), 2, "{gate}");
    assert_eq!(gate.matches("| tail -1").count(), 2, "{gate}");
    assert!(!gate.contains("grep '^test result' \"$output\" | head -1"));
    assert!(!gate.contains("grep '^test result' \"$lib_output\" | head -1"));
}

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

#[test]
fn shared_support_alias_specializations_keep_methods_constants_and_bounds() {
    let output = Command::new("python3")
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/test_rust_item_aliases.py"))
        .output()
        .expect("run structural and actual-merger alias controls");
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn shared_support_lexical_summaries_are_content_keyed_and_immutable() {
    let script_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts");
    let output = Command::new("python3")
        .arg("-c")
        .arg(r#"
import runpy, sys
sys.path.insert(0, sys.argv[1])
module = runpy.run_path(sys.argv[1] + '/merge-shared-support-facades.py')
text = 'fn first() { helper::<Key>(); /* Marker */ let s = "Word"; }'
names = module['identifier_names']
expected = set(module['IDENTIFIER_RE'].findall(text))
summary = names(text)
assert isinstance(summary, frozenset)
assert summary == expected
assert names(text) is summary
for candidates in [{'helper', 'Key'}, {'first', 'Marker', 'Word', 'absent'}, set()]:
    for result in [module['referenced_function_names'](text, candidates),
                   module['referenced_nominal_names']([text, text], candidates)]:
        assert result == expected.intersection(candidates)
        result.clear()
    assert module['referenced_function_names'](text, candidates) == expected.intersection(candidates)
changed = text.replace('Key', 'Other')
assert 'Other' in names(changed) and 'Key' not in names(changed)
assert names(text) is summary
impl = '''impl Thing {
    pub const LIMIT: i32 = 2;
    pub fn new() -> Self { Self }
    pub const fn value(&self) -> i32 { 2 }
}
'''
methods = module['extract_methods'](impl)
assert isinstance(methods, tuple) and all(isinstance(item, tuple) for item in methods)
assert [name for name, _ in methods] == ['new', 'value']
assert module['extract_methods'](impl) is methods
assert module['extract_consts'](impl) == (('LIMIT', 'pub const LIMIT: i32 = 2;'),)
assert '3' in module['extract_methods'](impl.replace('{ 2 }', '{ 3 }'))[1][1]
assert '2' in methods[1][1]
trait = '''pub trait Thing {
    fn first(&self);
    fn second(&self) -> i32 { 2 }
}
'''
trait_methods = module['extract_trait_methods'](trait)
assert isinstance(trait_methods, tuple)
assert [name for name, _ in trait_methods] == ['first', 'second']
assert module['extract_trait_methods'](trait) is trait_methods
print('content-keyed lexical summaries: ok')
"#)
        .arg(script_dir)
        .output()
        .expect("test shared support lexical summaries");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn run_idiom_sweep(root: &Path) -> HashMap<String, usize> {
    run_idiom_sweep_with_extra(root, &[])
}

fn run_idiom_sweep_with_extra(root: &Path, extra_rust: &[&Path]) -> HashMap<String, usize> {
    let out_tsv = root.join("idiom-sweep.tsv");
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/sweep-generated-idioms.py");
    let mut command = Command::new("python3");
    command
        .arg(script)
        .arg("--generated-root")
        .arg(root)
        .arg("--out-tsv")
        .arg(&out_tsv);
    for path in extra_rust {
        command.arg("--extra-rust").arg(path);
    }
    let output = command.output().expect("run idiom sweep");
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
fn focused_package_reconciles_source_surfaces_before_indexing_and_after_final_rewrite() {
    let script_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/translate-focused-package.sh");
    let script = fs::read_to_string(script_path).unwrap();
    let invocation = "restore_missing_source_type_items \"$rust_path\" \"$raw_generated_backup_dir/$(basename \"$rust_path\")\"";

    // Restoration is shared by both boundary passes. Follow the helper calls
    // rather than requiring its implementation to be copied into each loop.
    assert_eq!(script.matches(invocation).count(), 1, "{script}");
    let restore_helper = script
        .find("restore_generated_module_dependencies() {")
        .expect("shared restoration helper");
    let restore_body = script[restore_helper..]
        .split_once("\n}")
        .expect("restoration helper body")
        .0;
    assert!(restore_body.contains(invocation), "{restore_body}");
    let finalize_helper = script
        .find("finalize_generated_module_dependencies() {")
        .expect("final restoration helper");
    let finalize_body = script[finalize_helper..]
        .split_once("\n}")
        .expect("finalization helper body")
        .0;
    assert!(
        finalize_body.contains("restore_generated_module_dependencies \"$rust_path\""),
        "{finalize_body}"
    );
    let merge = script
        .find("python3 \"$repo_root/scripts/merge-shared-support-facades.py\"")
        .expect("shared facade merge invocation");
    let first_reconcile = script
        .find("assembly_stage_start_job restore_generated_module_dependencies \"$rust_path\"")
        .expect("source reconciliation invocation");
    let final_reconcile = script
        .find("assembly_stage_start_job finalize_generated_module_dependencies \"$rust_path\"")
        .expect("final source reconciliation invocation");
    let module_index = script
        .find("print module \"\\t\" name")
        .expect("generated module type indexing");
    let import_rewrite = script
        .find("mv \"$tmp_import_path\" \"$rust_path\"")
        .expect("generated module import rewrite");
    let crate_root = script
        .find("printf \"#![allow(dead_code")
        .expect("crate root generation");
    assert!(
        first_reconcile > merge,
        "source reconciliation must follow merge"
    );
    assert!(
        first_reconcile < module_index,
        "nominal restoration must precede type indexing"
    );
    assert!(
        final_reconcile > import_rewrite,
        "surface closure must follow the last module rewrite"
    );
    assert!(
        final_reconcile < crate_root,
        "surface closure must precede crate compilation"
    );
}

#[test]
fn focused_package_default_binary_is_incrementally_rebuilt() {
    let script_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/translate-focused-package.sh");
    let script = fs::read_to_string(script_path).unwrap();

    assert!(
        script.contains("else\n  # The focused package gate is an end-to-end translator test.")
            && script
                .contains("cargo build --quiet \"${build_profile_args[@]}\" --bin new-java2rust"),
        "default focused runs must ask Cargo to refresh the release translator:\n{script}"
    );
    assert!(
        !script
            .contains("if [[ ! -x \"$binary\" ]]; then\n  if [[ -n \"${NEW_JAVA2RUST_BIN:-}\" ]]"),
        "an existing default binary must not bypass Cargo's freshness check:\n{script}"
    );
}

#[test]
fn focused_package_source_conversion_restore_respects_shared_coherence_keys() {
    let script_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/translate-focused-package.sh");
    let script = fs::read_to_string(script_path).unwrap();

    assert!(
        script.contains("shared_trait_keys.add((parts[0], parts[1]))"),
        "shared support trait implementations must be indexed by exact coherence key"
    );
    assert!(
        script.contains("and key not in shared_trait_keys"),
        "source reconciliation must not restore a crate-global duplicate"
    );
    assert!(
        script.contains(
            "restore_missing_source_type_items \"$rust_path\" \"$raw_generated_backup_dir/$(basename \"$rust_path\")\" \"$shared_support_types_path\" \"$shared_support_path\" \"$repo_root/scripts\""
        ),
        "the finalized shared facade must be visible to source reconciliation"
    );
    assert!(
        script.contains("from_source_re = re.compile(r\"^From<(.+)>$\")"),
        "wrapped source conversions such as Arc<Mutex<Source>> must remain supported"
    );
}

#[test]
fn focused_package_source_conversion_restore_skips_only_exact_shared_key() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let script = fs::read_to_string(root.join("scripts/translate-focused-package.sh")).unwrap();
    let function_start = script
        .find("restore_missing_source_type_items() {")
        .expect("source restoration function");
    let function_tail = &script[function_start..];
    let function_end = function_tail
        .find("\n}\n\nif grep -R -q '^pub struct JavaException'")
        .expect("end of source restoration function")
        + 3;

    let dir = unique_temp_dir("focused_from_shared_coherence");
    let harness = dir.join("restore.sh");
    fs::write(
        &harness,
        format!(
            "{}\nrestore_missing_source_type_items \"$1\" \"$2\" \"$3\" \"$4\" \"$5\"\n",
            &function_tail[..function_end]
        ),
    )
    .unwrap();
    let current = dir.join("00001_Source.rs");
    let raw = dir.join("00001_Source.raw.rs");
    let shared_types = dir.join("shared-types.txt");
    let shared_support = dir.join("shared-support.rs");
    fs::write(&current, "pub struct Source;\n").unwrap();
    fs::write(
        &raw,
        r#"pub struct Source;
impl From<Source> for SharedCarrier {
    fn from(_value: Source) -> Self { SharedCarrier }
}
impl From<std::sync::Arc<std::sync::Mutex<Source>>> for OtherCarrier {
    fn from(_value: std::sync::Arc<std::sync::Mutex<Source>>) -> Self { OtherCarrier }
}
"#,
    )
    .unwrap();
    fs::write(&shared_types, "SharedCarrier\nOtherCarrier\n").unwrap();
    fs::write(
        &shared_support,
        r#"impl From<Source> for SharedCarrier {
    fn from(_value: Source) -> Self { SharedCarrier }
}
"#,
    )
    .unwrap();

    let output = Command::new("bash")
        .arg(&harness)
        .arg(&current)
        .arg(&raw)
        .arg(&shared_types)
        .arg(&shared_support)
        .arg(root.join("scripts"))
        .output()
        .expect("run source conversion restoration");
    assert!(
        output.status.success(),
        "restore failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let restored = fs::read_to_string(current).unwrap();
    assert!(
        !restored.contains("impl From<Source> for SharedCarrier"),
        "crate-global duplicate must stay suppressed:\n{restored}"
    );
    assert!(
        restored.contains("impl From<std::sync::Arc<std::sync::Mutex<Source>>> for OtherCarrier"),
        "a different wrapped-source coherence key must still be restored:\n{restored}"
    );
}

#[test]
fn focused_package_does_not_restore_methods_twice_with_missing_nominal_type() {
    let script_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/translate-focused-package.sh");
    let script = fs::read_to_string(script_path).unwrap();

    assert!(
        script.contains("surface_owned_names = source_owned_names - missing_names"),
        "{script}"
    );
    assert!(
        script.contains("inherent_method_names(current_lines_for_surface, surface_owned_names)"),
        "{script}"
    );
    assert!(
        script.contains("source_owned_trait_impl(key[0], target, surface_owned_names)"),
        "{script}"
    );
}

#[test]
fn focused_package_restores_source_owned_boxed_dyn_clone_impls() {
    let script_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/translate-focused-package.sh");
    let script = fs::read_to_string(script_path).unwrap();

    assert!(
        script.contains("def boxed_dyn_trait_target_name(target: str):"),
        "{script}"
    );
    assert!(
        script.contains(r#"r"(?:(?:std|alloc)::boxed::)?Box\s*<\s*dyn\s+""#),
        "{script}"
    );
    assert!(
        script.contains("trait == \"Clone\"")
            && script.contains("boxed_dyn_trait_target_name(target) in owned_names"),
        "{script}"
    );
    assert!(
        script.contains("source_owned_trait_impl(key[0], target, surface_owned_names)"),
        "{script}"
    );
}

#[test]
fn focused_package_source_restore_executes_for_trimmomatic_boxed_dyn_clones() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let script = fs::read_to_string(root.join("scripts/translate-focused-package.sh")).unwrap();
    let function_start = script
        .find("restore_missing_source_type_items() {")
        .expect("source restoration function");
    let function_tail = &script[function_start..];
    let function_end = function_tail
        .find("\n}\n\nif grep -R -q '^pub struct JavaException'")
        .expect("end of source restoration function")
        + 3;

    let dir = unique_temp_dir("focused_boxed_dyn_clone_restore");
    let harness = dir.join("restore.sh");
    fs::write(
        &harness,
        format!(
            "{}\nrestore_missing_source_type_items \"$1\" \"$2\" \"$3\" \"$4\" \"$5\"\n",
            &function_tail[..function_end]
        ),
    )
    .unwrap();
    let shared_types = dir.join("shared-types.txt");
    let shared_support = dir.join("shared-support.rs");
    fs::write(&shared_types, "").unwrap();
    fs::write(&shared_support, "").unwrap();

    for trait_name in ["Trimmer", "BlockData", "ParallelCompressor"] {
        let current = dir.join(format!("00001_{trait_name}.rs"));
        let raw = dir.join(format!("00001_{trait_name}.raw.rs"));
        let trait_surface = format!(
            "pub trait {trait_name} {{\n    fn clone_box(&self) -> std::boxed::Box<dyn {trait_name}>;\n}}\n"
        );
        fs::write(&current, &trait_surface).unwrap();
        fs::write(
            &raw,
            format!(
                "{trait_surface}\nimpl Clone for std::boxed::Box<dyn {trait_name}> {{\n    fn clone(&self) -> Self {{ {trait_name}::clone_box(&**self) }}\n}}\n"
            ),
        )
        .unwrap();

        let output = Command::new("bash")
            .arg(&harness)
            .arg(&current)
            .arg(&raw)
            .arg(&shared_types)
            .arg(&shared_support)
            .arg(root.join("scripts"))
            .output()
            .expect("run source surface restoration");
        assert!(
            output.status.success(),
            "restore failed for {trait_name}\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );

        let restored = fs::read_to_string(&current).unwrap();
        let clone_impl = format!("impl Clone for std::boxed::Box<dyn {trait_name}>");
        assert_eq!(restored.matches(&clone_impl).count(), 1, "{restored}");
    }

    // When the captured full-assembly artifacts are present, exercise the
    // helper against those exact raw/final modules and the actual shared
    // manifests. This caught the production loss that the compact harness
    // above models, while keeping the test runnable in a clean checkout where
    // `.tmp` is intentionally absent.
    let artifact_root = root.join(".tmp/trimmomatic-full-v7");
    let artifact_generated = artifact_root.join("generated-crate/src/generated");
    let artifact_raw = artifact_root.join("generated-raw-before-support-strip");
    let artifact_shared_types = artifact_root.join("generated-shared-support-types.tsv");
    let artifact_shared_support = artifact_root.join("generated-shared-support.rs");
    if artifact_generated.is_dir()
        && artifact_raw.is_dir()
        && artifact_shared_types.is_file()
        && artifact_shared_support.is_file()
    {
        let real_copy_dir = dir.join("real-artifact-copies");
        fs::create_dir_all(&real_copy_dir).unwrap();
        for (stem, trait_name) in [
            ("00051_Trimmer", "Trimmer"),
            ("00055_BlockData", "BlockData"),
            ("00063_ParallelCompressor", "ParallelCompressor"),
        ] {
            // Keep the production basename: source ownership is intentionally
            // derived from the numbered generated filename.
            let current = real_copy_dir.join(format!("{stem}.rs"));
            fs::copy(artifact_generated.join(format!("{stem}.rs")), &current).unwrap();
            let output = Command::new("bash")
                .arg(&harness)
                .arg(&current)
                .arg(artifact_raw.join(format!("{stem}.rs")))
                .arg(&artifact_shared_types)
                .arg(&artifact_shared_support)
                .arg(root.join("scripts"))
                .output()
                .expect("run source restoration against real assembly artifact");
            assert!(
                output.status.success(),
                "real artifact restore failed for {trait_name}\nstdout:\n{}\nstderr:\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );

            let restored = fs::read_to_string(current).unwrap();
            let clone_impl = format!("impl Clone for std::boxed::Box<dyn {trait_name}>");
            assert_eq!(
                restored.matches(&clone_impl).count(),
                1,
                "real {stem} artifact did not recover its boxed-dyn Clone impl:\n{restored}"
            );
        }
    }
}

#[test]
fn focused_package_source_restore_recovers_generic_interface_forwarding_impls() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let script = fs::read_to_string(root.join("scripts/translate-focused-package.sh")).unwrap();
    let function_start = script
        .find("restore_missing_source_type_items() {")
        .expect("source restoration function");
    let function_tail = &script[function_start..];
    let function_end = function_tail
        .find("\n}\n\nif grep -R -q '^pub struct JavaException'")
        .expect("end of source restoration function")
        + 3;

    let dir = unique_temp_dir("focused_generic_interface_forwarding_restore");
    let harness = dir.join("restore.sh");
    fs::write(
        &harness,
        format!(
            "{}\nrestore_missing_source_type_items \"$1\" \"$2\" \"$3\" \"$4\" \"$5\"\n",
            &function_tail[..function_end]
        ),
    )
    .unwrap();
    let current = dir.join("00001_Distribution.rs");
    let raw = dir.join("00001_Distribution.raw.rs");
    let shared_types = dir.join("shared-types.txt");
    let shared_support = dir.join("shared-support.rs");
    fs::write(
        &current,
        "pub trait Distribution<O> {\n    fn generate(&mut self) -> O;\n}\n",
    )
    .unwrap();
    fs::write(
        &raw,
        r#"pub trait Distribution<O> {
    fn generate(&mut self) -> O;
}
impl<O: 'static, __JavaShared: Distribution<O> + 'static> Distribution<O> for std::rc::Rc<std::cell::RefCell<__JavaShared>> {
    fn generate(&mut self) -> O { Distribution::<O>::generate(&mut *self.borrow_mut()) }
}
impl<O: 'static, __JavaBoxed: Distribution<O> + ?Sized> Distribution<O> for std::boxed::Box<__JavaBoxed> {
    fn generate(&mut self) -> O { Distribution::<O>::generate(&mut **self) }
}
"#,
    )
    .unwrap();
    fs::write(&shared_types, "").unwrap();
    fs::write(&shared_support, "").unwrap();

    let output = Command::new("bash")
        .arg(&harness)
        .arg(&current)
        .arg(&raw)
        .arg(&shared_types)
        .arg(&shared_support)
        .arg(root.join("scripts"))
        .output()
        .expect("run source forwarding restoration");
    assert!(
        output.status.success(),
        "restore failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let restored = fs::read_to_string(current).unwrap();
    assert_eq!(
        restored.matches("Distribution<O> for std::rc::Rc").count(),
        1,
        "{restored}"
    );
    assert_eq!(
        restored
            .matches("Distribution<O> for std::boxed::Box<__JavaBoxed>")
            .count(),
        1,
        "{restored}"
    );
}

#[test]
fn source_restore_keeps_nested_trait_companions_before_the_primary_declaration() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let script = fs::read_to_string(root.join("scripts/translate-focused-package.sh")).unwrap();
    let tail = &script[script
        .find("restore_missing_source_type_items() {")
        .unwrap()..];
    let end = tail
        .find("\n}\n\nif grep -R -q '^pub struct JavaException'")
        .unwrap()
        + 3;
    let dir = unique_temp_dir("nested_trait_companion_restore");
    let harness = dir.join("restore.sh");
    fs::write(
        &harness,
        format!(
            "{}\nrestore_missing_source_type_items \"$1\" \"$2\" \"$3\" \"$4\" \"$5\"\n",
            &tail[..end],
        ),
    )
    .unwrap();
    let current = dir.join("00001_Container.rs");
    let raw = dir.join("raw.rs");
    let shared_types = dir.join("shared-types.txt");
    let shared = dir.join("shared.rs");
    let declarations = "pub trait Container_View<T> { fn clone_box(&self) -> Box<dyn Container_View<T>>; }\npub struct Container;\n";
    fs::write(&current, declarations).unwrap();
    fs::write(
        &raw,
        r#"
pub struct Prelude;
impl Clone for Prelude { fn clone(&self) -> Self { Prelude } }
pub trait Container_View<T> { fn clone_box(&self) -> Box<dyn Container_View<T>>; }
impl<T> Clone for Box<dyn Container_View<T>> {
    fn clone(&self) -> Self { Container_View::<T>::clone_box(&**self) }
}
pub struct Container;
"#,
    )
    .unwrap();
    fs::write(&shared_types, "").unwrap();
    fs::write(&shared, "").unwrap();
    for _ in 0..2 {
        let output = Command::new("bash")
            .arg(&harness)
            .arg(&current)
            .arg(&raw)
            .arg(&shared_types)
            .arg(&shared)
            .arg(root.join("scripts"))
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let restored = fs::read_to_string(current).unwrap();
    assert_eq!(
        restored
            .matches("Clone for Box<dyn Container_View<T>>")
            .count(),
        1,
        "{restored}"
    );
    assert!(!restored.contains("Clone for Prelude"), "{restored}");
}

#[test]
fn focused_package_source_restore_recovers_aliases_to_nested_source_nominals() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let script = fs::read_to_string(root.join("scripts/translate-focused-package.sh")).unwrap();
    let function_start = script
        .find("restore_missing_source_type_items() {")
        .expect("source restoration function");
    let function_tail = &script[function_start..];
    let function_end = function_tail
        .find("\n}\n\nif grep -R -q '^pub struct JavaException'")
        .expect("end of source restoration function")
        + 3;

    let dir = unique_temp_dir("focused_nested_source_alias_restore");
    let harness = dir.join("restore.sh");
    fs::write(
        &harness,
        format!(
            "{}\nrestore_missing_source_type_items \"$1\" \"$2\" \"$3\" \"$4\" \"$5\"\n",
            &function_tail[..function_end]
        ),
    )
    .unwrap();
    let current = dir.join("00001_CommandLine.rs");
    let raw = dir.join("00001_CommandLine.raw.rs");
    let shared_types = dir.join("shared-types.txt");
    let shared_support = dir.join("shared-support.rs");
    fs::write(
        &current,
        "pub enum CommandLine_Arguments { A, B }\npub struct CommandLine;\n",
    )
    .unwrap();
    fs::write(
        &raw,
        r#"pub enum CommandLine_Arguments { A, B }
pub type Arguments = CommandLine_Arguments;
pub type ArgumentList<T> = CommandLine_Arguments;
pub struct CommandLine;
"#,
    )
    .unwrap();
    fs::write(&shared_types, "Arguments\nArgumentList\n").unwrap();
    fs::write(&shared_support, "").unwrap();

    let output = Command::new("bash")
        .arg(&harness)
        .arg(&current)
        .arg(&raw)
        .arg(&shared_types)
        .arg(&shared_support)
        .arg(root.join("scripts"))
        .output()
        .expect("run nested source alias restoration");
    assert!(
        output.status.success(),
        "restore failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let restored = fs::read_to_string(current).unwrap();
    assert_eq!(
        restored
            .matches("pub type Arguments = CommandLine_Arguments;")
            .count(),
        1,
        "{restored}"
    );
    assert_eq!(
        restored
            .matches("pub type ArgumentList<T> = CommandLine_Arguments;")
            .count(),
        1,
        "{restored}"
    );
}

#[test]
fn focused_package_source_restore_recovers_wrapper_target_impls() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let script = fs::read_to_string(root.join("scripts/translate-focused-package.sh")).unwrap();
    let function_start = script
        .find("restore_missing_source_type_items() {")
        .expect("source restoration function");
    let function_tail = &script[function_start..];
    let function_end = function_tail
        .find("\n}\n\nif grep -R -q '^pub struct JavaException'")
        .expect("end of source restoration function")
        + 3;

    let dir = unique_temp_dir("focused_wrapper_target_restore");
    let harness = dir.join("restore.sh");
    fs::write(
        &harness,
        format!(
            "{}\nrestore_missing_source_type_items \"$1\" \"$2\" \"$3\" \"$4\" \"$5\"\n",
            &function_tail[..function_end]
        ),
    )
    .unwrap();
    let current = dir.join("00001_LeadingTrimmer.rs");
    let raw = dir.join("00001_LeadingTrimmer.raw.rs");
    let shared_types = dir.join("shared-types.txt");
    let shared_support = dir.join("shared-support.rs");
    fs::write(&current, "pub struct LeadingTrimmer;\n").unwrap();
    fs::write(
        &raw,
        r#"pub struct LeadingTrimmer;
impl Trimmer for std::rc::Rc<LeadingTrimmer> {
    fn trim(&self) {}
}
impl Trimmer for std::sync::Arc<LeadingTrimmer> {
    fn trim(&self) {}
}
impl Dispatch for std::rc::Rc<std::cell::RefCell<LeadingTrimmer>> {
    fn dispatch(&self) {}
}
impl<E: JavaEnum> GenericTrait<E> for LeadingTrimmer<E> {
    fn generic_dispatch(&self) {}
}
"#,
    )
    .unwrap();
    fs::write(&shared_types, "Trimmer\nDispatch\n").unwrap();
    fs::write(
        &shared_support,
        r#"impl Dispatch for std::rc::Rc<std::cell::RefCell<LeadingTrimmer>> {
    fn dispatch(&self) {}
}
"#,
    )
    .unwrap();

    for _ in 0..2 {
        let output = Command::new("bash")
            .arg(&harness)
            .arg(&current)
            .arg(&raw)
            .arg(&shared_types)
            .arg(&shared_support)
            .arg(root.join("scripts"))
            .output()
            .expect("run wrapper-target source restoration");
        assert!(
            output.status.success(),
            "restore failed\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let restored = fs::read_to_string(&current).unwrap();
    for impl_header in [
        "impl Trimmer for std::rc::Rc<LeadingTrimmer>",
        "impl Trimmer for std::sync::Arc<LeadingTrimmer>",
        "impl<E: JavaEnum> GenericTrait<E> for LeadingTrimmer<E>",
    ] {
        assert_eq!(restored.matches(impl_header).count(), 1, "{restored}");
    }
    assert_eq!(
        restored
            .matches("impl Dispatch for std::rc::Rc<std::cell::RefCell<LeadingTrimmer>>")
            .count(),
        0,
        "an exact impl already owned by shared support must not be restored locally:\n{restored}"
    );

    // Exercise the bounded generic source impls which JAHMM exposed. The
    // captured artifacts are optional so this remains runnable in a clean
    // checkout where `.tmp` is deliberately absent.
    let jahmm_artifact_root = root.join(".tmp/jahmm-full-v24");
    let jahmm_generated = jahmm_artifact_root.join("generated-crate/src/generated");
    let jahmm_raw = jahmm_artifact_root.join("generated-raw-before-support-strip");
    let jahmm_shared_types = jahmm_artifact_root.join("generated-shared-support-types.tsv");
    let jahmm_shared_support = jahmm_artifact_root.join("generated-shared-support.rs");
    if jahmm_generated.is_dir()
        && jahmm_raw.is_dir()
        && jahmm_shared_types.is_file()
        && jahmm_shared_support.is_file()
    {
        for (file, expected_impl) in [
            (
                "00030_OpdfDiscrete.rs",
                "impl<E: JavaEnum + std::any::Any> Opdf<std::rc::Rc<std::cell::RefCell<ObservationDiscrete<E>>>> for OpdfDiscrete<E>",
            ),
            (
                "00031_OpdfDiscreteFactory.rs",
                "impl<E: JavaEnum + std::any::Any> OpdfFactory<OpdfDiscrete<E>> for OpdfDiscreteFactory<E>",
            ),
        ] {
            // Source ownership is derived from the numbered basename.
            let copied = dir.join(file);
            fs::copy(jahmm_generated.join(file), &copied).unwrap();
            let output = Command::new("bash")
                .arg(&harness)
                .arg(&copied)
                .arg(jahmm_raw.join(file))
                .arg(&jahmm_shared_types)
                .arg(&jahmm_shared_support)
                .arg(root.join("scripts"))
                .output()
                .expect("restore exact JAHMM bounded-generic artifact");
            assert!(output.status.success(), "restore failed for {file}");
            let restored = fs::read_to_string(copied).unwrap();
            assert_eq!(restored.matches(expected_impl).count(), 1, "{restored}");
        }
    }

    // Exercise the same final restoration helper against exact captured v4
    // modules when the local full-corpus artifact is available.
    let artifact_root = root.join(".tmp/trimmomatic-all-68-v4");
    let artifact_generated = artifact_root.join("generated-crate/src/generated");
    let artifact_raw = artifact_root.join("generated-raw-before-support-strip");
    let artifact_shared_types = artifact_root.join("generated-shared-support-types.tsv");
    let artifact_shared_support = artifact_root.join("generated-shared-support.rs");
    if artifact_generated.is_dir()
        && artifact_raw.is_dir()
        && artifact_shared_types.is_file()
        && artifact_shared_support.is_file()
    {
        for (file, expected_impl) in [
            (
                "00042_LeadingTrimmer.rs",
                "impl Trimmer for std::rc::Rc<LeadingTrimmer>",
            ),
            (
                "00054_PositionTrackingInputStream.rs",
                "impl JavaInputStreamDispatch for std::rc::Rc<PositionTrackingInputStream>",
            ),
            (
                "00057_Bzip2BlockData.rs",
                "impl BlockData for std::rc::Rc<Bzip2BlockData>",
            ),
        ] {
            let copied = dir.join(file);
            fs::copy(artifact_generated.join(file), &copied).unwrap();
            let output = Command::new("bash")
                .arg(&harness)
                .arg(&copied)
                .arg(artifact_raw.join(file))
                .arg(&artifact_shared_types)
                .arg(&artifact_shared_support)
                .arg(root.join("scripts"))
                .output()
                .expect("restore exact v4 wrapper artifact");
            assert!(output.status.success(), "restore failed for {file}");
            let restored = fs::read_to_string(copied).unwrap();
            assert_eq!(restored.matches(expected_impl).count(), 1, "{restored}");
        }
    }
}

#[test]
fn focused_package_final_coherence_cleanup_is_exact_and_preserves_unique_impls() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let script = fs::read_to_string(root.join("scripts/translate-focused-package.sh")).unwrap();
    let function_start = script
        .find("deduplicate_generated_coherence_items() {")
        .expect("coherence cleanup function");
    let function_tail = &script[function_start..];
    let function_end = function_tail
        .find("\n}\n\nrestore_missing_source_type_items() {")
        .expect("end of coherence cleanup function")
        + 3;
    assert!(
        script.rfind("deduplicate_generated_coherence_items \"$rust_path\"")
            > script.rfind("restore_missing_source_type_items \"$rust_path\""),
        "exact-key cleanup must run after final source restoration:\n{script}"
    );

    let dir = unique_temp_dir("focused_final_coherence_cleanup");
    let harness = dir.join("deduplicate.sh");
    fs::write(
        &harness,
        format!(
            "{}\ndeduplicate_generated_coherence_items \"$1\" \"$2\" \"${{3:-}}\"\n",
            &function_tail[..function_end]
        ),
    )
    .unwrap();
    let fixture = dir.join("fixture.rs");
    fs::write(
        &fixture,
        r#"static STATE: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(1);
static STATE: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(2);
impl From<Source> for Carrier { fn from(_: Source) -> Self { todo!() } }
impl From<Source> for Carrier { fn from(_: Source) -> Self { panic!() } }
impl Clone for std::boxed::Box<dyn SourceTrait> { fn clone(&self) -> Self { todo!() } }
"#,
    )
    .unwrap();
    let output = Command::new("bash")
        .arg(&harness)
        .arg(&fixture)
        .arg(root.join("scripts"))
        .output()
        .expect("run final coherence cleanup");
    assert!(
        output.status.success(),
        "cleanup failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let cleaned = fs::read_to_string(&fixture).unwrap();
    assert_eq!(cleaned.matches("static STATE:").count(), 1, "{cleaned}");
    assert_eq!(
        cleaned.matches("impl From<Source> for Carrier").count(),
        1,
        "{cleaned}"
    );
    assert_eq!(
        cleaned
            .matches("impl Clone for std::boxed::Box<dyn SourceTrait>")
            .count(),
        1,
        "{cleaned}"
    );

    let shared = dir.join("shared.rs");
    fs::write(
        &shared,
        "impl<T: Bound> SharedTrait for Source<T> { fn shared(&self) {} }\n",
    )
    .unwrap();
    let cross_module = dir.join("cross_module.rs");
    fs::write(
        &cross_module,
        r#"impl<T: Bound> SharedTrait for Source<T> { fn shared(&self) { panic!() } }
impl<T: Bound> UniqueTrait for Source<T> { fn unique(&self) {} }
"#,
    )
    .unwrap();
    let output = Command::new("bash")
        .arg(&harness)
        .arg(&cross_module)
        .arg(root.join("scripts"))
        .arg(&shared)
        .output()
        .expect("run crate-wide coherence cleanup");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let cross_cleaned = fs::read_to_string(&cross_module).unwrap();
    assert!(
        !cross_cleaned.contains("impl<T: Bound> SharedTrait"),
        "{cross_cleaned}"
    );
    assert!(
        cross_cleaned.contains("impl<T: Bound> UniqueTrait"),
        "{cross_cleaned}"
    );

    // Exercise the exact fresh full-corpus artifact when it is available.
    // The captured compile log proves this fixture contains the five E0428
    // and eighteen E0119 failures that motivated this ordering regression.
    let artifact_root = root.join(".tmp/trimmomatic-full-v8");
    let artifact_generated = artifact_root.join("generated-crate/src/generated");
    let artifact_log = artifact_root.join("compile.log");
    if artifact_generated.is_dir() && artifact_log.is_file() {
        let log = fs::read_to_string(&artifact_log).unwrap();
        assert_eq!(log.matches("error[E0428]").count(), 5, "{log}");
        assert_eq!(log.matches("error[E0119]").count(), 18, "{log}");

        let copies = dir.join("v8-generated");
        fs::create_dir_all(&copies).unwrap();
        for entry in fs::read_dir(&artifact_generated).unwrap() {
            let source = entry.unwrap().path();
            if source.extension().and_then(|value| value.to_str()) != Some("rs") {
                continue;
            }
            let copy = copies.join(source.file_name().unwrap());
            fs::copy(&source, &copy).unwrap();
            let output = Command::new("bash")
                .arg(&harness)
                .arg(&copy)
                .arg(root.join("scripts"))
                .output()
                .expect("clean exact v8 generated module");
            assert!(
                output.status.success(),
                "cleanup failed for {}",
                copy.display()
            );

            let text = fs::read_to_string(&copy).unwrap();
            let mut statics = std::collections::HashSet::new();
            let mut trait_impls = std::collections::HashSet::new();
            for line in text.lines() {
                let trimmed = line.strip_prefix("pub ").unwrap_or(line);
                if let Some(tail) = trimmed.strip_prefix("static ") {
                    let name = tail
                        .split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_')
                        .next()
                        .unwrap();
                    assert!(
                        statics.insert(name),
                        "duplicate static {name} in {}",
                        copy.display()
                    );
                }
                if line.starts_with("impl ") && line.contains(" for ") {
                    let header = line.split(['{', ';']).next().unwrap().trim();
                    assert!(
                        trait_impls.insert(header),
                        "duplicate trait impl {header} in {}",
                        copy.display()
                    );
                }
            }
        }
        for (stem, trait_name) in [
            ("00051_Trimmer.rs", "Trimmer"),
            ("00055_BlockData.rs", "BlockData"),
            ("00063_ParallelCompressor.rs", "ParallelCompressor"),
        ] {
            let text = fs::read_to_string(copies.join(stem)).unwrap();
            assert_eq!(
                text.matches(&format!("impl Clone for std::boxed::Box<dyn {trait_name}>"))
                    .count(),
                1,
                "unique boxed-dyn Clone disappeared from {stem}"
            );
        }
    }
}

#[test]
fn focused_package_top_slowest_reports_do_not_trip_pipefail() {
    let script_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/translate-focused-package.sh");
    let script = fs::read_to_string(script_path).unwrap();

    assert!(!script.contains("| head -20"), "{script}");
    assert_eq!(script.matches("| awk 'NR <= 20'").count(), 2, "{script}");
}

#[test]
fn focused_package_ndarray_shim_supports_java_outer_length_and_chained_indexing() {
    let script_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/translate-focused-package.sh");
    let script = fs::read_to_string(script_path).unwrap();

    assert!(
        script.contains("pub fn len(&self) -> usize {\n        self.rows"),
        "{script}"
    );
    assert!(
        script.contains("impl<T> Index<usize> for Array2<T>"),
        "{script}"
    );
    assert!(script.contains("type Output = [T];"), "{script}");
    assert!(
        script.contains("impl<T> IndexMut<usize> for Array2<T>"),
        "{script}"
    );
}

#[test]
fn focused_package_preserves_source_hierarchy_carriers_and_conversions() {
    let script_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/translate-focused-package.sh");
    let script = fs::read_to_string(script_path).unwrap();

    assert!(script.contains("post_primary_names.add(name)"), "{script}");
    assert!(
        script.contains("or match.group(2) in post_primary_names"),
        "{script}"
    );
    assert!(
        script.contains("bool(from_source_names & source_owned_names)"),
        "{script}"
    );
    assert!(
        script.contains("source_surface_chunks.append(\"\".join(raw_lines[index:end]))"),
        "{script}"
    );
}

#[test]
fn focused_package_shared_iterator_matches_fallible_runtime_abi() {
    let script_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/translate-focused-package.sh");
    let script = fs::read_to_string(script_path).unwrap();

    assert!(script.contains("pub fn next_result(&mut self) -> Result<T, JavaException>"));
    assert!(script.contains("pub fn remove_result(&mut self) -> Result<(), JavaException>"));
    assert!(script.contains("Java iterator has no more elements"));
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
fn focused_package_prelude_restore_treats_shared_imports_as_satisfied() {
    let script_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/translate-focused-package.sh");
    let script = fs::read_to_string(script_path).unwrap();

    assert!(script.contains("def imported_top_level_names"), "{script}");
    assert!(
        script.contains("present_functions.update(imported_top_level_names(current_lines))"),
        "{script}"
    );
    assert!(
        script.contains("present_functions.update(shared_types)"),
        "{script}"
    );
}

#[test]
fn focused_package_source_restore_keeps_referenced_static_storage() {
    let script_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/translate-focused-package.sh");
    let script = fs::read_to_string(script_path).unwrap();

    assert!(script.contains("static_re = re.compile"), "{script}");
    assert!(
        script.contains("static_reference_text = current + \"\".join(source_surface_chunks)"),
        "{script}"
    );
    assert!(
        script.contains("final_static_names = {") && script.contains("missing_static_chunks = ["),
        "{script}"
    );
    assert!(script.contains("referenced_static_names = {"), "{script}");
    assert!(
        script.contains(
            "chunks = [raw_static_items[name] for name in sorted(referenced_static_names)]"
        ),
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
fn shared_rust_item_parser_handles_nested_impl_generic_bounds() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"));
    let output = Command::new("python3")
        .current_dir(repo)
        .args([
            "-c",
            "import sys; sys.path.insert(0, 'scripts'); from rust_item_utils import impl_header_parts, inherent_impl_target_name; header = 'impl<O: Opdf<Box<dyn std::any::Any>> + std::any::Any> OpdfReader<O> {'; assert impl_header_parts(header) == (None, 'OpdfReader<O>'); assert inherent_impl_target_name(header) == 'OpdfReader'; trait_header = 'impl<O: Opdf<Box<dyn std::any::Any>>> Reader<O> for OpdfReader<O> {'; assert impl_header_parts(trait_header) == ('Reader<O>', 'OpdfReader<O>')",
        ])
        .output()
        .expect("run shared Rust item parser regression");
    assert!(output.status.success(), "{:?}", output);

    let script = fs::read_to_string(repo.join("scripts/translate-focused-package.sh")).unwrap();
    assert!(
        script.contains(
            "from rust_item_utils import impl_header_parts, impl_target_name, inherent_impl_target_name"
        )
            && script.contains("target = inherent_impl_target_name(lines[index])")
            && script.contains(
                "target = inherent_impl_target_name(line) if raw_after_primary else None"
            ),
        "{script}"
    );
    assert!(
        script.matches("from rust_item_utils import").count() >= 4
            && !script.contains("inherent_impl_re = re.compile")
            && !script.contains("trait_impl_re = re.compile"),
        "every Python reconciliation phase should reuse the structural impl parser"
    );
}

#[test]
fn focused_package_reuses_translator_emitted_string_runtime() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"));
    let script = fs::read_to_string(repo.join("scripts/translate-focused-package.sh")).unwrap();

    assert!(
        script.contains("scripts/merge-shared-support-facades.py"),
        "focused assembly must retain the shared support dependency-closure merger"
    );
    assert!(
        !script.contains("pub fn java_string_format")
            && !script.contains("pub fn java_string_replace_all")
            && !script.contains("pub fn java_string_substring"),
        "focused assembly must not maintain divergent copies of translator string runtime functions"
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
        script.contains("for[[:space:]]+&?([[:space:]]*)?([A-Za-z_][A-Za-z0-9_]*::)*[A-Za-z_]"),
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

    pub const fn compile_static(value: &'static str) -> VCFConstants {
        let _ = value;
        VCFConstants
    }

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

    pub const fn compile_static(value: &'static str) -> VCFConstants {
        let _ = value;
        VCFConstants
    }

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
        shared.contains("pub const fn compile_static(value: &'static str) -> VCFConstants")
            && shared.contains("let _ = value;")
            && !shared.contains(
                "pub const fn compile_static(value: &'static str) -> VCFConstants {\n    pub"
            ),
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
fn shared_support_merger_deduplicates_multiline_where_trait_impls() {
    let dir = unique_temp_dir("shared_support_multiline_impl");
    let generated_dir = dir.join("generated");
    fs::create_dir_all(&generated_dir).unwrap();
    let module = |primary: &str| {
        format!(
            r#"
pub trait Deep {{
    fn deep(&self) -> usize;
}}

pub struct SharedMap<K, V> {{
    entries: Vec<(K, V)>,
}}

impl<K, V> Deep for SharedMap<K, V>
where
    K: Clone,
    V: Clone,
{{
    fn deep(&self) -> usize {{ self.entries.len() }}
}}

pub struct {primary};
"#
        )
    };
    fs::write(generated_dir.join("00001_UseA.rs"), module("UseA")).unwrap();
    fs::write(generated_dir.join("00002_UseB.rs"), module("UseB")).unwrap();

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
    assert_eq!(
        shared
            .matches("impl<K, V> Deep for SharedMap<K, V>")
            .count(),
        1,
        "{shared}"
    );
    for name in ["00001_UseA.rs", "00002_UseB.rs"] {
        let source = fs::read_to_string(generated_dir.join(name)).unwrap();
        assert!(
            !source.contains("impl<K, V> Deep for SharedMap<K, V>"),
            "{name}: {source}"
        );
    }
}

#[test]
fn shared_support_merger_selects_constructor_arity_used_by_generated_calls() {
    let dir = unique_temp_dir("shared_support_constructor_arity");
    let generated_dir = dir.join("generated");
    fs::create_dir_all(&generated_dir).unwrap();
    let facade = r#"
#[derive(Clone, Debug, Default)]
pub struct SummaryValue;

impl SummaryValue {
    pub fn new() -> Self { Self }
}

impl SummaryValue {
    pub fn new<T0, T1>(_arg0: T0, _arg1: T1) -> Self {
        unimplemented!("source-summary constructor")
    }
}
"#;
    fs::write(
        generated_dir.join("00001_UseSummary.rs"),
        format!(
            "{facade}\npub struct UseSummary;\nimpl UseSummary {{ pub fn make() -> SummaryValue {{ SummaryValue::new(1, nested(2, 3)) }} }}\n"
        ),
    )
    .unwrap();
    fs::write(
        generated_dir.join("00002_OtherUse.rs"),
        format!("{facade}\npub struct OtherUse;\n"),
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
        shared.contains("pub fn new<T0, T1>(_arg0: T0, _arg1: T1)")
            && !shared.contains("pub fn new() -> Self"),
        "{shared}"
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
    for (file, primary) in [("00001_Sequence.rs", "Sequence"), ("00002_UseB.rs", "UseB")] {
        let text = format!(
            r#"
#[derive(Clone, Debug, Default)]
pub struct SequenceInputStream;

impl From<SequenceInputStream> for InputStream {{
    fn from(_value: SequenceInputStream) -> Self {{ Default::default() }}
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
            .matches("impl From<SequenceInputStream> for InputStream")
            .count(),
        1,
        "{shared}"
    );
    for file in ["00001_Sequence.rs", "00002_UseB.rs"] {
        let module = fs::read_to_string(generated_dir.join(file)).unwrap();
        assert!(
            !module.contains("impl From<SequenceInputStream> for InputStream"),
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
    fn from(_value: ReadsFormatWriteOption) -> Self { panic!("unsupported Java external upcast: ReadsFormatWriteOption -> WriteOption") }
}

impl From<ReadsFormatWriteOption> for std::path::PathBuf {
    fn from(_value: ReadsFormatWriteOption) -> Self { std::path::PathBuf::new() }
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
        shared.contains("impl From<ReadsFormatWriteOption> for std::path::PathBuf"),
        "{shared}"
    );
    let shared_type_names = fs::read_to_string(shared_types).unwrap();
    assert!(!shared_type_names.lines().any(|name| name == "std"));
    assert!(!shared_type_names.lines().any(|name| name == "PathBuf"));
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
        let _support_limit = JAVA_STRING_BUFFER_LIMIT + JAVA_STRING_BUFFER_OFFSET;
        java_string_buffer_set_length(&mut self.value, new_len);
        java_string_buffer_multiline_helper(
            &self.value,
            new_len,
        );
    }
}

const JAVA_STRING_BUFFER_LIMIT: i32 =
    1024;
static JAVA_STRING_BUFFER_OFFSET: i32 = 0;

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
        shared.contains("pub(crate) fn java_string_buffer_set_length"),
        "{shared}"
    );
    assert!(
        shared.contains("pub(crate) fn java_string_buffer_byte_index"),
        "{shared}"
    );
    assert!(
        shared.contains("pub(crate) fn java_string_buffer_multiline_helper"),
        "{shared}"
    );
    assert!(
        shared.contains("pub(crate) const JAVA_STRING_BUFFER_LIMIT: i32"),
        "{shared}"
    );
    assert!(
        shared.contains("pub(crate) static JAVA_STRING_BUFFER_OFFSET: i32"),
        "{shared}"
    );
    assert!(
        shared.contains("java_string_buffer_byte_index(value, char_index)"),
        "{shared}"
    );
    assert!(
        shared.contains("pub(crate) fn java_existing_shared_helper"),
        "{shared}"
    );
    assert_eq!(
        shared.matches("fn java_already_shared_helper").count(),
        1,
        "{shared}"
    );
    assert!(
        shared.contains("pub(crate) fn java_pattern_matches"),
        "{shared}"
    );
    assert!(
        shared.contains("pub(crate) fn java_pattern_simple_regex_literal"),
        "{shared}"
    );
    let shared_type_names = fs::read_to_string(shared_types).unwrap();
    for helper in [
        "java_string_buffer_set_length",
        "java_string_buffer_byte_index",
        "java_string_buffer_multiline_helper",
        "java_existing_shared_helper",
        "java_already_shared_helper",
        "java_pattern_matches",
        "java_pattern_simple_regex_literal",
        "JAVA_STRING_BUFFER_LIMIT",
        "JAVA_STRING_BUFFER_OFFSET",
    ] {
        assert!(
            shared_type_names.lines().any(|name| name == helper),
            "missing {helper} from shared import manifest:\n{shared_type_names}"
        );
    }

    for file in ["00001_UseA.rs", "00002_UseB.rs"] {
        let source = fs::read_to_string(generated_dir.join(file)).unwrap();
        assert!(!source.contains("pub struct StringBuffer"), "{source}");
        assert!(!source.contains("pub struct Pattern"), "{source}");
        assert!(
            !source.contains("fn java_string_buffer_set_length"),
            "{source}"
        );
        assert!(!source.contains("fn java_pattern_matches"), "{source}");
        assert!(
            !source.contains("const JAVA_STRING_BUFFER_LIMIT"),
            "{source}"
        );
        assert!(
            !source.contains("static JAVA_STRING_BUFFER_OFFSET"),
            "{source}"
        );
    }
}

#[test]
fn shared_support_manifest_restores_single_module_trait_and_impls() {
    let dir = unique_temp_dir("shared_support_manifest_single_module");
    let generated_dir = dir.join("generated");
    fs::create_dir_all(&generated_dir).unwrap();
    fs::write(
        generated_dir.join("00001_UseValue.rs"),
        r#"
pub trait SharedValue {
    fn value(&self) -> i32;
}

impl SharedValue for i32 {
    fn value(&self) -> i32 { *self }
}

pub struct UseValue;
"#,
    )
    .unwrap();

    let shared_support = dir.join("shared.rs");
    let shared_types = dir.join("shared-types.tsv");
    let conflicts = dir.join("conflicts.tsv");
    fs::write(&shared_support, "").unwrap();
    fs::write(&shared_types, "SharedValue\n").unwrap();
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
    assert!(shared.contains("pub trait SharedValue"), "{shared}");
    assert!(shared.contains("impl SharedValue for i32"), "{shared}");
    let module = fs::read_to_string(generated_dir.join("00001_UseValue.rs")).unwrap();
    assert!(!module.contains("pub trait SharedValue"), "{module}");
    assert!(!module.contains("impl SharedValue for i32"), "{module}");
}

#[test]
fn shared_support_seed_closes_nominal_trait_dependencies() {
    let dir = unique_temp_dir("shared_support_seed_nominal_closure");
    let generated_dir = dir.join("generated");
    fs::create_dir_all(&generated_dir).unwrap();
    fs::write(
        generated_dir.join("00001_UseValue.rs"),
        r#"
pub trait SharedValue {
    fn value(&self) -> i32;
}

impl SharedValue for i32 {
    fn value(&self) -> i32 { *self }
}

pub struct UseValue;
"#,
    )
    .unwrap();

    let shared_support = dir.join("shared.rs");
    let shared_types = dir.join("shared-types.tsv");
    let conflicts = dir.join("conflicts.tsv");
    fs::write(
        &shared_support,
        "pub fn read_shared<T: SharedValue>(value: &T) -> i32 { value.value() }\n",
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
    assert!(shared.contains("pub trait SharedValue"), "{shared}");
    assert!(shared.contains("impl SharedValue for i32"), "{shared}");
    let shared_names = fs::read_to_string(shared_types).unwrap();
    assert!(
        shared_names.lines().any(|name| name == "SharedValue"),
        "{shared_names}"
    );
}

#[test]
fn promoted_helper_closes_its_nominal_trait_dependencies() {
    let dir = unique_temp_dir("promoted_helper_nominal_closure");
    let generated_dir = dir.join("generated");
    fs::create_dir_all(&generated_dir).unwrap();
    fs::write(
        generated_dir.join("00001_UseValue.rs"),
        r#"
pub trait SharedValue {
    fn value(&self) -> i32;
}

impl SharedValue for i32 {
    fn value(&self) -> i32 { *self }
}

pub fn read_shared<T: SharedValue>(value: &T) -> i32 { value.value() }

pub struct UseValue;

impl UseValue {
    pub fn read() -> i32 { read_shared(&7) }
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

    let shared = fs::read_to_string(shared_support).unwrap();
    assert!(shared.contains("pub fn read_shared"), "{shared}");
    assert!(shared.contains("pub trait SharedValue"), "{shared}");
    assert!(shared.contains("impl SharedValue for i32"), "{shared}");
    let shared_names = fs::read_to_string(shared_types).unwrap();
    for name in ["read_shared", "SharedValue"] {
        assert!(
            shared_names.lines().any(|candidate| candidate == name),
            "missing {name}:\n{shared_names}"
        );
    }
}

#[test]
fn nominal_closure_keeps_aliased_nested_source_type_in_its_module() {
    let dir = unique_temp_dir("nominal_closure_nested_source_owner");
    let generated_dir = dir.join("generated");
    fs::create_dir_all(&generated_dir).unwrap();
    fs::write(
        generated_dir.join("00001_Outer.rs"),
        r#"
pub struct Outer_Child<L, K>(std::marker::PhantomData<(L, K)>);
pub type Child<L, K> = Outer_Child<L, K>;

impl<L, K> std::fmt::Display for Outer_Child<L, K> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("child")
    }
}

pub fn nested_value() -> Outer_Child<i32, i32> { Outer_Child(std::marker::PhantomData) }

pub struct Outer;

impl Outer {
    pub fn child() -> Outer_Child<i32, i32> { nested_value() }
}
"#,
    )
    .unwrap();

    let shared_support = dir.join("shared.rs");
    let shared_types = dir.join("shared-types.tsv");
    let conflicts = dir.join("conflicts.tsv");
    // Model an earlier assembly phase that seeded a summary facade but failed
    // to add it to the shared-type manifest. Structural source ownership must
    // still override both the stale declaration and all associated impls.
    fs::write(
        &shared_support,
        r#"
pub struct Outer_Child<L, K>(std::marker::PhantomData<(L, K)>);
pub type Child = Outer_Child;
impl<L, K> Outer_Child<L, K> {
    pub fn summary_only(&self) -> i32 { 0 }
}
"#,
    )
    .unwrap();
    fs::write(&shared_types, "Child\nOuter_Child\n").unwrap();
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
    assert!(output.status.success(), "{:?}", output);

    let shared = fs::read_to_string(shared_support).unwrap();
    assert!(!shared.contains("pub struct Outer_Child"), "{shared}");
    assert!(!shared.contains("pub type Child"), "{shared}");
    assert!(!shared.contains("summary_only"), "{shared}");
    let shared_names = fs::read_to_string(shared_types).unwrap();
    assert!(!shared_names.lines().any(|name| name == "Child"));
    let module = fs::read_to_string(generated_dir.join("00001_Outer.rs")).unwrap();
    assert!(module.contains("pub struct Outer_Child"), "{module}");
    assert!(
        module.contains("pub type Child<L, K> = Outer_Child<L, K>"),
        "{module}"
    );
    assert!(
        module.contains("impl<L, K> std::fmt::Display for Outer_Child<L, K>"),
        "{module}"
    );
}

#[test]
fn raw_source_ownership_preserves_nested_impls_after_structural_alias_is_stripped() {
    let dir = unique_temp_dir("raw_nested_source_ownership");
    let generated_dir = dir.join("generated");
    let raw_dir = dir.join("raw");
    fs::create_dir_all(&generated_dir).unwrap();
    fs::create_dir_all(&raw_dir).unwrap();

    let raw = r#"
pub struct Outer_Child;
pub type Child = Outer_Child;

impl std::fmt::Display for Outer_Child {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("child")
    }
}

pub struct Outer;
"#;
    fs::write(raw_dir.join("00001_Outer.rs"), raw).unwrap();
    // Earlier assembly passes are allowed to remove compatibility aliases.
    // The raw translator output remains the authoritative ownership record.
    fs::write(
        generated_dir.join("00001_Outer.rs"),
        raw.replace("pub type Child = Outer_Child;\n", ""),
    )
    .unwrap();

    let shared_support = dir.join("shared.rs");
    let shared_types = dir.join("shared-types.tsv");
    let conflicts = dir.join("conflicts.tsv");
    fs::write(&shared_support, "").unwrap();
    fs::write(&shared_types, "Outer_Child\n").unwrap();
    let script =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/merge-shared-support-facades.py");
    let output = Command::new("python3")
        .arg(script)
        .arg("--generated-dir")
        .arg(&generated_dir)
        .arg("--merge-input-dir")
        .arg(&raw_dir)
        .arg("--shared-support")
        .arg(&shared_support)
        .arg("--shared-types")
        .arg(&shared_types)
        .arg("--conflicts")
        .arg(&conflicts)
        .output()
        .expect("run shared support merger");
    assert!(output.status.success(), "{:?}", output);

    let shared = fs::read_to_string(shared_support).unwrap();
    assert!(!shared.contains("Outer_Child"), "{shared}");
    let module = fs::read_to_string(generated_dir.join("00001_Outer.rs")).unwrap();
    assert!(module.contains("pub struct Outer_Child"), "{module}");
    assert!(
        module.contains("impl std::fmt::Display for Outer_Child"),
        "{module}"
    );
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
    fn factor(&self) -> Box<dyn std::any::Any>;
}

impl JavaErasedObjectFacade for Box<dyn std::any::Any> {
    fn close(&self) -> Result<(), JavaException> { Ok(()) }
    fn factor(&self) -> Box<dyn std::any::Any> { panic!() }
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
    fn factor(&self) -> ConcreteProduct;
}

impl JavaErasedObjectFacade for Box<dyn std::any::Any> {
    fn length(&self) -> () { () }
    fn factor(&self) -> ConcreteProduct { panic!() }
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
        shared
            .matches("fn factor(&self) -> Box<dyn std::any::Any>")
            .count()
            == 2,
        "merged trait and impl must share the same erased ABI:\n{shared}"
    );
    assert!(
        !shared.contains("fn factor(&self) -> ConcreteProduct"),
        "{shared}"
    );
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
fn nested_leaf_alias_does_not_replace_existing_shared_facade() {
    let dir = unique_temp_dir("shared_support_nested_leaf_shared_collision");
    let generated_dir = dir.join("generated");
    fs::create_dir_all(&generated_dir).unwrap();
    fs::write(
        generated_dir.join("00001_Owner.rs"),
        r#"
pub struct Double;

impl Double {
    pub fn double_to_long_bits(value: f64) -> i64 { value.to_bits() as i64 }
}

pub struct Owner_Double;
pub type Double = Owner_Double;
pub struct Owner;
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

    let shared = fs::read_to_string(&shared_support).unwrap();
    assert!(shared.contains("pub struct Double"), "{shared}");
    assert!(shared.contains("double_to_long_bits"), "{shared}");
    assert!(!shared.contains("type Double = Owner_Double"), "{shared}");
    assert!(
        fs::read_to_string(&shared_types)
            .unwrap()
            .lines()
            .any(|name| name == "Double")
    );

    let module = fs::read_to_string(generated_dir.join("00001_Owner.rs")).unwrap();
    assert!(module.contains("pub struct Owner_Double"), "{module}");
    assert!(
        !module.contains("pub type Double = Owner_Double"),
        "{module}"
    );
    assert!(!module.contains("pub fn double_to_long_bits"), "{module}");
}

#[test]
fn shared_support_merger_replaces_fallback_impl_with_richer_representation() {
    let dir = unique_temp_dir("shared_support_richer_representation");
    let generated_dir = dir.join("generated");
    fs::create_dir_all(&generated_dir).unwrap();
    fs::write(
        generated_dir.join("00001_UseState.rs"),
        r#"
pub struct State { value: i32 }

impl State {
    pub fn new(value: i32) -> State { State { value } }
    pub fn value(&self) -> i32 { self.value }
}

pub struct UseState;
"#,
    )
    .unwrap();
    fs::write(
        generated_dir.join("00002_UseStateAgain.rs"),
        r#"
pub struct State { value: i32 }

impl State {
    pub fn new(value: i32) -> State { State { value } }
    pub fn value(&self) -> i32 { self.value }
}

pub struct UseStateAgain;
"#,
    )
    .unwrap();

    let shared_support = dir.join("shared.rs");
    let shared_types = dir.join("shared-types.tsv");
    let conflicts = dir.join("conflicts.tsv");
    fs::write(
        &shared_support,
        r#"
pub struct State;

impl State {
    pub fn new(value: i32) -> State { let _ = value; State }
}
"#,
    )
    .unwrap();
    fs::write(&shared_types, "State\n").unwrap();

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
    assert_eq!(shared.matches("pub struct State").count(), 1, "{shared}");
    assert_eq!(shared.matches("pub fn new").count(), 1, "{shared}");
    assert!(shared.contains("State { value }"), "{shared}");
    assert!(!shared.contains("let _ = value"), "{shared}");
    assert!(shared.contains("pub fn value(&self)"), "{shared}");
}

#[test]
fn shared_support_merger_removes_facade_shadowed_by_primary_source_type() {
    let dir = unique_temp_dir("shared_support_primary_source_shadow");
    let generated_dir = dir.join("generated");
    fs::create_dir_all(&generated_dir).unwrap();
    fs::write(
        generated_dir.join("00001_IOUtils.rs"),
        r#"
pub struct IOUtils;

impl IOUtils {
    pub const DEFAULT_BUFFER_SIZE: i32 = 4096;
    pub fn copy() -> i64 { 1 }
}
"#,
    )
    .unwrap();
    fs::write(generated_dir.join("00002_UseIO.rs"), "pub struct UseIO;\n").unwrap();

    let shared_support = dir.join("shared.rs");
    let shared_types = dir.join("shared-types.tsv");
    let conflicts = dir.join("conflicts.tsv");
    fs::write(
        &shared_support,
        r#"
pub struct IOUtils;

impl IOUtils {
    pub fn get_path<T>(_path: T) {}
}
"#,
    )
    .unwrap();
    fs::write(&shared_types, "IOUtils\n").unwrap();

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
    let shared_types = fs::read_to_string(shared_types).unwrap();
    assert!(!shared.contains("struct IOUtils"), "{shared}");
    assert!(!shared.contains("impl IOUtils"), "{shared}");
    assert!(!shared_types.lines().any(|line| line == "IOUtils"));
    let source = fs::read_to_string(generated_dir.join("00001_IOUtils.rs")).unwrap();
    assert!(source.contains("DEFAULT_BUFFER_SIZE"), "{source}");
}

#[test]
fn shared_support_merger_removes_alias_that_shadows_source_declaration() {
    let dir = unique_temp_dir("shared_support_source_shadowing_alias");
    let generated_dir = dir.join("generated");
    fs::create_dir_all(&generated_dir).unwrap();
    fs::write(
        generated_dir.join("00001_Primary.rs"),
        r#"
pub struct QualifiedPrimary;
pub type Primary = QualifiedPrimary;

pub struct Primary { value: i32 }

impl Primary {
    pub fn value(&self) -> i32 { self.value }
}
"#,
    )
    .unwrap();
    fs::write(
        generated_dir.join("00002_UsePrimary.rs"),
        r#"
pub struct QualifiedPrimary;
pub type Primary = QualifiedPrimary;

pub struct UsePrimary;
"#,
    )
    .unwrap();

    let shared_support = dir.join("shared.rs");
    let shared_types = dir.join("shared-types.tsv");
    let conflicts = dir.join("conflicts.tsv");
    fs::write(
        &shared_support,
        "pub struct QualifiedPrimary;\npub type Primary = QualifiedPrimary;\n",
    )
    .unwrap();
    fs::write(&shared_types, "Primary\nQualifiedPrimary\n").unwrap();

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
    assert!(!shared.contains("pub type Primary ="), "{shared}");
    assert!(
        !fs::read_to_string(&shared_types)
            .unwrap()
            .lines()
            .any(|line| line == "Primary")
    );

    let primary = fs::read_to_string(generated_dir.join("00001_Primary.rs")).unwrap();
    assert!(!primary.contains("pub type Primary ="), "{primary}");
    assert!(
        primary.contains("pub struct Primary { value: i32 }"),
        "{primary}"
    );
    assert!(primary.contains("impl Primary"), "{primary}");

    let user = fs::read_to_string(generated_dir.join("00002_UsePrimary.rs")).unwrap();
    assert!(!user.contains("pub type Primary ="), "{user}");
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

impl<T: Clone, M: Iterator<Item = T>> SharedBox<(T, M)> {
    pub fn new(value: T) -> SharedBox<T> {
        SharedBox { value: Some((value, std::iter::empty())) }
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
    assert!(
        shared.contains("impl<T: Clone, M: Iterator<Item = T>> SharedBox<(T, M)> {"),
        "{shared}"
    );
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
fn shared_support_merger_reconciles_raw_surface_into_seeded_facade() {
    let dir = unique_temp_dir("shared_support_seed_surface_reconciliation");
    let generated_dir = dir.join("generated");
    let raw_dir = dir.join("raw");
    fs::create_dir_all(&generated_dir).unwrap();
    fs::create_dir_all(&raw_dir).unwrap();

    let raw_support = r#"
#[derive(Clone, Debug, Default)]
pub struct ClassLoader;

impl ClassLoader {
    pub fn get_resource<S: Into<String>>(&self, _path: S) -> Option<InputStream> {
        Some(InputStream::default())
    }

    pub fn get_system_resource_as_stream<S: Into<String>>(_path: S) -> InputStream {
        InputStream::default()
    }
}

pub type Properties = std::collections::HashMap<String, String>;

pub trait JavaPropertiesExt {
    fn load(&mut self, stream: InputStream) -> Result<(), JavaException>;
}

impl JavaPropertiesExt for Properties {
    fn load(&mut self, _stream: InputStream) -> Result<(), JavaException> {
        Ok(())
    }
}
"#;
    for (file, primary) in [("00001_UseOne.rs", "UseOne"), ("00002_UseTwo.rs", "UseTwo")] {
        fs::write(
            raw_dir.join(file),
            format!("{raw_support}\npub struct {primary};\n"),
        )
        .unwrap();
        // This is the post-strip assembly input: the merger must source facade
        // surfaces from the raw backup while leaving only primary code here.
        fs::write(generated_dir.join(file), format!("pub struct {primary};\n")).unwrap();
    }

    let shared_support = dir.join("shared.rs");
    let shared_types = dir.join("shared-types.tsv");
    let conflicts = dir.join("conflicts.tsv");
    fs::write(
        &shared_support,
        r#"
#[derive(Clone, Debug, Default)]
pub struct ClassLoader;

impl ClassLoader {
    pub fn get_resource<S: Into<String>>(&self, _path: S) -> Option<InputStream> {
        Some(InputStream::default())
    }
}
"#,
    )
    .unwrap();
    fs::write(&shared_types, "ClassLoader\n").unwrap();

    let script =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/merge-shared-support-facades.py");
    let output = Command::new("python3")
        .arg(script)
        .arg("--generated-dir")
        .arg(&generated_dir)
        .arg("--merge-input-dir")
        .arg(&raw_dir)
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
        shared.matches("pub struct ClassLoader;").count(),
        1,
        "{shared}"
    );
    assert_eq!(
        shared.matches("pub fn get_resource<").count(),
        1,
        "{shared}"
    );
    assert!(
        shared.contains("pub fn get_system_resource_as_stream<"),
        "{shared}"
    );
    assert!(shared.contains("pub trait JavaPropertiesExt"), "{shared}");
    assert!(
        shared.contains("pub type Properties = std::collections::HashMap<String, String>;"),
        "{shared}"
    );
    assert!(
        shared.contains("impl JavaPropertiesExt for Properties"),
        "{shared}"
    );
    let shared_types = fs::read_to_string(shared_types).unwrap();
    assert!(shared_types.lines().any(|line| line == "JavaPropertiesExt"));
}

#[test]
fn shared_support_merger_keeps_struct_constructors_with_richer_declaration() {
    let dir = unique_temp_dir("shared_support_struct_representation_family");
    let generated_dir = dir.join("generated");
    let raw_dir = dir.join("raw");
    fs::create_dir_all(&generated_dir).unwrap();
    fs::create_dir_all(&raw_dir).unwrap();

    let rich_support = r#"
#[derive(Clone, Default)]
pub struct InputStream {
    bytes: Option<Vec<u8>>,
    position: Option<usize>,
    marked_position: Option<usize>,
    closed: Option<bool>,
    error_message: Option<String>,
}

impl InputStream {
    pub const fn empty() -> InputStream {
        InputStream { bytes: None, position: None, marked_position: None, closed: None, error_message: None }
    }

    pub fn from_bytes(bytes: Vec<u8>) -> InputStream {
        InputStream { bytes: Some(bytes), position: Some(0), marked_position: Some(0), closed: Some(false), error_message: None }
    }
}

#[derive(Clone)]
pub struct JavaException {
    class_name: &'static str,
    message: String,
    cause: Option<Box<JavaException>>,
    suppressed: Vec<JavaException>,
}

impl JavaException {
    pub fn new(class_name: &'static str, message: String) -> JavaException {
        JavaException { class_name, message, cause: None, suppressed: Vec::new() }
    }

    pub fn from_display(class_name: &'static str, err: impl std::fmt::Display) -> JavaException {
        JavaException { class_name, message: err.to_string(), cause: None, suppressed: Vec::new() }
    }
}
"#;
    for (file, primary) in [("00001_UseOne.rs", "UseOne"), ("00002_UseTwo.rs", "UseTwo")] {
        fs::write(
            raw_dir.join(file),
            format!("{rich_support}\npub struct {primary};\n"),
        )
        .unwrap();
        fs::write(generated_dir.join(file), format!("pub struct {primary};\n")).unwrap();
    }

    let shared_support = dir.join("shared.rs");
    let shared_types = dir.join("shared-types.tsv");
    let conflicts = dir.join("conflicts.tsv");
    fs::write(
        &shared_support,
        r#"
#[derive(Clone, Default)]
pub struct InputStream {
    bytes: Option<Vec<u8>>,
    position: Option<usize>,
    closed: Option<bool>,
}

impl InputStream {
    pub const fn empty() -> InputStream {
        InputStream { bytes: None, position: None, closed: None }
    }

    pub fn from_bytes(bytes: Vec<u8>) -> InputStream {
        InputStream { bytes: Some(bytes), position: Some(0), closed: Some(false) }
    }
}

#[derive(Clone)]
pub struct JavaException {
    class_name: &'static str,
    message: String,
    cause: Option<Box<JavaException>>,
}

impl JavaException {
    pub fn new(class_name: &'static str, message: String) -> JavaException {
        JavaException { class_name, message, cause: None }
    }

    pub fn from_display(class_name: &'static str, err: impl std::fmt::Display) -> JavaException {
        JavaException { class_name, message: err.to_string(), cause: None }
    }
}
"#,
    )
    .unwrap();
    fs::write(&shared_types, "InputStream\nJavaException\n").unwrap();

    let script =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/merge-shared-support-facades.py");
    let output = Command::new("python3")
        .arg(script)
        .arg("--generated-dir")
        .arg(&generated_dir)
        .arg("--merge-input-dir")
        .arg(&raw_dir)
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
        shared.contains("marked_position: Option<usize>"),
        "{shared}"
    );
    assert!(shared.contains("error_message: Option<String>"), "{shared}");
    assert!(
        shared.contains("suppressed: Vec<JavaException>"),
        "{shared}"
    );
    assert_eq!(
        shared.matches("marked_position: None").count(),
        1,
        "{shared}"
    );
    assert_eq!(
        shared.matches("suppressed: Vec::new()").count(),
        2,
        "{shared}"
    );

    let compiled = Command::new("rustc")
        .args(["--edition=2024", "--crate-type=lib"])
        .arg(&shared_support)
        .arg("-o")
        .arg(dir.join("shared.rlib"))
        .output()
        .expect("compile merged struct support");
    assert!(
        compiled.status.success(),
        "merged struct support did not compile: {}\n{shared}",
        String::from_utf8_lossy(&compiled.stderr)
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
    fn from(_value: ZipOutputStream) -> Self { panic!("unsupported Java external upcast: ZipOutputStream -> OutputStream") }
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
            "noop_facade_call_sites",
            new_java2rust::output_markers::count_noop_facade_call_sites(&rust),
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

#[test]
fn python_noop_facade_call_site_sweep_ignores_prefixes_comments_and_strings() {
    let rust = r#"
impl Font {
    pub fn new_3<T0, T1, T2>(_arg0: T0, _arg1: T1, _arg2: T2) -> Font { Default::default() }
}

pub fn build() -> Font {
    // Font::new_3("comment", 0, 12)
    let _debug = "Font::new_3(\"string\", 0, 12)";
    let _other = MyFont::new_3("prefix", 0, 12);
    Font::new_3("real", 0, 12)
}
"#;

    let dir = unique_temp_dir("sweep_noop_facade_boundaries");
    fs::write(dir.join("generated.rs"), rust).unwrap();
    let swept = run_idiom_sweep(&dir);

    assert_eq!(
        swept.get("noop_facade_call_sites").copied(),
        Some(1),
        "the Python sweep should count only the real qualified constructor call"
    );
}

#[test]
fn idiom_sweep_includes_shared_support_file_signals() {
    let dir = unique_temp_dir("idiom_sweep_shared_support");
    let generated = dir.join("generated");
    fs::create_dir_all(&generated).unwrap();
    fs::write(generated.join("generated.rs"), "pub fn local() {}\n").unwrap();
    let shared_support = dir.join("generated-shared-support.rs");
    fs::write(
        &shared_support,
        r#"
impl From<ZipOutputStream> for OutputStream {
    /// Unmodelled Java upcast: ZipOutputStream -> OutputStream.
    fn from(_value: ZipOutputStream) -> Self { panic!("unsupported Java external upcast: ZipOutputStream -> OutputStream") }
}
"#,
    )
    .unwrap();

    let without_support = run_idiom_sweep(&generated);
    assert_eq!(
        without_support
            .get("unmodelled_upcast_conversions")
            .copied()
            .unwrap_or(0),
        0,
        "the generated root alone should not see support-only markers"
    );

    let with_support = run_idiom_sweep_with_extra(&generated, &[&shared_support]);
    assert_eq!(
        with_support.get("unmodelled_upcast_conversions").copied(),
        Some(1),
        "support-only markers must be included in focused-package sweep metrics"
    );
}

#[test]
fn shared_support_merger_emits_richer_replacement_for_conversion_target() {
    let dir = unique_temp_dir("shared_support_richer_conversion_target");
    let generated_dir = dir.join("generated");
    fs::create_dir_all(&generated_dir).unwrap();
    let generated = r#"
#[derive(Clone, Default)]
pub struct SharedState {
    first: i32,
    second: i32,
}

impl From<SourceState> for SharedState {
    fn from(_value: SourceState) -> Self { SharedState::default() }
}

pub struct SOURCE_TYPE;
"#;
    fs::write(
        generated_dir.join("00001_UseA.rs"),
        generated.replace("SOURCE_TYPE", "UseA"),
    )
    .unwrap();
    fs::write(
        generated_dir.join("00002_UseB.rs"),
        generated.replace("SOURCE_TYPE", "UseB"),
    )
    .unwrap();

    let shared_support = dir.join("shared.rs");
    fs::write(
        &shared_support,
        r#"
#[derive(Clone, Default)]
pub struct SharedState {
    first: i32,
}
"#,
    )
    .unwrap();
    let shared_types = dir.join("shared-types.tsv");
    fs::write(&shared_types, "SharedState\n").unwrap();
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
    assert_eq!(
        shared.matches("pub struct SharedState").count(),
        1,
        "{shared}"
    );
    assert!(shared.contains("second: i32"), "{shared}");
    assert!(
        shared.contains("impl From<SourceState> for SharedState"),
        "{shared}"
    );
}

#[test]
fn shared_support_merger_keeps_scalar_impls_for_a_promoted_trait() {
    let dir = unique_temp_dir("shared_support_scalar_trait_impls");
    let generated_dir = dir.join("generated");
    fs::create_dir_all(&generated_dir).unwrap();
    let support = r#"
pub trait DeepElement {
    fn render(&self) -> String;
}

impl DeepElement for bool {
    fn render(&self) -> String { self.to_string() }
}

impl DeepElement for i32 {
    fn render(&self) -> String { self.to_string() }
}

pub struct SOURCE;
"#;
    fs::write(
        generated_dir.join("00001_UseA.rs"),
        support.replace("SOURCE", "UseA"),
    )
    .unwrap();
    fs::write(
        generated_dir.join("00002_UseB.rs"),
        support.replace("SOURCE", "UseB"),
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
    assert!(shared.contains("impl DeepElement for bool"), "{shared}");
    assert!(shared.contains("impl DeepElement for i32"), "{shared}");
}

#[test]
fn shared_support_merger_preserves_monitor_state_and_qualified_sync_types() {
    let dir = unique_temp_dir("shared_support_monitor_dependencies");
    let generated_dir = dir.join("generated");
    fs::create_dir_all(&generated_dir).unwrap();
    let support = r#"
#[derive(Default)]
pub struct JavaMonitorState {
    owner: Option<std::thread::ThreadId>,
    depth: usize,
}

pub struct JavaMonitor {
    state: std::sync::Mutex<JavaMonitorState>,
    changed: std::sync::Condvar,
}

impl JavaMonitor {
    pub const fn new() -> Self {
        Self {
            state: std::sync::Mutex::new(JavaMonitorState { owner: None, depth: 0 }),
            changed: std::sync::Condvar::new(),
        }
    }
}

pub struct SOURCE;
"#;
    fs::write(
        generated_dir.join("00001_UseA.rs"),
        support.replace("SOURCE", "UseA"),
    )
    .unwrap();
    fs::write(
        generated_dir.join("00002_UseB.rs"),
        support.replace("SOURCE", "UseB"),
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
    assert!(shared.contains("pub struct JavaMonitorState"), "{shared}");
    assert!(
        shared.contains("state: std::sync::Mutex<JavaMonitorState>"),
        "{shared}"
    );
    assert!(shared.contains("changed: std::sync::Condvar"), "{shared}");

    let compiled = Command::new("rustc")
        .args(["--edition=2024", "--crate-type=lib"])
        .arg(&shared_support)
        .arg("-o")
        .arg(dir.join("shared.rlib"))
        .output()
        .expect("compile merged monitor support");
    assert!(
        compiled.status.success(),
        "merged monitor support did not compile: {}\n{shared}",
        String::from_utf8_lossy(&compiled.stderr)
    );
}

#[test]
fn shared_support_merger_prefers_complete_forwarding_method_generic_bounds() {
    let dir = unique_temp_dir("shared_support_forwarding_generic_bounds");
    let generated_dir = dir.join("generated");
    fs::create_dir_all(&generated_dir).unwrap();
    fs::write(
        generated_dir.join("00001_UseA.rs"),
        r#"
pub struct ForwardValue;

impl ForwardValue {
    pub fn compute<O: ObservationBound + Clone>(&mut self, value: O) { let _ = value; }
}

pub struct UseA;
"#,
    )
    .unwrap();
    fs::write(
        generated_dir.join("00002_UseB.rs"),
        r#"
pub struct ForwardValue;

impl ForwardValue {
    pub fn compute<O: Clone>(&mut self, value: O) { let _ = value; }
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
        shared.contains("pub fn compute<O: ObservationBound + Clone>"),
        "{shared}"
    );
    assert!(!shared.contains("pub fn compute<O: Clone>"), "{shared}");
}
