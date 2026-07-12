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
    assert_eq!(fixtures.len(), 38);
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
    assert!(translation.rust.contains("let x: i32 = (a + (b * c));"));
    assert!(translation.rust.contains("let y: i32 = ((a + b) * c);"));
    assert!(translation.rust.contains("let z: i32 = (x << 1);"));
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
fn translates_constructor_delegation_to_resolved_constructor_calls() {
    let translation = translate_source(&fixture("03-methods/ConstructorOverloads.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    assert!(
        !translation
            .rust
            .contains("unsupported Java statement: constructor delegation")
    );
    assert!(
        translation
            .rust
            .contains("pub fn new() -> ConstructorOverloads")
    );
    assert!(
        translation
            .rust
            .contains("return Self::new_string_int(\"empty\".to_string(), 0);")
    );
    assert!(
        translation
            .rust
            .contains("return Self::new_string_int(label, 1);")
    );
    assert!(
        translation
            .rust
            .contains("return format!(\"{}:{}\", self.label, self.count);")
    );
}

#[test]
fn translates_switch_fallthrough_without_compile_error() {
    let translation = translate_source(&fixture("02-control-flow/SwitchFallthrough.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    assert!(
        !translation
            .rust
            .contains("unsupported Java statement: switch")
    );
    assert!(translation.rust.contains("'java_switch: loop"));
    assert!(
        translation
            .rust
            .contains("let mut __java_switch_matched = false;")
    );
    assert!(translation.rust.contains("break 'java_switch;"));
}

#[test]
fn translates_throw_only_method_to_result() {
    let translation = translate_source(&fixture("06-stdlib/ThrowOnly.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    assert!(
        !translation.diagnostics.has_code("unsupported_exceptions"),
        "{:?}",
        translation.diagnostics
    );
    assert!(translation.rust.contains("pub struct JavaException"));
    assert!(
        translation
            .rust
            .contains("pub fn require_positive(&self, value: i32) -> Result<i32, JavaException>")
    );
    assert!(translation.rust.contains(
        "return Err(JavaException::new(\"IllegalArgumentException\", \"negative\".to_string()));"
    ));
    assert!(translation.rust.contains("return Ok(value);"));
}

#[test]
fn throw_only_generated_rust_preserves_behavior() {
    let translation = translate_source(&fixture("06-stdlib/ThrowOnly.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    let test_source = format!(
        "{}\n{}",
        translation.rust,
        r#"
#[cfg(test)]
mod generated_tests {
    use super::*;

    #[test]
    fn throw_only_uses_result_surface() {
        let checker = ThrowOnly {};
        assert_eq!(checker.require_positive(3).unwrap(), 3);
        let err = checker.require_positive(-1).unwrap_err();
        assert_eq!(err.class_name, "IllegalArgumentException");
        assert_eq!(err.message, "negative");
    }
}
"#
    );
    let rust_path = Path::new("/tmp").join("new_java2rust_throw_only_test.rs");
    let bin_path = Path::new("/tmp").join("new_java2rust_throw_only_test");
    fs::write(&rust_path, test_source).unwrap();
    let output = Command::new("rustc")
        .arg("--test")
        .arg(&rust_path)
        .arg("-o")
        .arg(&bin_path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "throw-only test did not compile\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let output = Command::new(&bin_path).output().unwrap();
    assert!(
        output.status.success(),
        "throw-only behavior test failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn translates_simple_stream_sum_to_iterator_chain() {
    let translation = translate_source(&fixture("06-stdlib/StreamsUse.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    assert!(!translation.diagnostics.has_code("unsupported_stream"));
    assert!(!translation.diagnostics.has_code("unsupported_lambda"));
    assert!(
        translation
            .rust
            .contains("pub fn positive_sum(&self, values: Vec<i32>) -> i32")
    );
    assert!(
        translation
            .rust
            .contains("values.iter().filter(|v| (**v > 0)).map(|v| *v).sum::<i32>()")
    );
}

#[test]
fn simple_stream_sum_generated_rust_preserves_behavior() {
    let translation = translate_source(&fixture("06-stdlib/StreamsUse.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    let test_source = format!(
        "{}\n{}",
        translation.rust,
        r#"
#[cfg(test)]
mod generated_tests {
    use super::*;

    #[test]
    fn stream_sum_filters_positive_values() {
        let streams = StreamsUse {};
        assert_eq!(streams.positive_sum(vec![-2, 0, 3, 4]), 7);
    }
}
"#
    );
    let rust_path = Path::new("/tmp").join("new_java2rust_streams_use_test.rs");
    let bin_path = Path::new("/tmp").join("new_java2rust_streams_use_test");
    fs::write(&rust_path, test_source).unwrap();
    let output = Command::new("rustc")
        .arg("--test")
        .arg(&rust_path)
        .arg("-o")
        .arg(&bin_path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "streams test did not compile\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let output = Command::new(&bin_path).output().unwrap();
    assert!(
        output.status.success(),
        "streams behavior test failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn translates_stream_collect_to_list_to_iterator_chain() {
    let translation = translate_source(&fixture("06-stdlib/StreamsCollect.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    assert!(
        translation
            .rust
            .contains("pub fn clean(&self, values: Vec<String>) -> Vec<String>")
    );
    assert!(translation.rust.contains(
        "values.iter().filter(|v| !v.is_empty()).map(|v| v.trim()).collect::<Vec<_>>().into_iter().map(|v| v.to_string()).collect::<Vec<_>>()"
    ));
}

#[test]
fn stream_collect_generated_rust_preserves_behavior() {
    let translation = translate_source(&fixture("06-stdlib/StreamsCollect.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    let test_source = format!(
        "{}\n{}",
        translation.rust,
        r#"
#[cfg(test)]
mod generated_tests {
    use super::*;

    #[test]
    fn stream_collect_filters_and_trims() {
        let streams = StreamsCollect {};
        let values = vec![" one ".to_string(), "".to_string(), "two".to_string()];
        assert_eq!(streams.clean(values), vec!["one".to_string(), "two".to_string()]);
    }
}
"#
    );
    let rust_path = Path::new("/tmp").join("new_java2rust_streams_collect_test.rs");
    let bin_path = Path::new("/tmp").join("new_java2rust_streams_collect_test");
    fs::write(&rust_path, test_source).unwrap();
    let output = Command::new("rustc")
        .arg("--test")
        .arg(&rust_path)
        .arg("-o")
        .arg(&bin_path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "streams collect test did not compile\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let output = Command::new(&bin_path).output().unwrap();
    assert!(
        output.status.success(),
        "streams collect behavior test failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn translates_stream_find_first_and_any_match_through_slot_coercion() {
    let translation = translate_source(&fixture("06-stdlib/StreamsFindMatch.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    assert!(
        translation
            .rust
            .contains("pub fn first_long(&self, values: Vec<String>) -> Option<String>")
    );
    assert!(
        translation
            .rust
            .contains("values.iter().filter(|v| (v.len() as i32 > 3)).next().cloned()")
    );
    assert!(
        translation
            .rust
            .contains("values.iter().any(|v| v.is_empty())")
    );
}

#[test]
fn stream_find_match_generated_rust_preserves_behavior() {
    let translation = translate_source(&fixture("06-stdlib/StreamsFindMatch.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    let test_source = format!(
        "{}\n{}",
        translation.rust,
        r#"
#[cfg(test)]
mod generated_tests {
    use super::*;

    #[test]
    fn stream_find_first_and_any_match() {
        let streams = StreamsFindMatch {};
        let values = vec!["one".to_string(), "three".to_string(), "".to_string()];
        assert_eq!(streams.first_long(values.clone()), Some("three".to_string()));
        assert!(streams.has_empty(values));
    }
}
"#
    );
    let rust_path = Path::new("/tmp").join("new_java2rust_streams_find_match_test.rs");
    let bin_path = Path::new("/tmp").join("new_java2rust_streams_find_match_test");
    fs::write(&rust_path, test_source).unwrap();
    let output = Command::new("rustc")
        .arg("--test")
        .arg(&rust_path)
        .arg("-o")
        .arg(&bin_path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "streams find/match test did not compile\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let output = Command::new(&bin_path).output().unwrap();
    assert!(
        output.status.success(),
        "streams find/match behavior test failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn switch_fallthrough_generated_rust_preserves_behavior() {
    let translation = translate_source(&fixture("02-control-flow/SwitchFallthrough.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    let test_source = format!(
        "{}\n{}",
        translation.rust,
        r#"
#[cfg(test)]
mod generated_tests {
    use super::*;

    #[test]
    fn switch_scores_match_java_fallthrough() {
        let scorer = SwitchFallthrough {};
        assert_eq!(scorer.score("A".to_string()), 4);
        assert_eq!(scorer.score("B".to_string()), 2);
        assert_eq!(scorer.score("C".to_string()), 2);
        assert_eq!(scorer.score("D".to_string()), -1);
    }
}
"#
    );
    let rust_path = Path::new("/tmp").join("new_java2rust_switch_fallthrough_test.rs");
    let bin_path = Path::new("/tmp").join("new_java2rust_switch_fallthrough_test");
    fs::write(&rust_path, test_source).unwrap();
    let output = Command::new("rustc")
        .arg("--test")
        .arg(&rust_path)
        .arg("-o")
        .arg(&bin_path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "switch test did not compile\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let output = Command::new(&bin_path).output().unwrap();
    assert!(
        output.status.success(),
        "switch behavior test failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn translates_same_arity_overload_call_sites() {
    let translation = translate_source(&fixture("03-methods/OverloadSameArity.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    assert!(
        translation
            .rust
            .contains("pub fn choose_int(&self, value: i32) -> String")
    );
    assert!(
        translation
            .rust
            .contains("pub fn choose_long(&self, value: i64) -> String")
    );
    assert!(
        translation
            .rust
            .contains("pub fn choose_string(&self, value: String) -> String")
    );
    assert!(translation.rust.contains("self.choose_int(1)"));
    assert!(translation.rust.contains("self.choose_long(1i64)"));
    assert!(
        translation
            .rust
            .contains("self.choose_string(\"one\".to_string())")
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
            "unsupported_anonymous_class",
        ),
        ("05-types/WildcardBounds.java", "unsupported_wildcard"),
        ("06-stdlib/MultiCatch.java", "unsupported_exceptions"),
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
        "02-control-flow/SwitchFallthrough.java",
        "03-methods/MethodsAndOverloads.java",
        "03-methods/ConstructorOverloads.java",
        "03-methods/OverloadSameArity.java",
        "04-objects/SimpleClass.java",
        "06-stdlib/ThrowOnly.java",
        "06-stdlib/StreamsUse.java",
        "06-stdlib/StreamsCollect.java",
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
