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

#[test]
fn stateful_multi_method_anonymous_interface_preserves_named_field_handle() {
    let translation = translate_source(
        r#"
interface Reader { int read(); int echo(); }
class Node { int value; Node(int value) { this.value = value; } }
class NamedFieldObject {
    static Reader make(Node incoming) {
        return new Reader() {
            Node node = incoming;
            public int read() { return this.node.value; }
            public int echo() { return this.node.value; }
        };
    }
    static int run() {
        Node node = new Node(61);
        Reader value = make(node);
        node.value = 67;
        return value.read() + value.echo();
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
        "stateful_multi_method_anonymous_named_field",
        translation.rust,
        r#"
#[test]
fn named_field_uses_shared_handle_in_object_adapter() {
    assert_eq!(NamedFieldObject::run(), 134);
}
"#,
    );
}

#[test]
fn stateful_anonymous_interface_writes_through_named_reference_field() {
    let translation = translate_source(
        r#"
interface Reader { void set(int value); int read(); }
class Node { int value; Node(int value) { this.value = value; } }
class NamedReferenceWriter {
    static Reader make(Node incoming) {
        return new Reader() {
            Node node = incoming;
            public void set(int value) { this.node.value = value; }
            public int read() { return this.node.value; }
        };
    }
    static int run() {
        Node node = new Node(3);
        Reader reader = make(node);
        reader.set(8);
        return node.value + reader.read();
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
        "stateful_anonymous_interface_named_reference_write",
        translation.rust,
        r#"
#[test]
fn adapter_and_caller_observe_the_same_reference_field_handle() {
    assert_eq!(NamedReferenceWriter::run(), 16);
}
"#,
    );
}

#[test]
fn anonymous_class_carrier_writes_through_named_reference_field() {
    let translation = translate_source(
        r#"
class Node { int value; Node(int value) { this.value = value; } }
class Base { Base() {} int write(boolean enabled, int value) { return value; } }
class AnonymousClassDirectMemberWrite {
    static Base make(Node incoming) {
        return new Base() {
            Node node = incoming;
            @Override int write(boolean enabled, int value) {
                int next = value + 1;
                if (enabled) { this.node.value = next; } else { this.node.value = next + 2; }
                return this.node.value;
            }
        };
    }
    static int run() {
        Node node = new Node(3);
        Base value = make(node);
        value.write(true, 23);
        return node.value;
    }
    static int skipped() {
        Node node = new Node(3);
        Base value = make(node);
        value.write(false, 23);
        return node.value;
    }
    static int looped() {
        Node node = new Node(3);
        Base value = new Base() {
            Node retained = node;
            @Override int write(boolean enabled, int input) {
                int remaining = 1;
                while (remaining > 0) {
                    this.retained.value = input;
                    remaining = remaining - 1;
                }
                return this.retained.value;
            }
        };
        value.write(true, 29);
        return node.value;
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
        "anonymous_class_carrier_named_reference_write",
        translation.rust,
        r#"
#[test]
fn carrier_and_caller_observe_the_same_reference_field_handle() {
    assert_eq!(AnonymousClassDirectMemberWrite::run(), 24);
    assert_eq!(AnonymousClassDirectMemberWrite::skipped(), 26);
    assert_eq!(AnonymousClassDirectMemberWrite::looped(), 29);
}
"#,
    );
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
class Parent {
    int inherited(int input) { return input + 1; }
}
class Base extends Parent {
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
        translation
            .rust
            .contains("struct __JavaAnonymousAdder_object")
            && translation.rust.contains("base: std::cell::Cell<i32>"),
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
            .contains("struct __JavaAnonymousSelector_object"),
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
fn stateful_anonymous_counter_combines_capture_with_mutable_instance_state() {
    let translation = translate_source(
        r#"
interface CapturedCounter { int add(int delta); }
class CapturedAnonymousCounter {
    static CapturedCounter make(int bonus) {
        return new CapturedCounter() {
            int total = 0;
            public int add(int delta) {
                this.total = total + delta + bonus;
                return total;
            }
        };
    }
    static int aliased() {
        CapturedCounter first = make(2);
        CapturedCounter second = first;
        first.add(1);
        return second.add(2);
    }
}
"#,
    );
    assert!(
        !translation
            .diagnostics
            .has_code("unsupported_anonymous_class"),
        "{:?}",
        translation.diagnostics
    );
    cargo_test_translation(
        "captured_stateful_anonymous_counter",
        translation.rust,
        r#"
#[test]
fn capture_and_counter_state_survive_successive_calls() {
    let counter = CapturedAnonymousCounter::make(2);
    assert_eq!(counter.add(1), 3);
    assert_eq!(counter.add(2), 7);
    assert_eq!(CapturedAnonymousCounter::aliased(), 7);
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
            .contains("fn increment(&mut self) { Counter::increment(&mut *self.borrow_mut()) }"),
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
fn fieldless_multi_method_anonymous_interface_keeps_factory_capture() {
    let translation = translate_source(
        r#"
interface CapturedPairOps {
    int first();
    int second(int input);
}
class AnonymousCapturedPairOps {
    static CapturedPairOps make(int bonus) {
        return new CapturedPairOps() {
            public int first() { return bonus; }
            public int second(int input) { return bonus + input; }
        };
    }
}
"#,
    );
    assert!(
        !translation
            .diagnostics
            .has_code("unsupported_anonymous_class"),
        "{:?}",
        translation.diagnostics
    );
    cargo_test_translation(
        "fieldless_captured_multi_method_anonymous_interface",
        translation.rust,
        r#"
#[test]
fn both_methods_read_the_escaped_factory_capture() {
    let value = AnonymousCapturedPairOps::make(5);
    assert_eq!(value.first() + value.second(4), 14);
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
fn anonymous_interface_collapses_matching_parent_contracts() {
    let translation = translate_source(
        r#"
interface Left { int shared(); }
interface Right { int shared(); }
interface Combined extends Left, Right {}
class CollidingAnonymousContracts {
    static Combined make() {
        return new Combined() {
            public int shared() { return 42; }
        };
    }
}
class CollidingConcreteContracts implements Combined {
    public int shared() { return 7; }
}
class CollidingConcreteFactory {
    static Combined make() { return new CollidingConcreteContracts(); }
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
        translation.rust.contains("pub trait Combined")
            && translation.rust.contains("fn shared(&self) -> i32;"),
        "{}",
        translation.rust
    );
    cargo_test_translation(
        "colliding_anonymous_interface_contracts",
        translation.rust,
        r#"
#[test]
fn one_java_implementation_serves_both_parent_contracts() {
    assert_eq!(CollidingAnonymousContracts::make().shared(), 42);
    assert_eq!(CollidingConcreteFactory::make().shared(), 7);
}
"#,
    );
}

#[test]
fn anonymous_interface_collisions_select_covariant_return_independent_of_parent_order() {
    let translation = translate_source(
        r#"
interface GeneralResult { Object value(); }
interface SpecificResult { String value(); }
interface GeneralFirst extends GeneralResult, SpecificResult {}
interface SpecificFirst extends SpecificResult, GeneralResult {}
class GeneralFirstConcrete implements GeneralFirst {
    public String value() { return "abc"; }
}
class SpecificFirstConcrete implements SpecificFirst {
    public String value() { return "def"; }
}
class CovariantAnonymousContracts {
    static GeneralFirst first() {
        return new GeneralFirst() { public String value() { return "hello"; } };
    }
    static SpecificFirst second() {
        return new SpecificFirst() { public String value() { return "world"; } };
    }
    static GeneralFirst concreteFirst() { return new GeneralFirstConcrete(); }
    static SpecificFirst concreteSecond() { return new SpecificFirstConcrete(); }
    static int run() {
        return first().value().length() + second().value().length()
            + concreteFirst().value().length() + concreteSecond().value().length();
    }
}
"#,
    );
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    cargo_test_translation(
        "anonymous_covariant_parent_contracts",
        translation.rust,
        r#"
#[test]
fn both_parent_orders_return_the_specific_java_type() {
    assert_eq!(CovariantAnonymousContracts::run(), 16);
}
"#,
    );
}

#[test]
fn anonymous_generic_parent_contract_specializes_adapter_and_upcast() {
    let translation = translate_source(
        r#"
interface Source<T> { T value(); T echo(T input); }
interface Relay<U> extends Source<U> {}
interface Named { String value(); }
interface Joined extends Relay<String>, Named {}
class GenericAnonymousContracts {
    static Joined make() {
        return new Joined() {
            public String value() { return "hello"; }
            public String echo(String input) { return input; }
        };
    }
    static int run() {
        Source<String> source = make();
        Joined joined = make();
        return source.value().length() + joined.value().length()
            + joined.echo("hi").length();
    }
}

"#,
    );
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    cargo_test_translation(
        "anonymous_generic_parent_contract",
        translation.rust,
        r#"
#[test]
fn generic_parent_adapter_and_upcast_preserve_string_contract() {
    assert_eq!(GenericAnonymousContracts::run(), 12);
}
"#,
    );
}

#[test]
fn anonymous_generic_parent_default_method_preserves_specialized_contract() {
    let translation = translate_source(
        r#"
interface SourceWithDefault<T> {
    T value();
    default T forwarded() { return value(); }
    default int plus(int input) { return input + 5; }
}
interface StringSource extends SourceWithDefault<String> {}
class GenericAnonymousDefault {
    static StringSource make() {
        return new StringSource() { public String value() { return "hello"; } };
    }
    static int run() {
        StringSource source = make();
        SourceWithDefault<String> parent = source;
        return source.forwarded().length() + parent.forwarded().length();
    }
    static int runArgs() {
        StringSource source = make();
        SourceWithDefault<String> parent = source;
        return source.plus(2) + parent.plus(3);
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
        "anonymous_generic_parent_default",
        translation.rust,
        r#"
#[test]
fn generic_parent_default_dispatches_through_the_anonymous_object() {
    assert_eq!(GenericAnonymousDefault::run(), 10);
    assert_eq!(GenericAnonymousDefault::run_args(), 15);
}
"#,
    );
}

#[test]
fn anonymous_interface_default_method_uses_canonical_colliding_parent_contract() {
    let translation = translate_source(
        r#"
interface Left { int shared(); }
interface Right { int shared(); }
interface Combined extends Left, Right {
    default int incremented() { return shared() + 1; }
}
class DefaultCollidingAnonymousContracts {
    static Combined make() {
        return new Combined() { public int shared() { return 42; } };
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
        "default_colliding_anonymous_interface_contracts",
        translation.rust,
        r#"
#[test]
fn default_method_sees_the_single_canonical_parent_contract() {
    let value = DefaultCollidingAnonymousContracts::make();
    assert_eq!(value.shared(), 42);
    assert_eq!(value.incremented(), 43);
}
"#,
    );
}

#[test]
fn anonymous_interface_upcasts_to_parent_at_source_call_argument() {
    let translation = translate_source(
        r#"
interface Left { int shared(); }
interface Right { int shared(); }
interface Combined extends Left, Right {}
class InterfaceUpcast {
    static Combined make() {
        return new Combined() { public int shared() { return 42; } };
    }
    static int take(Left value) { return value.shared(); }
    static int run() { return take(make()); }
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
        "anonymous_interface_parent_upcast",
        translation.rust,
        r#"
#[test]
fn anonymous_derived_interface_is_accepted_by_parent_parameter() {
    assert_eq!(InterfaceUpcast::run().unwrap(), 42);
}
"#,
    );
}

#[test]
fn anonymous_interface_upcasts_to_parent_at_local_assignment() {
    let translation = translate_source(
        r#"
interface ParentReading { int value(); }
interface OtherReading { int other(); }
interface ChildReading extends ParentReading, OtherReading {}
class LocalInterfaceUpcast {
    static ChildReading make() {
        return new ChildReading() {
            public int value() { return 42; }
            public int other() { return 7; }
        };
    }
    static int run() {
        ParentReading parent = make();
        OtherReading other = make();
        return parent.value() + other.other();
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
        "anonymous_interface_local_parent_upcast",
        translation.rust,
        r#"
#[test]
fn declared_parent_local_keeps_parent_dispatch() {
    assert_eq!(LocalInterfaceUpcast::run(), 49);
}
"#,
    );
}

#[test]
fn anonymous_interface_redeclared_parent_contract_has_both_trait_impls() {
    let translation = translate_source(
        r#"
interface BaseReading { int value(); }
interface RefinedReading extends BaseReading { int value(); }
class RedeclaredInterfaceContract {
    static RefinedReading make() {
        return new RefinedReading() { public int value() { return 43; } };
    }
    static int run() { return make().value(); }
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
        "anonymous_interface_redeclared_parent_contract",
        translation.rust,
        r#"
#[test]
fn child_and_parent_redeclaration_share_java_implementation() {
    assert_eq!(RedeclaredInterfaceContract::run(), 43);
}
"#,
    );
}

#[test]
fn anonymous_interface_matches_overloads_by_erased_parameter_contract() {
    let translation = translate_source(
        r#"
interface Overloaded {
    int value(int input);
    int value(boolean input);
}
class OverloadedAnonymousContracts {
    static Overloaded make() {
        return new Overloaded() {
            public int value(int input) { return input + 1; }
            public int value(boolean input) { return input ? 10 : 20; }
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
        "overloaded_anonymous_interface_contracts",
        translation.rust,
        r#"
#[test]
fn anonymous_adapter_uses_each_overload_implementation() {
    let value = OverloadedAnonymousContracts::make();
    assert_eq!(value.value_int(4), 5);
    assert_eq!(value.value_boolean(true), 10);
    assert_eq!(value.value_boolean(false), 20);
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
fn stateful_anonymous_interface_reads_field_through_local_variable() {
    let translation = translate_source(
        r#"
interface Measure {
    int measure(int value);
}
class LocalReadAnonymousMeasure {
    static Measure make() {
        return new Measure() {
            int offset = 5;
            public int measure(int value) {
                int shifted = value + offset;
                return shifted * 2;
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
        "local_read_anonymous_interface",
        translation.rust,
        r#"
#[test]
fn anonymous_field_is_available_to_an_ordinary_local_declaration() {
    assert_eq!(LocalReadAnonymousMeasure::make().measure(4), 18);
}
"#,
    );
}

#[test]
fn stateful_anonymous_interface_distinguishes_capture_from_sibling_local_shadow() {
    let translation = translate_source(
        r#"
interface ShadowMeasure {
    int capture(int value);
    int shadow(int value);
    int fieldShadow(int value);
}
class AnonymousInterfaceShadow {
    static ShadowMeasure make(int bonus) {
        return new ShadowMeasure() {
            int base = 1;
            public int capture(int value) { return base + bonus + value; }
            public int shadow(int value) { int bonus = value; return bonus + base; }
            public int fieldShadow(int value) { int base = value; return base + this.base; }
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
        "anonymous_interface_capture_shadow",
        translation.rust,
        r#"
#[test]
fn interface_capture_and_local_shadow_have_distinct_values() {
    let value = AnonymousInterfaceShadow::make(5);
    assert_eq!(value.capture(3) + value.shadow(7) + value.field_shadow(7), 25);
}
"#,
    );
}

#[test]
fn stateful_anonymous_interface_reassigns_receiver_aware_local_in_branch() {
    let translation = translate_source(
        r#"
interface BranchMeasure { int measure(boolean enabled); }
class AnonymousInterfaceLocalBranch {
    static BranchMeasure make(int bonus) {
        return new BranchMeasure() {
            int base = 1;
            public int measure(boolean enabled) {
                int local = this.base;
                if (enabled) { int bonus = 7; local = bonus; }
                else { local = local + bonus; }
                return local;
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
        "anonymous_interface_local_branch",
        translation.rust,
        r#"
#[test]
fn receiver_aware_local_reassignment_preserves_branch_order() {
    let value = AnonymousInterfaceLocalBranch::make(5);
    assert_eq!(value.measure(false), 6);
    assert_eq!(value.measure(true), 7);
}
"#,
    );
}

#[test]
fn stateful_anonymous_interface_reassigns_locals_across_loop_forms() {
    let translation = translate_source(
        r#"
interface LoopMeasure {
    int whileMeasure(int limit);
    int doMeasure(int limit);
    int forMeasure();
    int whileBreak();
    int forContinue();
    int forShadow();
}
class AnonymousInterfaceLocalLoops {
    static LoopMeasure make(int bonus) {
        return new LoopMeasure() {
            int base = 1;
            public int whileMeasure(int limit) {
                int local = this.base;
                while (local < limit) { local = local + bonus; }
                return local;
            }
            public int doMeasure(int limit) {
                int local = this.base;
                do { local = local + bonus; } while (local < limit);
                return local;
            }
            public int forMeasure() {
                int local = this.base;
                for (int step = 0; step < 2; step++) { local = local + bonus; }
                return local;
            }
            public int whileBreak() {
                int local = this.base;
                while (local < 10) { local = local + bonus; if (local >= 3) { break; } }
                return local;
            }
            public int forContinue() {
                int local = this.base;
                for (int step = 0; step < 3; step++) {
                    if (step == 1) { continue; }
                    local = local + bonus;
                }
                return local;
            }
            public int forShadow() {
                int local = this.base;
                for (int bonus = 0; bonus < 2; bonus++) { local = local + bonus; }
                return local;
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
        "anonymous_interface_local_loops",
        translation.rust,
        r#"
#[test]
fn receiver_aware_local_reassignment_preserves_loop_order() {
    let value = AnonymousInterfaceLocalLoops::make(2);
    assert_eq!(value.while_measure(5), 5);
    assert_eq!(value.do_measure(1), 3);
    assert_eq!(value.for_measure(), 5);
    assert_eq!(value.while_break(), 3);
    assert_eq!(value.for_continue(), 5);
    assert_eq!(value.for_shadow(), 2);
}
"#,
    );
}

#[test]
fn stateful_anonymous_interface_retains_captured_factory_values() {
    let translation = translate_source(
        r#"
interface Measure {
    int measure(int value);
}
class CapturedStatefulAnonymousMeasure {
    static Measure make(int offset) {
        int factor = 2;
        return new Measure() {
            int base = offset;
            public int measure(int value) {
                int shifted = base + value;
                return shifted * factor;
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
        "captured_stateful_anonymous_interface",
        translation.rust,
        r#"
#[test]
fn anonymous_object_keeps_captured_factory_values_with_its_own_state() {
    assert_eq!(CapturedStatefulAnonymousMeasure::make(3).measure(4), 14);
    assert_eq!(CapturedStatefulAnonymousMeasure::make(1).measure(4), 10);
}
"#,
    );
}

#[test]
fn stateful_anonymous_interface_retains_captured_string_value() {
    let translation = translate_source(
        r#"
interface Labeler {
    String label(String suffix);
}
class CapturedStatefulAnonymousLabeler {
    static Labeler make(String prefix) {
        String separator = ":";
        return new Labeler() {
            String label(String suffix) {
                return prefix + separator + suffix;
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
        "captured_stateful_anonymous_string_interface",
        translation.rust,
        r#"
#[test]
fn anonymous_object_keeps_captured_string_factory_values() {
    assert_eq!(
        CapturedStatefulAnonymousLabeler::make("left".to_string()).label("x".to_string()),
        "left:x"
    );
    assert_eq!(
        CapturedStatefulAnonymousLabeler::make("right".to_string()).label("y".to_string()),
        "right:y"
    );
}
"#,
    );
}

#[test]
fn stateful_anonymous_interface_preserves_captured_mutable_alias() {
    let translation = translate_source(
        r#"
interface Reader { int read(); }
class Node {
    int value;
    Node(int value) { this.value = value; }
}
class CapturedMutableAnonymousReader {
    static Reader make(Node node) {
        return new Reader() { public int read() { return node.value; } };
    }
    static int run() {
        Node node = new Node(3);
        Reader reader = make(node);
        node.value = 8;
        return reader.read();
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
        "captured_mutable_anonymous_interface",
        translation.rust,
        r#"
#[test]
fn anonymous_object_observes_mutation_of_its_captured_factory_argument() {
    assert_eq!(CapturedMutableAnonymousReader::run().unwrap(), 8);
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
fn mutable_stateful_anonymous_interface_updates_field_through_local_variable() {
    let translation = translate_source(
        r#"
interface Counter {
    int add(int delta);
    int value();
}
class LocalMutableAnonymousCounter {
    static Counter make() {
        return new Counter() {
            int total = 1;
            public int add(int delta) {
                int adjusted = delta + 1;
                int next = total;
                next = next + adjusted;
                this.total = next;
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
    assert!(
        translation.rust.contains("let mut next") && translation.rust.contains("total.set(next)"),
        "{}",
        translation.rust
    );
    cargo_test_translation(
        "local_mutable_anonymous_interface",
        translation.rust,
        r#"
#[test]
fn anonymous_counter_uses_local_before_persisting_its_field() {
    let counter = LocalMutableAnonymousCounter::make();
    assert_eq!(counter.add(2), 4);
    assert_eq!(counter.add(4), 9);
    assert_eq!(counter.value(), 9);
}
"#,
    );
}

#[test]
fn mutable_stateful_anonymous_interface_updates_string_field_through_local_variable() {
    let translation = translate_source(
        r#"
interface Labeler {
    String append(String suffix);
    String value();
}
class LocalMutableAnonymousLabeler {
    static Labeler make() {
        return new Labeler() {
            String label = "a";
            public String append(String suffix) {
                String next = label + suffix;
                this.label = next;
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
    cargo_test_translation(
        "local_mutable_anonymous_string_interface",
        translation.rust,
        r#"
#[test]
fn anonymous_labeler_uses_a_string_local_before_persisting() {
    let labeler = LocalMutableAnonymousLabeler::make();
    assert_eq!(labeler.append("b".to_string()), "ab");
    assert_eq!(labeler.append("c".to_string()), "abc");
    assert_eq!(labeler.value(), "abc");
}
"#,
    );
}

#[test]
fn mutable_stateful_anonymous_interface_conditionally_updates_string_local_before_field_write() {
    let translation = translate_source(
        r#"
interface Labeler {
    String append(String suffix);
}
class ConditionalLocalMutableAnonymousLabeler {
    static Labeler make() {
        return new Labeler() {
            String label = "a";
            boolean enabled = true;
            public String append(String suffix) {
                String next = label;
                if (enabled) {
                    next = next + suffix;
                }
                this.label = next;
                return label;
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
        "conditional_local_mutable_anonymous_string_interface",
        translation.rust,
        r#"
#[test]
fn anonymous_labeler_conditionally_updates_a_string_local_before_persisting() {
    let labeler = ConditionalLocalMutableAnonymousLabeler::make();
    assert_eq!(labeler.append("b".to_string()), "ab");
    assert_eq!(labeler.append("c".to_string()), "abc");
}
"#,
    );
}

#[test]
fn mutable_stateful_anonymous_interface_conditionally_updates_local_before_field_write() {
    let translation = translate_source(
        r#"
interface Counter {
    int add(int delta);
    int value();
}
class ConditionalLocalMutableAnonymousCounter {
    static Counter make() {
        return new Counter() {
            int total = 1;
            boolean enabled = true;
            public int add(int delta) {
                int next = total;
                if (enabled) {
                    next = next + delta;
                }
                this.total = next;
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
        "conditional_local_mutable_anonymous_interface",
        translation.rust,
        r#"
#[test]
fn anonymous_counter_conditionally_updates_a_local_before_persisting() {
    let counter = ConditionalLocalMutableAnonymousCounter::make();
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
fn interface_forwarder_borrows_owned_array_for_slice_implementation() {
    let translation = translate_source(
        r#"
interface ArrayMetric { int count(int[] values); }
class ArrayMetricImpl implements ArrayMetric {
    public int count(int[] values) { return values.length; }
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
            .contains("impl ArrayMetric for ArrayMetricImpl"),
        "{}",
        translation.rust
    );
    cargo_check_translation("interface_forwarder_array_slice", &translation.rust);
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

#[test]
fn anonymous_source_class_super_uses_virtual_carrier_payload() {
    let translation = translate_source(
        r#"
class Parent {
    int inherited(int input) { return input + 1; }
}
class Base extends Parent {
    final int offset;
    Base(int offset) { this.offset = offset; }
    int value(int input) { return input + offset; }
    int doubled(int input) { return value(input) * 2; }
    int adjusted(int input) { return input; }
    int tick(int input) { return input; }
    int guarded(boolean enabled, int input) { return input; }
    int guardedElse(boolean enabled, int input) { return input; }
    String mutableLabel() { return "base"; }
    String setMutableLabel(String value) { return value; }
    String conditionalLabel(boolean enabled) { return "base"; }
    int observed() { return 0; }
    int mutateObserved(int value) { return value; }
    int directMemberWrite(int value) { return value; }
    int capturedConditional(boolean enabled, int value) { return value; }
    int viaHelper() { return 0; }
    int payloadName(int __java_carrier_payload) { return __java_carrier_payload; }
    int localPlus(int input) { return input; }
    int chainedLocalPlus(int input) { return input; }
    int manyLocalPlus(int input) { return input; }
    int localControl(boolean enabled, int input) { return input; }
    int interleavedUpdate(boolean enabled, int input) { return input; }
    int branchSuper(boolean enabled, int input) { return input; }
    int loopUpdate(int limit) { return limit; }
    int forUpdate(int limit) { return limit; }
    int doWhileUpdate(int limit) { return limit; }
    int breakUpdate(int limit) { return limit; }
    int forAssignmentUpdate(int limit) { return limit; }
    int whileContinue(int limit) { return limit; }
    int forContinue(int limit) { return limit; }
    int doWhileContinue(int limit) { return limit; }
    int unaryBranch(boolean enabled, int input) { return input; }
    int statementIncrement(int limit) { return limit; }
    int localNodeValue() { return 0; }
    int localNodeAssign(int value) { return value; }
    int localNodeReassign() { return 0; }
    int localNodeFromField() { return 0; }
    int localNodeAliasMutate() { return 0; }
    int localNodeCopy() { return 0; }
    int localNodeAssignAlias() { return 0; }
    int localNodeNestedBranch(boolean enabled) { return 0; }
    int localNodeNestedLoop() { return 0; }
    int localNodeDoWhile() { return 0; }
    int localNodeForInitializer() { return 0; }
    int localCaptureShadow(int input) { return input; }
    int directFieldPlus(int input) { return input; }
    String label() { return "base"; }
}
class CapturedNode {
    int value;
    CapturedNode(int value) { this.value = value; }
    int read() { return value; }
    void set(int value) { this.value = value; }
}
class AnonymousClassSuper {
    static Base plain() { return new Base(8) {}; }
    static Base captureOnly(int bonus) { return new Base(3) { @Override int adjusted(int input) { return bonus; } }; }
    static Base captureWithShadows(int bonus) { return new Base(3) { @Override int adjusted(int input) { return bonus + input; } @Override int localPlus(int bonus) { return bonus; } @Override int chainedLocalPlus(int input) { int bonus = input; return bonus; } }; }
    static Base captureNode(CapturedNode node) { return new Base(3) { int helperObserved() { return node.read(); } @Override int observed() { return this.helperObserved(); } @Override int mutateObserved(int value) { node.set(value); return node.read(); } @Override int capturedConditional(boolean enabled, int value) { if (enabled) { node.set(value); } return node.read(); } }; }
    static Base fieldNode(CapturedNode node) { return new Base(3) { CapturedNode localNode = node; @Override int observed() { return this.localNode.value; } @Override int mutateObserved(int value) { this.localNode.set(value); return this.localNode.read(); } @Override int directMemberWrite(int value) { this.localNode.value = value; return this.localNode.value; } }; }
    static Base replaceFieldNode() { return new Base(3) { CapturedNode localNode = new CapturedNode(3); @Override int observed() { return this.localNode.value; } @Override int mutateObserved(int value) { this.localNode = new CapturedNode(value); return this.localNode.value; } }; }
    static Base copyFieldNode() { return new Base(3) { CapturedNode sourceNode = new CapturedNode(11); CapturedNode targetNode = new CapturedNode(3); @Override int observed() { return this.targetNode.value; } @Override int mutateObserved(int value) { this.targetNode = this.sourceNode; return this.targetNode.value; } }; }
    static Base make(int bonus, String label) {
        return new Base(3) {
            int capturedBonus = bonus;
            String capturedLabel = label;
            String mutableLabel = "cold";
            CapturedNode localNode = new CapturedNode(7);
            int total = 0;
            int helperTotal() { return this.total; }
            @Override int value(int input) {
                int base = super.value(input);
                return base * 2;
            }
            @Override int doubled(int input) { return this.value(input); }
            @Override int adjusted(int input) { return super.adjusted(input) + this.capturedBonus; }
            @Override int inherited(int input) { return super.inherited(input) + 1; }
            @Override int tick(int input) { int next = this.total + input; this.total = next; return total; }
            @Override int guarded(boolean enabled, int input) { if (enabled) { this.total += input; } return total; }
            @Override int guardedElse(boolean enabled, int input) { if (enabled) { this.total += input; } else { this.total += input * 2; } return total; }
            @Override int viaHelper() { return this.helperTotal(); }
            @Override int observed() { return this.localNode.read(); }
            @Override int mutateObserved(int value) { this.localNode.set(value); return this.localNode.read(); }
            @Override int payloadName(int __java_carrier_payload) { return __java_carrier_payload; }
            @Override int localPlus(int input) { int local = this.capturedBonus; return local + input; }
            @Override int chainedLocalPlus(int input) { int first = this.capturedBonus; int second = first; return second + input; }
            @Override int manyLocalPlus(int input) { int first = this.helperTotal(); int second = this.capturedBonus; int third = first + second; return third + input; }
            @Override int localControl(boolean enabled, int input) { int running = this.capturedBonus; if (enabled) { running = running + 1; if (input > 3) { running = running + input; } } else { running = running - 1; } return running; }
            @Override int interleavedUpdate(boolean enabled, int input) { int local = this.total + input; if (enabled) { this.total = local; local = local + 1; } else { this.total = local + 2; } return this.total + local; }
            @Override int branchSuper(boolean enabled, int input) { if (enabled) { return super.branchSuper(input) + this.capturedBonus; } return this.capturedBonus; }
            @Override int loopUpdate(int limit) { int local = 0; while (local < limit) { this.total = this.total + 1; local = local + 1; } return this.total; }
            @Override int forUpdate(int limit) { for (int local = 0; local < limit; local++) { this.total = this.total + 1; } return this.total; }
            @Override int doWhileUpdate(int limit) { int local = 0; do { this.total = this.total + 1; local = local + 1; } while (local < limit); return this.total; }
            @Override int breakUpdate(int limit) { int local = 0; while (local < limit) { if (local == 2) { break; } this.total = this.total + 1; local = local + 1; } return this.total; }
            @Override int forAssignmentUpdate(int limit) { for (int local = 0; local < limit; local = local + 1) { this.total = this.total + 1; } return this.total; }
            @Override int whileContinue(int limit) { int local = 0; while (local < limit) { local = local + 1; if (local == 1) { continue; } this.total = this.total + 1; } return this.total; }
            @Override int forContinue(int limit) { for (int local = 0; local < limit; local++) { if (local == 1) { continue; } this.total = this.total + 1; } return this.total; }
            @Override int doWhileContinue(int limit) { int local = 0; do { local = local + 1; if (local < limit) { continue; } this.total = this.total + 1; } while (local < limit); return this.total; }
            @Override int unaryBranch(boolean enabled, int input) { if (!enabled) { return this.capturedBonus; } return -input; }
            @Override int statementIncrement(int limit) { int local = 0; while (local < limit) { local++; this.total = this.total + 1; } return this.total; }
            @Override int localNodeValue() { CapturedNode local = new CapturedNode(31); return local.value; }
            @Override int localNodeAssign(int value) { CapturedNode local = new CapturedNode(value); this.localNode = local; return this.localNode.value; }
            @Override int localNodeReassign() { CapturedNode local = new CapturedNode(3); local = new CapturedNode(41); return local.value; }
            @Override int localNodeFromField() { CapturedNode local = this.localNode; return local.value; }
            @Override int localNodeAliasMutate() { CapturedNode local = this.localNode; local.set(43); return this.localNode.value; }
            @Override int localNodeCopy() { CapturedNode first = new CapturedNode(47); CapturedNode second = first; second.set(48); return first.value; }
            @Override int localNodeAssignAlias() { CapturedNode first = new CapturedNode(51); CapturedNode second = new CapturedNode(3); second = first; second.set(52); return first.value; }
            @Override int localNodeNestedBranch(boolean enabled) { CapturedNode local = this.localNode; if (enabled) { local.set(53); } return this.localNode.value; }
            @Override int localNodeNestedLoop() { CapturedNode local = this.localNode; int count = 0; while (count < 2) { local.set(local.read() + 1); count = count + 1; } return this.localNode.value; }
            @Override int localNodeDoWhile() { CapturedNode local = this.localNode; int count = 0; do { local.set(local.read() + 1); count = count + 1; } while (count < 2); return this.localNode.value; }
            @Override int localNodeForInitializer() { int count = 0; for (CapturedNode local = this.localNode; count < 2; count = count + 1) { local.set(local.read() + 1); } return this.localNode.value; }
            @Override int localCaptureShadow(int input) { int bonus = input; return bonus; }
            @Override int directFieldPlus(int input) { return this.capturedBonus + input; }
            @Override String mutableLabel() { return this.mutableLabel; }
            @Override String setMutableLabel(String value) { String next = value; this.mutableLabel = next; return mutableLabel; }
            @Override String conditionalLabel(boolean enabled) { if (enabled) { this.mutableLabel = "yes"; } else { this.mutableLabel = "no"; } return mutableLabel; }
            @Override String label() { return this.capturedLabel; }
        };
    }
    static int run() { return make(5, "left").value(4); }
    static int baseOffset() { return make(5, "left").offset; }
    static int forwarded() { return make(5, "left").doubled(4); }
    static int adjusted() { return make(5, "left").adjusted(4); }
    static int inheritedRun() { return make(5, "left").inherited(5); }
    static int mutableRun() { Base value = make(5, "left"); return value.tick(2) + value.tick(3); }
    static int guardedRun() { Base value = make(5, "left"); value.guarded(true, 4); return value.guarded(false, 99); }
    static int guardedElseRun() { Base value = make(5, "left"); value.guardedElse(true, 2); return value.guardedElse(false, 3); }
    static int helperRun() { Base value = make(5, "left"); value.tick(6); return value.viaHelper(); }
    static int fieldNodeValue() { return make(5, "left").observed(); }
    static int fieldNodeMutate() { return make(5, "left").mutateObserved(17); }
    static int fieldNodeAlias() { CapturedNode node = new CapturedNode(3); Base value = fieldNode(node); node.value = 12; return value.observed(); }
    static int fieldNodeWrite() { CapturedNode node = new CapturedNode(3); Base value = fieldNode(node); value.mutateObserved(17); return node.value; }
    static int fieldNodeDirectMemberWrite() { CapturedNode node = new CapturedNode(3); Base value = fieldNode(node); value.directMemberWrite(23); return node.value; }
    static int fieldNodeReplace() { Base value = replaceFieldNode(); value.mutateObserved(21); return value.observed(); }
    static int fieldNodeCopy() { Base value = copyFieldNode(); return value.mutateObserved(0); }
    static int payloadName() { return make(5, "left").payloadName(19); }
    static int localPlus() { return make(5, "left").localPlus(4); }
    static int chainedLocalPlus() { return make(5, "left").chainedLocalPlus(4); }
    static int manyLocalPlus() { return make(5, "left").manyLocalPlus(4); }
    static int localControlTrue() { return make(5, "left").localControl(true, 4); }
    static int localControlFalse() { return make(5, "left").localControl(false, 4); }
    static int interleavedUpdateTrue() { return make(5, "left").interleavedUpdate(true, 2); }
    static int interleavedUpdateFalse() { return make(5, "left").interleavedUpdate(false, 2); }
    static int branchSuperTrue() { return make(5, "left").branchSuper(true, 4); }
    static int branchSuperFalse() { return make(5, "left").branchSuper(false, 4); }
    static int loopUpdate() { return make(5, "left").loopUpdate(3); }
    static int forUpdate() { return make(5, "left").forUpdate(4); }
    static int doWhileUpdate() { return make(5, "left").doWhileUpdate(0); }
    static int breakUpdate() { return make(5, "left").breakUpdate(5); }
    static int forAssignmentUpdate() { return make(5, "left").forAssignmentUpdate(3); }
    static int whileContinue() { return make(5, "left").whileContinue(3); }
    static int forContinue() { return make(5, "left").forContinue(3); }
    static int doWhileContinue() { return make(5, "left").doWhileContinue(3); }
    static int unaryBranchFalse() { return make(5, "left").unaryBranch(false, 4); }
    static int unaryBranchTrue() { return make(5, "left").unaryBranch(true, 4); }
    static int statementIncrement() { return make(5, "left").statementIncrement(3); }
    static int localNodeValue() { return make(5, "left").localNodeValue(); }
    static int localNodeAssign() { return make(5, "left").localNodeAssign(29); }
    static int localNodeReassign() { return make(5, "left").localNodeReassign(); }
    static int localNodeFromField() { return make(5, "left").localNodeFromField(); }
    static int localNodeAliasMutate() { return make(5, "left").localNodeAliasMutate(); }
    static int localNodeCopy() { return make(5, "left").localNodeCopy(); }
    static int localNodeAssignAlias() { return make(5, "left").localNodeAssignAlias(); }
    static int localNodeNestedBranch() { return make(5, "left").localNodeNestedBranch(true); }
    static int localNodeNestedLoop() { return make(5, "left").localNodeNestedLoop(); }
    static int localNodeDoWhile() { return make(5, "left").localNodeDoWhile(); }
    static int localNodeForInitializer() { return make(5, "left").localNodeForInitializer(); }
    static int localCaptureShadow() { return make(5, "left").localCaptureShadow(13); }
    static int directFieldPlus() { return make(5, "left").directFieldPlus(4); }
    static String mutableLabel() { Base value = make(5, "left"); value.setMutableLabel("right"); return value.mutableLabel(); }
    static String conditionalLabel() { Base value = make(5, "left"); value.conditionalLabel(true); return value.conditionalLabel(false); }
    static int plainValue() { return plain().value(1); }
    static int capturedValue() { return captureOnly(11).adjusted(0); }
    static int capturedWithShadows() { Base value = captureWithShadows(5); return value.adjusted(4) + value.localPlus(7) + value.chainedLocalPlus(11); }
    static int capturedAlias() { CapturedNode node = new CapturedNode(3); Base value = captureNode(node); node.value = 12; return value.observed(); }
    static int capturedWrite() { CapturedNode node = new CapturedNode(3); Base value = captureNode(node); value.mutateObserved(17); return node.value; }
    static int capturedConditional() { CapturedNode node = new CapturedNode(3); Base value = captureNode(node); value.capturedConditional(false, 9); return value.capturedConditional(true, 12) + node.value; }
    static String label() { return make(5, "left").label(); }
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
        translation.rust.contains("enum BaseValue")
            && translation
                .rust
                .contains("Anonymous(std::rc::Rc<dyn BaseAnonymousDispatch>)")
            && translation
                .rust
                .contains("Base::value(&self.__base, input)")
            && translation.rust.contains("total: std::cell::Cell<i32>"),
        "{}",
        translation.rust
    );
    cargo_test_translation(
        "anonymous_source_class_super",
        translation.rust,
        r#"
#[test]
fn anonymous_override_calls_initialized_base_super_method() {
    assert_eq!(AnonymousClassSuper::run(), 14);
    assert_eq!(AnonymousClassSuper::base_offset(), 3);
    assert_eq!(AnonymousClassSuper::forwarded(), 14);
    assert_eq!(AnonymousClassSuper::adjusted(), 9);
    assert_eq!(AnonymousClassSuper::inherited_run(), 7);
    assert_eq!(AnonymousClassSuper::mutable_run(), 7);
    assert_eq!(AnonymousClassSuper::guarded_run(), 4);
    assert_eq!(AnonymousClassSuper::guarded_else_run(), 8);
    assert_eq!(AnonymousClassSuper::helper_run(), 6);
    assert_eq!(AnonymousClassSuper::field_node_value(), 7);
    assert_eq!(AnonymousClassSuper::field_node_mutate(), 17);
    assert_eq!(AnonymousClassSuper::field_node_alias(), 12);
    assert_eq!(AnonymousClassSuper::field_node_write(), 17);
    assert_eq!(AnonymousClassSuper::field_node_direct_member_write(), 23);
    assert_eq!(AnonymousClassSuper::field_node_replace(), 21);
    assert_eq!(AnonymousClassSuper::field_node_copy(), 11);
    assert_eq!(AnonymousClassSuper::payload_name(), 19);
    assert_eq!(AnonymousClassSuper::local_plus(), 9);
    assert_eq!(AnonymousClassSuper::chained_local_plus(), 9);
    assert_eq!(AnonymousClassSuper::many_local_plus(), 9);
    assert_eq!(AnonymousClassSuper::local_control_true(), 10);
    assert_eq!(AnonymousClassSuper::local_control_false(), 4);
    assert_eq!(AnonymousClassSuper::interleaved_update_true(), 5);
    assert_eq!(AnonymousClassSuper::interleaved_update_false(), 6);
    assert_eq!(AnonymousClassSuper::branch_super_true(), 9);
    assert_eq!(AnonymousClassSuper::branch_super_false(), 5);
    assert_eq!(AnonymousClassSuper::loop_update(), 3);
    assert_eq!(AnonymousClassSuper::for_update(), 4);
    assert_eq!(AnonymousClassSuper::do_while_update(), 1);
    assert_eq!(AnonymousClassSuper::break_update(), 2);
    assert_eq!(AnonymousClassSuper::for_assignment_update(), 3);
    assert_eq!(AnonymousClassSuper::while_continue(), 2);
    assert_eq!(AnonymousClassSuper::for_continue(), 2);
    assert_eq!(AnonymousClassSuper::do_while_continue(), 1);
    assert_eq!(AnonymousClassSuper::unary_branch_false(), 5);
    assert_eq!(AnonymousClassSuper::unary_branch_true(), -4);
    assert_eq!(AnonymousClassSuper::statement_increment(), 3);
    assert_eq!(AnonymousClassSuper::local_node_value(), 31);
    assert_eq!(AnonymousClassSuper::local_node_assign(), 29);
    assert_eq!(AnonymousClassSuper::local_node_reassign(), 41);
    assert_eq!(AnonymousClassSuper::local_node_from_field(), 7);
    assert_eq!(AnonymousClassSuper::local_node_alias_mutate(), 43);
    assert_eq!(AnonymousClassSuper::local_node_copy(), 48);
    assert_eq!(AnonymousClassSuper::local_node_assign_alias(), 52);
    assert_eq!(AnonymousClassSuper::local_node_nested_branch(), 53);
    assert_eq!(AnonymousClassSuper::local_node_nested_loop(), 9);
    assert_eq!(AnonymousClassSuper::local_node_do_while(), 9);
    assert_eq!(AnonymousClassSuper::local_node_for_initializer(), 9);
    assert_eq!(AnonymousClassSuper::local_capture_shadow(), 13);
    assert_eq!(AnonymousClassSuper::direct_field_plus(), 9);
    assert_eq!(AnonymousClassSuper::mutable_label(), "right");
    assert_eq!(AnonymousClassSuper::conditional_label(), "no");
    assert_eq!(AnonymousClassSuper::plain_value(), 9);
    assert_eq!(AnonymousClassSuper::captured_value(), 11);
    assert_eq!(AnonymousClassSuper::captured_with_shadows(), 27);
    assert_eq!(AnonymousClassSuper::captured_alias(), Ok(12));
    assert_eq!(AnonymousClassSuper::captured_write(), Ok(17));
    assert_eq!(AnonymousClassSuper::captured_conditional(), Ok(24));
    assert_eq!(AnonymousClassSuper::label(), "left");
}
"#,
    );
}

#[test]
fn anonymous_source_class_enhanced_for_rebinds_array_parameter_and_state() {
    let translation = translate_source(
        r#"
class EachBase {
    EachBase() {}
    int sum(int[] values) { return 0; }
}
class EachAnonymousClass {
    static EachBase make() {
        return new EachBase() {
            int total = 0;
            @Override int sum(int[] values) {
                outer: for (int value : values) {
                    if (value == 2) continue outer;
                    this.total = this.total + value;
                    if (value == 3) break outer;
                }
                return this.total;
            }
        };
    }
    static int run() {
        EachBase value = make();
        int[] first = {1, 2, 3, 4};
        int[] second = {5};
        return value.sum(first) + value.sum(second);
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
        "anonymous_class_enhanced_for",
        translation.rust,
        r#"
#[test]
fn enhanced_for_uses_the_array_parameter_and_escaped_cell() {
    assert_eq!(EachAnonymousClass::run(), 13);
}
"#,
    );
}

#[test]
fn anonymous_source_class_inherited_method_mutates_array_argument() {
    let translation = translate_source(
        r#"
class ArrayWriteBase {
    ArrayWriteBase() {}
    int bump(int[] values) {
        values[0] = values[0] + 1;
        return values[0];
    }
}
class ArrayWriteAnonymousClass {
    static ArrayWriteBase make() { return new ArrayWriteBase() {}; }
    static int run() {
        int[] values = {4};
        ArrayWriteBase value = make();
        return value.bump(values) + values[0];
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
        "anonymous_class_array_mutation",
        translation.rust,
        r#"
#[test]
fn inherited_virtual_method_keeps_array_alias() {
    assert_eq!(ArrayWriteAnonymousClass::run().unwrap(), 10);
}
"#,
    );
}

#[test]
fn anonymous_source_class_override_preserves_fallible_base_contract() {
    let translation = translate_source(
        r#"
class CheckedArrayBase {
    CheckedArrayBase() {}
    int first(int[] values) { return values[0]; }
}
class CheckedArrayAnonymousClass {
    static CheckedArrayBase make() {
        return new CheckedArrayBase() {
            @Override int first(int[] values) { return values.length; }
        };
    }
    static int run() {
        int[] empty = {};
        CheckedArrayBase value = make();
        return value.first(empty);
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
        "anonymous_class_fallible_override",
        translation.rust,
        r#"
#[test]
fn override_keeps_the_base_exception_abi() {
    assert_eq!(CheckedArrayAnonymousClass::run().unwrap(), 0);
}
"#,
    );
}

#[test]
fn anonymous_source_class_override_propagates_its_own_array_failure() {
    let translation = translate_source(
        r#"
class CheckedOverrideBase {
    CheckedOverrideBase() {}
    int select(int[] values) { return values[0]; }
}
class CheckedOverrideAnonymousClass {
    static CheckedOverrideBase make() {
        return new CheckedOverrideBase() {
            @Override int select(int[] values) { return values[1]; }
        };
    }
    static int run() {
        int[] values = {4, 7};
        CheckedOverrideBase value = make();
        return value.select(values);
    }
    static int runEmpty() {
        int[] values = {};
        CheckedOverrideBase value = make();
        return value.select(values);
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
        "anonymous_class_fallible_body",
        translation.rust,
        r#"
#[test]
fn override_propagates_checked_array_access() {
    assert_eq!(CheckedOverrideAnonymousClass::run().unwrap(), 7);
    assert!(CheckedOverrideAnonymousClass::run_empty().is_err());
}
"#,
    );
}

#[test]
fn anonymous_source_class_fallible_super_return_preserves_result() {
    let translation = translate_source(
        r#"
class CheckedSuperBase {
    CheckedSuperBase() {}
    int read(int[] values) { return values[0] + 2; }
    int readPlus(int[] values) { return values[0] + 2; }
}
class CheckedSuperAnonymousClass {
    static CheckedSuperBase make() {
        return new CheckedSuperBase() {
            @Override int read(int[] values) { return super.read(values); }
            @Override int readPlus(int[] values) {
                int base = super.readPlus(values);
                return base + 1;
            }
        };
    }
    static int run() {
        int[] values = {5};
        CheckedSuperBase value = make();
        return value.read(values);
    }
    static int runEmpty() {
        int[] values = {};
        CheckedSuperBase value = make();
        return value.read(values);
    }
    static int runPlus() {
        int[] values = {5};
        CheckedSuperBase value = make();
        return value.readPlus(values);
    }
    static int runPlusEmpty() {
        int[] values = {};
        CheckedSuperBase value = make();
        return value.readPlus(values);
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
        "anonymous_class_fallible_super",
        translation.rust,
        r#"
#[test]
fn super_result_is_forwarded_without_double_wrapping() {
    assert_eq!(CheckedSuperAnonymousClass::run().unwrap(), 7);
    assert!(CheckedSuperAnonymousClass::run_empty().is_err());
    assert_eq!(CheckedSuperAnonymousClass::run_plus().unwrap(), 8);
    assert!(CheckedSuperAnonymousClass::run_plus_empty().is_err());
}
"#,
    );
}

#[test]
fn anonymous_source_class_super_call_uses_the_resolved_other_method() {
    let translation = translate_source(
        r#"
class OtherSuperBase {
    OtherSuperBase() {}
    int base(int value) { return value + 2; }
    int base(String ignored) { return -100; }
    int direct(int value) { return -1; }
    int viaLocal(int value) { return -2; }
    int inExpr(int value) { return -3; }
    int inExprRight(int value) { return -5; }
    int checked(int[] values) { return values[0] + 2; }
    int checked(String ignored) { return -6; }
    int checkedViaLocal(int[] values) { return values[0]; }
    int checkedExpr(int[] values) { return values[0]; }
    int checkedExprRight(int[] values) { return values[0]; }
    int checkedByOverride(int[] values) { return -4; }
}

class OtherSuperAnonymousClass {
    static OtherSuperBase make() {
        return new OtherSuperBase() {
            @Override int direct(int value) { return super.base(value); }
            @Override int viaLocal(int value) {
                int fromBase = super.base(value);
                return fromBase * 2;
            }
            @Override int inExpr(int value) {
                return super.base(value + 1) + 1;
            }
            @Override int inExprRight(int value) {
                return 1 + super.base(value + 1);
            }
            @Override int checkedViaLocal(int[] values) {
                int fromBase = super.checked(values);
                return fromBase + 1;
            }
            @Override int checkedExpr(int[] values) {
                return super.checked(values) + 1;
            }
            @Override int checkedExprRight(int[] values) {
                return 1 + super.checked(values);
            }
            @Override int checkedByOverride(int[] values) {
                return super.checked(values) + 1;
            }
        };
    }
    static int run() {
        OtherSuperBase value = make();
        return value.direct(5) + value.viaLocal(5);
    }
    static int runChecked() {
        int[] values = {5};
        OtherSuperBase value = make();
        return value.checkedViaLocal(values);
    }
    static int runExpr() {
        OtherSuperBase value = make();
        return value.inExpr(5);
    }
    static int runCheckedEmpty() {
        int[] values = {};
        OtherSuperBase value = make();
        return value.checkedViaLocal(values);
    }
    static int runCheckedExpr() {
        int[] values = {5};
        OtherSuperBase value = make();
        return value.checkedExpr(values);
    }
    static int runCheckedExprEmpty() {
        int[] values = {};
        OtherSuperBase value = make();
        return value.checkedExpr(values);
    }
    static int runExprRight() {
        OtherSuperBase value = make();
        return value.inExprRight(5);
    }
    static int runCheckedExprRight() {
        int[] values = {5};
        OtherSuperBase value = make();
        return value.checkedExprRight(values);
    }
    static int runCheckedExprRightEmpty() {
        int[] values = {};
        OtherSuperBase value = make();
        return value.checkedExprRight(values);
    }
    static int runCheckedByOverride() {
        int[] values = {5};
        OtherSuperBase value = make();
        return value.checkedByOverride(values);
    }
    static int runCheckedByOverrideEmpty() {
        int[] values = {};
        OtherSuperBase value = make();
        return value.checkedByOverride(values);
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
        "anonymous_class_other_super_target",
        translation.rust,
        r#"
#[test]
fn other_super_target_is_not_the_override_itself() {
    assert_eq!(OtherSuperAnonymousClass::run(), 21);
    assert_eq!(OtherSuperAnonymousClass::run_expr(), 9);
    assert_eq!(OtherSuperAnonymousClass::run_expr_right(), 9);
    assert_eq!(OtherSuperAnonymousClass::run_checked().unwrap(), 8);
    assert!(OtherSuperAnonymousClass::run_checked_empty().is_err());
    assert_eq!(OtherSuperAnonymousClass::run_checked_expr().unwrap(), 8);
    assert!(OtherSuperAnonymousClass::run_checked_expr_empty().is_err());
    assert_eq!(OtherSuperAnonymousClass::run_checked_expr_right().unwrap(), 8);
    assert!(OtherSuperAnonymousClass::run_checked_expr_right_empty().is_err());
    assert_eq!(OtherSuperAnonymousClass::run_checked_by_override().unwrap(), 8);
    assert!(OtherSuperAnonymousClass::run_checked_by_override_empty().is_err());
}
"#,
    );
}

#[test]
fn anonymous_source_class_nested_super_expression_propagates_fallibility() {
    let translation = translate_source(
        r#"
class DeepSuperBase {
    DeepSuperBase() {}
    int checked(int[] values) { return values[0] + 2; }
    int adjusted(int[] values) { return -1; }
    int negated(int[] values) { return -1; }
}
class DeepSuperUse {
    static DeepSuperBase make() {
        return new DeepSuperBase() {
            @Override int adjusted(int[] values) {
                return 2 * (1 + super.checked(values));
            }
            @Override int negated(int[] values) {
                return -(1 + super.checked(values));
            }
        };
    }
    static int run() {
        DeepSuperBase value = make();
        int[] values = {5};
        return value.adjusted(values);
    }
    static int runEmpty() {
        DeepSuperBase value = make();
        int[] values = {};
        return value.adjusted(values);
    }
    static int runNegated() {
        DeepSuperBase value = make();
        int[] values = {5};
        return value.negated(values);
    }
    static int runNegatedEmpty() {
        DeepSuperBase value = make();
        int[] values = {};
        return value.negated(values);
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
        "anonymous_class_nested_super_expression",
        translation.rust,
        r#"
#[test]
fn nested_super_call_uses_base_target_and_propagates_error() {
    assert_eq!(DeepSuperUse::run().unwrap(), 16);
    assert!(DeepSuperUse::run_empty().is_err());
    assert_eq!(DeepSuperUse::run_negated().unwrap(), -8);
    assert!(DeepSuperUse::run_negated_empty().is_err());
}
"#,
    );
}

#[test]
fn anonymous_source_class_direct_array_access_promotes_base_contract() {
    let translation = translate_source(
        r#"
class DirectArrayBase {
    DirectArrayBase() {}
    int first(int[] values) { return -4; }
}
class DirectArrayAnonymousClass {
    static DirectArrayBase make() {
        return new DirectArrayBase() {
            @Override int first(int[] values) { return values[0]; }
        };
    }
    static int run() {
        int[] values = {5};
        return make().first(values);
    }
    static int runEmpty() {
        int[] values = {};
        return make().first(values);
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
        "anonymous_class_direct_array_contract",
        translation.rust,
        r#"
#[test]
fn override_array_access_is_catchable_through_base_slot() {
    assert_eq!(DirectArrayAnonymousClass::run().unwrap(), 5);
    assert!(DirectArrayAnonymousClass::run_empty().is_err());
}
"#,
    );
}

#[test]
fn anonymous_source_class_direct_throw_promotes_base_contract() {
    let translation = translate_source(
        r#"
class DirectThrowBase {
    DirectThrowBase() {}
    int read(boolean fail) { return -4; }
}
class DirectThrowAnonymousClass {
    static DirectThrowBase make() {
        return new DirectThrowBase() {
            @Override int read(boolean fail) {
                if (fail) throw new IllegalStateException("boom");
                return 7;
            }
        };
    }
    static int run() { return make().read(false); }
    static int runFail() { return make().read(true); }
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
        "anonymous_class_direct_throw_contract",
        translation.rust,
        r#"
#[test]
fn override_throw_is_catchable_through_base_slot() {
    assert_eq!(DirectThrowAnonymousClass::run().unwrap(), 7);
    assert!(DirectThrowAnonymousClass::run_fail().is_err());
}
"#,
    );
}

#[test]
fn anonymous_source_class_labeled_block_preserves_receiver_and_super() {
    let translation = translate_source(
        r#"
class BlockBase {
    BlockBase() {}
    int value() { return 1; }
}
class BlockAnonymousClass {
    static BlockBase make() {
        return new BlockBase() {
            int total = 0;
            @Override int value() {
                int i = 0;
                outer: {
                    int bonus = 3;
                    while (i < 4) {
                        i++;
                        {
                            int local = bonus + i;
                            this.total = this.total + local;
                        }
                        if (i == 2) break outer;
                    }
                    this.total = 100;
                }
                return this.total * 10 + i + super.value();
            }
        };
    }
    static int run() { return make().value(); }
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
        "anonymous_class_labeled_block_receiver",
        translation.rust,
        r#"
#[test]
fn labeled_block_break_keeps_class_receiver_state_and_super() {
    assert_eq!(BlockAnonymousClass::run(), 93);
}
"#,
    );
}

#[test]
fn anonymous_source_class_labeled_while_rebinds_receiver_and_control_flow() {
    let translation = translate_source(
        r#"
class LabeledBase {
    LabeledBase() {}
    int run(int limit) { return 0; }
    int runFor(int limit) { return 0; }
    int runDo(int limit) { return 0; }
    int runNested(int limit) { return 0; }
}
class LabeledAnonymousClass {
    static LabeledBase make() {
        return new LabeledBase() {
            int total = 0;
            @Override int run(int limit) {
                int i = 0;
                outer: while (i < limit) {
                    i++;
                    if (i == 2) continue outer;
                    this.total = this.total + i;
                    if (i == 3) break outer;
                }
                return this.total;
            }
            @Override int runFor(int limit) {
                int i = 0;
                outer: for (i = 0; i < limit; i++) {
                    if (i == 1) continue outer;
                    this.total = this.total + i;
                    if (i == 3) break outer;
                }
                return this.total + i;
            }
            @Override int runDo(int limit) {
                int i = 0;
                outer: do {
                    i++;
                    if (i == 2) continue outer;
                    this.total = this.total + i;
                    if (i == 3) break outer;
                } while (i < limit);
                return this.total + i;
            }
            @Override int runNested(int limit) {
                int i = 0;
                outer: for (i = 0; i < limit; i++) {
                    int inner = 0;
                    while (inner < 3) {
                        inner++;
                        if (inner == 2) continue outer;
                        this.total = this.total + 1;
                    }
                }
                return this.total + i;
            }
        };
    }
    static int run() {
        LabeledBase value = make();
        return value.run(5) + value.run(1);
    }
    static int runFor() {
        LabeledBase value = make();
        return value.runFor(5) + value.runFor(2);
    }
    static int runDo() {
        LabeledBase value = make();
        return value.runDo(5) + value.runDo(1);
    }
    static int runNested() {
        LabeledBase value = make();
        return value.runNested(3) + value.runNested(1);
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
        "anonymous_class_labeled_while",
        translation.rust,
        r#"
#[test]
fn labeled_while_uses_the_same_escaped_cell_after_continue_and_break() {
    assert_eq!(LabeledAnonymousClass::run(), 9);
    assert_eq!(LabeledAnonymousClass::run_for(), 15);
    assert_eq!(LabeledAnonymousClass::run_do(), 13);
    assert_eq!(LabeledAnonymousClass::run_nested(), 11);
}
"#,
    );
}

#[test]
fn anonymous_source_class_assignment_shares_mutable_instance_state() {
    let translation = translate_source(
        r#"
class AliasBase {
    AliasBase() {}
    int bump(int delta) { return delta; }
    int read() { return 0; }
}
class AnonymousClassAlias {
    static AliasBase make() {
        return new AliasBase() {
            int count = 1;
            @Override int bump(int delta) {
                this.count = this.count + delta;
                return this.count;
            }
            @Override int read() { return this.count; }
        };
    }
    static int run() {
        AliasBase first = make();
        AliasBase second = first;
        first.bump(3);
        second.bump(2);
        return first.read() + second.read();
    }
}
"#,
    );
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    cargo_test_translation(
        "anonymous_source_class_assignment_alias",
        translation.rust,
        r#"
#[test]
fn aliases_observe_the_same_anonymous_state() {
    assert_eq!(AnonymousClassAlias::run(), 12);
}
"#,
    );
}

#[test]
fn static_reachable_anonymous_class_keeps_dispatch_and_state() {
    let translation = translate_source(
        r#"
class StaticBase {
    StaticBase() {}
    int read() { return 0; }
    int bump(int delta) { return delta; }
}
class StaticAnonymousClass {
    static StaticBase cached = new StaticBase() {
        int count = 1;
        @Override int read() { return this.count; }
        @Override int bump(int delta) {
            this.count = this.count + delta;
            return this.count;
        }
    };
    static int run() {
        cached.bump(3);
        return cached.read();
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
        translation.rust.contains(
            "Anonymous(std::sync::Arc<std::sync::Mutex<dyn StaticBaseAnonymousDispatch + Send>>)"
        ) && translation
            .rust
            .contains("StaticBaseValue::Anonymous(std::sync::Arc::new(std::sync::Mutex::new("),
        "{}",
        translation.rust
    );
    cargo_test_translation(
        "static_reachable_anonymous_class",
        translation.rust,
        r#"
#[test]
fn static_field_keeps_anonymous_mutation() {
    assert_eq!(StaticAnonymousClass::run(), 4);
}
"#,
    );
}

#[test]
fn static_reachable_anonymous_interface_keeps_dispatch_and_state() {
    let translation = translate_source(
        r#"
interface StaticCounter {
    int add(int delta);
    int read();
    default int doubled() { return read() * 2; }
}
class StaticAnonymousInterface {
    static StaticCounter counter = new StaticCounter() {
        int total = 1;
        public int add(int delta) {
            this.total = this.total + delta;
            return this.total;
        }
        public int read() { return this.total; }
    };
    static int run() {
        counter.add(3);
        return counter.read();
    }
    static int doubled() { return counter.doubled(); }
}
"#,
    );
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    assert!(
        translation.rust.contains("pub trait StaticCounter: Send")
            && translation
                .rust
                .contains("StaticCounter for std::sync::Arc<std::sync::Mutex<__JavaShared>>")
            && translation
                .rust
                .contains("std::boxed::Box::new(std::sync::Arc::new(std::sync::Mutex::new("),
        "{}",
        translation.rust
    );
    cargo_test_translation(
        "static_reachable_anonymous_interface",
        translation.rust,
        r#"
#[test]
fn static_field_keeps_interface_mutation() {
    assert_eq!(StaticAnonymousInterface::run(), 4);
    assert_eq!(StaticAnonymousInterface::doubled(), 8);
}
"#,
    );
}

#[test]
fn static_reachable_single_method_anonymous_interface_uses_send_object() {
    let translation = translate_source(
        r#"
interface StaticAdder { int add(int value); }
class StaticAnonymousAdder {
    static StaticAdder adder = new StaticAdder() {
        public int add(int value) { return value + 1; }
    };
    static int run() { return adder.add(4); }
}
"#,
    );
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    assert!(
        translation.rust.contains("pub trait StaticAdder: Send")
            && translation
                .rust
                .contains("struct __JavaAnonymousStaticAdder_object")
            && !translation
                .rust
                .contains("struct __JavaAnonymousStaticAdder_add"),
        "{}",
        translation.rust
    );
    cargo_test_translation(
        "static_single_method_anonymous_interface",
        translation.rust,
        r#"
#[test]
fn static_functional_field_calls_anonymous_object() {
    assert_eq!(StaticAnonymousAdder::run(), 5);
}
"#,
    );
}

#[test]
fn statically_reachable_field_carries_anonymous_interface_safely() {
    let translation = translate_source(
        r#"
interface EmbeddedCounter { int add(int delta); int read(); }
class CounterHolder {
    EmbeddedCounter counter = new EmbeddedCounter() {
        int total = 1;
        public int add(int delta) {
            this.total = this.total + delta;
            return this.total;
        }
        public int read() { return this.total; }
    };
    CounterHolder() {}
    int run() {
        counter.add(3);
        return counter.read();
    }
}

class StaticHolderRoot {
    static CounterHolder holder = new CounterHolder();
    static int run() { return holder.run(); }
}
"#,
    );
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    cargo_test_translation(
        "transitively_static_anonymous_interface_field",
        translation.rust,
        r#"
#[test]
fn static_holder_retains_nested_interface_state() {
    assert_eq!(StaticHolderRoot::run(), 4);
}
"#,
    );
}

#[test]
fn static_subclass_reaches_inherited_anonymous_interface_field() {
    let translation = translate_source(
        r#"
interface InheritedCounter { int add(int delta); int read(); }
class InheritedCounterBase {
    InheritedCounter counter = new InheritedCounter() {
        int total = 1;
        public int add(int delta) {
            this.total = this.total + delta;
            return this.total;
        }
        public int read() { return this.total; }
    };
    InheritedCounterBase() {}
    int run() {
        counter.add(3);
        return counter.read();
    }
}
class InheritedCounterChild extends InheritedCounterBase {
    InheritedCounterChild() {}
}
class InheritedCounterRoot {
    static InheritedCounterChild holder = new InheritedCounterChild();
    static int run() { return holder.run(); }
}
"#,
    );
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    cargo_test_translation(
        "static_inherited_anonymous_interface_field",
        translation.rust,
        r#"
#[test]
fn static_child_retains_inherited_interface_state() {
    assert_eq!(InheritedCounterRoot::run(), 4);
}
"#,
    );
}

#[test]
fn static_array_element_carries_anonymous_interface_state() {
    let translation = translate_source(
        r#"
interface ArrayCounter { int add(int delta); int read(); }
class ArrayCounterRoot {
    static ArrayCounter[] counters = new ArrayCounter[1];
    static int run() {
        counters[0] = new ArrayCounter() {
            int total = 1;
            public int add(int delta) {
                this.total = this.total + delta;
                return this.total;
            }
            public int read() { return this.total; }
        };
        counters[0].add(3);
        return counters[0].read();
    }
}
"#,
    );
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    cargo_test_translation(
        "static_array_anonymous_interface_element",
        translation.rust,
        r#"
#[test]
fn static_array_preserves_interface_element_state() {
    assert_eq!(ArrayCounterRoot::run(), 4);
}
"#,
    );
}

#[test]
fn local_generic_collection_element_carries_anonymous_interface_state() {
    let translation = translate_source(
        r#"
interface ListCounter { int add(int delta); int read(); }
class ListCounterRoot {
    static int run() {
        java.util.ArrayList<ListCounter> counters = new java.util.ArrayList<>();
        counters.add(new ListCounter() {
            int total = 1;
            public int add(int delta) {
                this.total = this.total + delta;
                return this.total;
            }
            public int read() { return this.total; }
        });
        counters.get(0).add(3);
        return counters.get(0).read();
    }
}
"#,
    );
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    cargo_test_translation(
        "local_list_anonymous_interface_element",
        translation.rust,
        r#"
#[test]
fn local_list_preserves_interface_element_state() {
    assert_eq!(ListCounterRoot::run(), 4);
}
"#,
    );
}

#[test]
fn static_generic_collection_element_carries_anonymous_interface_state() {
    let translation = translate_source(
        r#"
interface StaticListCounter { int add(int delta); int read(); }
class StaticListCounterRoot {
    static java.util.ArrayList<StaticListCounter> counters = new java.util.ArrayList<>();
    static int run() {
        counters.add(new StaticListCounter() {
            int total = 1;
            public int add(int delta) {
                this.total = this.total + delta;
                return this.total;
            }
            public int read() { return this.total; }
        });
        counters.add(counters.get(0));
        counters.add(0, counters.get(0));
        counters.get(2).add(3);
        return counters.get(1).read();
    }
    static int structural() {
        StaticListCounter displaced = counters.set(1, new StaticListCounter() {
            int total = 7;
            public int add(int delta) {
                this.total = this.total + delta;
                return this.total;
            }
            public int read() { return this.total; }
        });
        StaticListCounter removed = counters.remove(0);
        removed.add(3);
        counters.get(0).add(3);
        return counters.size() * 100 + counters.get(0).read() * 10 + displaced.read();
    }
}
"#,
    );
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    cargo_test_translation(
        "static_list_anonymous_interface_element",
        translation.rust,
        r#"
#[test]
fn static_list_preserves_interface_element_state() {
    assert_eq!(StaticListCounterRoot::run(), 4);
    assert_eq!(StaticListCounterRoot::structural(), 307);
}
"#,
    );
}

#[test]
fn anonymous_interface_method_reads_local_after_field_write() {
    let translation = translate_source(
        r#"
interface SnapshotCounter { int step(int delta); int read(); }
class SnapshotCounterUse {
    static int run() {
        SnapshotCounter counter = new SnapshotCounter() {
            int total = 1;
            public int step(int delta) {
                int next = this.total + delta;
                this.total = next;
                int snapshot = this.total;
                return snapshot;
            }
            public int read() { return this.total; }
        };
        counter.step(3);
        return counter.step(2) * 10 + counter.read();
    }
}
"#,
    );
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    cargo_test_translation(
        "anonymous_interface_post_write_local",
        translation.rust,
        r#"
#[test]
fn post_write_local_reads_same_anonymous_state() {
    assert_eq!(SnapshotCounterUse::run(), 66);
}
"#,
    );
}

#[test]
fn anonymous_interface_method_interleaves_two_field_writes_and_locals() {
    let translation = translate_source(
        r#"
interface PairCounter { int step(int delta); int read(); }
class PairCounterUse {
    static int run() {
        PairCounter counter = new PairCounter() {
            int first = 1;
            int second = 2;
            public int step(int delta) {
                int next = this.first + delta;
                this.first = next;
                int later = this.second + next;
                this.second = later;
                int snapshot = this.first + this.second;
                return snapshot;
            }
            public int read() { return this.first + this.second; }
        };
        counter.step(3);
        return counter.step(2) * 100 + counter.read();
    }
}
"#,
    );
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    cargo_test_translation(
        "anonymous_interface_two_post_write_locals",
        translation.rust,
        r#"
#[test]
fn two_field_writes_preserve_order_and_receiver_state() {
    assert_eq!(PairCounterUse::run(), 1818);
}
"#,
    );
}

#[test]
fn anonymous_interface_void_method_preserves_two_field_writes() {
    let translation = translate_source(
        r#"
interface PairAction { void step(int delta); int read(); }
class PairActionUse {
    static int run() {
        PairAction action = new PairAction() {
            int first = 1;
            int second = 2;
            public void step(int delta) {
                int next = this.first + delta;
                this.first = next;
                this.second = this.second + next;
            }
            public int read() { return this.first + this.second; }
        };
        action.step(3);
        return action.read();
    }
}
"#,
    );
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    cargo_test_translation(
        "anonymous_interface_void_two_writes",
        translation.rust,
        r#"
#[test]
fn void_method_updates_both_receiver_fields() {
    assert_eq!(PairActionUse::run(), 10);
}
"#,
    );
}

#[test]
fn anonymous_interface_nested_branch_writes_keep_local_and_receiver_state() {
    let translation = translate_source(
        r#"
interface BranchCounter { int update(int delta, boolean choose); int read(); }
class BranchCounterUse {
    static int run() {
        BranchCounter counter = new BranchCounter() {
            int first = 1;
            int second = 2;
            public int update(int delta, boolean choose) {
                int amount = delta + 1;
                if (choose) {
                    int next = this.first + amount;
                    this.first = next;
                } else {
                    int next = this.second + amount;
                    this.second = next;
                }
                int snapshot = this.first + this.second;
                return snapshot;
            }
            public int read() { return this.first + this.second; }
        };
        int before = counter.update(2, true);
        int after = counter.update(1, false);
        return before * 100 + after * 10 + counter.read();
    }
}
"#,
    );
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    cargo_test_translation(
        "anonymous_interface_nested_branch_writes",
        translation.rust,
        r#"
#[test]
fn nested_writes_share_one_object_and_keep_lexical_locals() {
    assert_eq!(BranchCounterUse::run(), 688);
}
"#,
    );
}

#[test]
fn anonymous_interface_nested_writes_can_reassign_outer_local() {
    let translation = translate_source(
        r#"
interface BranchSnapshot { int update(int delta, boolean choose); int read(); }
class BranchSnapshotUse {
    static int run() {
        BranchSnapshot counter = new BranchSnapshot() {
            int first = 1;
            int second = 2;
            public int update(int delta, boolean choose) {
                int snapshot = this.first + this.second;
                if (choose) {
                    snapshot = snapshot + delta;
                    this.first = snapshot;
                } else {
                    snapshot = snapshot - delta;
                    this.second = snapshot;
                }
                return snapshot;
            }
            public int read() { return this.first + this.second; }
        };
        int before = counter.update(2, true);
        int after = counter.update(1, false);
        return before * 100 + after * 10 + counter.read();
    }
}
"#,
    );
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    cargo_test_translation(
        "anonymous_interface_nested_outer_local",
        translation.rust,
        r#"
#[test]
fn branch_reassignment_remains_visible_after_receiver_write() {
    assert_eq!(BranchSnapshotUse::run(), 571);
}
"#,
    );
}

#[test]
fn anonymous_interface_field_projection_preserves_name_boundaries_and_literals() {
    let translation = translate_source(
        r#"
interface NameProbe { int sum(); String text(); }
class NameProbeUse {
    static int run() {
        NameProbe probe = new NameProbe() {
            int value = 2;
            int valueExtra = 3;
            public int sum() { return this.value + this.valueExtra; }
            public String text() { return "self.value"; }
        };
        return probe.sum() * 10 + probe.text().length();
    }
}
"#,
    );
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    cargo_test_translation(
        "anonymous_interface_field_projection_boundary",
        translation.rust,
        r#"
#[test]
fn field_name_prefix_and_string_literal_are_not_rewritten() {
    assert_eq!(NameProbeUse::run(), 60);
}
"#,
    );
}

#[test]
fn anonymous_interface_while_loop_writes_receiver_state() {
    let translation = translate_source(
        r#"
interface LoopCounter { int step(int count); int read(); }
class LoopCounterUse {
    static int run() {
        LoopCounter counter = new LoopCounter() {
            int total = 1;
            public int step(int count) {
                int index = 0;
                while (index < count && this.total < 100) {
                    this.total = this.total + index + 1;
                    index = index + 1;
                }
                return this.total;
            }
            public int read() { return this.total; }
        };
        int before = counter.step(3);
        int after = counter.step(2);
        return before * 100 + after * 10 + counter.read();
    }
}
"#,
    );
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    cargo_test_translation(
        "anonymous_interface_while_receiver_write",
        translation.rust,
        r#"
#[test]
fn loop_carried_cell_writes_and_index_reassignment_match_java() {
    assert_eq!(LoopCounterUse::run(), 810);
}
"#,
    );
}

#[test]
fn anonymous_interface_while_receiver_write_respects_break_and_continue() {
    let translation = translate_source(
        r#"
interface LoopControlCounter { int step(int count); int read(); }
class LoopControlCounterUse {
    static int run() {
        LoopControlCounter counter = new LoopControlCounter() {
            int total = 1;
            public int step(int count) {
                int index = 0;
                while (index < count) {
                    index = index + 1;
                    if (index == 2) { continue; }
                    this.total = this.total + index;
                    if (index == 3) { break; }
                }
                return this.total;
            }
            public int read() { return this.total; }
        };
        int before = counter.step(5);
        int after = counter.step(1);
        return before * 100 + after * 10 + counter.read();
    }
}
"#,
    );
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    cargo_test_translation(
        "anonymous_interface_while_receiver_write_loop_control",
        translation.rust,
        r#"
#[test]
fn loop_control_preserves_receiver_writes_and_iteration_order() {
    assert_eq!(LoopControlCounterUse::run(), 566);
}
"#,
    );
}

#[test]
fn anonymous_interface_do_while_receiver_write_checks_condition_after_continue() {
    let translation = translate_source(
        r#"
interface DoCounter { int step(int count); int read(); }
class DoCounterUse {
    static int run() {
        DoCounter counter = new DoCounter() {
            int total = 1;
            public int step(int count) {
                int index = 0;
                do {
                    index = index + 1;
                    if (index == 2) { continue; }
                    this.total = this.total + index;
                } while (index < count && this.total < 100);
                return this.total;
            }
            public int read() { return this.total; }
        };
        int before = counter.step(3);
        int after = counter.step(2);
        return before * 100 + after * 10 + counter.read();
    }
}
"#,
    );
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    cargo_test_translation(
        "anonymous_interface_do_while_receiver_write",
        translation.rust,
        r#"
#[test]
fn do_while_continue_checks_condition_after_body() {
    assert_eq!(DoCounterUse::run(), 566);
}
"#,
    );
}

#[test]
fn anonymous_interface_for_receiver_write_runs_update_after_continue() {
    let translation = translate_source(
        r#"
interface ForCounter { int step(int count); int read(); }
class ForCounterUse {
    static int run() {
        ForCounter counter = new ForCounter() {
            int total = 1;
            public int step(int count) {
                for (int index = 0; index < count && this.total < 100; index++) {
                    if (index == 1) { continue; }
                    this.total = this.total + index + 1;
                    if (index == 3) { break; }
                }
                return this.total;
            }
            public int read() { return this.total; }
        };
        int before = counter.step(5);
        int after = counter.step(2);
        return before * 100 + after * 10 + counter.read();
    }
}
"#,
    );
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    cargo_test_translation(
        "anonymous_interface_for_receiver_write",
        translation.rust,
        r#"
#[test]
fn for_continue_runs_update_before_rechecking_condition() {
    assert_eq!(ForCounterUse::run(), 1010);
}
"#,
    );
}

#[test]
fn anonymous_interface_labeled_for_controls_outer_receiver_loop() {
    let translation = translate_source(
        r#"
interface LabeledCounter { int advance(int limit); int read(); }
class LabeledCounterUse {
    static int run() {
        LabeledCounter counter = new LabeledCounter() {
            int total = 0;
            public int advance(int limit) {
                int i = 0;
                outer: for (i = 0; i < limit; i++) {
                    int inner = 0;
                    while (inner < 3) {
                        inner++;
                        if (inner == 2) continue outer;
                        this.total = this.total + i + inner;
                        if (i == 3) break outer;
                    }
                }
                return this.total + i;
            }
            public int read() { return this.total; }
        };
        return counter.advance(5) * 100 + counter.read();
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
        "anonymous_interface_labeled_for_receiver",
        translation.rust,
        r#"
#[test]
fn labeled_continue_updates_outer_for_and_break_exits_it() {
    assert_eq!(LabeledCounterUse::run(), 1310);
}
"#,
    );
}

#[test]
fn anonymous_interface_labeled_while_controls_outer_receiver_loop() {
    let translation = translate_source(
        r#"
interface WhileCounter { int advance(); int read(); }
class WhileCounterUse {
    static int run() {
        WhileCounter counter = new WhileCounter() {
            int total = 0;
            public int advance() {
                int i = 0;
                outer: while (i < 5) {
                    i++;
                    int inner = 0;
                    while (inner < 2) {
                        inner++;
                        if (i == 2) continue outer;
                        this.total = this.total + i + inner;
                        if (i == 4) break outer;
                    }
                }
                return this.total * 10 + i;
            }
            public int read() { return this.total; }
        };
        return counter.advance() * 100 + counter.read();
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
        "anonymous_interface_labeled_while_receiver",
        translation.rust,
        r#"
#[test]
fn labeled_continue_and_break_target_outer_while() {
    assert_eq!(WhileCounterUse::run(), 19419);
}
"#,
    );
}

#[test]
fn anonymous_interface_labeled_do_while_continue_checks_condition() {
    let translation = translate_source(
        r#"
interface DoCounter { int advance(); int read(); }
class DoCounterUse {
    static int run() {
        DoCounter counter = new DoCounter() {
            int total = 0;
            public int advance() {
                int i = 0;
                outer: do {
                    i++;
                    int inner = 0;
                    while (inner < 1) {
                        inner++;
                        if (i == 4) continue outer;
                        this.total = this.total + i + inner;
                    }
                } while (i < 4);
                return this.total * 10 + i;
            }
            public int read() { return this.total; }
        };
        return counter.advance() * 100 + counter.read();
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
        "anonymous_interface_labeled_do_while_receiver",
        translation.rust,
        r#"
#[test]
fn labeled_continue_checks_outer_do_while_condition() {
    assert_eq!(DoCounterUse::run(), 9409);
}
"#,
    );
}

#[test]
fn anonymous_interface_labeled_block_preserves_receiver_and_local_scope() {
    let translation = translate_source(
        r#"
interface BlockCounter { int advance(); int read(); }
class BlockCounterUse {
    static int run() {
        BlockCounter counter = new BlockCounter() {
            int total = 0;
            public int advance() {
                int i = 0;
                outer: {
                    int bonus = 3;
                    while (i < 4) {
                        i++;
                        {
                            int local = bonus + i;
                            this.total = this.total + local;
                        }
                        if (i == 2) break outer;
                    }
                    this.total = 100;
                }
                return this.total * 10 + i;
            }
            public int read() { return this.total; }
        };
        return counter.advance() * 100 + counter.read();
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
        "anonymous_interface_labeled_block_receiver",
        translation.rust,
        r#"
#[test]
fn labeled_block_break_keeps_receiver_state_and_local_scope() {
    assert_eq!(BlockCounterUse::run(), 9209);
}
"#,
    );
}

#[test]
fn anonymous_labeled_block_keeps_unlabeled_loop_jumps_on_enclosing_loop() {
    let translation = translate_source(
        r#"
interface BlockJump { int run(); }
class BlockJumpBase { BlockJumpBase() {} int run() { return 0; } }
class BlockJumpUse {
    static BlockJumpBase make() {
        return new BlockJumpBase() {
            int total = 0;
            @Override int run() {
                int i = 0;
                while (i < 4) {
                    i++;
                    block: {
                        if (i == 2) continue;
                        if (i == 3) break;
                        this.total = this.total + i;
                    }
                }
                return this.total * 10 + i;
            }
        };
    }
    static int run() {
        BlockJump value = new BlockJump() {
            int total = 0;
            public int run() {
                int i = 0;
                while (i < 4) {
                    i++;
                    block: {
                        if (i == 2) continue;
                        if (i == 3) break;
                        this.total = this.total + i;
                    }
                }
                return this.total * 10 + i;
            }
        };
        return value.run() * 100 + make().run();
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
        "anonymous_labeled_block_unlabeled_loop_jumps",
        translation.rust,
        r#"
#[test]
fn unlabeled_break_and_continue_skip_the_labeled_block_as_a_target() {
    assert_eq!(BlockJumpUse::run(), 1313);
}
"#,
    );
}

#[test]
fn anonymous_class_reference_local_mutation_preserves_super_and_alias() {
    let translation = translate_source(
        r#"
class AliasNode {
    int value;
    AliasNode(int value) { this.value = value; }
    int read() { return this.value; }
}
class AliasBase {
    AliasBase() {}
    int step(int delta) { return -1; }
    int bonus() { return 2; }
}
class AliasUse {
    static AliasBase make(AliasNode original) {
        return new AliasBase() {
            AliasNode node = original;
            @Override int step(int delta) {
                AliasNode local = this.node;
                if (delta > 0) { local.value = local.value + delta; }
                return this.node.read() + super.bonus();
            }
        };
    }
    static int run() {
        AliasNode original = new AliasNode(3);
        AliasBase value = make(original);
        return value.step(2) * 10 + original.read();
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
        "anonymous_class_reference_local_alias",
        translation.rust,
        r#"
#[test]
fn class_local_alias_mutates_original_then_calls_super() {
    assert_eq!(AliasUse::run(), 75);
}
"#,
    );
}

#[test]
fn anonymous_generic_parameter_collision_throw_preserves_parent_results() {
    let translation = translate_source(
        r#"
interface GenericEcho<T> { T echo(T input, boolean fail); }
interface StringEcho { String echo(String input, boolean fail); }
interface JoinedEcho extends GenericEcho<String>, StringEcho {}
class GenericEchoThrowUse {
    static JoinedEcho make() {
        return new JoinedEcho() {
            int marker = 1;
            public String echo(String input, boolean fail) {
                if (fail) throw new IllegalStateException("boom");
                return input;
            }
        };
    }
    static int run() {
        JoinedEcho joined = make();
        GenericEcho<String> generic = joined;
        StringEcho specific = joined;
        return generic.echo("b", false).length()
            + specific.echo("c", false).length();
    }
    static int runFail() {
        GenericEcho<String> generic = make();
        return generic.echo("d", true).length();
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
        "anonymous_generic_parameter_collision_throw",
        translation.rust,
        r#"
#[test]
fn generic_parameter_collision_forwards_result_through_both_parents() {
    assert_eq!(GenericEchoThrowUse::run().unwrap(), 2);
    assert!(GenericEchoThrowUse::run_fail().is_err());
}
"#,
    );
}

#[test]
fn anonymous_generic_collision_throw_preserves_parent_results() {
    let translation = translate_source(
        r#"
interface GenericSource<T> { T value(boolean fail); }
interface StringNamed { String value(boolean fail); }
interface StringJoined extends GenericSource<String>, StringNamed {}
class GenericCollisionThrowUse {
    static StringJoined make() {
        return new StringJoined() {
            String label = "hello";
            public String value(boolean fail) {
                if (fail) throw new IllegalStateException("boom");
                return this.label;
            }
        };
    }
    static int run() {
        StringJoined joined = make();
        GenericSource<String> source = joined;
        StringNamed named = joined;
        return joined.value(false).length() + source.value(false).length()
            + named.value(false).length();
    }
    static int runFail() { return make().value(true).length(); }
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
        "anonymous_generic_collision_throw",
        translation.rust,
        r#"
#[test]
fn generic_collision_forwards_result_through_both_parents() {
    assert_eq!(GenericCollisionThrowUse::run().unwrap(), 15);
    assert!(GenericCollisionThrowUse::run_fail().is_err());
}
"#,
    );
}

#[test]
fn anonymous_duplicate_parent_throw_contract_forwards_through_each_interface() {
    let translation = translate_source(
        r#"
interface LeftCheck { int value(boolean fail); }
interface RightCheck { int value(boolean fail); }
interface BothCheck extends LeftCheck, RightCheck {}
class DuplicateThrowUse {
    static BothCheck make() {
        return new BothCheck() {
            int total = 3;
            public int value(boolean fail) {
                if (fail) throw new IllegalStateException("boom");
                return this.total;
            }
        };
    }
    static int run() { return make().value(false); }
    static int runLeft() {
        LeftCheck left = make();
        return left.value(false);
    }
    static int runRight() {
        RightCheck right = make();
        return right.value(false);
    }
    static int runFail() { return make().value(true); }
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
        "anonymous_duplicate_parent_throw_contract",
        translation.rust,
        r#"
#[test]
fn duplicate_parent_contracts_forward_result() {
    assert_eq!(DuplicateThrowUse::run().unwrap(), 3);
    assert_eq!(DuplicateThrowUse::run_left().unwrap(), 3);
    assert_eq!(DuplicateThrowUse::run_right().unwrap(), 3);
    assert!(DuplicateThrowUse::run_fail().is_err());
}
"#,
    );
}

#[test]
fn anonymous_interface_read_only_direct_throw_preserves_virtual_result() {
    let translation = translate_source(
        r#"
interface ReadOnlyThrower { int read(boolean fail); int state(); }
class ReadOnlyThrowerUse {
    static ReadOnlyThrower make() {
        return new ReadOnlyThrower() {
            int total = 4;
            public int read(boolean fail) {
                if (fail) throw new IllegalStateException("boom");
                return this.total;
            }
            public int state() { return this.total; }
        };
    }
    static int run() {
        ReadOnlyThrower value = make();
        return value.read(false) + value.state();
    }
    static int runFail() {
        ReadOnlyThrower value = make();
        return value.read(true);
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
        "anonymous_interface_read_only_direct_throw",
        translation.rust,
        r#"
#[test]
fn read_only_throw_uses_fallible_virtual_contract() {
    assert_eq!(ReadOnlyThrowerUse::run().unwrap(), 8);
    assert!(ReadOnlyThrowerUse::run_fail().is_err());
}
"#,
    );
}

#[test]
fn anonymous_interface_fallible_void_mutator_returns_ok_after_write() {
    let translation = translate_source(
        r#"
interface FallibleVoidCounter { void add(int[] values); int read(); }
class FallibleVoidCounterUse {
    static FallibleVoidCounter make() {
        return new FallibleVoidCounter() {
            int total = 0;
            public void add(int[] values) { this.total = this.total + values[0]; }
            public int read() { return this.total; }
        };
    }
    static int run() {
        FallibleVoidCounter counter = make();
        int[] first = {3};
        int[] second = {2};
        counter.add(first);
        counter.add(second);
        return counter.read();
    }
    static int runFail() {
        FallibleVoidCounter counter = make();
        int[] values = {};
        counter.add(values);
        return counter.read();
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
        "anonymous_interface_fallible_void_mutator",
        translation.rust,
        r#"
#[test]
fn void_mutator_returns_ok_and_propagates_failure() {
    assert_eq!(FallibleVoidCounterUse::run().unwrap(), 5);
    assert!(FallibleVoidCounterUse::run_fail().is_err());
}
"#,
    );
}

#[test]
fn anonymous_interface_read_only_array_failure_preserves_virtual_result() {
    let translation = translate_source(
        r#"
interface FallibleReadCounter { int read(int[] values); int state(); }
class FallibleReadCounterUse {
    static FallibleReadCounter make() {
        return new FallibleReadCounter() {
            int total = 7;
            public int read(int[] values) { return values[0] + this.total; }
            public int state() { return this.total; }
        };
    }
    static int run() {
        FallibleReadCounter counter = make();
        int[] values = {3};
        return counter.read(values) + counter.state();
    }
    static int runFail() {
        FallibleReadCounter counter = make();
        int[] values = {};
        return counter.read(values);
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
        "anonymous_interface_read_only_array_result",
        translation.rust,
        r#"
#[test]
fn read_only_method_uses_fallible_virtual_contract() {
    assert_eq!(FallibleReadCounterUse::run().unwrap(), 17);
    assert!(FallibleReadCounterUse::run_fail().is_err());
}
"#,
    );
}

#[test]
fn anonymous_interface_direct_throw_promotes_virtual_contract() {
    let translation = translate_source(
        r#"
interface ThrowingCounter { int step(boolean fail); int read(); }
class ThrowingCounterUse {
    static ThrowingCounter make() {
        return new ThrowingCounter() {
            int total = 0;
            public int step(boolean fail) {
                if (fail) throw new IllegalStateException("boom");
                this.total = this.total + 1;
                return this.total;
            }
            public int read() { return this.total; }
        };
    }
    static int run() {
        ThrowingCounter counter = make();
        return counter.step(false) + counter.read();
    }
    static int runFail() {
        ThrowingCounter counter = make();
        return counter.step(true);
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
        "anonymous_interface_direct_throw_contract",
        translation.rust,
        r#"
#[test]
fn interface_throw_propagates_without_making_factory_fallible() {
    assert_eq!(ThrowingCounterUse::run().unwrap(), 2);
    assert!(ThrowingCounterUse::run_fail().is_err());
}
"#,
    );
}

#[test]
fn anonymous_interface_enhanced_for_rebinds_array_parameter_and_state() {
    let translation = translate_source(
        r#"
interface ArrayCounter { int sum(int[] values); int read(); }
class ArrayCounterUse {
    static int run() {
        ArrayCounter counter = new ArrayCounter() {
            int total = 0;
            public int sum(int[] values) {
                outer: for (int value : values) {
                    if (value == 2) continue outer;
                    this.total = this.total + value;
                    if (value == 3) break outer;
                }
                return this.total;
            }
            public int read() { return this.total; }
        };
        int[] first = {1, 2, 3, 4};
        int[] second = {5};
        return counter.sum(first) * 100 + counter.sum(second) * 10 + counter.read();
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
        "anonymous_interface_enhanced_for_array_parameter",
        translation.rust,
        r#"
#[test]
fn array_foreach_preserves_labeled_control_and_receiver_state() {
    assert_eq!(ArrayCounterUse::run(), 499);
}
"#,
    );
}

#[test]
fn anonymous_interface_receiver_reference_local_keeps_shared_field_handle() {
    let translation = translate_source(
        r#"
interface ReferenceCounter { int compute(); int computeDirect(); int read(); }
class ReferenceNode {
    int value;
    ReferenceNode(int value) { this.value = value; }
    void set(int value) { this.value = value; }
    int read() { return this.value; }
}
class ReferenceCounterUse {
    static ReferenceCounter make(ReferenceNode original) {
        return new ReferenceCounter() {
            ReferenceNode node = original;
            int total = 0;
            public int compute() {
                ReferenceNode alias = this.node;
                alias.set(alias.read() + 1);
                this.total = alias.read();
                return this.total;
            }
            public int computeDirect() {
                if (this.total == 6) {
                    ReferenceNode alias = this.node;
                    alias.value = alias.value + 2;
                    this.total = alias.value;
                }
                return this.total;
            }
            public int read() { return this.node.value + this.total; }
        };
    }
    static int run() {
        ReferenceNode original = new ReferenceNode(3);
        ReferenceCounter counter = make(original);
        original.value = 5;
        return counter.compute() * 1000 + counter.computeDirect() * 100
            + counter.read() * 10 + original.read();
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
        "anonymous_interface_receiver_reference_local",
        translation.rust,
        r#"
#[test]
fn local_alias_reads_same_node_after_caller_mutation() {
    assert_eq!(ReferenceCounterUse::run(), 6968);
}
"#,
    );
}

#[test]
fn anonymous_interface_nested_string_field_write_uses_owned_slot() {
    let translation = translate_source(
        r#"
interface LabelChooser { String choose(boolean left); String read(); }
class LabelChooserUse {
    static int run() {
        LabelChooser chooser = new LabelChooser() {
            String label = "start";
            public String choose(boolean left) {
                if (left) { this.label = "left"; }
                else { this.label = "right"; }
                return this.label;
            }
            public String read() { return this.label; }
        };
        int first = chooser.choose(true).length();
        int second = chooser.choose(false).length();
        return first * 100 + second * 10 + chooser.read().length();
    }
}
"#,
    );
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    cargo_test_translation(
        "anonymous_interface_nested_owned_string_write",
        translation.rust,
        r#"
#[test]
fn branch_writes_replace_owned_string_without_losing_state() {
    assert_eq!(LabelChooserUse::run(), 455);
}
"#,
    );
}

#[test]
fn anonymous_interface_expression_statement_mutates_captured_alias() {
    let translation = translate_source(
        r#"
class CounterSink {
    int value;
    void add(int amount) { this.value = this.value + amount; }
    int read() { return this.value; }
}
interface Recorder { int step(int delta); int read(); }
class RecorderUse {
    static int run() {
        CounterSink sink = new CounterSink();
        Recorder recorder = new Recorder() {
            int total = 1;
            public int step(int delta) {
                sink.add(this.total + delta);
                this.total = this.total + delta;
                return sink.read();
            }
            public int read() { return this.total; }
        };
        int first = recorder.step(2);
        int second = recorder.step(3);
        return first * 100 + second * 10 + sink.read() + recorder.read();
    }
}
"#,
    );
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    cargo_test_translation(
        "anonymous_interface_captured_alias_expression_stmt",
        translation.rust,
        r#"
#[test]
fn ordinary_call_mutates_the_same_captured_sink() {
    assert_eq!(RecorderUse::run(), 405);
}
"#,
    );
}

#[test]
fn anonymous_interface_read_only_body_allows_captured_call_statement() {
    let translation = translate_source(
        r#"
class TraceBox {
    int value;
    void add(int amount) { this.value = this.value + amount; }
    int read() { return this.value; }
}
interface Runner { int run(int delta); int current(); }
class RunnerUse {
    static int run() {
        TraceBox sink = new TraceBox();
        Runner runner = new Runner() {
            int base = 2;
            public int run(int delta) {
                sink.add(this.base + delta);
                return sink.read();
            }
            public int current() { return sink.read(); }
        };
        int first = runner.run(3);
        int second = runner.run(4);
        return first * 100 + second * 10 + runner.current() + sink.read();
    }
}
"#,
    );
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    cargo_test_translation(
        "anonymous_interface_read_only_captured_call",
        translation.rust,
        r#"
#[test]
fn read_only_receiver_body_can_call_through_shared_capture() {
    assert_eq!(RunnerUse::run(), 632);
}
"#,
    );
}

#[test]
fn anonymous_interface_increment_statements_update_receiver_cell() {
    let translation = translate_source(
        r#"
interface Ticker { int tick(); int read(); }
class TickerUse {
    static int run() {
        Ticker ticker = new Ticker() {
            int total = 1;
            public int tick() {
                this.total++;
                ++this.total;
                return this.total;
            }
            public int read() { return this.total; }
        };
        int first = ticker.tick();
        int second = ticker.tick();
        return first * 100 + second * 10 + ticker.read();
    }
}
"#,
    );
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    cargo_test_translation(
        "anonymous_interface_increment_statements",
        translation.rust,
        r#"
#[test]
fn prefix_and_postfix_increment_mutate_the_same_cell() {
    assert_eq!(TickerUse::run(), 355);
}
"#,
    );
}

#[test]
fn anonymous_interface_decrement_statements_update_receiver_cell() {
    let translation = translate_source(
        r#"
interface Countdown { int tick(); int read(); }
class CountdownUse {
    static int run() {
        Countdown counter = new Countdown() {
            int total = 5;
            public int tick() {
                this.total--;
                --this.total;
                return this.total;
            }
            public int read() { return this.total; }
        };
        int first = counter.tick();
        int second = counter.tick();
        return first * 100 + second * 10 + counter.read();
    }
}
"#,
    );
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    cargo_test_translation(
        "anonymous_interface_decrement_statements",
        translation.rust,
        r#"
#[test]
fn prefix_and_postfix_decrement_mutate_the_same_cell() {
    assert_eq!(CountdownUse::run(), 311);
}
"#,
    );
}

#[test]
fn anonymous_class_nested_reference_field_replacement_preserves_source_alias() {
    let translation = translate_source(
        r#"
class BranchNode {
    int value;
    BranchNode(int value) { this.value = value; }
    int read() { return this.value; }
}
class BranchBase {
    BranchBase() {}
    int value(boolean replace) { return 0; }
}
class BranchClassUse {
    static int run() {
        BranchNode original = new BranchNode(3);
        BranchBase branch = new BranchBase() {
            BranchNode node = original;
            @Override int value(boolean replace) {
                if (replace) { this.node = new BranchNode(7); }
                return this.node.read();
            }
        };
        return branch.value(false) * 100 + branch.value(true) * 10 + original.read();
    }
}
"#,
    );
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    cargo_test_translation(
        "anonymous_class_nested_reference_field_replacement",
        translation.rust,
        r#"
#[test]
fn branch_replacement_does_not_change_the_original_handle() {
    assert_eq!(BranchClassUse::run(), 373);
}
"#,
    );
}

#[test]
fn anonymous_class_factory_parameter_field_keeps_shared_handle() {
    let translation = translate_source(
        r#"
class ParameterBranchNode {
    int value;
    ParameterBranchNode(int value) { this.value = value; }
    int read() { return this.value; }
}
class ParameterBranchBase {
    ParameterBranchBase() {}
    int value() { return 0; }
    boolean same() { return false; }
}
class ParameterBranchUse {
    static ParameterBranchBase make(ParameterBranchNode original, boolean replace) {
        return new ParameterBranchBase() {
            ParameterBranchNode node = original;
            @Override int value() {
                if (replace) { this.node = new ParameterBranchNode(7); }
                return this.node.read();
            }
            @Override boolean same() { return this.node == original; }
        };
    }
    static int run() {
        ParameterBranchNode original = new ParameterBranchNode(3);
        ParameterBranchBase first = make(original, true);
        ParameterBranchBase second = make(original, false);
        if (!first.same() || !second.same()) { return -1; }
        int firstValue = first.value();
        if (first.same() || !second.same()) { return -2; }
        return firstValue * 100 + second.value() * 10 + original.read();
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
            .contains("pub fn make(original: std::rc::Rc<ParameterBranchNode>")
            && translation.rust.contains("Self::make(original.clone()")
            && translation
                .rust
                .contains("node: std::cell::RefCell::new(original.clone())")
            && translation.rust.contains("std::rc::Rc::ptr_eq"),
        "{}",
        translation.rust
    );
    cargo_test_translation(
        "anonymous_class_factory_parameter_field_alias",
        translation.rust,
        r#"
#[test]
fn factory_parameter_remains_a_shared_handle() {
    assert_eq!(ParameterBranchUse::run(), 733);
}
"#,
    );
}
