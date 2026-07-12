use std::fs;
use std::path::Path;
use std::process::Command;

use new_java2rust::diagnostics::Severity;
use new_java2rust::fixtures::load_java_fixtures;
use new_java2rust::translate_source;

fn fixture(path: &str) -> String {
    fs::read_to_string(Path::new("examples/java").join(path)).unwrap()
}

#[test]
fn all_java_fixtures_are_loadable() {
    let fixtures = load_java_fixtures("examples/java").unwrap();
    assert_eq!(fixtures.len(), 36);
    assert!(
        fixtures
            .iter()
            .any(|f| f.relative_path.ends_with("04-objects/SimpleClass.java"))
    );
}

#[test]
fn translates_simple_class_shape() {
    let translation = translate_source(&fixture("04-objects/SimpleClass.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    assert!(translation.rust.contains("pub struct SimpleClass"));
    assert!(translation.rust.contains("name: String"));
    assert!(translation.rust.contains("count: i32"));
    assert!(
        translation
            .rust
            .contains("pub fn new(name: String) -> SimpleClass")
    );
    assert!(translation.rust.contains("pub fn increment(&mut self)"));
    assert!(translation.rust.contains("self.count += 1;"));
    assert!(
        translation
            .rust
            .contains("format!(\"{}:{}\", self.name, self.count)")
    );
}

#[test]
fn translates_expression_precedence_shape() {
    let translation = translate_source(&fixture("01-expressions/ExpressionPrecedence.java"));
    assert!(!translation.diagnostics.has_errors());
    assert!(translation.rust.contains("let mut x: i32 = (a + (b * c));"));
    assert!(translation.rust.contains("let mut y: i32 = ((a + b) * c);"));
    assert!(translation.rust.contains("let mut z: i32 = (x << 1);"));
    assert!(
        translation
            .rust
            .contains("return if (z > y) { (z - y) } else { (y - z) };")
    );
}

#[test]
fn translates_methods_with_overload_suffixes() {
    let translation = translate_source(&fixture("03-methods/MethodsAndOverloads.java"));
    assert!(!translation.diagnostics.has_errors());
    assert!(
        translation
            .rust
            .contains("pub fn clamp(value: i32, min: i32, max: i32) -> i32")
    );
    assert!(
        translation
            .rust
            .contains("pub fn format_int(&self, value: i32) -> String")
    );
    assert!(
        translation
            .rust
            .contains("pub fn format_string(&self, value: String) -> String")
    );
}

#[test]
fn guardrail_fixtures_emit_structured_unsupported_diagnostics() {
    let cases = [
        (
            "04-objects/InheritanceAndSuper.java",
            "unsupported_inheritance",
        ),
        (
            "04-objects/AnonymousInterface.java",
            "unsupported_interface",
        ),
        ("05-types/WildcardBounds.java", "unsupported_wildcard"),
        ("06-stdlib/StreamsUse.java", "unsupported_lambda"),
        ("06-stdlib/CheckedException.java", "unsupported_exceptions"),
    ];

    for (path, code) in cases {
        let translation = translate_source(&fixture(path));
        assert!(
            translation.diagnostics.has_code(code),
            "{path} missing {code}: {:?}",
            translation.diagnostics
        );
        assert!(
            translation
                .diagnostics
                .iter()
                .any(|d| matches!(d.severity, Severity::Unsupported))
        );
    }
}

#[test]
fn first_slice_generated_rust_compiles() {
    let cases = [
        "01-expressions/ExpressionPrecedence.java",
        "02-control-flow/LoopsAndBranches.java",
        "03-methods/MethodsAndOverloads.java",
        "04-objects/SimpleClass.java",
    ];

    for path in cases {
        let translation = translate_source(&fixture(path));
        assert!(
            !translation.diagnostics.has_errors(),
            "{path} diagnostics: {:?}",
            translation.diagnostics
        );
        let rust_path = Path::new("/tmp").join(format!(
            "new_java2rust_{}.rs",
            path.replace(['/', '.', '-'], "_")
        ));
        fs::write(&rust_path, translation.rust).unwrap();
        let output = Command::new("rustc")
            .arg("--crate-type")
            .arg("lib")
            .arg(&rust_path)
            .arg("-o")
            .arg(rust_path.with_extension("rlib"))
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{path} did not compile\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
