mod common;

use std::fs;
use std::path::Path;

use common::{
    GeneratedCrateModule, cargo_check_generated_modules, cargo_check_generated_modules_fails_with,
    cargo_test_generated_modules, load_java_fixtures,
};
use new_java2rust::diagnostics::Severity;
use new_java2rust::{translate_source, translate_source_with_context_sources};

fn fixture(path: &str) -> String {
    fs::read_to_string(Path::new("examples/java").join(path)).unwrap()
}

fn cargo_check_translation(crate_name: &str, rust: &str) {
    cargo_check_generated_modules(
        crate_name,
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: rust.to_string(),
            tests: "",
        }],
    );
}

fn cargo_test_translation(crate_name: &str, rust: String, tests: &'static str) {
    cargo_test_generated_modules(
        crate_name,
        vec![GeneratedCrateModule {
            name: "fixture",
            rust,
            tests,
        }],
    );
}

#[test]
fn all_java_fixtures_are_loadable() {
    let fixtures = load_java_fixtures("examples/java").unwrap();
    assert_eq!(fixtures.len(), 67);
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
        translation.rust.contains(
            "unimplemented!(\"unresolved Java static call ExternalLibStaticUtility.read_value is not modelled\")"
        ),
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
    assert!(
        translation
            .rust
            .contains("return StaticUtility::read_value(path);")
    );
    assert!(!translation.rust.contains("external.lib"));
    cargo_check_translation("static_import_utility_owner", &translation.rust);
}

#[test]
fn null_overload_ambiguity_emits_loud_compile_error() {
    let translation = translate_source(&fixture("03-methods/NullOverloadAmbiguity.java"));
    assert!(translation.diagnostics.has_code("resolve_call_ambiguous"));
    assert!(
        translation
            .rust
            .contains("compile_error!(\"ambiguous overload of pick for null argument\")"),
        "{}",
        translation.rust
    );
    assert!(!translation.rust.contains("return pick(None);"));
    cargo_check_generated_modules_fails_with(
        "null_overload_ambiguity",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
        "ambiguous overload of pick for null argument",
    );
}

#[test]
fn local_class_capture_lowers_captured_parameters_and_locals() {
    let translation = translate_source(&fixture("04-objects/LocalClassCapture.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    assert!(translation.rust.contains("pub struct Adder"));
    assert!(translation.rust.contains("base: i32"));
    assert!(translation.rust.contains("offset: i32"));
    assert!(
        translation
            .rust
            .contains("return Adder::new(base, offset).apply(value);"),
        "{}",
        translation.rust
    );
    cargo_test_translation(
        "local_class_capture",
        translation.rust,
        r#"
#[test]
fn method_local_class_retains_its_parameter_and_local_captures() {
    assert_eq!(LocalClassCapture {}.add_later(4, 5), 12);
}
"#,
    );
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
            .contains("unimplemented!(\"unresolved Java static call ExternalLibExternalUtility.read_value is not modelled\")"),
        "{}",
        translation.rust
    );
    assert!(
        translation.rust.contains("pub struct ExternalUtility;"),
        "{}",
        translation.rust
    );
    assert!(
        translation
            .rust
            .contains("pub fn read_value<R, T0>(&self, _arg0: T0) -> R"),
        "{}",
        translation.rust
    );
    assert!(
        translation.rust.contains("local.read_value(path)"),
        "{}",
        translation.rust
    );
    assert!(!translation.rust.contains("external_utility::read_value"));
    cargo_check_translation("imported_static_utility_owner", &translation.rust);
}

#[test]
fn static_owner_shadowing_uses_source_owner_before_jdk_facade() {
    let translation = translate_source(&fixture("06-stdlib/StaticOwnerShadowing.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    assert!(
        translation.rust.contains(
            "return StaticOwnerShadowing_Objects::equals(left.as_str(), right.as_str());"
        ) && translation
            .rust
            .contains("return StaticOwnerShadowing_Collections::empty_list();")
            && translation
                .rust
                .contains("return StaticOwnerShadowing_Integer::max(left, right);"),
        "{}",
        translation.rust
    );
    cargo_test_translation(
        "static_owner_shadowing",
        translation.rust,
        r#"
#[cfg(test)]
mod generated_tests {
    use super::*;

    #[test]
    fn static_owner_shadowing_uses_nested_source_types() {
        let subject = StaticOwnerShadowing {};
        assert!(!subject.same("a".to_string(), "a".to_string()));
        assert_eq!(subject.count(), 42);
        assert_eq!(subject.max(9, 4), 5);
    }
}
"#,
    );
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
    cargo_check_translation("imported_static_known_return", &translation.rust);
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
    assert!(
        translation
            .rust
            .contains("unimplemented!(\"unresolved Java static call OrgBroadinstituteHellbenderUtilsUtils.non_null is not modelled\")"),
        "{}",
        translation.rust
    );
    cargo_check_translation("imported_static_non_null", &translation.rust);
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
    cargo_check_translation("external_string_constant_argument", &translation.rust);
}

#[test]
fn enum_constants_with_fields_compile_with_single_payload_accessor() {
    let translation = translate_source(&fixture("04-objects/EnumConstantsWithFields.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    assert_eq!(
        translation.rust.matches("pub fn id(&self) -> i32").count(),
        1,
        "{}",
        translation.rust
    );
    assert!(
        !translation.rust.contains("return self.id();"),
        "{}",
        translation.rust
    );
    cargo_test_translation(
        "enum_constants_with_fields",
        translation.rust,
        r#"
#[cfg(test)]
mod generated_tests {
    use super::*;

    #[test]
    fn enum_payload_ids_match_constructor_args() {
        let fixture = EnumConstantsWithFields {};
        assert_eq!(fixture.id(EnumConstantsWithFields_Mode::FAST).unwrap(), 1);
        assert_eq!(fixture.id(EnumConstantsWithFields_Mode::SLOW).unwrap(), 2);
    }
}
"#,
    );
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
            .contains("values.iter().cloned().collect::<Vec<_>>()"),
        "{}",
        translation.rust
    );
    assert!(!translation.rust.contains("Objects::non_null"));
    assert!(
        !translation
            .rust
            .contains("unsupported Java expression: method reference")
    );
    cargo_check_translation("stream_non_null_filter_method_reference", &translation.rust);
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
            .contains(".map(|value| { if value.is_empty() { None } else { Some(value.to_lowercase()) } }).filter_map(|item| item).collect::<Vec<_>>()"),
        "{}",
        translation.rust
    );
    assert!(!translation.rust.contains("Objects::non_null"));
    assert!(
        !translation
            .rust
            .contains("unsupported Java expression: lambda")
    );
    cargo_check_translation(
        "stream_nullable_map_then_non_null_filter",
        &translation.rust,
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
            "groups.iter().flat_map(|group| { let names = group.names().expect(\"Java null receiver\"); names.into_iter().filter(move |name| !name.is_empty()).map(|name| name.trim().to_string()) }).collect::<Vec<_>>()"
        ),
        "{}",
        translation.rust
    );
    assert!(
        !translation
            .rust
            .contains("unsupported Java expression: lambda")
    );
    cargo_check_translation("stream_flat_map_block_pipeline", &translation.rust);
}

