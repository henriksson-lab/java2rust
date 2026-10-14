use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn temp_dir(label: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "new_java2rust_{label}_{}_{}",
        std::process::id(),
        std::thread::current().name().unwrap_or("test")
    ));
    fs::create_dir_all(&path).unwrap();
    path
}

fn run_merger(dir: &Path) -> (PathBuf, PathBuf) {
    let generated = dir.join("generated");
    let shared = dir.join("shared.rs");
    let shared_types = dir.join("shared-types.tsv");
    let conflicts = dir.join("conflicts.tsv");
    let mut command = Command::new("python3");
    command
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/merge-shared-support-facades.py"))
        .arg("--generated-dir")
        .arg(&generated);
    let raw = dir.join("raw");
    if raw.exists() {
        command.arg("--merge-input-dir").arg(raw);
    }
    let output = command
        .arg("--shared-support")
        .arg(&shared)
        .arg("--shared-types")
        .arg(&shared_types)
        .arg("--conflicts")
        .arg(&conflicts)
        .output()
        .expect("run shared support merger");
    assert!(
        output.status.success(),
        "merger failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    (shared, shared_types)
}

#[test]
fn shared_nominal_closure_promotes_private_state_dependencies() {
    let dir = temp_dir("private_support_dependency");
    let generated = dir.join("generated");
    fs::create_dir_all(&generated).unwrap();
    let support = r#"
enum JavaSinkTarget { Memory(Vec<u8>) }

struct JavaSinkState { target: JavaSinkTarget }

#[derive(Clone)]
pub struct OutputStream {
    state: std::rc::Rc<std::cell::RefCell<JavaSinkState>>,
}

impl OutputStream {
    pub fn new() -> Self {
        Self { state: std::rc::Rc::new(std::cell::RefCell::new(JavaSinkState {
            target: JavaSinkTarget::Memory(Vec::new()),
        })) }
    }
}
"#;
    fs::write(
        generated.join("00001_UseA.rs"),
        format!("{support}\npub struct UseA;\n"),
    )
    .unwrap();
    fs::write(
        generated.join("00002_UseB.rs"),
        format!("{support}\npub struct UseB;\n"),
    )
    .unwrap();

    let (shared, shared_types) = run_merger(&dir);
    let shared = fs::read_to_string(shared).unwrap();
    assert!(shared.contains("pub struct OutputStream"), "{shared}");
    assert!(
        shared.contains("pub(crate) enum JavaSinkTarget"),
        "{shared}"
    );
    assert!(
        shared.contains("pub(crate) struct JavaSinkState"),
        "{shared}"
    );
    let names = fs::read_to_string(shared_types).unwrap();
    assert!(names.lines().any(|name| name == "JavaSinkTarget"));
    assert!(names.lines().any(|name| name == "JavaSinkState"));
    for module in ["00001_UseA.rs", "00002_UseB.rs"] {
        let module = fs::read_to_string(generated.join(module)).unwrap();
        assert!(!module.contains("struct JavaSinkState"), "{module}");
        assert!(!module.contains("enum JavaSinkTarget"), "{module}");
    }
}

#[test]
fn shared_merger_coalesces_repeated_declaration_and_impl_surfaces() {
    let dir = temp_dir("repeated_support_surface");
    let generated = dir.join("generated");
    fs::create_dir_all(&generated).unwrap();
    let duplicated = r#"
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct TimeUnit { nanos: i64 }
impl TimeUnit {
    pub const fn new(nanos: i64) -> Self { Self { nanos } }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct TimeUnit { nanos: i64 }
impl TimeUnit {
    pub const fn new(nanos: i64) -> Self { Self { nanos } }
}
"#;
    fs::write(
        generated.join("00001_UseA.rs"),
        format!("{duplicated}\npub struct UseA;\n"),
    )
    .unwrap();
    fs::write(
        generated.join("00002_UseB.rs"),
        format!("{duplicated}\npub struct UseB;\n"),
    )
    .unwrap();

    let (shared, _) = run_merger(&dir);
    let shared = fs::read_to_string(shared).unwrap();
    assert_eq!(shared.matches("pub struct TimeUnit").count(), 1, "{shared}");
    assert_eq!(shared.matches("impl TimeUnit").count(), 1, "{shared}");
    assert_eq!(shared.matches("pub const fn new").count(), 1, "{shared}");
    for module in ["00001_UseA.rs", "00002_UseB.rs"] {
        let module = fs::read_to_string(generated.join(module)).unwrap();
        assert!(!module.contains("pub struct TimeUnit"), "{module}");
        assert!(!module.contains("impl TimeUnit"), "{module}");
    }
}

#[test]
fn shared_merger_keeps_rc_and_arc_impls_with_their_concrete_source() {
    let dir = temp_dir("source_owned_wrapper_impls");
    let generated = dir.join("generated");
    fs::create_dir_all(&generated).unwrap();
    let trait_decl = r#"
pub trait Trimmer {
    fn trim(&self) -> i32;
}
"#;
    fs::write(
        generated.join("00001_LeadingTrimmer.rs"),
        format!(
            r#"{trait_decl}
pub struct LeadingTrimmer;

impl Trimmer for LeadingTrimmer {{
    fn trim(&self) -> i32 {{ 1 }}
}}

impl Trimmer for std::rc::Rc<LeadingTrimmer> {{
    fn trim(&self) -> i32 {{ self.as_ref().trim() }}
}}

impl Trimmer for std::sync::Arc<LeadingTrimmer> {{
    fn trim(&self) -> i32 {{ self.as_ref().trim() }}
}}
"#
        ),
    )
    .unwrap();
    fs::write(
        generated.join("00002_TrimmerUser.rs"),
        format!("{trait_decl}\npub struct TrimmerUser;\n"),
    )
    .unwrap();

    let (shared, _) = run_merger(&dir);
    let shared = fs::read_to_string(shared).unwrap();
    assert!(shared.contains("pub trait Trimmer"), "{shared}");
    assert!(!shared.contains("Rc<LeadingTrimmer>"), "{shared}");
    assert!(!shared.contains("Arc<LeadingTrimmer>"), "{shared}");

    let module = fs::read_to_string(generated.join("00001_LeadingTrimmer.rs")).unwrap();
    assert!(
        module.contains("impl Trimmer for std::rc::Rc<LeadingTrimmer>"),
        "{module}"
    );
    assert!(
        module.contains("impl Trimmer for std::sync::Arc<LeadingTrimmer>"),
        "{module}"
    );
}

#[test]
fn shared_merger_keeps_wrapper_impls_for_secondary_top_level_source_types() {
    let dir = temp_dir("secondary_source_wrapper_impls");
    let generated = dir.join("generated");
    let raw = dir.join("raw");
    fs::create_dir_all(&generated).unwrap();
    fs::create_dir_all(&raw).unwrap();
    let trait_decl = r#"
pub trait Parser {
    fn parse(&self) -> i32;
}
"#;
    let owner = format!(
        r#"{trait_decl}
pub struct Owner;

pub struct SecondaryParser;

impl Parser for SecondaryParser {{
    fn parse(&self) -> i32 {{ 1 }}
}}

impl Parser for std::sync::Arc<SecondaryParser> {{
    fn parse(&self) -> i32 {{ self.as_ref().parse() }}
}}
"#
    );
    // The package script retains this raw snapshot before its coarse support
    // stripping pass. Simulate that pass having removed only the wrapper impl;
    // the semantic merger must recover it beside its secondary source owner.
    fs::write(raw.join("00001_Owner.rs"), &owner).unwrap();
    fs::write(
        generated.join("00001_Owner.rs"),
        owner.replace(
            r#"
impl Parser for std::sync::Arc<SecondaryParser> {
    fn parse(&self) -> i32 { self.as_ref().parse() }
}
"#,
            "\n",
        ),
    )
    .unwrap();
    let user = format!(
        r#"{trait_decl}
#[derive(Clone, Default)]
pub struct SecondaryParser;
impl SecondaryParser {{
    pub fn new() -> SecondaryParser {{ SecondaryParser }}
}}

pub struct User;
"#
    );
    fs::write(generated.join("00002_User.rs"), &user).unwrap();
    fs::write(raw.join("00002_User.rs"), user).unwrap();

    let (shared, _) = run_merger(&dir);
    let shared = fs::read_to_string(shared).unwrap();
    assert!(shared.contains("pub trait Parser"), "{shared}");
    assert!(!shared.contains("pub struct SecondaryParser"), "{shared}");
    assert!(!shared.contains("Arc<SecondaryParser>"), "{shared}");

    let owner = fs::read_to_string(generated.join("00001_Owner.rs")).unwrap();
    assert!(owner.contains("pub struct SecondaryParser"), "{owner}");
    assert!(
        owner.contains("impl Parser for std::sync::Arc<SecondaryParser>"),
        "{owner}"
    );
    let user = fs::read_to_string(generated.join("00002_User.rs")).unwrap();
    assert!(!user.contains("pub struct SecondaryParser"), "{user}");
    assert!(!user.contains("impl SecondaryParser"), "{user}");

    // Assembly can invoke the merger with an already-populated shared
    // manifest after an earlier coarse stripping phase. Even when there is
    // nothing left to promote, the raw source snapshot remains authoritative
    // for source-owned wrapper coherence impls.
    fs::write(
        generated.join("00001_Owner.rs"),
        owner.replace(
            r#"
impl Parser for std::sync::Arc<SecondaryParser> {
    fn parse(&self) -> i32 { self.as_ref().parse() }
}
"#,
            "\n",
        ),
    )
    .unwrap();
    run_merger(&dir);
    let owner = fs::read_to_string(generated.join("00001_Owner.rs")).unwrap();
    assert!(
        owner.contains("impl Parser for std::sync::Arc<SecondaryParser>"),
        "{owner}"
    );
}

#[test]
fn shared_merger_preserves_macros_that_generate_promoted_trait_impls() {
    let dir = temp_dir("macro_generated_trait_impls");
    let generated = dir.join("generated");
    fs::create_dir_all(&generated).unwrap();
    let support = r#"
pub trait JavaDisplayElement {
    fn java_display(&self) -> String;
}

macro_rules! impl_java_display_element {
    ($($ty:ty),* $(,)?) => {
        $(
            impl JavaDisplayElement for $ty {
                fn java_display(&self) -> String { self.to_string() }
            }
        )*
    };
}

impl_java_display_element!(i32, String);
"#;
    fs::write(
        generated.join("00001_UseA.rs"),
        format!("{support}\npub struct UseA;\n"),
    )
    .unwrap();
    fs::write(
        generated.join("00002_UseB.rs"),
        format!("{support}\npub struct UseB;\n"),
    )
    .unwrap();

    let (shared, _) = run_merger(&dir);
    let shared = fs::read_to_string(shared).unwrap();
    assert_eq!(
        shared.matches("pub trait JavaDisplayElement").count(),
        1,
        "{shared}"
    );
    assert_eq!(
        shared
            .matches("macro_rules! impl_java_display_element")
            .count(),
        1,
        "{shared}"
    );
    assert_eq!(
        shared
            .matches("impl_java_display_element!(i32, String);")
            .count(),
        1,
        "{shared}"
    );
    for module in ["00001_UseA.rs", "00002_UseB.rs"] {
        let module = fs::read_to_string(generated.join(module)).unwrap();
        assert!(
            !module.contains("macro_rules! impl_java_display_element"),
            "{module}"
        );
        assert!(!module.contains("impl_java_display_element!("), "{module}");
    }
}

#[test]
fn shared_merger_does_not_duplicate_impls_supplied_by_an_existing_macro_family() {
    let dir = temp_dir("existing_macro_impl_family");
    let generated = dir.join("generated");
    fs::create_dir_all(&generated).unwrap();
    let support = r#"
pub trait JavaDisplayElement {
    fn java_display(&self) -> String;
}

macro_rules! local_display_impls {
    ($($ty:ty),* $(,)?) => {
        $(impl JavaDisplayElement for $ty {
            fn java_display(&self) -> String { self.to_string() }
        })*
    };
}
local_display_impls!(i32, String);
"#;
    for module in ["00001_UseA.rs", "00002_UseB.rs"] {
        fs::write(
            generated.join(module),
            format!("{support}\npub struct Use;\n"),
        )
        .unwrap();
    }
    fs::write(
        dir.join("shared.rs"),
        r#"pub trait JavaDisplayElement {
    fn java_display(&self) -> String;
}
macro_rules! shared_display_impls {
    ($trait_name:ident) => {
        impl $trait_name for i32 {
            fn java_display(&self) -> String { self.to_string() }
        }
        impl $trait_name for String {
            fn java_display(&self) -> String { self.to_string() }
        }
    };
}
shared_display_impls!(JavaDisplayElement);
"#,
    )
    .unwrap();

    let (shared, _) = run_merger(&dir);
    let shared = fs::read_to_string(shared).unwrap();
    assert_eq!(
        shared
            .matches("shared_display_impls!(JavaDisplayElement)")
            .count(),
        1,
        "{shared}"
    );
    assert!(
        !shared.contains("macro_rules! local_display_impls"),
        "{shared}"
    );
    assert!(!shared.contains("local_display_impls!("), "{shared}");
}
