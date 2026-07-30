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
    assert_eq!(fixtures.len(), 65);
    assert!(
        fixtures
            .iter()
            .any(|f| f.relative_path.ends_with("04-objects/SimpleClass.java"))
    );
}

#[test]
fn translates_static_import_and_qualified_static_utility_owner() {
    let translation = translate_source(&fixture("06-stdlib/StaticImportUtilityOwner.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    assert!(
        translation.rust.contains("StaticUtility::read_value(path)"),
        "{}",
        translation.rust
    );
    assert!(
        translation
            .rust
            .contains("pub type StaticUtility = ExternalLibStaticUtility;"),
        "{}",
        translation.rust
    );
    assert!(
        translation.rust.contains("self.local_value(path.clone()"),
        "{}",
        translation.rust
    );
    assert!(!translation.rust.contains("return read_value(path)"));
    assert!(!translation.rust.contains("external.lib"));
}

#[test]
fn translates_imported_static_utility_owner_without_module_receiver() {
    let translation = translate_source(&fixture("06-stdlib/ImportedStaticUtilityOwner.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    assert!(
        translation
            .rust
            .contains("ExternalUtility::read_value(path)"),
        "{}",
        translation.rust
    );
    assert!(
        translation.rust.contains("pub struct ExternalUtility;"),
        "{}",
        translation.rust
    );
    assert!(
        translation.rust.contains("local.read_value(path)"),
        "{}",
        translation.rust
    );
    assert!(!translation.rust.contains("external_utility::read_value"));
}

#[test]
fn imported_static_known_return_declares_chained_facade_surface() {
    let translation = translate_source(
        r#"
import com.google.protobuf.util.JsonFormat;

class JsonFormatUse {
    void merge(String json, Builder builder) {
        JsonFormat.parser().merge(json, builder);
    }
}
"#,
    );

    assert!(
        translation.rust.contains("pub struct Parser;"),
        "{}",
        translation.rust
    );
    assert!(
        translation.rust.contains("pub fn parser() -> Parser"),
        "{}",
        translation.rust
    );
    assert!(
        translation
            .rust
            .contains("pub fn merge<T0, T1>(&self, _arg0: T0, _arg1: T1) -> ()"),
        "{}",
        translation.rust
    );
}

#[test]
fn imported_static_non_null_validator_returns_first_argument_facade() {
    let translation = translate_source(
        r#"
import org.broadinstitute.hellbender.utils.Utils;

class UtilsNonNullUse {
    String checked(String path) {
        return Utils.nonNull(path);
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("pub fn non_null<T0>(_arg0: T0) -> T0 { _arg0 }"),
        "{}",
        translation.rust
    );
    assert!(
        !translation.rust.contains("pub fn non_null<R: Default"),
        "{}",
        translation.rust
    );
}

#[test]
fn translates_external_string_constant_argument_without_downcast() {
    let translation = translate_source(&fixture("06-stdlib/ExternalStringConstantArgument.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    assert!(
        translation
            .rust
            .contains("GATKVCFConstants::RAW_MAPPING_QUALITY_WITH_DEPTH_KEY"),
        "{}",
        translation.rust
    );
    assert!(
        translation.rust.contains("pub struct GATKVCFConstants"),
        "{}",
        translation.rust
    );
    assert!(
        translation.rust.contains(
            "pub const RAW_MAPPING_QUALITY_WITH_DEPTH_KEY: &'static str = \"RAW_MAPPING_QUALITY_WITH_DEPTH_KEY\";"
        ),
        "{}",
        translation.rust
    );
    assert!(!translation.rust.contains("downcast::<String>()"));
}

#[test]
fn translates_stream_non_null_filter_method_reference() {
    let translation = translate_source(&fixture("06-stdlib/StreamFilterNonNull.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    assert!(
        translation
            .rust
            .contains("values.iter().collect::<Vec<_>>()"),
        "{}",
        translation.rust
    );
    assert!(!translation.rust.contains("Objects::non_null"));
    assert!(
        !translation
            .rust
            .contains("unsupported Java expression: method reference")
    );
}

#[test]
fn translates_stream_nullable_map_then_non_null_filter() {
    let translation = translate_source(&fixture("06-stdlib/StreamMapNullFilter.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    assert!(
        translation
            .rust
            .contains(".map(|value: &String| { if value.is_empty() { None } else { Some(value.to_lowercase()) } }).filter_map(|item| item).collect::<Vec<_>>()"),
        "{}",
        translation.rust
    );
    assert!(!translation.rust.contains("Objects::non_null"));
    assert!(
        !translation
            .rust
            .contains("unsupported Java expression: lambda")
    );
}

#[test]
fn translates_stream_flat_map_block_pipeline() {
    let translation = translate_source(&fixture("06-stdlib/StreamFlatMapBlockPipeline.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    assert!(
        translation.rust.contains(
            "groups.iter().flat_map(|group| { let names = group.names(); names.into_iter().filter(move |name| !name.is_empty()).map(|name: &String| name.trim()) }).collect::<Vec<_>>().into_iter().map(|v| v.to_string()).collect::<Vec<_>>()"
        ),
        "{}",
        translation.rust
    );
    assert!(
        !translation
            .rust
            .contains("unsupported Java expression: lambda")
    );
}

#[test]
fn translates_nullable_clear_assignment_as_option_take() {
    let translation = translate_source(&fixture("05-types/NullableClearAssignment.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    assert!(
        translation.rust.contains("self.cached.take();"),
        "{}",
        translation.rust
    );
    assert!(!translation.rust.contains("self.cached = None;"));
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
            .contains("format!(\"{}:{}\", self.name, (self.count).to_string())")
    );
}

#[test]
fn translates_multiline_string_concat_with_braces_without_closing_impl() {
    let translation = translate_source(
        r#"
abstract class BraceStringConcat {
    int minGQ;
    int maxGQ;

    abstract String createLabel(String sampleName);

    public abstract void add(int pos, int newEnd);

    String label() {
        return "HomRefBlock{" +
                "minGQ=" + minGQ +
                ", maxGQ=" + maxGQ +
                '}';
    }

    boolean withinBounds(int gq) {
        return gq >= minGQ && gq < maxGQ;
    }
}
"#,
    );
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}\n{}",
        translation.diagnostics,
        translation.rust
    );
    assert!(
        translation
            .rust
            .contains("format!(\"HomRefBlock{{minGQ={}, maxGQ={}{}\""),
        "{}",
        translation.rust
    );
    assert!(
        translation
            .rust
            .contains("pub fn within_bounds(&self, gq: i32) -> bool"),
        "{}",
        translation.rust
    );
    assert!(
        !translation
            .rust
            .contains("\n    }\n    pub fn within_bounds"),
        "{}",
        translation.rust
    );
    assert!(
        !translation
            .rust
            .contains("\n    }\n    pub fn create_label"),
        "{}",
        translation.rust
    );
}

#[test]
fn translates_expression_precedence_shape() {
    let translation = translate_source(&fixture("01-expressions/ExpressionPrecedence.java"));
    assert!(!translation.diagnostics.has_errors());
    assert!(translation.rust.contains("let x: i32 = a + b * c;"));
    assert!(translation.rust.contains("let y: i32 = (a + b) * c;"));
    assert!(translation.rust.contains("let z: i32 = x << 1;"));
    assert!(
        translation
            .rust
            .contains("return if z > y { z - y } else { y - z };")
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
fn inherited_overload_does_not_replace_child_override_body() {
    let translation = translate_source(
        r#"
class Base {
    int f(int a, int b) {
        return a + b + 100;
    }
}

class Child extends Base {
    int f(int a) {
        return a * 10;
    }

    int run() {
        return f(2) + f(2, 3);
    }
}
"#,
    );
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}\n{}",
        translation.diagnostics,
        translation.rust
    );
    assert!(
        translation.rust.contains("pub fn f(&self, a: i32) -> i32"),
        "{}",
        translation.rust
    );
    assert!(
        translation.rust.contains("return a * 10;"),
        "{}",
        translation.rust
    );
    assert!(
        translation
            .rust
            .contains("pub fn f_int_int(&self, a: i32, b: i32) -> i32"),
        "{}",
        translation.rust
    );
    assert!(
        translation.rust.contains("return (a + b) + 100;"),
        "{}",
        translation.rust
    );
    assert!(
        translation
            .rust
            .contains("self.f(2) + self.f_int_int(2, 3)"),
        "{}",
        translation.rust
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
            .contains("return format!(\"{}:{}\", self.label, (self.count).to_string());")
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
    let rust_path = std::env::temp_dir().join("new_java2rust_throw_only_test.rs");
    let bin_path = std::env::temp_dir().join("new_java2rust_throw_only_test");
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
            .contains("pub fn positive_sum(&self, values: &Vec<i32>) -> i32")
    );
    assert!(
        translation
            .rust
            .contains("values.iter().filter(|v| **v > 0).map(|v| *v).sum::<i32>()")
    );
}

#[test]
fn translates_map_get_null_ternary_with_borrowed_optional_carrier() {
    let translation = translate_source(&fixture("06-stdlib/CollectionsUse.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    assert!(
        translation
            .rust
            .contains("let old: Option<i32> = counts.get(name.as_str()).copied();")
            && translation
                .rust
                .contains("if old.is_none() { 1 } else { old.unwrap() + 1 }")
            && translation
                .rust
                .contains("counts.insert(name.clone(), if old.is_none()")
            && !translation.rust.contains("Some(counts.get")
            && !translation.rust.contains("(old + 1)"),
        "{}",
        translation.rust
    );
}

#[test]
fn translates_map_put_previous_value_as_optional_without_double_wrapping() {
    let translation = translate_source(
        r#"
import java.util.Map;

class MapPutPreviousValue {
    String replace(Map<String, String> values, String key, String value) {
        String old = values.put(key, value);
        return old == null ? "missing" : old;
    }
}
"#,
    );
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    assert!(
        translation
            .rust
            .contains("let old: Option<String> = values.insert(key, value);")
            && translation.rust.contains("if old.is_none()")
            && translation.rust.contains("else { old }")
            && !translation.rust.contains("Some(values.insert")
            && !translation.rust.contains("Some(Some("),
        "{}",
        translation.rust
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
        assert_eq!(streams.positive_sum(&vec![-2, 0, 3, 4]), 7);
    }
}
"#
    );
    let rust_path = std::env::temp_dir().join("new_java2rust_streams_use_test.rs");
    let bin_path = std::env::temp_dir().join("new_java2rust_streams_use_test");
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
fn stream_capture_generated_rust_preserves_behavior() {
    let translation = translate_source(&fixture("06-stdlib/StreamCapturesLocal.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    assert!(
        translation
            .rust
            .contains("values.iter().map(|v| *v + base).sum::<i32>()"),
        "{}",
        translation.rust
    );
    let test_source = format!(
        "{}\n{}",
        translation.rust,
        r#"
#[cfg(test)]
mod generated_tests {
    use super::*;

    #[test]
    fn stream_sum_uses_captured_base() {
        let streams = StreamCapturesLocal {};
        assert_eq!(streams.add_base(&vec![1, 2, 3], 10), 36);
    }
}
"#
    );
    let rust_path = std::env::temp_dir().join("new_java2rust_stream_captures_local_test.rs");
    let bin_path = std::env::temp_dir().join("new_java2rust_stream_captures_local_test");
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
        "stream capture test did not compile\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let output = Command::new(&bin_path).output().unwrap();
    assert!(
        output.status.success(),
        "stream capture behavior test failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn stream_flat_map_lambda_generated_rust_preserves_behavior() {
    let translation = translate_source(&fixture("06-stdlib/StreamFlatMapLambda.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    assert!(
        translation
            .rust
            .contains("groups.iter().flat_map(|group| group.iter()).collect::<Vec<_>>()"),
        "{}",
        translation.rust
    );
    let test_source = format!(
        "{}\n{}",
        translation.rust,
        r#"
#[cfg(test)]
mod generated_tests {
    use super::*;

    #[test]
    fn stream_flat_map_lambda_flattens_groups() {
        let streams = StreamFlatMapLambda {};
        let groups = vec![
            vec!["a".to_string(), "b".to_string()],
            vec!["c".to_string()],
        ];
        assert_eq!(
            streams.flatten(groups),
            vec!["a".to_string(), "b".to_string(), "c".to_string()]
        );
    }
}
"#
    );
    let rust_path = std::env::temp_dir().join("new_java2rust_stream_flat_map_lambda_test.rs");
    let bin_path = std::env::temp_dir().join("new_java2rust_stream_flat_map_lambda_test");
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
        "stream flatMap lambda test did not compile\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let output = Command::new(&bin_path).output().unwrap();
    assert!(
        output.status.success(),
        "stream flatMap lambda behavior test failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn stream_block_lambda_generated_rust_preserves_behavior() {
    let translation = translate_source(&fixture("06-stdlib/StreamBlockLambda.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    assert!(
        translation.rust.contains(
            "values.iter().map(|value: &String| { let trimmed = value.trim(); trimmed.to_lowercase() }).collect::<Vec<_>>()"
        ) && !translation.rust.contains("unsupported Java expression: lambda")
            && !translation.rust.contains(".cloned().collect::<Vec<_>>()"),
        "{}",
        translation.rust
    );
    let test_source = format!(
        "{}\n{}",
        translation.rust,
        r#"
#[cfg(test)]
mod generated_tests {
    use super::*;

    #[test]
    fn stream_block_lambda_normalizes_values() {
        let streams = StreamBlockLambda {};
        assert_eq!(
            streams.normalize(vec![" A ".to_string(), "Bc".to_string()]),
            vec!["a".to_string(), "bc".to_string()]
        );
    }
}
"#
    );
    let rust_path = std::env::temp_dir().join("new_java2rust_stream_block_lambda_test.rs");
    let bin_path = std::env::temp_dir().join("new_java2rust_stream_block_lambda_test");
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
        "stream block lambda test did not compile\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let output = Command::new(&bin_path).output().unwrap();
    assert!(
        output.status.success(),
        "stream block lambda behavior test failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn stream_flat_map_projection_generated_rust_preserves_behavior() {
    let translation = translate_source(&fixture("06-stdlib/StreamFlatMapProjection.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    assert!(
        translation.rust.contains(
            "groups.iter().flat_map(|group| group.names().into_iter()).collect::<Vec<_>>()"
        ),
        "{}",
        translation.rust
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
        "values.iter().filter(|v| !v.is_empty()).map(|v: &String| v.trim()).collect::<Vec<_>>().into_iter().map(|v| v.to_string()).collect::<Vec<_>>()"
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
    let rust_path = std::env::temp_dir().join("new_java2rust_streams_collect_test.rs");
    let bin_path = std::env::temp_dir().join("new_java2rust_streams_collect_test");
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
fn stream_static_method_references_use_call_target_plan() {
    let translation = translate_source(&fixture("06-stdlib/StreamStaticMethodReferences.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    assert!(
        translation
            .rust
            .contains("value.as_str().parse::<i32>().unwrap()")
            && translation
                .rust
                .contains("value.as_str().parse::<f64>().unwrap()")
            && !translation.rust.contains("Integer::parse_int")
            && !translation.rust.contains("Double::parse_double")
            && !translation.rust.contains("*value.as_str().parse"),
        "{}",
        translation.rust
    );
    let test_source = format!(
        "{}\n{}",
        translation.rust,
        r#"
#[cfg(test)]
mod generated_tests {
    use super::*;

    #[test]
    fn stream_static_method_references_parse_values() {
        let streams = StreamStaticMethodReferences {};
        assert_eq!(
            streams.parse_ints(vec!["1".to_string(), "2".to_string(), "3".to_string()]),
            vec![1, 2, 3]
        );
        assert_eq!(
            streams.parse_doubles(vec!["1.5".to_string(), "2.25".to_string()]),
            vec![1.5, 2.25]
        );
    }
}
"#
    );
    let rust_path = std::env::temp_dir().join("new_java2rust_stream_static_method_refs_test.rs");
    let bin_path = std::env::temp_dir().join("new_java2rust_stream_static_method_refs_test");
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
        "stream static method refs test did not compile\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let output = Command::new(&bin_path).output().unwrap();
    assert!(
        output.status.success(),
        "stream static method refs behavior test failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn stream_string_method_references_use_call_target_plan() {
    let translation = translate_source(&fixture("06-stdlib/StreamStringMethodReferences.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    assert!(
        translation
            .rust
            .contains("values.iter().map(|value: &String| value.len() as i32).collect::<Vec<_>>()")
            && translation.rust.contains(
                "values.iter().map(|value: &String| value.trim()).collect::<Vec<_>>().into_iter().map(|v| v.to_string()).collect::<Vec<_>>()"
            )
            && translation
                .rust
                .contains("values.iter().any(|value| value.is_empty())")
            && !translation.rust.contains("String::length")
            && !translation.rust.contains("String::trim")
            && !translation.rust.contains(".into_iter().cloned().collect::<Vec<_>>()"),
        "{}",
        translation.rust
    );
    let test_source = format!(
        "{}\n{}",
        translation.rust,
        r#"
#[cfg(test)]
mod generated_tests {
    use super::*;

    #[test]
    fn stream_string_method_references_apply_native_string_calls() {
        let streams = StreamStringMethodReferences {};
        let values = vec![" a ".to_string(), "".to_string(), "bc".to_string()];
        assert_eq!(streams.lengths(values.clone()), vec![3, 0, 2]);
        assert_eq!(
            streams.trimmed(values.clone()),
            vec!["a".to_string(), "".to_string(), "bc".to_string()]
        );
        assert!(streams.any_empty(&values));
    }
}
"#
    );
    let rust_path = std::env::temp_dir().join("new_java2rust_stream_string_method_refs_test.rs");
    let bin_path = std::env::temp_dir().join("new_java2rust_stream_string_method_refs_test");
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
        "stream string method refs test did not compile\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let output = Command::new(&bin_path).output().unwrap();
    assert!(
        output.status.success(),
        "stream string method refs behavior test failed\nstdout:\n{}\nstderr:\n{}",
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
            .contains("pub fn first_long(&self, values: &Vec<String>) -> Option<String>")
    );
    assert!(
        translation
            .rust
            .contains("values.iter().filter(|v| (v.len() as i32) > 3).next().cloned()")
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
        assert_eq!(streams.first_long(&values), Some("three".to_string()));
        assert!(streams.has_empty(&values));
    }
}
"#
    );
    let rust_path = std::env::temp_dir().join("new_java2rust_streams_find_match_test.rs");
    let bin_path = std::env::temp_dir().join("new_java2rust_streams_find_match_test");
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
    let rust_path = std::env::temp_dir().join("new_java2rust_switch_fallthrough_test.rs");
    let bin_path = std::env::temp_dir().join("new_java2rust_switch_fallthrough_test");
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
fn nested_static_builder_call_uses_resolved_type_owner() {
    let translation = translate_source(&fixture("06-stdlib/NestedStaticBuilderCall.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    assert!(
        translation.rust.contains(
            "let builder: NestedStaticBuilderCallBuilder = NestedStaticBuilderCallCoordinatesGenomicsDBColumn::new_builder();"
        )
    );
    assert!(!translation.rust.contains("coordinates.genomics_d_b_column"));
    assert!(!translation.rust.contains("Coordinates::GenomicsDBColumn"));
}

#[test]
fn stream_block_lambda_static_owner_uses_type_owner() {
    let translation = translate_source(&fixture("06-stdlib/StreamBlockStaticOwner.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    assert!(translation.rust.contains(
        "let mut builder = GenomicsDBColumn::new_builder(); builder.set_name(value.clone());"
    ));
    assert!(!translation.rust.contains("coordinates.genomics_d_b_column"));
}

#[test]
fn collection_capacity_constructor_uses_with_capacity() {
    let translation = translate_source(&fixture("06-stdlib/CollectionCapacityConstructor.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    assert!(
        translation
            .rust
            .contains("Vec::with_capacity(self.names.as_ref().unwrap().len() as i32 as usize)")
    );
    assert!(!translation.rust.contains("self.names.size()"));
}

#[test]
fn collectors_to_map_lowers_to_hash_map_collect() {
    let translation = translate_source(&fixture("06-stdlib/CollectorsToMap.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    assert!(translation.rust.contains(
        "values.iter().map(|v| ((*v).clone(), v.len() as i32)).collect::<std::collections::HashMap<_, _>>()"
    ));
    assert!(!translation.rust.contains("collectors.to_map"));
    assert!(
        !translation
            .rust
            .contains("unsupported Java expression: lambda")
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
        "04-objects/DynTraitMethodCompatibility.java",
        "04-objects/SimpleClass.java",
        "05-types/NullableLocalReturn.java",
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
        let rust_path = std::env::temp_dir().join(format!(
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