#[test]
fn try_catch_finally_catch_return_generated_rust_compiles() {
    let translation = translate_source(&fixture("06-stdlib/TryCatchFinally.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    assert!(
        translation
            .rust
            .contains("return Ok(Some(\"error\".to_string()));")
            && translation
                .rust
                .contains("println!(\"{}\", \"done\".to_string());")
            && !translation
                .rust
                .contains("return Some(\"error\".to_string());")
            && !translation
                .rust
                .contains("unsupported Java statement: try/catch/finally"),
        "{}",
        translation.rust
    );
    cargo_check_translation("try_catch_finally", &translation.rust);
}

#[test]
fn try_with_resources_nullable_read_and_closeable_cleanup_compiles() {
    let translation = translate_source(&fixture("06-stdlib/TryWithResources.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    assert!(
        translation.rust.contains("reader.read_line()?.unwrap()")
            && !translation.rust.contains("reader.read_line()?.unwrap()?")
            && translation.rust.contains(
                "let reader: BufferedReader = JavaNioFileFiles::new_buffered_reader(&path)?;"
            )
            && !translation
                .rust
                .contains("let reader: Box<dyn std::any::Any>")
            && translation.rust.contains("match audit.close()")
            && translation.rust.contains("impl Closeable")
            && translation
                .rust
                .contains("pub fn close(&self) -> Result<(), JavaException>"),
        "{}",
        translation.rust
    );
    cargo_check_translation("try_with_resources", &translation.rust);
}

#[test]
fn try_with_resources_finally_order_fixture_lowers_without_placeholder() {
    let translation = translate_source(include_str!("differential/TryWithResourcesFinally.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    assert!(
        !translation
            .rust
            .contains("unsupported Java statement: try/catch/finally"),
        "{}",
        translation.rust
    );
    assert!(
        translation.rust.contains("__java_twr_result")
            && translation.rust.contains("resource.close")
            && translation
                .rust
                .contains("__set_static_out(format!(\"{}f\""),
        "{}",
        translation.rust
    );
    cargo_check_translation("try_with_resources_finally_order", &translation.rust);
}

#[test]
fn try_with_resources_catch_finally_order_fixture_lowers_without_placeholder() {
    let translation = translate_source(include_str!(
        "differential/TryWithResourcesCatchFinally.java"
    ));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    assert!(
        !translation
            .rust
            .contains("unsupported Java statement: try/catch/finally"),
        "{}",
        translation.rust
    );
    assert!(
        translation.rust.contains("__java_twr_result")
            && translation.rust.contains("__java_try_result")
            && translation.rust.contains("resource.close")
            && translation.rust.contains("RuntimeException")
            && translation
                .rust
                .contains("__set_static_out(format!(\"{}f\""),
        "{}",
        translation.rust
    );
    cargo_check_translation("try_with_resources_catch_finally_order", &translation.rust);
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
    cargo_check_translation("nullable_clear_assignment", &translation.rust);
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
    assert!(translation.rust.contains("name: Option<String>"));
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
            .contains("format!(\"{}:{}\", self.name.as_ref().map(|value| std::convert::AsRef::<str>::as_ref(value)).unwrap_or(\"null\"), (self.count).to_string())")
    );
    cargo_check_translation("simple_class_shape", &translation.rust);
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
    cargo_check_translation("multiline_string_concat_braces", &translation.rust);
}

#[test]
fn translates_expression_precedence_shape() {
    let translation = translate_source(&fixture("01-expressions/ExpressionPrecedence.java"));
    assert!(!translation.diagnostics.has_errors());
    assert!(
        translation
            .rust
            .contains("let x: i32 = (a).wrapping_add((b).wrapping_mul(c));")
    );
    assert!(
        translation
            .rust
            .contains("let y: i32 = ((a).wrapping_add(b)).wrapping_mul(c);")
    );
    assert!(
        translation
            .rust
            .contains("let z: i32 = (x).wrapping_shl((1) as u32);")
    );
    assert!(
        translation
            .rust
            .contains("return if z > y { (z).wrapping_sub(y) } else { (y).wrapping_sub(z) };")
    );
    cargo_check_translation("expression_precedence_shape", &translation.rust);
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
            .contains("pub fn format_string(&self, value: &str) -> String")
    );
    cargo_check_translation("methods_with_overload_suffixes", &translation.rust);
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
        translation
            .rust
            .contains("pub fn f_int(&self, a: i32) -> i32"),
        "{}",
        translation.rust
    );
    assert!(
        translation.rust.contains("return (a).wrapping_mul(10);"),
        "{}",
        translation.rust
    );
    assert!(
        translation
            .rust
            .contains("pub fn f(&self, a: i32, b: i32) -> i32"),
        "{}",
        translation.rust
    );
    assert!(
        translation
            .rust
            .contains("return ((a).wrapping_add(b)).wrapping_add(100);"),
        "{}",
        translation.rust
    );
    assert!(
        translation
            .rust
            .contains("return (self.f_int(2)).wrapping_add(self.f(2, 3));"),
        "{}",
        translation.rust
    );
    cargo_check_translation("inherited_overload_child_override", &translation.rust);
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
            .contains("return format!(\"{}:{}\", self.label.as_ref().map(|value| std::convert::AsRef::<str>::as_ref(value)).unwrap_or(\"null\"), (self.count).to_string());")
    );
    cargo_check_translation("constructor_delegation_resolved_calls", &translation.rust);
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
    assert!(
        translation
            .rust
            .contains("value = match grade { \"A\" => (value).wrapping_add(4), \"B\" | \"C\" => (value).wrapping_add(2), _ => (value).wrapping_sub(1) };")
    );
    assert!(!translation.rust.contains("'java_switch: loop"));
    cargo_check_translation("switch_fallthrough_shape", &translation.rust);
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
        "return Err(JavaException::new(\"IllegalArgumentException\", \"negative\".to_string().to_string()));"
    ));
    assert!(translation.rust.contains("return Ok(value);"));
    cargo_check_translation("throw_only_result_shape", &translation.rust);
}

#[test]
fn throw_only_generated_rust_preserves_behavior() {
    let translation = translate_source(&fixture("06-stdlib/ThrowOnly.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    cargo_test_translation(
        "throw_only",
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
        assert_eq!(err.class_name(), "IllegalArgumentException");
        assert_eq!(err.message(), "negative");
    }
}
"#,
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
            .contains("pub fn positive_sum(&self, values: &[i32]) -> Result<i32, JavaException>")
    );
    assert!(
        translation
            .rust
            .contains("values.iter().filter(|v| **v > 0).map(|v| *v).sum::<i32>()")
    );
    cargo_check_translation("simple_stream_sum_to_iterator_chain", &translation.rust);
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
    cargo_check_translation("map_get_null_ternary_borrowed_optional", &translation.rust);
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
            && translation.rust.contains("else { old.unwrap() }")
            && translation.rust.contains("return Ok(Some(")
            && !translation.rust.contains("Some(values.insert")
            && !translation.rust.contains("Some(Some("),
        "{}",
        translation.rust
    );
    cargo_check_translation("map_put_previous_optional", &translation.rust);
}

#[test]
fn simple_stream_sum_generated_rust_preserves_behavior() {
    let translation = translate_source(&fixture("06-stdlib/StreamsUse.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    cargo_test_translation(
        "streams_use",
        translation.rust,
        r#"
#[cfg(test)]
mod generated_tests {
    use super::*;

    #[test]
    fn stream_sum_filters_positive_values() {
        let streams = StreamsUse {};
        assert_eq!(streams.positive_sum(&vec![-2, 0, 3, 4]).unwrap(), 7);
    }
}
"#,
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
    cargo_test_translation(
        "stream_captures_local",
        translation.rust,
        r#"
#[cfg(test)]
mod generated_tests {
    use super::*;

    #[test]
    fn stream_sum_uses_captured_base() {
        let streams = StreamCapturesLocal {};
        assert_eq!(streams.add_base(&vec![1, 2, 3], 10).unwrap(), 36);
    }
}
"#,
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
            .contains("groups.iter().flat_map(|group| group.iter()).cloned().collect::<Vec<_>>()"),
        "{}",
        translation.rust
    );
    cargo_test_translation(
        "stream_flat_map_lambda",
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
            streams.flatten(groups).unwrap(),
            vec!["a".to_string(), "b".to_string(), "c".to_string()]
        );
    }
}
"#,
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
            "values.iter().map(|value| { let trimmed = value.trim().to_string(); trimmed.to_lowercase() }).collect::<Vec<_>>()"
        ) && !translation.rust.contains("unsupported Java expression: lambda")
            && !translation.rust.contains(".cloned().collect::<Vec<_>>()"),
        "{}",
        translation.rust
    );
    cargo_test_translation(
        "stream_block_lambda",
        translation.rust,
        r#"
#[cfg(test)]
mod generated_tests {
    use super::*;

    #[test]
    fn stream_block_lambda_normalizes_values() {
        let streams = StreamBlockLambda {};
        assert_eq!(
            streams.normalize(vec![" A ".to_string(), "Bc".to_string()]).unwrap(),
            vec!["a".to_string(), "bc".to_string()]
        );
    }
}
"#,
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
            "groups.iter().flat_map(|group| group.names().unwrap().into_iter()).collect::<Vec<_>>()"
        ),
        "{}",
        translation.rust
    );
    cargo_test_translation(
        "stream_flat_map_projection",
        translation.rust,
        r#"
#[cfg(test)]
mod generated_tests {
    use super::*;

    #[test]
    fn stream_flat_map_projection_flattens_named_groups() {
        let streams = StreamFlatMapProjection {};
        let groups = vec![
            NameGroup::new(vec!["a".to_string(), "b".to_string()]),
            NameGroup::new(vec!["c".to_string()]),
        ];
        assert_eq!(
            streams.flatten(&groups).unwrap(),
            vec!["a".to_string(), "b".to_string(), "c".to_string()]
        );
    }
}
"#,
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
    assert!(translation.rust.contains(
        "pub fn clean(&self, values: Vec<String>) -> Result<Vec<String>, JavaException>"
    ));
    assert!(translation.rust.contains(
        "values.iter().filter(|v| !v.is_empty()).map(|v| v.trim().to_string()).collect::<Vec<_>>()"
    ));
    cargo_check_translation(
        "stream_collect_to_list_to_iterator_chain",
        &translation.rust,
    );
}

