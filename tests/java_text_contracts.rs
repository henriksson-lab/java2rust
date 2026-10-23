#[path = "common/mod.rs"]
mod common;
#[path = "../src/java_string_identity.rs"]
mod java_string_identity;
#[path = "../src/java_text.rs"]
mod java_text;
mod lossless_string {
    use crate::java_string_identity::JavaStringIdentity;
    use crate::java_text::{
        JavaCodeUnit, JavaStringSearchNeedle, JavaTextBounds, JavaTextView, JavaUtf16,
    };
    include!("../src/java_utf16_string.rs");
}

#[test]
fn lossless_runtime_assembly_promotes_one_identity_and_payload_contract() {
    use std::process::Command;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};
    let scratch = std::env::temp_dir().join(format!(
        "java2rust-text-assembly-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let generated = scratch.join("generated");
    std::fs::create_dir_all(&generated).unwrap();
    let runtime = new_java2rust::RuntimeSupport::LosslessJavaText.emit_rust();
    for name in ["Left", "Right"] {
        std::fs::write(generated.join(format!("{name}.rs")), format!(r#"{runtime}
pub struct {name};
impl {name} {{
    pub fn text() -> JavaUtf16String {{ JavaUtf16String::from_units(vec![0xd800,65,0xdc00]).intern() }}
    pub fn search_checks() {{
        let text = Self::text();
        let high = JavaUtf16String::from_units(vec![0xd800]);
        let low = JavaUtf16String::from_units(vec![0xdc00]);
        assert_eq!(java_string_length(&text), 3);
        assert_eq!(java_string_length(&high), 1);
        assert_eq!(java_string_index_of(&text, &high), 0);
        assert_eq!(java_string_last_index_of_from(&text, &low, 2), 2);
        assert!(java_string_contains(&text, &high));
        assert!(java_string_starts_with(&text, &high));
        assert!(java_string_ends_with(&text, &low));
        assert_eq!(java_string_index_of(&text, 0xdc00i32), 2);
    }}
}}
"#)).unwrap();
    }
    let shared = scratch.join("shared.rs");
    let types = scratch.join("types.tsv");
    std::fs::write(&types, "").unwrap();
    let assembled = common::run_with_watchdog(
        Command::new("python3")
            .arg(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("scripts/merge-shared-support-facades.py"),
            )
            .arg("--generated-dir")
            .arg(&generated)
            .arg("--shared-support")
            .arg(&shared)
            .arg("--shared-types")
            .arg(&types)
            .arg("--conflicts")
            .arg(scratch.join("conflicts.tsv")),
        Duration::from_secs(30),
    );
    assert!(
        assembled.status.is_some_and(|s| s.success()),
        "{}",
        assembled.stderr
    );
    let support = std::fs::read_to_string(&shared).unwrap();
    for name in [
        "JavaStringIdentity",
        "JavaUtf16",
        "JavaUtf16String",
        "JavaCodeUnit",
        "JavaTextBounds",
    ] {
        assert!(
            support.contains(&format!("pub struct {name}"))
                || support.contains(&format!("pub enum {name}")),
            "{name}"
        );
    }
    assert!(support.contains("pub trait JavaTextView"));
    let source = format!(
        "{support}\nmod left {{ use crate::*; {} }}\nmod right {{ use crate::*; {} }}\nfn main() {{ let left: JavaUtf16String = left::Left::text(); let right: JavaUtf16String = right::Right::text(); assert!(left.java_identity_eq(&right)); assert_eq!(left.units(), &[0xd800,65,0xdc00]); assert!(left.scalar_string().is_err()); assert!(matches!(left.java_text_view(), std::borrow::Cow::Borrowed(_))); left::Left::search_checks(); right::Right::search_checks(); }}\n",
        std::fs::read_to_string(generated.join("Left.rs")).unwrap(),
        std::fs::read_to_string(generated.join("Right.rs")).unwrap()
    );
    std::fs::write(scratch.join("main.rs"), source).unwrap();
    let compiled = common::run_with_watchdog(
        Command::new("rustc")
            .arg("--edition=2024")
            .arg(scratch.join("main.rs"))
            .arg("-o")
            .arg(scratch.join("probe")),
        Duration::from_secs(30),
    );
    assert!(
        compiled.status.is_some_and(|s| s.success()),
        "{}",
        compiled.stderr
    );
    let run = common::run_with_watchdog(
        &mut Command::new(scratch.join("probe")),
        Duration::from_secs(30),
    );
    assert!(run.status.is_some_and(|s| s.success()), "{}", run.stderr);
    std::fs::remove_dir_all(scratch).unwrap();
}
mod caller {
    use crate::lossless_string::JavaUtf16String as Text;
    include!("corpus/numbers/JavaTextKernelOracle.rs");
}

#[test]
fn lossless_string_values_and_identity_match_independent_java_in_host_and_emitted_runtime() {
    use std::process::Command;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};
    let scratch = std::env::temp_dir().join(format!(
        "java2rust-text-kernel-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&scratch).unwrap();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let compiled = common::run_with_watchdog(
        Command::new("javac")
            .arg("-encoding")
            .arg("UTF-8")
            .arg("-d")
            .arg(&scratch)
            .arg(root.join("tests/corpus/numbers/JavaTextKernelOracle.java")),
        Duration::from_secs(30),
    );
    assert!(
        compiled.status.is_some_and(|s| s.success()),
        "{}",
        compiled.stderr
    );
    let java = common::run_with_watchdog(
        Command::new("java")
            .arg("-Xmx128m")
            .arg("-cp")
            .arg(&scratch)
            .arg("JavaTextKernelOracle"),
        Duration::from_secs(30),
    );
    assert!(java.status.is_some_and(|s| s.success()), "{}", java.stderr);
    let expected: Vec<_> = java.stdout.lines().collect();
    let observed = caller::observations();
    assert_eq!(expected.len(), 5077);
    assert_eq!(observed.len(), expected.len());
    for (index, (actual, expected)) in observed.iter().zip(&expected).enumerate() {
        assert_eq!(actual, expected, "host observation {index}");
    }
    let tests = format!(
        r#"
use JavaUtf16String as Text;
{}
#[test] fn emitted_runtime_matches_jdk() {{
    let expected: Vec<_> = {:?}.lines().collect();
    let observed = observations();
    assert_eq!(observed.len(), 5077);
    for (actual, expected) in observed.iter().zip(expected) {{ assert_eq!(actual, expected); }}
}}
"#,
        include_str!("corpus/numbers/JavaTextKernelOracle.rs"),
        java.stdout
    );
    common::cargo_verify_generated_modules_with_edition(
        "lossless_text_runtime_jdk",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: new_java2rust::RuntimeSupport::LosslessJavaText.emit_rust(),
            tests: Box::leak(tests.into_boxed_str()),
        }],
        "test",
        "2024",
    );
    // Arbitrary Java String values must not accidentally satisfy an implicit
    // scalar-text contract. This guards against "fixing" consumer compilation
    // with a lossy AsRef<str>, Deref<str> or Display implementation later.
    let runtime = new_java2rust::RuntimeSupport::LosslessJavaText.emit_rust();
    for (name, body, carrier) in [
        (
            "as_ref",
            "fn scalar(_: impl AsRef<str>) {} fn main() { scalar(JavaUtf16String::from_units(vec![0xd800])); }",
            "JavaUtf16String",
        ),
        (
            "deref",
            "fn main() { let value = JavaUtf16String::from_units(vec![0xd800]); let _: &str = &value; }",
            "JavaUtf16String",
        ),
        (
            "display",
            "fn main() { let value = JavaUtf16String::from_units(vec![0xd800]); let _ = format!(\"{}\", value); }",
            "JavaUtf16String",
        ),
        (
            "code_unit_display",
            "fn main() { let value = JavaCodeUnit(0xd800); let _ = format!(\"{}\", value); }",
            "JavaCodeUnit",
        ),
    ] {
        let source = scratch.join(format!("{name}.rs"));
        std::fs::write(&source, format!("{runtime}\n{body}")).unwrap();
        let rejected = common::run_with_watchdog(
            Command::new("rustc")
                .arg("--edition=2024")
                .arg("--emit=metadata")
                .arg(&source)
                .arg("-o")
                .arg(scratch.join(format!("{name}.rmeta"))),
            Duration::from_secs(30),
        );
        assert!(
            rejected.status.is_some_and(|s| !s.success()),
            "{name} must require explicit scalar interop"
        );
        assert!(rejected.stderr.contains(carrier), "{}", rejected.stderr);
        std::fs::remove_file(source).unwrap();
    }
    std::fs::remove_file(scratch.join("JavaTextKernelOracle.class")).unwrap();
    std::fs::remove_dir(&scratch).unwrap();
}