#[test]
fn stream_collect_generated_rust_preserves_behavior() {
    let translation = translate_source(&fixture("06-stdlib/StreamsCollect.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    cargo_test_translation(
        "streams_collect",
        translation.rust,
        r#"
#[cfg(test)]
mod generated_tests {
    use super::*;

    #[test]
    fn stream_collect_filters_and_trims() {
        let streams = StreamsCollect {};
        let values = vec![" one ".to_string(), "".to_string(), "two".to_string()];
        assert_eq!(
            streams.clean(values).unwrap(),
            vec!["one".to_string(), "two".to_string()]
        );
    }
}
"#,
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
            .contains("value.as_str().parse::<i32>().map_err")
            && translation
                .rust
                .contains("value.as_str().parse::<f64>().map_err")
            && !translation.rust.contains("Integer::parse_int")
            && !translation.rust.contains("Double::parse_double")
            && !translation.rust.contains("*value.as_str().parse"),
        "{}",
        translation.rust
    );
    cargo_test_translation(
        "stream_static_method_refs",
        translation.rust,
        r#"
#[cfg(test)]
mod generated_tests {
    use super::*;

    #[test]
    fn stream_static_method_references_parse_values() {
        let streams = StreamStaticMethodReferences {};
        assert_eq!(
            streams
                .parse_ints(vec!["1".to_string(), "2".to_string(), "3".to_string()])
                .unwrap(),
            vec![1, 2, 3]
        );
        assert_eq!(
            streams
                .parse_doubles(vec!["1.5".to_string(), "2.25".to_string()])
                .unwrap(),
            vec![1.5, 2.25]
        );
    }
}
"#,
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
            .contains("values.iter().map(|value| value.len() as i32).collect::<Vec<_>>()")
            && translation.rust.contains(
                "values.iter().map(|value| value.trim().to_string()).collect::<Vec<_>>()"
            )
            && translation
                .rust
                .contains("values.iter().any(|value| value.is_empty())")
            && !translation.rust.contains("String::length")
            && !translation.rust.contains("String::trim")
            && !translation
                .rust
                .contains(".into_iter().cloned().collect::<Vec<_>>()"),
        "{}",
        translation.rust
    );
    cargo_test_translation(
        "stream_string_method_refs",
        translation.rust,
        r#"
#[cfg(test)]
mod generated_tests {
    use super::*;

    #[test]
    fn stream_string_method_references_apply_native_string_calls() {
        let streams = StreamStringMethodReferences {};
        let values = vec![" a ".to_string(), "".to_string(), "bc".to_string()];
        assert_eq!(streams.lengths(values.clone()).unwrap(), vec![3, 0, 2]);
        assert_eq!(
            streams.trimmed(values.clone()).unwrap(),
            vec!["a".to_string(), "".to_string(), "bc".to_string()]
        );
        assert!(streams.any_empty(&values).unwrap());
    }
}
"#,
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
    assert!(translation.rust.contains(
        "pub fn first_long(&self, values: &[String]) -> Result<Option<String>, JavaException>"
    ));
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
    cargo_check_translation("stream_find_first_and_any_match", &translation.rust);
}

#[test]
fn stream_find_match_generated_rust_preserves_behavior() {
    let translation = translate_source(&fixture("06-stdlib/StreamsFindMatch.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    cargo_test_translation(
        "streams_find_match",
        translation.rust,
        r#"
#[cfg(test)]
mod generated_tests {
    use super::*;

    #[test]
    fn stream_find_first_and_any_match() {
        let streams = StreamsFindMatch {};
        let values = vec!["one".to_string(), "three".to_string(), "".to_string()];
        assert_eq!(streams.first_long(&values).unwrap(), Some("three".to_string()));
        assert!(streams.has_empty(&values).unwrap());
    }
}
"#,
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
    cargo_test_translation(
        "switch_fallthrough",
        translation.rust,
        r#"
#[cfg(test)]
mod generated_tests {
    use super::*;

    #[test]
    fn switch_scores_match_java_fallthrough() {
        let scorer = SwitchFallthrough {};
        assert_eq!(scorer.score("A"), 4);
        assert_eq!(scorer.score("B"), 2);
        assert_eq!(scorer.score("C"), 2);
        assert_eq!(scorer.score("D"), -1);
    }
}
"#,
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
            .contains("pub fn choose_string(&self, value: &str) -> String")
    );
    assert!(translation.rust.contains("self.choose_int(1)"));
    assert!(translation.rust.contains("self.choose_long(1i64)"));
    assert!(translation.rust.contains("self.choose_string(\"one\")"));
    cargo_check_translation("same_arity_overload_call_sites", &translation.rust);
}

#[test]
fn nested_static_builder_call_uses_resolved_type_owner() {
    let translation = translate_source(&fixture("06-stdlib/NestedStaticBuilderCall.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    assert!(translation.rust.contains(
        "pub type GenomicsDBColumn = NestedStaticBuilderCall_Coordinates_GenomicsDBColumn;"
    ));
    assert!(translation.rust.contains(
        "let builder: NestedStaticBuilderCall_Builder = GenomicsDBColumn::new_builder();"
    ));
    assert!(!translation.rust.contains("coordinates.genomics_d_b_column"));
    assert!(!translation.rust.contains("Coordinates::GenomicsDBColumn"));
    cargo_check_translation("nested_static_builder_resolved_owner", &translation.rust);
}

#[test]
fn stream_block_lambda_static_owner_uses_type_owner() {
    let translation = translate_source(&fixture("06-stdlib/StreamBlockStaticOwner.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    assert!(
        translation
            .rust
            .contains("let mut builder = GenomicsDBColumn::new_builder(); builder.set_name((*value).clone().clone());")
    );
    assert!(!translation.rust.contains("coordinates.genomics_d_b_column"));
    cargo_check_translation("stream_block_lambda_static_owner", &translation.rust);
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
            .contains("Vec::with_capacity((self.names.as_ref().unwrap().len() as i32) as usize)")
    );
    assert!(!translation.rust.contains("self.names.size()"));
    cargo_test_translation(
        "collection_capacity_constructor",
        translation.rust,
        r#"
#[cfg(test)]
mod generated_tests {
    use super::*;

    #[test]
    fn collection_capacity_constructor_compiles_and_returns_empty_headers() {
        let subject = CollectionCapacityConstructor {
            names: Some(vec!["id".to_string(), "sample".to_string()]),
        };
        assert_eq!(subject.copy_headers(), Vec::<String>::new());
    }
}
"#,
    );
}

#[test]
fn checked_propagation_string_length_after_fallible_optional_read_compiles() {
    let translation = translate_source(&fixture("06-stdlib/CheckedPropagation.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    assert!(
        translation
            .rust
            .contains("reader.read_line()?.as_ref().ok_or_else(|| JavaException::new(\"NullPointerException\", \"Java null receiver\".to_string()))?.as_str().len() as i32"),
        "{}",
        translation.rust
    );
    assert!(
        !translation
            .rust
            .contains(".unwrap().expect(\"Java null receiver\").as_str().len()"),
        "{}",
        translation.rust
    );
    cargo_check_translation("checked_propagation", &translation.rust);
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
    cargo_check_translation("collectors_to_map_lowers_to_hash_map", &translation.rust);
}

#[test]
fn guardrail_fixtures_emit_structured_unsupported_diagnostics() {
    let cases = [(
        "04-objects/InheritanceAndSuper.java",
        "unsupported_inheritance",
    )];

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
                .any(|d| matches!(d.severity(), Severity::Unsupported))
        );
    }
}

#[test]
fn anonymous_interface_return_uses_boxed_dyn_trait_slot() {
    let translation = translate_source(&fixture("04-objects/AnonymousInterface.java"));

    assert!(
        !translation
            .diagnostics
            .has_code("unsupported_anonymous_class"),
        "{:?}",
        translation.diagnostics
    );
    assert!(
        translation
            .rust
            .contains("pub fn positive(&self) -> Box<dyn AnonymousInterface_IntPredicate>")
            && translation
                .rust
                .contains("struct __JavaAnonymousAnonymousInterface_IntPredicate_test")
            && translation
                .rust
                .contains("__call: std::rc::Rc<dyn Fn(i32) -> bool>")
            && !translation
                .rust
                .contains("pub fn positive(&self) -> AnonymousInterface_IntPredicate")
            && !translation
                .rust
                .contains("<dyn AnonymousInterface_IntPredicate>::anonymous()")
            && !translation.rust.contains("return Default::default();"),
        "{}",
        translation.rust
    );

    cargo_test_translation(
        "anonymous_interface_return",
        translation.rust,
        r#"
#[test]
fn anonymous_predicate_invokes_its_body() {
    let predicate = AnonymousInterface {}.positive();
    assert!(predicate.test(1));
    assert!(!predicate.test(0));
}
"#,
    );
}

#[test]
fn stateful_anonymous_functional_interface_preserves_this_field() {
    let translation = translate_source(
        r#"
interface Adder { int add(int value); }
class StatefulAnonymous {
    static Adder make() {
        return new Adder() {
            int base = 4;
            public int add(int value) { return this.base + value; }
        };
    }
}
"#,
    );
    assert!(
        !translation
            .diagnostics
            .has_code("unsupported_anonymous_class"),
        "{:?}\n{}",
        translation.diagnostics,
        translation.rust
    );
    assert!(
        translation.rust.contains("struct __JavaAnonymousAdder_add")
            && translation.rust.contains("base: i32"),
        "{}",
        translation.rust
    );
    cargo_test_translation(
        "stateful_anonymous_functional_interface",
        translation.rust,
        r#"
#[test]
fn anonymous_state_is_read_through_this() {
    assert_eq!(StatefulAnonymous::make().add(3), 7);
}
"#,
    );
}

#[test]
fn stateful_anonymous_functional_interface_rebinds_nested_bare_field_reads() {
    let translation = translate_source(
        r#"
interface Selector { int choose(int value); }
class NestedFunctionalAnonymousSelector {
    static Selector make() {
        return new Selector() {
            int fallback = 9;
            public int choose(int value) { return value > 0 ? fallback + value : fallback; }
        };
    }
}
"#,
    );
    assert!(
        !translation
            .diagnostics
            .has_code("unsupported_anonymous_class"),
        "{:?}\n{}",
        translation.diagnostics,
        translation.rust
    );
    assert!(
        translation
            .rust
            .contains("struct __JavaAnonymousSelector_choose"),
        "{}",
        translation.rust
    );
    cargo_test_translation(
        "nested_functional_anonymous_selector",
        translation.rust,
        r#"
#[test]
fn nested_bare_field_reads_use_single_method_adapter_state() {
    let selector = NestedFunctionalAnonymousSelector::make();
    assert_eq!(selector.choose(2), 11);
    assert_eq!(selector.choose(0), 9);
}
"#,
    );
}

#[test]
fn stateful_anonymous_counter_preserves_mutable_this_field() {
    let translation = translate_source(
        r#"
interface Counter { int add(int value); }
class StatefulAnonymousCounter {
    static Counter make() {
        return new Counter() {
            int total = 1;
            public int add(int value) { this.total += value; return total; }
        };
    }
}
"#,
    );
    assert!(
        !translation
            .diagnostics
            .has_code("unsupported_anonymous_class"),
        "{:?}\n{}",
        translation.diagnostics,
        translation.rust
    );
    assert!(
        translation.rust.contains("total: std::cell::Cell<i32>"),
        "{:?}\n{}",
        translation.diagnostics,
        translation.rust
    );
    cargo_test_translation(
        "stateful_anonymous_counter",
        translation.rust,
        r#"
#[test]
fn anonymous_counter_retains_object_state() {
    let counter = StatefulAnonymousCounter::make();
    assert_eq!(counter.add(2), 3);
    assert_eq!(counter.add(4), 7);
}
"#,
    );
}

#[test]
fn stateful_anonymous_counter_rebinds_camel_case_source_field() {
    let translation = translate_source(
        r#"
interface Counter { int add(int value); }
class CamelCaseAnonymousCounter {
    static Counter make() {
        return new Counter() {
            int totalValue = 1;
            public int add(int value) { this.totalValue += value; return totalValue; }
        };
    }
}
"#,
    );
    assert!(
        !translation
            .diagnostics
            .has_code("unsupported_anonymous_class"),
        "{:?}\n{}",
        translation.diagnostics,
        translation.rust
    );
    assert!(
        translation
            .rust
            .contains("total_value: std::cell::Cell<i32>")
    );
    cargo_test_translation(
        "camel_case_stateful_anonymous_counter",
        translation.rust,
        r#"
#[test]
fn source_field_spelling_rebinds_before_rust_snake_case() {
    let counter = CamelCaseAnonymousCounter::make();
    assert_eq!(counter.add(2), 3);
    assert_eq!(counter.add(4), 7);
}
"#,
    );
}

#[test]
fn shared_mutating_interface_implementation_satisfies_generic_bound() {
    let translation = translate_source(
        r#"
interface Counter {
    void increment();
    int value();
}
class CounterImpl implements Counter {
    int n;
    public void increment() { n = n + 1; }
    public int value() { return n; }
}
class CounterUse {
    static <T extends Counter> int tick(T counter) {
        counter.increment();
        return counter.value();
    }
    static int run() {
        CounterImpl first = new CounterImpl();
        CounterImpl second = first;
        return tick(first) + second.value();
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
            .contains("Counter for std::rc::Rc<std::cell::RefCell<__JavaShared>>"),
        "{}",
        translation.rust
    );
    assert!(
        translation
            .rust
            .contains("fn increment(&mut self) { self.borrow_mut().increment() }"),
        "{}",
        translation.rust
    );
    cargo_test_translation(
        "shared_mutating_interface_bound",
        translation.rust,
        r#"
#[test]
fn generic_bound_keeps_the_shared_counter_identity() {
    assert_eq!(CounterUse::run().unwrap(), 2);
}
"#,
    );
}

#[test]
fn shared_sync_mutating_interface_implementation_satisfies_generic_bound() {
    let translation = translate_source(
        r#"
interface Counter {
    void increment();
    int value();
}
class SyncCounterImpl implements Counter, Runnable {
    int n;
    public void increment() { n = n + 1; }
    public int value() { return n; }
    public void run() { }
}
class SyncCounterUse {
    static <T extends Counter> int tick(T counter) {
        counter.increment();
        return counter.value();
    }
    static int run() {
        SyncCounterImpl first = new SyncCounterImpl();
        SyncCounterImpl second = first;
        return tick(first) + second.value();
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
            .contains("Counter for std::sync::Arc<std::sync::Mutex<__JavaShared>>"),
        "{}",
        translation.rust
    );
    cargo_test_translation(
        "shared_sync_mutating_interface_bound",
        translation.rust,
        r#"
#[test]
fn generic_bound_keeps_shared_sync_counter_identity() {
    assert_eq!(SyncCounterUse::run().unwrap(), 2);
}
"#,
    );
}

#[test]
fn shared_sync_interface_getter_preserves_mutable_item_handle() {
    let translation = translate_source(
        r#"
class Item {
    int value;
    Item(int value) { this.value = value; }
    void bump() { value++; }
    int value() { return value; }
}
interface ItemSource { Item item(); }
class SyncItemSource implements ItemSource, Runnable {
    Item stored = new Item(1);
    int runs;
    public Item item() { return stored; }
    public void run() { runs++; }
}
class SyncItemSourceUse {
    static <T extends ItemSource> int bump(T source) {
        Item item = source.item();
        item.bump();
        return item.value();
    }
    static int run() {
        SyncItemSource first = new SyncItemSource();
        SyncItemSource second = first;
        first.run();
        bump(first);
        return second.item().value();
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
            .contains("ItemSource for std::sync::Arc<std::sync::Mutex<__JavaShared>>"),
        "{}",
        translation.rust
    );
    cargo_test_translation(
        "shared_sync_interface_mutable_item_getter",
        translation.rust,
        r#"
#[test]
fn getter_through_sync_handle_keeps_the_mutable_item_identity() {
    assert_eq!(SyncItemSourceUse::run().unwrap(), 2);
}
"#,
    );
}

#[test]
fn shared_interface_getter_preserves_mutable_item_handle() {
    let translation = translate_source(
        r#"
class Item {
    int value;
    Item(int value) { this.value = value; }
    void bump() { value++; }
    int value() { return value; }
}
interface ItemSource { Item item(); }
class SharedItemSource implements ItemSource, Runnable {
    Item stored = new Item(1);
    public Item item() { return stored; }
    public void run() { }
}
class SharedItemSourceUse {
    static <T extends ItemSource> int bump(T source) {
        Item item = source.item();
        item.bump();
        return item.value();
    }
    static int run() {
        SharedItemSource first = new SharedItemSource();
        SharedItemSource second = first;
        bump(first);
        return second.item().value();
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
            .contains("std::rc::Rc<std::cell::RefCell<Item>>"),
        "{}",
        translation.rust
    );
    assert!(
        !translation.rust.contains("stored: Box<dyn std::any::Any>"),
        "{}",
        translation.rust
    );
    cargo_test_translation(
        "shared_interface_mutable_item_getter",
        translation.rust,
        r#"
#[test]
fn getter_round_trip_keeps_the_mutable_item_identity() {
    assert_eq!(SharedItemSourceUse::run().unwrap(), 2);
}
"#,
    );
}

#[test]
fn stateful_anonymous_interface_helper_mutates_instance_field() {
    let translation = translate_source(
        r#"
interface Counter { int next(); int current(); }
class AnonymousCounter {
    static Counter make() {
        return new Counter() {
            int value = 0;
            int bump() { this.value = this.value + 1; return value; }
            public int next() { return this.bump(); }
            public int current() { return this.value; }
        };
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
    cargo_test_translation(
        "stateful_anonymous_this_helper_mutation",
        translation.rust,
        r#"
#[test]
fn anonymous_helper_mutates_its_instance_field() {
    let counter = AnonymousCounter::make();
    assert_eq!(counter.next(), 1);
    assert_eq!(counter.current(), 1);
    assert_eq!(counter.next(), 2);
}
"#,
    );
}

#[test]
fn stateful_anonymous_interface_helper_reads_instance_field() {
    let translation = translate_source(
        r#"
interface PairValues {
    int one();
    int twice();
}
class AnonymousPairValues {
    static PairValues make() {
        return new PairValues() {
            int base = 4;
            int value() { return this.base; }
            public int one() { return this.value(); }
            public int twice() { return this.value() * 2; }
        };
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
    cargo_test_translation(
        "stateful_anonymous_this_helper_field",
        translation.rust,
        r#"
#[test]
fn anonymous_helper_reads_its_instance_field() {
    let values = AnonymousPairValues::make();
    assert_eq!(values.one(), 4);
    assert_eq!(values.twice(), 8);
}
"#,
    );
}

#[test]
fn fieldless_anonymous_interface_supports_this_helper_parameters() {
    let translation = translate_source(
        r#"
interface PairValues {
    int one();
    int twice();
}
class AnonymousPairValues {
    static PairValues make() {
        return new PairValues() {
            int plus(int value) { return value + 3; }
            public int one() { return this.plus(1); }
            public int twice() { return this.plus(5); }
        };
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
    cargo_test_translation(
        "fieldless_anonymous_this_helper_parameter",
        translation.rust,
        r#"
#[test]
fn anonymous_helper_accepts_primitive_parameters() {
    let values = AnonymousPairValues::make();
    assert_eq!(values.one(), 4);
    assert_eq!(values.twice(), 8);
}
"#,
    );
}

#[test]
fn fieldless_anonymous_interface_supports_this_helper_method() {
    let translation = translate_source(
        r#"
interface PairValues {
    int one();
    int twice();
}
class AnonymousPairValues {
    static PairValues make() {
        return new PairValues() {
            int base() { return 4; }
            public int one() { return this.base(); }
            public int twice() { return this.base() * 2; }
        };
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
    cargo_test_translation(
        "fieldless_anonymous_this_helper",
        translation.rust,
        r#"
#[test]
fn anonymous_helper_uses_its_own_receiver() {
    let values = AnonymousPairValues::make();
    assert_eq!(values.one(), 4);
    assert_eq!(values.twice(), 8);
}
"#,
    );
}

#[test]
fn fieldless_multi_method_anonymous_interface_is_object_backed() {
    let translation = translate_source(
        r#"
interface PairOps {
    int add(int left, int right);
    int subtract(int left, int right);
}
class AnonymousPairOps {
    static PairOps make() {
        return new PairOps() {
            public int add(int left, int right) { return left + right; }
            public int subtract(int left, int right) { return left - right; }
        };
    }
}
"#,
    );
    assert!(
        !translation
            .diagnostics
            .has_code("unsupported_anonymous_class"),
        "{:?}\n{}",
        translation.diagnostics,
        translation.rust
    );
    assert!(
        translation
            .rust
            .contains("struct __JavaAnonymousPairOps_object"),
        "{}",
        translation.rust
    );
    assert!(!translation.rust.contains("<dyn PairOps>::anonymous()"));
    cargo_test_translation(
        "fieldless_multi_method_anonymous_interface",
        translation.rust,
        r#"
#[test]
fn anonymous_object_implements_each_interface_method() {
    let ops = AnonymousPairOps::make();
    assert_eq!(ops.add(7, 4), 11);
    assert_eq!(ops.subtract(7, 4), 3);
}
"#,
    );
}

#[test]
fn fieldless_anonymous_interface_inherits_default_method() {
    let translation = translate_source(
        r#"
interface Named {
    int value();
    void add(int delta);
    default int doubled() { return value() * 2; }
}
interface Offset {
    int offset();
    default int shifted() { return offset() + 1; }
}
interface Scaled extends Named, Offset {
    int value();
    void add(int delta);
    int viaSuper();
    default int tripled() { return value() * 3; }
}
class InheritedAnonymousValue {
    static Scaled make() {
        return new Scaled() {
            int value = 7;
            public int value() { return value; }
            public void add(int delta) { this.value += delta; }
            public int offset() { return 3; }
            public int viaSuper() { return Scaled.super.tripled(); }
        };
    }
}
"#,
    );
    assert!(
        !translation.diagnostics.has_errors()
            && translation
                .rust
                .contains("struct __JavaAnonymousScaled_object"),
        "{:?}\n{}",
        translation.diagnostics,
        translation.rust
    );
    cargo_test_translation(
        "fieldless_anonymous_inherited_default",
        translation.rust,
        r#"
#[test]
fn anonymous_adapter_uses_inherited_default_methods() {
    let value = InheritedAnonymousValue::make();
    assert_eq!(value.value(), 7);
    assert_eq!(value.doubled(), 14);
    assert_eq!(value.tripled(), 21);
    assert_eq!(value.shifted(), 4);
    assert_eq!(value.via_super(), 21);
    value.add(2);
    assert_eq!(value.doubled(), 18);
    assert_eq!(value.tripled(), 27);
    assert_eq!(value.via_super(), 27);
    assert_eq!(value.shifted(), 4);
}
"#,
    );
}

#[test]
fn stateful_multi_method_anonymous_interface_preserves_instance_fields() {
    let translation = translate_source(
        r#"
interface PairOps {
    int add(int left, int right);
    int subtract(int left, int right);
}
class StatefulAnonymousPairOps {
    static PairOps make() {
        return new PairOps() {
            int offset = 5;
            public int add(int left, int right) { return left + right + this.offset; }
            public int subtract(int left, int right) { return left - right + offset; }
        };
    }
}
"#,
    );
    assert!(
        !translation
            .diagnostics
            .has_code("unsupported_anonymous_class"),
        "{:?}\n{}",
        translation.diagnostics,
        translation.rust
    );
    assert!(
        translation.rust.contains("offset: i32"),
        "{}",
        translation.rust
    );
    cargo_test_translation(
        "stateful_multi_method_anonymous_interface",
        translation.rust,
        r#"
#[test]
fn anonymous_object_keeps_one_shared_instance_field_for_all_methods() {
    let ops = StatefulAnonymousPairOps::make();
    assert_eq!(ops.add(7, 4), 16);
    assert_eq!(ops.subtract(7, 4), 8);
}
"#,
    );
}

#[test]
fn mutable_stateful_multi_method_anonymous_interface_preserves_counter() {
    let translation = translate_source(
        r#"
interface Counter {
    void add(int value);
    int value();
}
class MutableAnonymousCounter {
    static Counter make() {
        return new Counter() {
            int total = 1;
            public void add(int value) { this.total += value; }
            public int value() { return total; }
        };
    }
}
"#,
    );
    assert!(
        !translation
            .diagnostics
            .has_code("unsupported_anonymous_class"),
        "{:?}\n{}",
        translation.diagnostics,
        translation.rust
    );
    assert!(
        translation.rust.contains("total: std::cell::Cell<i32>"),
        "{}",
        translation.rust
    );
    cargo_test_translation(
        "mutable_stateful_multi_method_anonymous_interface",
        translation.rust,
        r#"
#[test]
fn anonymous_counter_state_is_shared_between_mutator_and_reader() {
    let counter = MutableAnonymousCounter::make();
    assert_eq!(counter.value(), 1);
    counter.add(2);
    counter.add(4);
    assert_eq!(counter.value(), 7);
}
"#,
    );
}

#[test]
fn mutable_stateful_multi_method_anonymous_interface_returns_updated_counter() {
    let translation = translate_source(
        r#"
interface Counter {
    int add(int value);
    int value();
}
class ReturningMutableAnonymousCounter {
    static Counter make() {
        return new Counter() {
            int total = 1;
            public int add(int value) { this.total += value; return total; }
            public int value() { return total; }
        };
    }
}
"#,
    );
    assert!(
        !translation
            .diagnostics
            .has_code("unsupported_anonymous_class"),
        "{:?}\n{}",
        translation.diagnostics,
        translation.rust
    );
    assert!(
        translation.rust.contains("total: std::cell::Cell<i32>"),
        "{}",
        translation.rust
    );
    cargo_test_translation(
        "returning_mutable_multi_method_anonymous_interface",
        translation.rust,
        r#"
#[test]
fn anonymous_counter_returns_and_retains_the_updated_value() {
    let counter = ReturningMutableAnonymousCounter::make();
    assert_eq!(counter.add(2), 3);
    assert_eq!(counter.add(4), 7);
    assert_eq!(counter.value(), 7);
}
"#,
    );
}

#[test]
fn mutable_stateful_multi_method_anonymous_interface_preserves_boolean_field() {
    let translation = translate_source(
        r#"
interface Flag {
    void set(boolean value);
    boolean value();
}

class MutableAnonymousFlag {
    static Flag make() {
        return new Flag() {
            boolean enabled = false;
            public void set(boolean value) { this.enabled = value; }
            public boolean value() { return enabled; }
        };
    }
}
"#,
    );
    assert!(
        !translation
            .diagnostics
            .has_code("unsupported_anonymous_class"),
        "{:?}\n{}",
        translation.diagnostics,
        translation.rust
    );
    assert!(
        translation.rust.contains("enabled: std::cell::Cell<bool>"),
        "{}",
        translation.rust
    );
    cargo_test_translation(
        "mutable_boolean_multi_method_anonymous_interface",
        translation.rust,
        r#"
#[test]
fn anonymous_boolean_field_persists_across_methods() {
    let flag = MutableAnonymousFlag::make();
    assert!(!flag.value());
    flag.set(true);
    assert!(flag.value());
}
"#,
    );
}

#[test]
fn mutable_stateful_anonymous_interface_allows_guarded_primitive_update() {
    let translation = translate_source(
        r#"
interface Counter {
    void addIf(boolean enabled, int value);
    int value();
}
class GuardedMutableAnonymousCounter {
    static Counter make() {
        return new Counter() {
            int total = 1;
            public void addIf(boolean enabled, int value) {
                if (enabled) { this.total += value; }
            }
            public int value() { return total; }
        };
    }
}
"#,
    );
    assert!(
        !translation
            .diagnostics
            .has_code("unsupported_anonymous_class"),
        "{:?}\n{}",
        translation.diagnostics,
        translation.rust
    );
    assert!(
        translation.rust.contains("total: std::cell::Cell<i32>"),
        "{}",
        translation.rust
    );
    cargo_test_translation(
        "guarded_mutable_anonymous_interface",
        translation.rust,
        r#"
#[test]
fn guarded_update_keeps_the_adapter_instance_state() {
    let counter = GuardedMutableAnonymousCounter::make();
    counter.add_if(false, 9);
    assert_eq!(counter.value(), 1);
    counter.add_if(true, 4);
    assert_eq!(counter.value(), 5);
}
"#,
    );
}

#[test]
fn mutable_stateful_anonymous_interface_allows_two_branch_primitive_update() {
    let translation = translate_source(
        r#"
interface Counter {
    void add(boolean enabled, int value);
    int value();
}
class TwoBranchMutableAnonymousCounter {
    static Counter make() {
        return new Counter() {
            int total = 10;
            public void add(boolean enabled, int value) {
                if (enabled) { this.total += value; } else { this.total -= value; }
            }
            public int value() { return total; }
        };
    }
}
"#,
    );
    assert!(
        !translation
            .diagnostics
            .has_code("unsupported_anonymous_class"),
        "{:?}\n{}",
        translation.diagnostics,
        translation.rust
    );
    cargo_test_translation(
        "two_branch_mutable_anonymous_interface",
        translation.rust,
        r#"
#[test]
fn both_branches_update_one_adapter_cell() {
    let counter = TwoBranchMutableAnonymousCounter::make();
    counter.add(true, 7);
    counter.add(false, 4);
    assert_eq!(counter.value(), 13);
}
"#,
    );
}

#[test]
fn mutable_stateful_anonymous_interface_reads_cell_backed_condition_field() {
    let translation = translate_source(
        r#"
interface Counter {
    void enable();
    void add(int value);
    int value();
}
class ConditionalMutableAnonymousCounter {
    static Counter make() {
        return new Counter() {
            boolean enabled = false;
            int total = 1;
            public void enable() { this.enabled = true; }
            public void add(int value) { if (this.enabled) { this.total += value; } }
            public int value() { return total; }
        };
    }
}
"#,
    );
    assert!(
        !translation
            .diagnostics
            .has_code("unsupported_anonymous_class"),
        "{:?}\n{}",
        translation.diagnostics,
        translation.rust
    );
    cargo_test_translation(
        "conditional_mutable_anonymous_interface",
        translation.rust,
        r#"
#[test]
fn guarded_update_reads_another_cell_backed_field() {
    let counter = ConditionalMutableAnonymousCounter::make();
    counter.add(9);
    assert_eq!(counter.value(), 1);
    counter.enable();
    counter.add(4);
    assert_eq!(counter.value(), 5);
}
"#,
    );
}

#[test]
fn mutable_stateful_anonymous_interface_returns_after_guarded_update() {
    let translation = translate_source(
        r#"
interface Counter {
    int addIf(boolean enabled, int value);
    int value();
}
class ReturningGuardedMutableAnonymousCounter {
    static Counter make() {
        return new Counter() {
            int total = 1;
            public int addIf(boolean enabled, int value) {
                if (enabled) { this.total += value; }
                return total;
            }
            public int value() { return total; }
        };
    }
}
"#,
    );
    assert!(
        !translation
            .diagnostics
            .has_code("unsupported_anonymous_class"),
        "{:?}\n{}",
        translation.diagnostics,
        translation.rust
    );
    cargo_test_translation(
        "returning_guarded_mutable_anonymous_interface",
        translation.rust,
        r#"
#[test]
fn guarded_mutator_returns_the_current_adapter_cell_value() {
    let counter = ReturningGuardedMutableAnonymousCounter::make();
    assert_eq!(counter.add_if(false, 9), 1);
    assert_eq!(counter.add_if(true, 4), 5);
    assert_eq!(counter.value(), 5);
}
"#,
    );
}

#[test]
fn mutable_stateful_anonymous_interface_allows_guarded_string_update() {
    let translation = translate_source(
        r#"
interface Label {
    String setIf(boolean enabled, String next);
    String value();
}
class GuardedMutableAnonymousLabel {
    static Label make() {
        return new Label() {
            String label = "initial";
            public String setIf(boolean enabled, String next) {
                if (enabled) { this.label = next; }
                return label;
            }
            public String value() { return label; }
        };
    }
}
"#,
    );
    assert!(
        !translation
            .diagnostics
            .has_code("unsupported_anonymous_class"),
        "{:?}\n{}",
        translation.diagnostics,
        translation.rust
    );
    assert!(
        translation
            .rust
            .contains("label: std::cell::RefCell<String>"),
        "{}",
        translation.rust
    );
    cargo_test_translation(
        "guarded_mutable_anonymous_string",
        translation.rust,
        r#"
#[test]
fn guarded_string_update_returns_owned_current_value() {
    let label = GuardedMutableAnonymousLabel::make();
    assert_eq!(label.set_if(false, "unused".to_string()), "initial");
    assert_eq!(label.set_if(true, "next".to_string()), "next");
    assert_eq!(label.value(), "next");
}
"#,
    );
}

#[test]
fn mutable_stateful_multi_method_anonymous_interface_preserves_string_field() {
    let translation = translate_source(
        r#"
interface Label {
    void set(String next);
    String value();
}
class MutableAnonymousLabel {
    static Label make() {
        return new Label() {
            String label = "initial";
            public void set(String next) { this.label = next; }
            public String value() { return label; }
        };
    }
}
"#,
    );
    assert!(
        !translation
            .diagnostics
            .has_code("unsupported_anonymous_class"),
        "{:?}\n{}",
        translation.diagnostics,
        translation.rust
    );
    assert!(
        translation
            .rust
            .contains("label: std::cell::RefCell<String>"),
        "{}",
        translation.rust
    );
    cargo_test_translation(
        "mutable_string_multi_method_anonymous_interface",
        translation.rust,
        r#"
#[test]
fn anonymous_string_field_persists_across_methods() {
    let label = MutableAnonymousLabel::make();
    assert_eq!(label.value(), "initial");
    label.set("next".to_string());
    assert_eq!(label.value(), "next");
}
"#,
    );
}

#[test]
fn mutable_stateful_multi_method_anonymous_interface_returns_updated_string() {
    let translation = translate_source(
        r#"
interface Label {
    String set(String next);
    String value();
}
class ReturningMutableAnonymousLabel {
    static Label make() {
        return new Label() {
            String label = "initial";
            public String set(String next) { this.label = next; return label; }
            public String value() { return label; }
        };
    }
}
"#,
    );
    assert!(
        !translation
            .diagnostics
            .has_code("unsupported_anonymous_class"),
        "{:?}\n{}",
        translation.diagnostics,
        translation.rust
    );
    cargo_test_translation(
        "returning_mutable_string_multi_method_anonymous_interface",
        translation.rust,
        r#"
#[test]
fn anonymous_string_setter_returns_an_owned_updated_value() {
    let label = ReturningMutableAnonymousLabel::make();
    assert_eq!(label.set("next".to_string()), "next");
    assert_eq!(label.value(), "next");
}
"#,
    );
}

#[test]
fn mutable_stateful_anonymous_interface_appends_string_field() {
    let translation = translate_source(
        r#"
interface Label {
    String append(String suffix);
    String value();
}
class AppendingMutableAnonymousLabel {
    static Label make() {
        return new Label() {
            String label = "a";
            public String append(String suffix) { this.label = this.label + suffix; return label; }
            public String value() { return label; }
        };
    }
}
"#,
    );
    assert!(
        !translation
            .diagnostics
            .has_code("unsupported_anonymous_class"),
        "{:?}\n{}",
        translation.diagnostics,
        translation.rust
    );
    cargo_test_translation(
        "appending_mutable_string_anonymous_interface",
        translation.rust,
        r#"
#[test]
fn anonymous_string_field_reads_before_replacing() {
    let label = AppendingMutableAnonymousLabel::make();
    assert_eq!(label.append("b".to_string()), "ab");
    assert_eq!(label.append("c".to_string()), "abc");
    assert_eq!(label.value(), "abc");
}
"#,
    );
}

#[test]
fn mutable_stateful_multi_method_anonymous_interface_keeps_multiple_fields() {
    let translation = translate_source(
        r#"
interface State {
    void add(int value);
    int total();
    void setLabel(String next);
    String label();
}
class MultiFieldMutableAnonymousState {
    static State make() {
        return new State() {
            int total = 1;
            String label = "initial";
            public void add(int value) { this.total += value; }
            public int total() { return total; }
            public void setLabel(String next) { this.label = next; }
            public String label() { return label; }
        };
    }
}
"#,
    );
    assert!(
        !translation
            .diagnostics
            .has_code("unsupported_anonymous_class"),
        "{:?}\n{}",
        translation.diagnostics,
        translation.rust
    );
    assert!(
        translation.rust.contains("total: std::cell::Cell<i32>")
            && translation
                .rust
                .contains("label: std::cell::RefCell<String>"),
        "{}",
        translation.rust
    );
    cargo_test_translation(
        "multiple_mutable_fields_anonymous_interface",
        translation.rust,
        r#"
#[test]
fn anonymous_object_keeps_independent_mutable_fields() {
    let state = MultiFieldMutableAnonymousState::make();
    state.add(4);
    state.set_label("next".to_string());
    assert_eq!(state.total(), 5);
    assert_eq!(state.label(), "next");
}
"#,
    );
}

#[test]
fn mutable_multi_method_anonymous_objects_keep_distinct_instance_state() {
    let translation = translate_source(
        r#"
interface Counter {
    void add(int value);
    int value();
}
class DistinctAnonymousCounters {
    static Counter make() {
        return new Counter() {
            int total = 1;
            public void add(int value) { this.total += value; }
            public int value() { return total; }
        };
    }
}
"#,
    );
    assert!(
        !translation
            .diagnostics
            .has_code("unsupported_anonymous_class"),
        "{:?}\n{}",
        translation.diagnostics,
        translation.rust
    );
    cargo_test_translation(
        "distinct_mutable_multi_method_anonymous_objects",
        translation.rust,
        r#"
#[test]
fn factory_calls_create_distinct_anonymous_instances() {
    let first = DistinctAnonymousCounters::make();
    let second = DistinctAnonymousCounters::make();
    first.add(4);
    second.add(9);
    assert_eq!(first.value(), 5);
    assert_eq!(second.value(), 10);
}
"#,
    );
}

#[test]
fn stateful_anonymous_interface_allows_read_only_branching() {
    let translation = translate_source(
        r#"
interface Selector { int choose(int value); }
class BranchingAnonymousSelector {
    static Selector make() {
        return new Selector() {
            int fallback = 9;
            public int choose(int value) {
                if (value > 0) { return value + fallback; }
                return fallback;
            }
        };
    }
}
"#,
    );
    assert!(
        !translation
            .diagnostics
            .has_code("unsupported_anonymous_class"),
        "{:?}\n{}",
        translation.diagnostics,
        translation.rust
    );
    cargo_test_translation(
        "stateful_anonymous_interface_branching",
        translation.rust,
        r#"
#[test]
fn field_reads_survive_read_only_branching() {
    let selector = BranchingAnonymousSelector::make();
    assert_eq!(selector.choose(2), 11);
    assert_eq!(selector.choose(0), 9);
}
"#,
    );
}

#[test]
fn stateful_multi_method_anonymous_interface_rebinds_nested_field_reads() {
    let translation = translate_source(
        r#"
interface Selector {
    int choose(int value);
    int fallback();
}
class NestedFieldAnonymousSelector {
    static Selector make() {
        return new Selector() {
            int fallback = 9;
            public int choose(int value) { return value > 0 ? fallback + value : fallback; }
            public int fallback() { return fallback; }
        };
    }
}
"#,
    );
    assert!(
        !translation
            .diagnostics
            .has_code("unsupported_anonymous_class"),
        "{:?}\n{}",
        translation.diagnostics,
        translation.rust
    );
    cargo_test_translation(
        "nested_field_multi_method_anonymous_interface",
        translation.rust,
        r#"
#[test]
fn nested_read_only_expressions_use_anonymous_instance_state() {
    let selector = NestedFieldAnonymousSelector::make();
    assert_eq!(selector.choose(2), 11);
    assert_eq!(selector.choose(0), 9);
    assert_eq!(selector.fallback(), 9);
}
"#,
    );
}

#[test]
fn context_anonymous_action_with_sharedlocal_capture_is_not_reported_unsupported() {
    let action = r#"
        interface Action {
            void run();
        }
    "#;
    let translation = translate_source_with_context_sources(
        r#"
        public class ContextAnonymousSharedLocalAction {
            static class Node {
                int v;
            }

            static class Owner {
                int apply(Action action, Node node) {
                    action.run();
                    return node.v;
                }
            }

            public static String run() {
                Node a = new Node();
                Node b = a;
                Owner owner = new Owner();
                return String.valueOf(owner.apply(new Action() {
                    public void run() {
                        a.v = 23;
                    }
                }, b));
            }
        }
        "#,
        &[action.to_string()],
        &Default::default(),
    );

    assert!(
        !translation
            .diagnostics
            .has_code("unsupported_anonymous_class"),
        "{:?}\n{}",
        translation.diagnostics,
        translation.rust
    );
    assert!(
        translation.rust.contains("let a = a.clone()"),
        "{}",
        translation.rust
    );
    assert!(
        translation.rust.contains("std::boxed::Box<dyn Action>"),
        "{}",
        translation.rust
    );
    assert!(
        !translation.rust.contains("<dyn Action>::anonymous()"),
        "the supported zero-arg anonymous action should lower to the closure \
         adapter, not the loud anonymous-object fallback\n{}",
        translation.rust
    );

    cargo_test_generated_modules(
        "context_anonymous_sharedlocal_action",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
#[cfg(test)]
mod generated_tests {
    use super::*;

    #[test]
    fn captured_shared_local_mutation_is_observable_through_the_alias() {
        assert_eq!(ContextAnonymousSharedLocalAction::run().unwrap(), "23");
    }
}
"#,
        }],
    );
}

#[test]
fn first_slice_generated_rust_compiles() {
    let cases = [
        (
            "expression_precedence",
            "01-expressions/ExpressionPrecedence.java",
        ),
        (
            "loops_and_branches",
            "02-control-flow/LoopsAndBranches.java",
        ),
        (
            "switch_fallthrough",
            "02-control-flow/SwitchFallthrough.java",
        ),
        (
            "methods_and_overloads",
            "03-methods/MethodsAndOverloads.java",
        ),
        (
            "constructor_overloads",
            "03-methods/ConstructorOverloads.java",
        ),
        ("overload_same_arity", "03-methods/OverloadSameArity.java"),
        (
            "dyn_trait_method_compatibility",
            "04-objects/DynTraitMethodCompatibility.java",
        ),
        ("simple_class", "04-objects/SimpleClass.java"),
        ("nullable_local_return", "05-types/NullableLocalReturn.java"),
        ("throw_only", "06-stdlib/ThrowOnly.java"),
        ("streams_use", "06-stdlib/StreamsUse.java"),
        ("streams_collect", "06-stdlib/StreamsCollect.java"),
    ];

    let mut modules = Vec::new();
    for (name, path) in cases {
        let translation = translate_source(&fixture(path));
        assert!(
            !translation.diagnostics.has_errors(),
            "{path} diagnostics: {:?}",
            translation.diagnostics
        );
        modules.push(GeneratedCrateModule {
            name,
            rust: translation.rust,
            tests: "",
        });
    }
    cargo_check_generated_modules("first_slice_generated_rust", modules);
}

#[test]
fn dyn_trait_interface_parameters_use_trait_object_views() {
    let translation = translate_source(&fixture("04-objects/DynTraitMethodCompatibility.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    assert!(
        translation
            .rust
            .contains("fn merge(&mut self, other: &dyn Metric) -> Result<(), JavaException>;"),
        "{}",
        translation.rust
    );
    assert!(
        translation.rust.contains(
            "fn merge(&mut self, other: &dyn Metric) -> Result<(), JavaException> { CountMetric::merge(self, other) }"
        ),
        "{}",
        translation.rust
    );
    assert!(
        !translation.rust.contains("other: Metric"),
        "{}",
        translation.rust
    );
    cargo_check_translation("dyn_trait_interface_parameters", &translation.rust);
}

#[test]
fn raw_types_generated_rust_compiles_with_erased_list_elements() {
    let translation = translate_source(&fixture("05-types/RawTypes.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    assert!(
        translation
            .rust
            .contains("values.push(Box::new(\"text\".to_string()) as Box<dyn std::any::Any>);"),
        "{}",
        translation.rust
    );
    assert!(
        translation
            .rust
            .contains("values.push(Box::new(1) as Box<dyn std::any::Any>);"),
        "{}",
        translation.rust
    );
    assert!(
        translation.rust.contains(
            "values.get(0 as usize).ok_or_else(|| JavaException::new(\"IndexOutOfBoundsException\""
        ) && translation.rust.contains("uncaught Java exception")
            && !translation
                .rust
                .contains(".expect(\"Java index out of bounds\")"),
        "{}",
        translation.rust
    );

    cargo_check_translation("raw_types_erased_list_elements", &translation.rust);
}

#[test]
fn wildcard_bounds_use_read_upper_bound_and_write_erased_carrier() {
    let translation = translate_source(&fixture("05-types/WildcardBounds.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    assert!(
        translation.rust.contains(
            "pub fn read_extends(&self, values: &[Number]) -> Result<i32, JavaException>"
        ),
        "{}",
        translation.rust
    );
    assert!(
        translation
            .rust
            .contains("values.push(Box::new(3) as Box<dyn std::any::Any>);"),
        "{}",
        translation.rust
    );
    assert!(
        !translation.rust.contains("}?.ok_or_else("),
        "{}",
        translation.rust
    );

    cargo_check_translation("wildcard_bounds", &translation.rust);
}
