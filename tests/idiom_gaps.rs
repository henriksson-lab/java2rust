use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use new_java2rust::facts::{
    ExternalApiArgumentEffectFact, ExternalApiEffectFact, ExternalApiRole,
    ExternalArgumentAliasPolicy, ExternalFieldFact, ExternalFieldKind, FactProvenance, FactSet,
    FieldKey, NullabilityState, ReceiverMutabilityFact,
};
use new_java2rust::java_ir::JavaType;
use new_java2rust::{
    translate_source, translate_source_with_context_sources,
    translate_source_with_context_summary_sources, translate_source_with_facts,
};

fn idiom_gap(path: &str) -> String {
    fs::read_to_string(Path::new("examples/idiom-gaps").join(path)).unwrap()
}

struct GeneratedCrateModule {
    name: &'static str,
    rust: String,
    tests: &'static str,
}

fn cargo_check_generated_modules(crate_name: &str, modules: Vec<GeneratedCrateModule>) {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let crate_dir = Path::new("/tmp").join(format!("new_java2rust_{crate_name}_{unique}"));
    let src_dir = crate_dir.join("src");
    fs::create_dir_all(&src_dir).unwrap();
    fs::write(
        crate_dir.join("Cargo.toml"),
        format!(
            "[package]\nname = \"new-java2rust-{crate_name}-{unique}\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\n[lib]\npath = \"src/lib.rs\"\n"
        ),
    )
    .unwrap();
    let mut lib_rs = String::new();
    for module in modules {
        lib_rs.push_str(&format!(
            "pub mod {} {{\n{}\n{}\n}}\n",
            module.name, module.rust, module.tests
        ));
    }
    fs::write(src_dir.join("lib.rs"), lib_rs).unwrap();
    let output = Command::new("cargo")
        .arg("check")
        .arg("--tests")
        .arg("--quiet")
        .current_dir(&crate_dir)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "generated crate check failed for {crate_name}\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn documents_gatk_acronym_identifier_splitting_gap() {
    let translation = translate_source(&idiom_gap("AcronymIdentifierNames.java"));

    assert!(
        translation.rust.contains("g_a_t_k_v_c_f_constants"),
        "{}",
        translation.rust
    );
    assert!(
        translation.rust.contains("plugin_package_names"),
        "{}",
        translation.rust
    );
}

#[test]
fn documents_java_string_method_spelling_gap() {
    let translation = translate_source(&idiom_gap("JavaStringMethods.java"));

    assert!(
        translation.rust.contains(".to_lowercase()"),
        "{}",
        translation.rust
    );
    assert!(
        translation
            .rust
            .contains("(left.to_lowercase() == right.to_lowercase())"),
        "{}",
        translation.rust
    );
    assert!(
        translation.rust.contains("value.len() as i32")
            && !translation.rust.contains(".length()")
            && !translation.rust.contains(".equals("),
        "{}",
        translation.rust
    );
}

#[test]
fn translates_java_numeric_relational_promotion_before_rust_comparison() {
    let translation = translate_source(
        r#"
class NumericRelationalPromotion {
    boolean beforeLimit(int end, long limit) {
        return end < limit;
    }
}
"#,
    );

    assert!(
        translation.rust.contains("return ((end as i64) < limit);"),
        "{}",
        translation.rust
    );
    assert!(
        !translation.rust.contains("return (end < limit);"),
        "{}",
        translation.rust
    );
}

#[test]
fn fallback_static_call_uses_translated_signature_arg_shapes() {
    let mut facts = FactSet::default();
    facts
        .external_api_argument_effect_facts
        .push(ExternalApiArgumentEffectFact {
            method_name: "fill".to_string(),
            receiver_type: None,
            parameter_index: 0,
            parameter_type: Some(JavaType::Named {
                name: "MutableConfig".to_string(),
                args: Vec::new(),
            }),
            parameter_nullability: NullabilityState::DefinitelyNonNull,
            argument_mutability: ReceiverMutabilityFact::MutatesReceiver,
            alias_policy: ExternalArgumentAliasPolicy::Borrowed,
            provenance: FactProvenance::UserOverride,
        });
    let translation =
        translate_source_with_facts(&idiom_gap("FallbackStaticMutBorrowArgument.java"), &facts);

    assert!(
        translation
            .rust
            .contains("StaticConfigMutator::refresh(&mut config);"),
        "{}",
        translation.rust
    );
}

#[test]
fn reused_map_argument_uses_owned_recovery_fact_for_clone() {
    let translation = translate_source(&idiom_gap("ReusedMapArgumentClone.java"));

    assert!(
        translation.rust.contains(
            "self.patch(values.clone()/* TODO(translation): validate added clone */, key.clone()"
        ),
        "{}",
        translation.rust
    );
}

#[test]
fn documents_string_format_static_call_gap() {
    let translation = translate_source(&idiom_gap("StringFormatStaticCall.java"));

    assert!(
        translation
            .rust
            .contains("return format!(\"{}:{}\", name, count);"),
        "{}",
        translation.rust
    );
    assert!(
        !translation.rust.contains("string.format"),
        "{}",
        translation.rust
    );
}

#[test]
fn dynamic_string_format_template_uses_runtime_helper() {
    let translation = translate_source(&idiom_gap("DynamicStringFormatTemplate.java"));

    assert!(
        translation.rust.contains("pub fn java_string_format")
            && translation
                .rust
                .contains("java_string_format(template, vec![count.to_string()])")
            && !translation.rust.contains("return format(template"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "dynamic_string_format_template",
        vec![GeneratedCrateModule {
            name: "dynamic_string_format_template",
            rust: translation.rust,
            tests: r#"
#[cfg(test)]
mod generated_tests {
    use super::*;

    #[test]
    fn formats_runtime_template() {
        let fixture = DynamicStringFormatTemplate {};
        assert_eq!(fixture.describe("count", 7), "count=7".to_string());
    }
}
"#,
        }],
    );
}

#[test]
fn nullable_string_format_and_concat_use_java_null_display_policy() {
    let translation = translate_source(&idiom_gap("NullableStringFormatAndConcat.java"));

    assert!(
        translation
            .rust
            .contains("pub fn describe(&self, value: Option<String>) -> String")
            && translation
                .rust
                .contains("value.as_deref().unwrap_or(\"null\")")
            && !translation.rust.contains("format!(\"value={}\", value)")
            && !translation
                .rust
                .contains("Option<String>` doesn't implement"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "nullable_string_format_and_concat",
        vec![GeneratedCrateModule {
            name: "nullable_string_format_and_concat",
            rust: translation.rust,
            tests: r#"
#[cfg(test)]
mod generated_tests {
    use super::*;

    #[test]
    fn preserves_java_null_display_text() {
        let fixture = NullableStringFormatAndConcat {};
        assert_eq!(fixture.describe(None), "value=null".to_string());
        assert_eq!(fixture.describe(Some("x".to_string())), "value=x".to_string());
    }
}
"#,
        }],
    );
}

#[test]
fn string_pattern_methods_borrow_owned_string_arguments() {
    let translation = translate_source(&idiom_gap("StringPatternBorrowing.java"));

    assert!(
        translation.rust.contains("value.contains(needle.as_str())")
            && translation
                .rust
                .contains("value.ends_with(suffix.as_str())")
            && !translation.rust.contains("contains(needle)")
            && !translation.rust.contains("ends_with(suffix)"),
        "{}",
        translation.rust
    );
    assert!(
        !translation
            .rust
            .contains("the trait bound `String: Pattern` is not satisfied"),
        "{}",
        translation.rust
    );
}

#[test]
fn string_pattern_chained_receiver_keeps_borrowed_pattern_argument() {
    let translation = translate_source(&idiom_gap("StringPatternChainedReceiver.java"));

    assert!(
        translation
            .rust
            .contains("path.to_string().to_lowercase().ends_with(suffix.as_str())")
            && !translation.rust.contains("ends_with(suffix)"),
        "{}",
        translation.rust
    );
}

#[test]
fn string_trim_owned_slots_convert_borrowed_view_to_string() {
    let translation = translate_source(&idiom_gap("StringTrimOwnedSlot.java"));

    assert!(
        translation
            .rust
            .contains("let value: String = raw.trim().to_string();")
            && translation
                .rust
                .contains("index = Some(pieces[2 as usize].trim().to_string());")
            && !translation.rust.contains("let value: String = raw.trim();")
            && !translation.rust.contains("Some(pieces[2 as usize].trim())"),
        "{}",
        translation.rust
    );
}

#[test]
fn java_string_split_with_limit_uses_runtime_helper() {
    let translation = translate_source(&idiom_gap("StringSplitWithLimit.java"));

    assert!(
        translation.rust.contains("pub fn java_string_split")
            && translation
                .rust
                .contains("java_string_split(line.as_str(), \"\\\\\\\\t\".to_string(), (-1))")
            && !translation
                .rust
                .contains(".split(\"\\\\t\".to_string(), (-1))"),
        "{}",
        translation.rust
    );
}

#[test]
fn indexed_string_array_reads_clone_only_for_owned_slots() {
    let translation = translate_source(&idiom_gap("IndexedStringArrayOwnership.java"));

    assert!(
        translation
            .rust
            .contains("self.valid(Some(parts[0 as usize].as_str()))")
            && translation
                .rust
                .contains("let sample: String = parts[0 as usize].clone()")
            && translation
                .rust
                .contains("parts[1 as usize].trim().is_empty()")
            && !translation.rust.contains(
                "parts[1 as usize].clone()/* TODO(translation): validate added clone */.trim()"
            ),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "indexed-string-array-ownership",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn nullable_path_overload_argument_uses_option_param_and_non_optional_resolve_sibling() {
    let translation = translate_source(&idiom_gap("NullablePathOverloadArgument.java"));

    assert!(
        translation
            .rust
            .contains("pub fn check_path_path(&self, value: Path, optional_index: Option<Path>)")
            && translation
                .rust
                .contains("self.check_path_path(value, None)")
            && translation
                .rust
                .contains("value.resolve_sibling(\"index\".to_string())")
            && !translation.rust.contains("Some(None)"),
        "{}",
        translation.rust
    );
}

#[test]
fn throwing_call_local_initializer_propagates_result_with_question_mark() {
    let translation = translate_source(&idiom_gap("ThrowingCallLocalResult.java"));

    assert!(
        translation
            .rust
            .contains("let value: String = self.read(good)?;")
            && !translation
                .rust
                .contains("let value: String = self.read(good);"),
        "{}",
        translation.rust
    );
}

#[test]
fn throwing_call_chain_promotes_caller_result_shells() {
    let translation = translate_source(&idiom_gap("ThrowingCallChainResultShell.java"));

    assert!(
        translation
            .rust
            .contains("pub fn middle(&self, ok: bool) -> Result<String, JavaException>")
            && translation
                .rust
                .contains("pub fn top(&self, ok: bool) -> Result<String, JavaException>")
            && translation.rust.contains("return Ok(self.leaf(ok)?);")
            && translation
                .rust
                .contains("let value: String = self.middle(ok)?;"),
        "{}",
        translation.rust
    );
}

#[test]
fn documents_to_array_empty_string_array_gap() {
    let translation = translate_source(&idiom_gap("ToArrayEmptyStringArray.java"));

    assert!(
        translation.rust.contains("names.to_array(vec![])"),
        "{}",
        translation.rust
    );
    assert!(
        !translation.rust.contains("Java array creation"),
        "{}",
        translation.rust
    );
}

#[test]
fn external_get_name_getter_no_longer_requires_runtime_downcast() {
    let translation = translate_source(&idiom_gap("TypedGetterAvoidsDowncast.java"));

    assert!(
        !translation.rust.contains("downcast::<String>()"),
        "{}",
        translation.rust
    );
    assert!(
        !translation.rust.contains("expect(\"invalid Java cast\")"),
        "{}",
        translation.rust
    );
    assert!(
        translation
            .rust
            .contains("let name: String = box.get_name();"),
        "{}",
        translation.rust
    );
}

#[test]
fn typed_feature_reader_header_cast_uses_external_pack_return_fact() {
    let translation = new_java2rust::translate_source_with_facts(
        &idiom_gap("TypedFeatureReaderHeaderCast.java"),
        &new_java2rust::external_packs::htsjdk_tribble_feature_pack_facts(),
    );

    assert!(
        translation.rust.contains("return reader.get_header();")
            && !translation.rust.contains("downcast::<VCFHeader>()")
            && !translation.rust.contains("Some(reader.get_header())")
            && !translation.rust.contains("invalid Java cast"),
        "{}",
        translation.rust
    );
}

#[test]
fn object_return_from_typed_external_getter_erases_once_at_boundary() {
    let translation = new_java2rust::translate_source_with_facts(
        &idiom_gap("ObjectReturnFromTypedExternalGetter.java"),
        &new_java2rust::external_packs::htsjdk_tribble_feature_pack_facts(),
    );

    assert!(
        translation.rust.contains("pub fn header(&self, reader:")
            && translation
                .rust
                .contains("-> Option<Box<dyn std::any::Any>>")
            && translation.rust.contains(
                "return reader.get_header().map(|value| Box::new(value) as Box<dyn std::any::Any>);"
            )
            && !translation.rust.contains("downcast::<VCFHeader>()")
            && !translation.rust.contains("invalid Java cast")
            && !translation.rust.contains("-> Object"),
        "{}",
        translation.rust
    );
}

#[test]
fn documents_nullable_concrete_object_gap() {
    let translation = translate_source(&idiom_gap("NullableParameterAndLocal.java"));

    assert!(
        translation
            .rust
            .contains("pub fn keep_or_default(&self, value: Option<Object>) -> Option<Object>")
            && translation
                .rust
                .contains("let current: Option<Object> = None;")
            && translation.rust.contains("return current;")
            && translation.rust.contains("return value;")
            && translation.rust.contains("if value.is_none()")
            && !translation.rust.contains("value == None")
            && !translation.rust.contains("let current: Object = None;")
            && !translation.rust.contains("return current.unwrap();")
            && !translation.rust.contains("return value.unwrap();"),
        "{}",
        translation.rust
    );
}

#[test]
fn nullable_ternary_branches_consume_branch_refinements_and_slot_shape() {
    let translation = translate_source(
        r#"
class TernaryNullBranchRefinement {
    Integer incrementOrOne(Integer old) {
        return old == null ? 1 : old + 1;
    }

    Integer maybeValue(boolean missing, Integer value) {
        return missing ? null : value;
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("pub fn increment_or_one(&self, old: Option<i32>) -> i32")
            && translation
                .rust
                .contains("if old.is_none() { 1 } else { (old.unwrap() + 1) }")
            && translation
                .rust
                .contains("if missing { None } else { Some(value) }")
            && !translation.rust.contains("(old + 1)")
            && !translation
                .rust
                .contains("if missing { None } else { value }"),
        "{}",
        translation.rust
    );
}

#[test]
fn nullable_ternary_call_receiver_uses_branch_refinement_before_optional_adapter() {
    let translation = translate_source(
        r#"
import java.nio.file.Path;

class TernaryNullReceiverRefinement {
    String maybeUri(Path path) {
        return path == null ? null : path.toAbsolutePath().toUri().toString();
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("path.unwrap().to_absolute_path().to_uri().to_string()")
            && !translation.rust.contains("path.unwrap().as_ref().unwrap()")
            && !translation.rust.contains("path.unwrap().as_ref().expect"),
        "{}",
        translation.rust
    );
}

#[test]
fn documents_proven_non_null_member_field_gap() {
    let translation = translate_source(&idiom_gap("NonNullFieldFromConstructors.java"));

    assert!(
        translation.rust.contains("name: String") && !translation.rust.contains("Option<String>"),
        "{}",
        translation.rust
    );
}

#[test]
fn documents_java_collection_api_gap() {
    let translation = translate_source(&idiom_gap("CollectionAddAllAndSize.java"));

    assert!(
        translation
            .rust
            .contains("let mut values: Vec<String> = Vec::new();")
            && translation
                .rust
                .contains("values.extend(vec![\"a\".to_string(), \"b\".to_string()]);")
            && translation.rust.contains("return values.len() as i32;")
            && !translation.rust.contains(".add_all(")
            && !translation.rust.contains(".size()")
            && !translation.rust.contains("ArrayList::new"),
        "{}",
        translation.rust
    );
}

#[test]
fn map_size_return_int_uses_java_int_width() {
    let translation = translate_source(&idiom_gap("MapSizeReturnInt.java"));

    assert!(
        translation.rust.contains("return values.len() as i32;")
            && !translation.rust.contains("values.len();")
            && !translation.rust.contains(".size()"),
        "{}",
        translation.rust
    );
}

#[test]
fn array_arguments_use_borrowed_slice_views() {
    let translation = translate_source(&idiom_gap("ArrayArgumentSliceView.java"));

    assert!(
        translation.rust.contains(
            "self.length_of_first(self.values.as_ref().expect(\"Java null receiver\").as_slice())"
        ) && translation
            .rust
            .contains("self.length_of_first(local.as_slice())")
            && !translation
                .rust
                .contains("length_of_first(self.values.unwrap())")
            && !translation.rust.contains("length_of_first(local)"),
        "{}",
        translation.rust
    );
}

#[test]
fn shared_self_field_reads_recover_owned_values_without_moves() {
    let translation = translate_source(&idiom_gap("SharedSelfFieldOwnedRecovery.java"));

    assert!(
        translation.rust.contains("return self.name.clone();")
            && translation.rust.contains("return self.workspace.clone();")
            && translation.rust.contains("return self.paths.clone();")
            && translation
                .rust
                .contains("self.length(self.workspace.as_str())")
            && !translation.rust.contains("return self.name;")
            && !translation.rust.contains("return self.workspace;")
            && !translation.rust.contains("return self.paths;")
            && !translation.rust.contains("self.length(self.workspace)"),
        "{}",
        translation.rust
    );
}

#[test]
fn bulk_add_all_borrowed_generated_rust_preserves_behavior() {
    let translation = translate_source(&idiom_gap("BulkAddAllBorrowed.java"));

    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    assert!(
        translation
            .rust
            .contains("pub fn merge(&self, values: &mut Vec<String>, other: &[String])")
            && translation
                .rust
                .contains("values.extend(other.iter().cloned());")
            && !translation.rust.contains("other: Vec<String>"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "bulk_add_all_borrowed",
        vec![GeneratedCrateModule {
            name: "bulk_add_all_borrowed",
            rust: translation.rust,
            tests: r#"
#[cfg(test)]
mod generated_tests {
    use super::*;

    #[test]
    fn typechecks_bulk_add_all_borrowed() {
        let fixture = BulkAddAllBorrowed {};
        let mut values = vec!["a".to_string()];
        let other = vec!["b".to_string(), "c".to_string()];
        fixture.merge(&mut values, &other);
        assert_eq!(
            values,
            vec!["a".to_string(), "b".to_string(), "c".to_string()]
        );
    }
}
"#,
        }],
    );
}

#[test]
fn documents_external_primitive_getter_dispatch_coercion_gap() {
    let translation = translate_source(&idiom_gap("PrimitiveGetterDispatchCoercion.java"));

    assert!(
        translation
            .rust
            .contains("let length: i32 = record.get_read_length();")
            && !translation
                .rust
                .contains("unsupported Java dispatch coercion")
            && !translation.rust.contains("downcast::<"),
        "{}",
        translation.rust
    );
}

#[test]
fn external_boolean_getter_no_longer_requires_runtime_downcast() {
    let translation = translate_source(&idiom_gap("BooleanGetterDispatchCoercion.java"));

    assert!(
        translation
            .rust
            .contains("let value: bool = read.is_paired();")
            && !translation
                .rust
                .contains("unsupported Java dispatch coercion"),
        "{}",
        translation.rust
    );
}

#[test]
fn documents_external_byte_array_getter_dispatch_coercion_gap() {
    let translation = translate_source(&idiom_gap("ByteArrayGetterDispatchCoercion.java"));

    assert!(
        translation
            .rust
            .contains("let value: Vec<i8> = read.get_bases_no_copy();")
            && !translation
                .rust
                .contains("unsupported Java dispatch coercion")
            && !translation.rust.contains("downcast::<"),
        "{}",
        translation.rust
    );
}

#[test]
fn documents_java_bean_getter_setter_naming_gap() {
    let translation = translate_source(&idiom_gap("JavaBeanGetterSetterNames.java"));

    assert!(
        translation.rust.contains("pub fn get_value")
            && translation.rust.contains("pub fn set_value")
            && translation.rust.contains("pub fn copy_from(&mut self")
            && translation
                .rust
                .contains("self.set_value(other.get_value())")
            && translation.rust.contains("return self.get_value();"),
        "{}",
        translation.rust
    );
}

#[test]
fn documents_collection_remove_if_lambda_gap() {
    let translation = translate_source(&idiom_gap("CollectionRemoveIfLambda.java"));

    assert!(
        translation
            .rust
            .contains("pub fn remove_blank_lines(&self, lines: &mut Vec<String>)")
            && translation
                .rust
                .contains("lines.retain(|line| !(line.is_empty()))")
            && !translation.rust.contains(".remove_if(")
            && !translation
                .rust
                .contains("unsupported Java expression: lambda"),
        "{}",
        translation.rust
    );
}

#[test]
fn documents_for_each_lambda_side_effect_gap() {
    let translation = translate_source(&idiom_gap("ForEachLambdaSideEffect.java"));

    assert!(
        translation
            .rust
            .contains("pub fn emit_all(&self, names: Vec<String>)")
            && translation.rust.contains("for name in names.iter()")
            && translation.rust.contains("self.emit(name.clone());")
            && !translation.rust.contains(".for_each(")
            && !translation
                .rust
                .contains("unsupported Java expression: lambda"),
        "{}",
        translation.rust
    );
}

#[test]
fn translates_map_for_each_block_lambda_as_loop() {
    let translation = translate_source(&idiom_gap("MapForEachBlockLambda.java"));

    assert!(
        translation
            .rust
            .contains("for (name, item) in source.iter()")
            && translation.rust.contains("dest.insert(name")
            && !translation.rust.contains(".for_each(")
            && !translation
                .rust
                .contains("unsupported Java expression: lambda"),
        "{}",
        translation.rust
    );
}

#[test]
fn translates_map_for_each_try_catch_lambda_without_placeholder() {
    let translation = translate_source(&idiom_gap("MapForEachTryCatchLambda.java"));

    assert!(
        translation
            .rust
            .contains("for (name, future) in futures.iter()")
            && translation
                .rust
                .contains("match (|| -> Result<(), JavaException>")
            && translation.rust.contains("dest.insert(name")
            && !translation
                .rust
                .contains("unsupported Java statement: try/catch/finally"),
        "{}",
        translation.rust
    );
}

#[test]
fn translates_executor_submit_block_lambda_as_closure() {
    let translation = translate_source(&idiom_gap("ExecutorSubmitBlockLambda.java"));

    assert!(
        translation.rust.contains(".submit(|| {")
            && translation.rust.contains("let mut value")
            && translation.rust.contains("return value.trim();")
            && !translation.rust.contains("downcast::<Callable>")
            && !translation
                .rust
                .contains("unsupported Java expression: lambda"),
        "{}",
        translation.rust
    );
}

#[test]
fn executor_submit_lambda_receiverless_helper_call_uses_current_self() {
    let translation = translate_source(
        r#"
class ExecutorSubmitReceiverlessHelper {
    Executor service;

    String helper(String value) {
        return value.trim();
    }

    Object run(String input) {
        return service.submit(() -> {
            String value = input;
            return helper(value);
        });
    }
}
"#,
    );

    assert!(
        translation.rust.contains(".submit(|| {")
            && translation
                .rust
                .contains("return self.helper(value.as_str());")
            && !translation.rust.contains("return helper(value")
            && !translation
                .rust
                .contains("unsupported Java expression: lambda"),
        "{}",
        translation.rust
    );
}

#[test]
fn executor_submit_future_slot_shapes_block_lambda_result() {
    let translation = translate_source(
        r#"
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.concurrent.Future;

class ExecutorSubmitFutureSlot {
    ExecutorService executor;

    Map<String, Reader> run(String name) {
        Map<String, Reader> readers = new LinkedHashMap<>();
        Map<String, Future<Reader>> futures = new LinkedHashMap<>();
        futures.put(name, executor.submit(() -> {
            try {
                return new Reader();
            } catch (IOException e) {
                throw new RuntimeException("bad", e);
            }
        }));
        return readers;
    }
}

class Reader {
}
"#,
    );

    assert!(
        translation.rust.contains(".submit(|| {")
            && translation
                .rust
                .contains("match (|| -> Result<Reader, JavaException>")
            && translation.rust.contains("Ok(value) => return Ok(value)")
            && translation
                .rust
                .contains("return Err(JavaException::with_cause(\"RuntimeException\"")
            && !translation
                .rust
                .contains("Result<std::collections::HashMap<String, Reader>, JavaException>")
            && !translation
                .rust
                .contains(".submit(|| {\n    match (|| -> Result<(), JavaException>"),
        "{}",
        translation.rust
    );
}

#[test]
fn unshaped_constructor_argument_call_that_throws_propagates_result() {
    let translation = translate_source(
        r#"
class UnshapedThrowingConstructorArg {
    Reader open(String path) throws IOException {
        return new Reader(path);
    }

    Wrapper wrap(String path) throws IOException {
        return new Wrapper(open(path), path);
    }
}

class Reader {
    Reader(String path) {}
}

class Wrapper {
    Wrapper(Reader reader, String path) {}
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("Wrapper::new(self.open(path.clone()")
            && translation.rust.contains(")?, path.clone()")
            && !translation
                .rust
                .contains("Wrapper::new(self.open(path), path)"),
        "{}",
        translation.rust
    );
}

#[test]
fn stream_block_lambda_return_uses_declared_local_receiver_not_static_owner() {
    let translation = translate_source(
        r#"
import java.util.List;
import java.util.stream.Collectors;

class StreamBlockLambdaLocalReceiver {
    List<Product> buildAll(List<String> names) {
        return names.stream().map(name -> {
            ProductBuilder builder = Product.newBuilder();
            builder.setName(name);
            return builder.build();
        }).collect(Collectors.toList());
    }
}
"#,
    );

    assert!(
        translation.rust.contains("builder.build()")
            && !translation.rust.contains("ProductBuilder::build()")
            && !translation.rust.contains("Builder::build()"),
        "{}",
        translation.rust
    );
}

#[test]
fn for_each_lambda_side_effect_generated_rust_compiles() {
    let translation = translate_source(&idiom_gap("ForEachLambdaSideEffect.java"));
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
    fn for_each_lambda_side_effect_compiles() {
        let fixture = ForEachLambdaSideEffect {};
        fixture.emit_all(vec!["a".to_string(), "b".to_string()]);
    }
}
"#
    );
    let rust_path = Path::new("/tmp").join("new_java2rust_for_each_lambda_side_effect_test.rs");
    let bin_path = Path::new("/tmp").join("new_java2rust_for_each_lambda_side_effect_test");
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
        "forEach lambda side-effect test did not compile\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn neutral_idiom_fixtures_generated_crate_cargo_checks() {
    let for_each = translate_source(&idiom_gap("ForEachLambdaSideEffect.java"));
    let flat_map = translate_source(&idiom_gap("MethodReferenceFlatMap.java"));
    let enum_switch = translate_source(&idiom_gap("SwitchReturnsEnumMatch.java"));
    let enum_equality = translate_source(&idiom_gap("EnumEqualityComparison.java"));
    let try_catch = translate_source(&idiom_gap("SimpleTryCatchReturn.java"));
    let try_finally = translate_source(&idiom_gap("SimpleTryFinallyCleanup.java"));
    let internal_throw = translate_source(&idiom_gap("InternalThrowingCallInTry.java"));
    let tail_recursive = translate_source(&idiom_gap("TailRecursiveAccumulator.java"));
    for translation in [
        &for_each,
        &flat_map,
        &enum_switch,
        &enum_equality,
        &try_catch,
        &try_finally,
        &internal_throw,
        &tail_recursive,
    ] {
        assert!(
            !translation.diagnostics.has_errors(),
            "{:?}",
            translation.diagnostics
        );
    }

    cargo_check_generated_modules(
        "neutral_idiom_fixtures",
        vec![
            GeneratedCrateModule {
                name: "for_each_lambda",
                rust: for_each.rust,
                tests: r#"
#[cfg(test)]
mod generated_tests {
    use super::*;

    #[test]
    fn typechecks_for_each_lambda() {
        let fixture = ForEachLambdaSideEffect {};
        fixture.emit_all(vec!["a".to_string(), "b".to_string()]);
    }
}
"#,
            },
            GeneratedCrateModule {
                name: "method_reference_flat_map",
                rust: flat_map.rust,
                tests: r#"
#[cfg(test)]
mod generated_tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn typechecks_method_reference_flat_map() {
        let fixture = MethodReferenceFlatMap {};
        let groups = vec![vec!["a".to_string()], vec!["b".to_string()]];
        let _: HashSet<String> = fixture.flatten(&groups);
    }
}
"#,
            },
            GeneratedCrateModule {
                name: "enum_switch",
                rust: enum_switch.rust,
                tests: r#"
#[cfg(test)]
mod generated_tests {
    use super::*;

    #[test]
    fn typechecks_enum_switch() {
        let fixture = SwitchReturnsEnumMatch {};
        let _: String = fixture.label(Type::SNP);
    }
}
"#,
            },
            GeneratedCrateModule {
                name: "enum_equality",
                rust: enum_equality.rust,
                tests: r#"
#[cfg(test)]
mod generated_tests {
    use super::*;

    #[test]
    fn typechecks_enum_equality() {
        let fixture = EnumEqualityComparison {};
        assert!(fixture.same(Kind::SNP, Kind::SNP));
        assert!(fixture.different(Kind::SNP, Kind::INDEL));
    }
}
"#,
            },
            GeneratedCrateModule {
                name: "simple_try_catch",
                rust: try_catch.rust,
                tests: r#"
#[cfg(test)]
mod generated_tests {
    use super::*;

    #[test]
    fn typechecks_simple_try_catch() {
        let fixture = SimpleTryCatchReturn {};
        let _: i32 = fixture.parse(false).unwrap();
    }
}
"#,
            },
            GeneratedCrateModule {
                name: "simple_try_finally",
                rust: try_finally.rust,
                tests: r#"
#[cfg(test)]
mod generated_tests {
    use super::*;

    #[test]
    fn typechecks_simple_try_finally() {
        let fixture = SimpleTryFinallyCleanup {};
        fixture.copy(Resource {}).unwrap();
    }
}
"#,
            },
            GeneratedCrateModule {
                name: "internal_throwing_call",
                rust: internal_throw.rust,
                tests: r#"
#[cfg(test)]
mod generated_tests {
    use super::*;

    #[test]
    fn typechecks_internal_throwing_call() {
        let fixture = InternalThrowingCallInTry {};
        fixture.copy(false).unwrap();
    }
}
"#,
            },
            GeneratedCrateModule {
                name: "tail_recursive_accumulator",
                rust: tail_recursive.rust,
                tests: r#"
#[cfg(test)]
mod generated_tests {
    use super::*;

    #[test]
    fn typechecks_tail_recursive_accumulator() {
        let fixture = TailRecursiveAccumulator {};
        let _: i64 = fixture.factorial(5);
    }
}
"#,
            },
        ],
    );
}

#[test]
fn documents_static_owner_and_utility_call_gap() {
    let translation = translate_source(&idiom_gap("StaticConstantOwner.java"));

    assert!(
        translation.rust.contains("Owner::CONSTANT"),
        "{}",
        translation.rust
    );
    assert!(
        translation.rust.contains("std::cmp::max(left, right)")
            && !translation.rust.contains("owner.CONSTANT")
            && !translation.rust.contains("math.max("),
        "{}",
        translation.rust
    );
}

#[test]
fn documents_primitive_array_allocation_gap() {
    let translation = translate_source(&idiom_gap("PrimitiveArrayWithComputedLength.java"));

    assert!(
        translation
            .rust
            .contains("let values: Vec<i8> = vec![0; len as usize];"),
        "{}",
        translation.rust
    );
    assert!(
        !translation.rust.contains("Java array creation"),
        "{}",
        translation.rust
    );
    assert!(
        translation.rust.contains("arrays.fill("),
        "{}",
        translation.rust
    );
}

#[test]
fn documents_string_array_literal_argument_gap() {
    let translation = translate_source(&idiom_gap("StringArrayLiteralArgument.java"));

    assert!(
        translation
            .rust
            .contains("writer.write_next(vec![format!(\"{}{}\", prefix, comment)], false);")
            && !translation.rust.contains("Java array creation")
            && !translation.rust.contains("Vec::new()"),
        "{}",
        translation.rust
    );
}

#[test]
fn documents_map_put_get_java_api_gap() {
    let translation = translate_source(&idiom_gap("MapPutGetJavaApi.java"));

    assert!(
        translation
            .rust
            .contains("values: &mut std::collections::HashMap<String, String>")
            && translation.rust.contains(
                "values.insert(key.clone()/* TODO(translation): validate added clone */, value);"
            )
            && translation
                .rust
                .contains("values.get(&key).expect(\"Java null receiver\").clone()")
            && !translation.rust.contains(".put(")
            && !translation.rust.contains("&mut Map"),
        "{}",
        translation.rust
    );
}

#[test]
fn field_map_put_reused_key_clones_at_owned_boundary() {
    let translation = translate_source(&idiom_gap("FieldMapPutKeyReuse.java"));

    assert!(
        translation
            .rust
            .contains("self.paths.insert(sample.clone()/* TODO(translation): validate added clone */, URI::new(path))")
            && translation.rust.contains("format!(\"duplicate {}\", sample)")
            && !translation.rust.contains("self.paths.insert(sample, URI::new(path));"),
        "{}",
        translation.rust
    );
}

#[test]
fn optional_map_put_reused_value_borrows_before_clone_unwrap() {
    let translation = translate_source(&idiom_gap("OptionalMapPutValueReuse.java"));

    assert!(
        translation.rust.contains(
            "path.as_ref().unwrap().clone()/* TODO(translation): validate added clone */"
        ) && translation.rust.contains(
            "path.as_ref().map(|value| value.to_string()).unwrap_or_else(|| \"null\".to_string())"
        ) && !translation
            .rust
            .contains("paths.insert(sample, path.unwrap())"),
        "{}",
        translation.rust
    );
}

#[test]
fn optional_argument_wrap_reused_value_clones_before_some() {
    let translation = translate_source(&idiom_gap("OptionalArgumentWrapReuse.java"));

    assert!(
        translation.rust.contains(
            "self.valid(Some(name.clone()/* TODO(translation): validate added clone */))"
        ) && translation.rust.contains("format!(\"bad {}\", name)")
            && !translation.rust.contains("self.valid(Some(name))"),
        "{}",
        translation.rust
    );
}

#[test]
fn map_get_optional_receiver_to_string_uses_java_null_receiver_policy() {
    let translation = translate_source(&idiom_gap("MapGetOptionalReceiverToString.java"));

    assert!(
        translation
            .rust
            .contains("paths.get(&sample).expect(\"Java null receiver\").to_string()")
            && !translation.rust.contains("paths.get(&sample).to_string()")
            && !translation
                .rust
                .contains("unwrap_or_else(|| \"null\".to_string())"),
        "{}",
        translation.rust
    );
}

#[test]
fn map_get_in_ternary_primitive_slot_unwraps_and_copies_value() {
    let translation = translate_source(&idiom_gap("MapGetTernaryPrimitiveIndex.java"));

    assert!(
        translation
            .rust
            .contains("indexes.get(&name).copied().expect(\"Java null receiver\")")
            && !translation.rust.contains("indexes.get(&name) } else"),
        "{}",
        translation.rust
    );
}

#[test]
fn constructor_string_argument_reuse_clones_at_owned_boundary() {
    let translation = translate_source(&idiom_gap("ConstructorStringReuse.java"));

    assert!(
        translation.rust.contains(
            "let uri: URI = URI::new(path.clone()/* TODO(translation): validate added clone */);"
        ) && translation.rust.contains("path.is_empty()")
            && !translation.rust.contains("let uri: URI = URI::new(path);"),
        "{}",
        translation.rust
    );
}

#[test]
fn method_argument_reuse_clones_at_owned_boundary() {
    let translation = translate_source(&idiom_gap("MethodArgumentReuseOwnedBoundary.java"));

    assert!(
        translation.rust.contains(
            "let kept: String = self.keep(name.clone()/* TODO(translation): validate added clone */);"
        ) && translation.rust.contains("name.is_empty()")
            && !translation.rust.contains("let kept: String = self.keep(name);"),
        "{}",
        translation.rust
    );
}

#[test]
fn optional_uri_constructor_path_arg_compiles_through_java_path_arg() {
    let translation = translate_source(&idiom_gap("OptionalUriConstructorPathArg.java"));

    assert!(
        translation.rust.contains("pub trait JavaPathArg")
            && translation
                .rust
                .contains("pub fn new<P: JavaPathArg>(value: P) -> URI")
            && translation.rust.contains("let uri: URI = URI::new(path);"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "optional-uri-constructor-path-arg",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn map_key_set_int_loop_casts_len_for_java_int_comparison() {
    let translation = translate_source(&idiom_gap("MapKeySetIntLoop.java"));

    assert!(
        translation
            .rust
            .contains("values.keys().cloned().collect::<Vec<_>>()")
            && translation.rust.contains("values.len() as i32")
            && translation
                .rust
                .contains("keys.get(i as usize).expect(\"Java index out of bounds\").clone()")
            && !translation.rust.contains("values.key_set()")
            && !translation.rust.contains("return keys.get(i);"),
        "{}",
        translation.rust
    );
}

#[test]
fn map_entry_set_iterator_next_get_key_uses_native_iterator_shape() {
    let translation = translate_source(&idiom_gap("MapEntrySetFirstKey.java"));

    assert!(
        translation
            .rust
            .contains("values().as_ref().expect(\"Java null receiver\").iter().next().expect(\"Java iterator exhausted\").0.clone()")
            && !translation.rust.contains(".entry_set()")
            && !translation.rust.contains(".iterator()")
            && !translation.rust.contains(".get_key()"),
        "{}",
        translation.rust
    );
}

#[test]
fn map_entry_set_iterator_next_get_key_uses_receiver_signature_shape() {
    let translation = translate_source(&idiom_gap("MapEntrySetFirstKeyFromNullableField.java"));

    assert!(
        translation.rust.contains("self.holder.as_ref().unwrap().get_values().as_ref().expect(\"Java null receiver\").iter().next().expect(\"Java iterator exhausted\").0.clone()")
            && !translation.rust.contains(".entry_set()")
            && !translation.rust.contains(".iterator()")
            && !translation.rust.contains(".get_key()"),
        "{}",
        translation.rust
    );
}

#[test]
fn optional_field_mutating_overload_call_uses_signature_name_and_mut_receiver() {
    let translation = translate_source(&idiom_gap("OptionalFieldMutatingOverloadCall.java"));

    assert!(
        translation
            .rust
            .contains("self.holder.as_mut().unwrap().add_string_int(name, value);")
            && !translation.rust.contains(".as_ref().unwrap().add(")
            && !translation.rust.contains(".add(name, value);"),
        "{}",
        translation.rust
    );
}

#[test]
fn context_translation_uses_sibling_overload_signature_for_call_target() {
    let caller = r#"
class ContextCaller {
    Holder holder;

    void apply(String name, int value) {
        holder.add(name, value);
    }
}
"#;
    let holder = r#"
class Holder {
    int count;

    void add(String name, int value) {
        count = value;
    }

    void add(String name, int value, int extra) {
        count = value + extra;
    }
}
"#;
    let translation =
        translate_source_with_context_sources(caller, &[holder.to_string()], &FactSet::default());

    assert!(
        translation
            .rust
            .contains("self.holder.as_mut().unwrap().add_string_int(name, value);")
            && !translation.rust.contains(".add(name, value);"),
        "{}",
        translation.rust
    );
}

#[test]
fn context_translation_uses_sibling_static_string_constant_shape() {
    let caller = r#"
class ContextConstantCaller {
    String name() {
        return Constants.NAME;
    }
}
"#;
    let constants = r#"
class Constants {
    static final String NAME = "name";
}
"#;
    let translation = translate_source_with_context_sources(
        caller,
        &[constants.to_string()],
        &FactSet::default(),
    );

    assert!(
        translation
            .rust
            .contains("return Constants::NAME.to_string();")
            && !translation.rust.contains("downcast::<String>()")
            && !translation.rust.contains("invalid Java cast"),
        "{}",
        translation.rust
    );
}

#[test]
fn context_translation_does_not_emit_opaque_facade_for_sibling_source_type() {
    let caller = r#"
class UsesSiblingSource {
    SiblingSource sibling;

    int read() {
        return sibling.value();
    }
}
"#;
    let sibling = r#"
class SiblingSource {
    int value() {
        return 7;
    }
}
"#;
    let translation =
        translate_source_with_context_sources(caller, &[sibling.to_string()], &FactSet::default());

    assert!(
        translation.rust.contains("sibling: Option<SiblingSource>,")
            && !translation.rust.contains("pub struct SiblingSource;"),
        "{}",
        translation.rust
    );
}

#[test]
fn source_summary_direct_field_type_emits_opaque_support_shell() {
    let caller = r#"
class UsesSummaryInput {
    FeatureInput headerOverride;
}
"#;
    let summary = r#"
class FeatureInput {
}
"#;
    let translation = translate_source_with_context_summary_sources(
        caller,
        &[summary.to_string()],
        &FactSet::default(),
    );

    assert!(
        translation
            .rust
            .contains("header_override: Option<FeatureInput>,")
            && translation.rust.contains("pub struct FeatureInput;"),
        "{}",
        translation.rust
    );

    let full_context_translation =
        translate_source_with_context_sources(caller, &[summary.to_string()], &FactSet::default());
    assert!(
        full_context_translation
            .rust
            .contains("header_override: Option<FeatureInput>,")
            && !full_context_translation
                .rust
                .contains("pub struct FeatureInput;"),
        "{}",
        full_context_translation.rust
    );
}

#[test]
fn source_summary_opaque_type_gets_used_inherited_method_surface() {
    let caller = r#"
class UsesSummaryInputMethod {
    FeatureInput headerOverride;

    Path headerPath() {
        return headerOverride.toPath();
    }
}
"#;
    let summary = r#"
class FeatureInput extends GATKPath {
}

class GATKPath {
    Path toPath() {
        return new Path();
    }
}

class Path {
}
"#;
    let translation = translate_source_with_context_summary_sources(
        caller,
        &[summary.to_string()],
        &FactSet::default(),
    );

    assert!(
        translation.rust.contains("pub struct FeatureInput;")
            && translation.rust.contains("impl FeatureInput")
            && translation.rust.contains("pub fn to_path(&self) -> Path")
            && translation
                .rust
                .contains("self.header_override.as_ref().unwrap().to_path()"),
        "{}",
        translation.rust
    );
}

#[test]
fn source_summary_method_surface_can_be_added_after_external_return_selects_type() {
    let caller = r#"
class LateSummarySurfaceUse {
    Reader reader;

    Dictionary read() {
        return reader.header().dictionary();
    }

    SAMFileHeader readHeader() {
        return reader.header().samHeader();
    }

    void touchIterator() {
        reader.header().iterator();
    }
}
"#;
    let summary = r#"
class Reader {
}

class Header {
    Dictionary dictionary() {
        return new Dictionary();
    }

    SAMFileHeader samHeader() {
        return null;
    }

    Iterator iterator() {
        return null;
    }
}

class Dictionary {
}
"#;
    let mut facts = FactSet::default();
    facts.external_api_effect_facts.push(ExternalApiEffectFact {
        method_name: "header".to_string(),
        receiver_type: Some("Reader".to_string()),
        arg_count: Some(0),
        role: ExternalApiRole::Getter,
        receiver_mutability: ReceiverMutabilityFact::ReadOnly,
        return_type: Some(JavaType::Named {
            name: "Header".to_string(),
            args: Vec::new(),
        }),
        return_nullability: NullabilityState::DefinitelyNonNull,
        thrown: Vec::new(),
        provenance: FactProvenance::UserOverride,
    });
    let translation =
        translate_source_with_context_summary_sources(caller, &[summary.to_string()], &facts);

    assert!(
        translation.rust.contains("pub struct Header;")
            && translation.rust.contains("pub struct Dictionary;")
            && translation.rust.contains("pub struct SAMFileHeader;")
            && translation.rust.contains("pub struct CloseableIterator;")
            && translation
                .rust
                .contains("pub fn dictionary(&self) -> Dictionary")
            && translation
                .rust
                .contains("pub fn sam_header(&self) -> SAMFileHeader")
            && translation
                .rust
                .contains("pub fn iterator(&self) -> CloseableIterator")
            && !translation.rust.contains("pub fn dictionary(&self) -> ()"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "late_source_summary_surface",
        vec![GeneratedCrateModule {
            name: "late_source_summary_surface",
            rust: translation.rust,
            tests: r#"
#[cfg(test)]
mod generated_tests {
    use super::*;

    #[test]
    fn typechecks_late_source_summary_surface() {
        let fixture = LateSummarySurfaceUse { reader: Some(Reader) };
        let _: Dictionary = fixture.read();
        let _: SAMFileHeader = fixture.read_header();
        fixture.touch_iterator();
    }
}
"#,
        }],
    );
}

#[test]
fn selected_opaque_support_type_gets_used_known_method_surfaces_by_arity() {
    let caller = r#"
class UsesIntervalArgumentCollectionSurface {
    IntervalArgumentCollection intervals;
    OtherOpaque other;

    boolean hasIntervals() {
        return intervals.intervalsSpecified();
    }

    List<SimpleInterval> readIntervals(SAMSequenceDictionary dictionary) {
        return intervals.getIntervals(dictionary);
    }

    void touchOther() {
        other.toString();
    }

    String contig(SimpleInterval interval) {
        return interval.getContig();
    }

    int span(SimpleInterval interval) {
        return interval.getEnd() - interval.getStart();
    }
}
"#;
    let translation = translate_source_with_context_summary_sources(
        caller,
        &Vec::<String>::new(),
        &FactSet::default(),
    );

    assert!(
        translation
            .rust
            .contains("pub struct IntervalArgumentCollection;")
            && translation
                .rust
                .contains("pub fn intervals_specified(&self) -> bool")
            && translation
                .rust
                .contains("pub fn get_intervals<T0>(&self, _arg0: T0) -> Vec<SimpleInterval>")
            && translation
                .rust
                .contains("self.intervals.as_ref().unwrap().intervals_specified()")
            && translation
                .rust
                .contains("self.intervals.as_ref().unwrap().get_intervals(dictionary)"),
        "{}",
        translation.rust
    );
    assert!(
        translation.rust.contains("impl SimpleInterval")
            && translation
                .rust
                .contains("pub fn get_contig(&self) -> String")
            && translation.rust.contains("pub fn get_start(&self) -> i32")
            && translation.rust.contains("pub fn get_end(&self) -> i32"),
        "{}",
        translation.rust
    );
    assert!(
        translation.rust.contains("pub struct OtherOpaque;")
            && !translation
                .rust
                .contains("impl OtherOpaque {\n    pub fn get_intervals"),
        "{}",
        translation.rust
    );
}

#[test]
fn selected_opaque_future_get_is_generic_instance_surface() {
    let source = r#"
import java.util.concurrent.Future;

class FutureGetUse {
    Future<FeatureReader> future;

    FeatureReader read() {
        return future.get();
    }
}
"#;
    let translation = translate_source(source);

    assert!(
        translation
            .rust
            .contains("pub fn get<T: Default>(&self) -> T"),
        "{}",
        translation.rust
    );
    assert!(
        translation
            .rust
            .contains("self.future.as_ref().unwrap().get()"),
        "{}",
        translation.rust
    );
    assert!(
        !translation.rust.contains("pub fn get<R: Default>() -> R"),
        "{}",
        translation.rust
    );
}

#[test]
fn emitted_type_extending_source_summary_super_gets_inherited_method_surface() {
    let caller = r#"
import java.util.Set;

class ImportTool extends GatkTool {
    Set<VCFHeaderLine> lines;

    void mergeDefaults() {
        lines.addAll(getDefaultToolVCFHeaderLines());
    }
}
"#;
    let summary = r#"
import java.util.Set;

class GatkTool {
    protected Set<VCFHeaderLine> getDefaultToolVCFHeaderLines() {
        return null;
    }
}

class VCFHeaderLine {
}
"#;
    let translation = translate_source_with_context_summary_sources(
        caller,
        &[summary.to_string()],
        &FactSet::default(),
    );

    assert!(
        translation.rust.contains("impl ImportTool")
            && translation
                .rust
                .contains("pub fn get_default_tool_v_c_f_header_lines(&self) -> std::collections::HashSet<VCFHeaderLine>")
            && translation
                .rust
                .contains("self.get_default_tool_v_c_f_header_lines()")
            && !translation
                .rust
                .contains("pub fn get_default_tool_v_c_f_header_lines(&self) -> ()"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "emitted_source_summary_super_method_surface",
        vec![GeneratedCrateModule {
            name: "emitted_source_summary_super_method_surface",
            rust: translation.rust,
            tests: r#"
#[cfg(test)]
mod generated_tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn typechecks_emitted_super_method_surface() {
        let mut fixture = ImportTool { lines: Some(HashSet::new()) };
        fixture.merge_defaults();
    }
}
"#,
        }],
    );
}

#[test]
fn external_pack_typed_set_and_dictionary_getters_shape_facade_returns() {
    let source = r#"
import htsjdk.samtools.SAMSequenceDictionary;
import htsjdk.variant.vcf.VCFHeader;
import htsjdk.variant.vcf.VCFHeaderLine;
import java.util.Set;

class TypedExternalGetterUse {
    Set<VCFHeaderLine> lines(VCFHeader header) {
        return header.getMetaDataInInputOrder();
    }

    SAMSequenceDictionary dictionary(VCFHeader header) {
        return header.getSequenceDictionary();
    }
}
"#;
    let pack_facts = new_java2rust::external_packs::htsjdk_vcf_pack_facts();
    let mut facts = FactSet::default();
    for method_name in ["getMetaDataInInputOrder", "getSequenceDictionary"] {
        facts.external_api_effect_facts.push(
            pack_facts
                .external_api_effect_for_receiver(Some("VCFHeader"), method_name)
                .unwrap_or_else(|| panic!("VCFHeader.{method_name} should be pack-owned"))
                .clone(),
        );
    }
    let translation = translate_source_with_facts(source, &facts);

    assert!(
        translation.rust.contains(
            "pub fn get_meta_data_in_input_order(&self) -> std::collections::HashSet<VCFHeaderLine>"
        ) && translation
            .rust
            .contains("pub fn get_sequence_dictionary(&self) -> SAMSequenceDictionary")
            && !translation
                .rust
                .contains("pub fn get_meta_data_in_input_order(&self) -> ()")
            && !translation
                .rust
                .contains("pub fn get_sequence_dictionary(&self) -> ()"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "typed_external_vcf_getters",
        vec![GeneratedCrateModule {
            name: "typed_external_vcf_getters",
            rust: translation.rust,
            tests: r#"
#[cfg(test)]
mod generated_tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn typechecks_typed_external_getters() {
        let fixture = TypedExternalGetterUse {};
        let _: HashSet<VCFHeaderLine> = fixture.lines(&VCFHeader);
        let _: SAMSequenceDictionary = fixture.dictionary(&VCFHeader);
    }
}
"#,
        }],
    );
}

#[test]
fn optional_local_external_receiver_getter_is_unwrapped_before_call() {
    let source = r#"
import htsjdk.samtools.SAMSequenceDictionary;
import htsjdk.variant.vcf.VCFHeader;

class OptionalExternalReceiverGetterUse {
    VCFHeader maybeHeader() {
        return null;
    }

    SAMSequenceDictionary dictionary() {
        VCFHeader header = maybeHeader();
        return header.getSequenceDictionary();
    }
}
"#;
    let pack_facts = new_java2rust::external_packs::htsjdk_vcf_pack_facts();
    let mut facts = FactSet::default();
    facts.external_api_effect_facts.push(
        pack_facts
            .external_api_effect_for_receiver(Some("VCFHeader"), "getSequenceDictionary")
            .expect("VCFHeader.getSequenceDictionary should be pack-owned")
            .clone(),
    );
    let translation = translate_source_with_facts(source, &facts);

    assert!(
        translation
            .rust
            .contains("header.as_ref().unwrap().get_sequence_dictionary()")
            && !translation
                .rust
                .contains("return header.get_sequence_dictionary()"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "optional_external_receiver_getter",
        vec![GeneratedCrateModule {
            name: "optional_external_receiver_getter",
            rust: translation.rust,
            tests: r#"
#[cfg(test)]
mod generated_tests {
    use super::*;

    #[test]
    fn typechecks_optional_receiver_getter() {
        let fixture = OptionalExternalReceiverGetterUse {};
        let _: SAMSequenceDictionary = fixture.dictionary();
    }
}
"#,
        }],
    );
}

#[test]
fn nullable_method_result_in_callback_local_stays_optional_for_optional_arg_slot() {
    let translation = translate_source(
        r#"
class NullableCallbackLocalSlot {
    ExecutorService executor;

    Path maybePath(String name) {
        return null;
    }

    Reader openWithIndex(String name, Path index) {
        if (index == null) {
            return new Reader();
        }
        return new Reader();
    }

    Future run(String name) {
        return executor.submit(() -> {
            Path path = maybePath(name);
            return openWithIndex(name, path);
        });
    }
}

class Path {
}

class Reader {
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("let mut path: Option<Path> = self.maybe_path(name);")
            && translation
                .rust
                .contains("self.open_with_index(name, path)")
            && !translation
                .rust
                .contains("let mut path: Path = self.maybe_path")
            && !translation.rust.contains("Some(path"),
        "{}",
        translation.rust
    );
}

#[test]
fn foreach_lambda_throw_wraps_enclosing_method_in_result() {
    let translation = translate_source(
        r#"
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.concurrent.Future;

class FutureGetForEachCatchThrow {
    Map<String, Reader> run(Map<String, Future<Reader>> futures) {
        Map<String, Reader> readers = new LinkedHashMap<>();
        futures.forEach((name, future) -> {
            try {
                Reader reader = future.get();
                readers.put(name, reader);
            } catch (ExecutionException e) {
                throw new UserException("bad", e);
            }
        });
        return readers;
    }
}

class Reader {
}
"#,
    );

    assert!(
        translation.rust.contains(
            "pub fn run(&self, futures: std::collections::HashMap<String, Future>) -> Result<std::collections::HashMap<String, Reader>, JavaException>"
        ) && translation
            .rust
            .contains("Err(e) if e.is_instance(&[\"ExecutionException\"])")
            && translation.rust.contains("return Err(JavaException::with_cause(\"UserException\"")
            && translation.rust.contains("return Ok(readers);"),
        "{}",
        translation.rust
    );
}

#[test]
fn external_pack_typed_string_list_getter_supports_indexed_sample_access() {
    let source = r#"
import htsjdk.variant.vcf.VCFHeader;

class TypedExternalStringListGetterUse {
    String firstSample(VCFHeader header) {
        return header.getGenotypeSamples().get(0);
    }
}
"#;
    let pack_facts = new_java2rust::external_packs::htsjdk_vcf_pack_facts();
    let mut facts = FactSet::default();
    facts.external_api_effect_facts.push(
        pack_facts
            .external_api_effect_for_receiver(Some("VCFHeader"), "getGenotypeSamples")
            .expect("VCFHeader.getGenotypeSamples should be pack-owned")
            .clone(),
    );
    let translation = translate_source_with_facts(source, &facts);

    assert!(
        translation
            .rust
            .contains("pub fn get_genotype_samples(&self) -> Vec<String>")
            && translation
                .rust
                .contains("get_genotype_samples().get(0 as usize)")
            && !translation
                .rust
                .contains("pub fn get_genotype_samples(&self) -> ()"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "typed_external_string_list_getter",
        vec![GeneratedCrateModule {
            name: "typed_external_string_list_getter",
            rust: translation.rust,
            tests: r#"
#[cfg(test)]
mod generated_tests {
    use super::*;

    #[test]
    fn typechecks_indexed_string_list_getter() {
        let fixture = TypedExternalStringListGetterUse {};
        let _: String = fixture.first_sample(&VCFHeader);
    }
}
"#,
        }],
    );
}

#[test]
fn external_pack_typed_locatable_coordinate_getters_support_numeric_and_string_use() {
    let source = r#"
import htsjdk.samtools.util.Locatable;

class TypedExternalLocatableGetterUse {
    String describe(Locatable interval) {
        return interval.getContig() + ":" + interval.getStart() + "-" + interval.getEnd();
    }

    boolean startsAtOne(Locatable interval) {
        return interval.getStart() == 1;
    }
}
"#;
    let pack_facts = new_java2rust::external_packs::htsjdk_samtools_util_pack_facts();
    let mut facts = FactSet::default();
    for method_name in ["getContig", "getStart", "getEnd"] {
        facts.external_api_effect_facts.push(
            pack_facts
                .external_api_effect_for_receiver(Some("Locatable"), method_name)
                .unwrap_or_else(|| panic!("Locatable.{method_name} should be pack-owned"))
                .clone(),
        );
    }
    let translation = translate_source_with_facts(source, &facts);

    assert!(
        translation
            .rust
            .contains("pub fn get_contig(&self) -> String")
            && translation.rust.contains("pub fn get_start(&self) -> i32")
            && translation.rust.contains("pub fn get_end(&self) -> i32")
            && !translation.rust.contains("pub fn get_contig(&self) -> ()")
            && !translation.rust.contains("pub fn get_start(&self) -> ()")
            && !translation.rust.contains("pub fn get_end(&self) -> ()"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "typed_external_locatable_getters",
        vec![GeneratedCrateModule {
            name: "typed_external_locatable_getters",
            rust: translation.rust,
            tests: r#"
#[cfg(test)]
mod generated_tests {
    use super::*;

    #[test]
    fn typechecks_coordinate_getters() {
        let fixture = TypedExternalLocatableGetterUse {};
        let _: String = fixture.describe(&Locatable);
        let _: bool = fixture.starts_at_one(&Locatable);
    }
}
"#,
        }],
    );
}

#[test]
fn source_summary_super_calls_lower_to_boundary_defaults_not_raw_super_receiver() {
    let caller = r#"
class SummarySuperCaller extends SummaryBase {
    void startup() throws IOException {
        super.startup();
    }

    String inheritedLabel() {
        return super.label();
    }
}
"#;
    let summary = r#"
class SummaryBase {
    void startup() throws IOException {
    }

    String label() {
        return "base";
    }
}

class IOException extends Exception {
}
"#;
    let translation = translate_source_with_context_summary_sources(
        caller,
        &[summary.to_string()],
        &FactSet::default(),
    );

    assert!(!translation.rust.contains("super."), "{}", translation.rust);
    assert!(
        translation.rust.contains("Ok(())?;")
            && translation.rust.contains("return Default::default();"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "source_summary_super_boundary",
        vec![GeneratedCrateModule {
            name: "source_summary_super_boundary",
            rust: translation.rust,
            tests: r#"
#[cfg(test)]
mod generated_tests {
    use super::*;

    #[test]
    fn typechecks_source_summary_super_boundary() {
        let fixture = SummarySuperCaller {};
        let _: String = fixture.inherited_label();
    }
}
"#,
        }],
    );
}

#[test]
fn summary_context_translation_resolves_sibling_symbols_without_lowering_bodies() {
    let caller = r#"
class SummaryContextCaller {
    Holder holder;

    String apply(String name, int value) {
        return holder.add(Constants.NAME, value);
    }
}
"#;
    let summary_holder = r#"
class Holder {
    String add(String name, int value) {
        return name + value;
    }

    String add(String name, int value, int extra) {
        return name + value + extra;
    }
}
"#;
    let summary_constants = r#"
final class Constants {
    static final String NAME = "summary";
}
"#;
    let translation = translate_source_with_context_summary_sources(
        caller,
        &[summary_holder.to_string(), summary_constants.to_string()],
        &FactSet::default(),
    );

    assert!(
        translation.rust.contains(
            "self.holder.as_ref().unwrap().add_string_int(Constants::NAME.to_string(), value)"
        ) && translation.rust.contains("impl Constants {")
            && translation
                .rust
                .contains("pub const NAME: &'static str = \"summary\";")
            && !translation.rust.contains("pub fn add_string_int("),
        "{}",
        translation.rust
    );
}

#[test]
fn collection_field_mutator_method_takes_mut_self() {
    let translation = translate_source(&idiom_gap("CollectionFieldMutator.java"));

    assert!(
        translation
            .rust
            .contains("pub fn put_value(&mut self, key: String, value: String)")
            && translation
                .rust
                .contains("self.values.as_mut().unwrap().insert(key, value);"),
        "{}",
        translation.rust
    );
}

#[test]
fn documents_method_reference_constructor_gap() {
    let translation = translate_source(&idiom_gap("MethodReferenceConstructor.java"));

    assert!(
        translation
            .rust
            .contains("DataLine::new(1, IllegalArgumentException::new)")
            && !translation
                .rust
                .contains("unsupported Java expression: method reference"),
        "{}",
        translation.rust
    );
}

#[test]
fn documents_method_reference_flat_map_gap() {
    let translation = translate_source(&idiom_gap("MethodReferenceFlatMap.java"));

    assert!(
        translation
            .rust
            .contains("pub fn flatten(&self, groups: Vec<Vec<String>>) -> std::collections::HashSet<String>")
            && translation
                .rust
                .contains("groups.iter().flat_map(|item| item.iter()).collect::<std::collections::HashSet<_>>().into_iter().cloned().collect::<std::collections::HashSet<_>>()")
            && !translation
                .rust
                .contains("unsupported Java expression: method reference")
            && !translation.rust.contains(".collect(collectors.to_set())")
            && !translation.rust.contains("downcast::<Set>()"),
        "{}",
        translation.rust
    );
}

#[test]
fn unresolved_set_return_in_add_all_uses_target_slot_shape() {
    let translation = translate_source(&idiom_gap("UnresolvedSetReturnAddAll.java"));

    assert!(
        translation
            .rust
            .contains("self.names.as_mut().unwrap().extend(default_names())")
            && !translation.rust.contains("downcast::<Set>()")
            && !translation.rust.contains("expect(\"invalid Java cast\")")
            && !translation.rust.contains(".add_all("),
        "{}",
        translation.rust
    );
}

#[test]
fn method_reference_flat_map_generated_rust_preserves_behavior() {
    let translation = translate_source(&idiom_gap("MethodReferenceFlatMap.java"));
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
    use std::collections::HashSet;

    #[test]
    fn flat_map_collects_unique_strings() {
        let streams = MethodReferenceFlatMap {};
        let groups = vec![
            vec!["a".to_string(), "b".to_string()],
            vec!["b".to_string(), "c".to_string()],
        ];
        let expected = vec!["a", "b", "c"]
            .into_iter()
            .map(|v| v.to_string())
            .collect::<HashSet<_>>();
        assert_eq!(streams.flatten(&groups), expected);
    }
}
"#
    );
    let rust_path = Path::new("/tmp").join("new_java2rust_method_reference_flat_map_test.rs");
    let bin_path = Path::new("/tmp").join("new_java2rust_method_reference_flat_map_test");
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
        "method-reference flatMap test did not compile\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let output = Command::new(&bin_path).output().unwrap();
    assert!(
        output.status.success(),
        "method-reference flatMap behavior test failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn documents_stream_callback_chain_gap() {
    let translation = translate_source(&idiom_gap("StreamCallbackChain.java"));

    assert!(
        translation
            .rust
            .contains("read_hits.map(|value| value._2())")
            && translation.rust.contains("Score::add")
            && !translation
                .rust
                .contains("unsupported Java expression: method reference")
            && translation
                .rust
                .contains("map_partitions_to_pair(|iter| compute_scores")
            && !translation
                .rust
                .contains("unsupported Java expression: lambda"),
        "{}",
        translation.rust
    );
}

#[test]
fn documents_class_literal_logger_gap() {
    let translation = translate_source(&idiom_gap("ClassLiteralLogger.java"));

    assert!(
        translation.rust.contains("logger: Logger") && !translation.rust.contains("LogManager"),
        "{}",
        translation.rust
    );
}

#[test]
fn documents_class_literal_registration_gap() {
    let translation = translate_source(&idiom_gap("ClassLiteralRegistration.java"));

    assert!(
        translation.rust.contains("pub struct JavaClass<T>")
            && translation
                .rust
                .contains("JavaClass::<RegisteredType>::literal(\"RegisteredType\")")
            && !translation
                .rust
                .contains("unsupported Java expression: class literal"),
        "{}",
        translation.rust
    );
}

#[test]
fn documents_instanceof_guard_narrowing_gap() {
    let translation = translate_source(&idiom_gap("InstanceofGuardNarrowsCast.java"));

    assert!(
        translation
            .rust
            .contains("unsupported Java instanceof shape")
            || translation.rust.contains("downcast::"),
        "{}",
        translation.rust
    );
}

#[test]
fn documents_super_constructor_state_gap() {
    let translation = translate_source(&idiom_gap("SuperConstructorState.java"));

    assert!(
        translation
            .rust
            .contains("super(...) requires inheritance lowering"),
        "{}",
        translation.rust
    );
}

#[test]
fn translates_external_exception_super_constructor_without_placeholder() {
    let translation = translate_source(&idiom_gap("ExceptionSubclassSuperConstructor.java"));

    assert!(
        translation
            .rust
            .contains("JavaException::new(\"WorkspaceProblem\"")
            && translation.rust.contains("pub fn new(message: String)")
            && !translation
                .rust
                .contains("super(...) requires inheritance lowering"),
        "{}",
        translation.rust
    );
}

#[test]
fn documents_constructor_delegation_with_extra_work_gap() {
    let translation = translate_source(&idiom_gap("ConstructorDelegationWithExtraWork.java"));

    assert!(
        translation
            .rust
            .contains("let mut __java_self = Self::new_int(1);")
            && translation.rust.contains("__java_self.audit();")
            && translation.rust.contains("return __java_self;")
            && !translation
                .rust
                .contains("this(...) followed by additional statements"),
        "{}",
        translation.rust
    );
}

#[test]
fn documents_unresolved_constructor_delegation_gap() {
    let translation = translate_source(&idiom_gap("UnresolvedConstructorDelegation.java"));

    assert!(
        translation
            .rust
            .contains("unresolved Java constructor delegation"),
        "{}",
        translation.rust
    );
}

#[test]
fn documents_enum_name_value_of_java_api_gap() {
    let translation = translate_source(&idiom_gap("EnumNameAndValueOf.java"));

    assert!(
        translation.rust.contains("mode.value_of(") || translation.rust.contains(".name()"),
        "{}",
        translation.rust
    );
}

#[test]
fn documents_optional_get_is_present_java_api_gap() {
    let translation = translate_source(&idiom_gap("OptionalGetIsPresent.java"));

    assert!(
        translation.rust.contains(".is_present()") && translation.rust.contains(".get()"),
        "{}",
        translation.rust
    );
}

#[test]
fn documents_try_with_resources_copy_gap() {
    let translation = translate_source(&idiom_gap("TryWithResourcesCopy.java"));

    assert!(
        translation.rust.contains("pub fn copy")
            && translation.rust.contains("new_input_stream")
            && translation.rust.contains("output.write(buffer, 0, n);")
            && translation.rust.contains("output.close()?;")
            && translation.rust.contains("input.close()?;")
            && !translation
                .rust
                .contains("unsupported Java statement: try/catch/finally"),
        "{}",
        translation.rust
    );
}

#[test]
fn translates_return_bearing_try_with_resources_without_placeholder() {
    let translation = translate_source(&idiom_gap("TryWithResourcesReturn.java"));

    assert!(
        translation.rust.contains("pub fn read")
            && translation.rust.contains("let __java_twr_result =")
            && translation.rust.contains("reader.close()?;")
            && translation.rust.contains("return __java_twr_result;")
            && !translation
                .rust
                .contains("unsupported Java statement: try/catch/finally"),
        "{}",
        translation.rust
    );
}

#[test]
fn documents_switch_state_machine_gap() {
    let translation = translate_source(&idiom_gap("SwitchReturnsEnumMatch.java"));

    assert!(
        translation.rust.contains("match type_ {"),
        "{}",
        translation.rust
    );
    assert!(
        translation
            .rust
            .contains("#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub enum Type")
            && translation.rust.contains("Type::SNP =>")
            && translation.rust.contains("Type::INDEL =>")
            && translation.rust.contains("Type::SYMBOLIC =>"),
        "{}",
        translation.rust
    );
    assert!(
        !translation.rust.contains("__java_switch_matched"),
        "{}",
        translation.rust
    );
}

#[test]
fn switch_returns_enum_match_generated_rust_compiles() {
    let translation = translate_source(&idiom_gap("SwitchReturnsEnumMatch.java"));
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
    fn enum_switch_labels_match() {
        let fixture = SwitchReturnsEnumMatch {};
        assert_eq!(fixture.label(Type::SNP), "snp".to_string());
        assert_eq!(fixture.label(Type::INDEL), "indel".to_string());
    }
}
"#
    );
    let rust_path = Path::new("/tmp").join("new_java2rust_switch_returns_enum_match_test.rs");
    let bin_path = Path::new("/tmp").join("new_java2rust_switch_returns_enum_match_test");
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
        "enum switch match test did not compile\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let output = Command::new(&bin_path).output().unwrap();
    assert!(
        output.status.success(),
        "enum switch match behavior test failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn simple_return_switches_lower_to_match() {
    let translation = translate_source(&idiom_gap("SimpleReturnSwitches.java"));

    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    assert!(
        translation.rust.contains("match value {")
            && translation.rust.contains("2 | 3 =>")
            && translation.rust.contains("match name.as_str() {")
            && translation.rust.contains("\"alpha\" =>")
            && !translation.rust.contains("__java_switch_matched"),
        "{}",
        translation.rust
    );
}

#[test]
fn documents_diamond_single_unknown_gap() {
    let translation = translate_source(&idiom_gap("DiamondSingleUnknown.java"));

    assert!(
        translation
            .rust
            .contains("let mut out: Vec<String> = Vec::new();")
            && translation
                .rust
                .contains("out.push(\"alpha\".to_string());")
            && !translation.rust.contains("ArrayList::new()")
            && !translation.rust.contains(".add("),
        "{}",
        translation.rust
    );
}

#[test]
fn documents_diamond_nested_unknowns_gap() {
    let translation = translate_source(&idiom_gap("DiamondNestedUnknowns.java"));

    assert!(
        translation
            .rust
            .contains("-> std::collections::HashMap<String, Vec<i32>>")
            && translation
                .rust
                .contains("let mut out: std::collections::HashMap<String, Vec<i32>> = std::collections::HashMap::new();")
            && translation.rust.contains("out.insert(\"values\".to_string(), values);")
            && !translation.rust.contains("let out: Map")
            && !translation.rust.contains(".put(")
            && !translation.rust.contains("ArrayList::new(values)"),
        "{}",
        translation.rust
    );
}

#[test]
fn final_sorted_map_receiver_mutation_declares_rust_local_mutable() {
    let translation = translate_source(&idiom_gap("FinalSortedMapReceiverMutation.java"));

    assert!(
        translation
            .rust
            .contains("let mut out: std::collections::HashMap<String, i32> = std::collections::HashMap::new();")
            && translation.rust.contains("out.insert(key, value);")
            && !translation.rust.contains("let out: std::collections::HashMap"),
        "{}",
        translation.rust
    );
}

#[test]
fn try_catch_reused_moved_argument_clones_before_try_body_move() {
    let translation = translate_source(&idiom_gap("TryCatchReusesMovedArgument.java"));

    assert!(
        translation.rust.contains(
            "Files::read_all_lines(path.clone()/* TODO(translation): validate added clone */)"
        ) && translation.rust.contains("path.to_string()"),
        "{}",
        translation.rust
    );
}

#[test]
fn documents_class_literal_token_gap() {
    let translation = translate_source(&idiom_gap("ClassLiteralToken.java"));

    assert!(
        translation.rust.contains("pub struct JavaClass<T>")
            && translation
                .rust
                .contains("JavaClass::<String>::literal(\"String\")")
            && !translation
                .rust
                .contains("unsupported Java expression: class literal"),
        "{}",
        translation.rust
    );
}

#[test]
fn documents_equals_and_objects_gap() {
    let translation = translate_source(&idiom_gap("EqualsAndObjects.java"));

    assert!(
        translation.rust.contains("return (left == right);")
            && !translation.rust.contains(".equals(")
            && !translation.rust.contains("objects.equals("),
        "{}",
        translation.rust
    );
}

#[test]
fn nullable_equals_comparisons_use_option_reference_policy() {
    let translation = translate_source(&idiom_gap("NullableEqualsComparison.java"));

    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    assert!(
        translation
            .rust
            .contains("return (left.as_ref() == right.as_ref());")
            && translation
                .rust
                .contains("return Some(&left) == right.as_ref();")
            && !translation.rust.contains(".equals(")
            && !translation.rust.contains("objects.equals("),
        "{}",
        translation.rust
    );
}

#[test]
fn nullable_trim_equals_original_uses_optional_string_view_policy() {
    let translation = translate_source(&idiom_gap("NullableTrimEqualsOriginal.java"));

    assert!(
        translation
            .rust
            .contains("Some(sample_name.as_ref().unwrap().trim()) == sample_name.as_deref()")
            && !translation.rust.contains("Option<&&str>")
            && !translation
                .rust
                .contains("Some(&sample_name.as_ref().unwrap().trim()) == sample_name.as_ref()"),
        "{}",
        translation.rust
    );
}

#[test]
fn custom_equals_method_call_stays_a_method_call() {
    let translation = translate_source(&idiom_gap("CustomEqualsMethod.java"));

    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    assert!(
        translation.rust.contains("return left.equals(right);")
            && !translation.rust.contains("return (left == right);"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "custom_equals_method",
        vec![GeneratedCrateModule {
            name: "custom_equals_method",
            rust: translation.rust,
            tests: r#"
#[cfg(test)]
mod generated_tests {
    use super::*;

    #[test]
    fn custom_equals_method_controls_comparison() {
        let fixture = CustomEqualsMethod {};
        let left = Value::new(7);
        let same = Value::new(7);
        assert!(fixture.same(left, same));
        assert!(!fixture.same(Value::new(7), Value::new(8)));
    }
}
"#,
        }],
    );
}

#[test]
fn nullable_custom_equals_method_uses_null_safe_custom_equals() {
    let translation = translate_source(&idiom_gap("NullableCustomEqualsMethod.java"));

    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    assert!(
        translation.rust.contains("match (left, right)")
            && translation.rust.contains("left.equals(right)")
            && !translation.rust.contains("left.as_ref() == right.as_ref()"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "nullable_custom_equals_method",
        vec![GeneratedCrateModule {
            name: "nullable_custom_equals_method",
            rust: translation.rust,
            tests: r#"
#[cfg(test)]
mod generated_tests {
    use super::*;

    #[test]
    fn nullable_custom_equals_method_controls_comparison() {
        let fixture = NullableCustomEqualsMethod {};
        assert!(fixture.same(None, None));
        assert!(!fixture.same(Some(NullableValue::new(1)), None));
        assert!(!fixture.same(None, Some(NullableValue::new(1))));
        assert!(fixture.same(Some(NullableValue::new(7)), Some(NullableValue::new(7))));
        assert!(!fixture.same(Some(NullableValue::new(7)), Some(NullableValue::new(8))));
    }
}
"#,
        }],
    );
}

#[test]
fn enum_binary_equality_uses_partial_eq_policy() {
    let translation = translate_source(&idiom_gap("EnumEqualityComparison.java"));

    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    assert!(
        translation
            .rust
            .contains("#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub enum Kind")
            && translation.rust.contains("return (left == right);")
            && translation.rust.contains("return (left != right);")
            && !translation.rust.contains(".equals("),
        "{}",
        translation.rust
    );
}

#[test]
fn documents_serializable_value_object_gap() {
    let translation = translate_source(&idiom_gap("SerializableValueObject.java"));

    assert!(
        translation.rust.contains("serial_version_u_i_d")
            || translation.rust.contains(".write_object(")
            || translation.rust.contains(".read_object("),
        "{}",
        translation.rust
    );
}

#[test]
fn documents_comparator_interface_gap() {
    let translation = translate_source(&idiom_gap("ComparatorInterface.java"));

    assert!(
        translation.rust.contains("collections.sort(")
            || translation.rust.contains("integer.compare(")
            || translation.rust.contains(".compare_to("),
        "{}",
        translation.rust
    );
}

#[test]
fn documents_core_functional_interfaces_gap() {
    let translation = translate_source(&idiom_gap("CoreFunctionalInterfaces.java"));

    assert!(
        translation.rust.contains(".test(")
            || translation.rust.contains(".apply(")
            || translation.rust.contains(".get()"),
        "{}",
        translation.rust
    );
}

#[test]
fn documents_mutual_recursion_gap() {
    let translation = translate_source(&idiom_gap("MutualRecursion.java"));

    assert!(
        translation.rust.contains("self.odd(") && translation.rust.contains("self.even("),
        "{}",
        translation.rust
    );
}

#[test]
fn documents_tail_recursive_accumulator_gap() {
    let translation = translate_source(&idiom_gap("TailRecursiveAccumulator.java"));

    assert!(
        translation.rust.contains("loop {")
            && translation
                .rust
                .contains("let __java_tail_arg_0 = (n - 1);")
            && translation
                .rust
                .contains("let __java_tail_arg_1 = (acc * n as i64);")
            && !translation
                .diagnostics
                .has_code("control_flow_rewrite_pending"),
        "{}",
        translation.rust
    );
}

#[test]
fn documents_while_assignment_not_null_gap() {
    let translation = translate_source(&idiom_gap("WhileAssignmentNotNull.java"));

    assert!(
        translation.rust.contains("loop {")
            && translation.rust.contains("cursor.next()")
            && translation.rust.contains("let mut row: Option<Row>;")
            && translation.rust.contains("row = cursor.next();")
            && translation.rust.contains("if row.is_none()")
            && translation
                .rust
                .contains("let row = row.as_ref().unwrap();")
            && !translation.rust.contains("(row = cursor.next())")
            && !translation.rust.contains("control_flow_rewrite_pending"),
        "{}",
        translation.rust
    );
    assert!(
        !translation
            .diagnostics
            .has_code("control_flow_rewrite_pending"),
        "{:?}",
        translation.diagnostics
    );
}

#[test]
fn documents_while_assignment_null_variants_gap() {
    let translation = translate_source(&idiom_gap("WhileAssignmentNullVariants.java"));

    assert!(
        translation.rust.contains("cursor.read()")
            && translation.rust.contains("cursor.advance()")
            && translation.rust.contains("row = cursor.read();")
            && translation.rust.contains("if row.is_none()")
            && translation.rust.contains("if row.is_some()")
            && translation
                .rust
                .contains("pub fn first_missing(&self, cursor: &mut Cursor) -> Row")
            && translation.rust.contains("return row.unwrap();")
            && !translation.rust.contains("(row = cursor.read())")
            && !translation.rust.contains("control_flow_rewrite_pending"),
        "{}",
        translation.rust
    );
    assert!(
        !translation
            .diagnostics
            .has_code("control_flow_rewrite_pending"),
        "{:?}",
        translation.diagnostics
    );
}

#[test]
fn documents_external_getter_setter_mutability_gap() {
    let translation = translate_source(&idiom_gap("ExternalGetterSetterMutability.java"));

    assert!(
        translation.rust.contains("counter.get_value()")
            && translation.rust.contains("counter.set_value(")
            && translation.rust.contains("counter: &mut ExternalCounter"),
        "{}",
        translation.rust
    );
}

#[test]
fn documents_external_fluent_mutation_gap() {
    let translation = translate_source(&idiom_gap("ExternalFluentMutation.java"));

    assert!(
        translation.rust.contains("request.set_name(")
            && translation.rust.contains(".set_limit(")
            && translation.rust.contains(".enable_retries("),
        "{}",
        translation.rust
    );
}

#[test]
fn documents_external_indexed_accessors_gap() {
    let translation = translate_source(&idiom_gap("ExternalIndexedAccessors.java"));

    assert!(
        translation.rust.contains("table.get(row, column)")
            && translation.rust.contains("table.set(row, column, value)")
            && translation.rust.contains("table: &mut ExternalTable"),
        "{}",
        translation.rust
    );
}

#[test]
fn documents_exception_subclass_catch_order_gap() {
    let translation = translate_source(&idiom_gap("ExceptionSubclassCatchOrder.java"));

    assert!(
        translation
            .rust
            .contains("match (|| -> Result<i32, JavaException>")
            && translation
                .rust
                .contains("Err(ex) if ex.is_instance(&[\"SpecificProblem\"])")
            && translation
                .rust
                .contains("Err(ex) if ex.is_instance(&[\"BaseProblem\", \"SpecificProblem\"])")
            && !translation
                .rust
                .contains("unsupported Java statement: try/catch/finally")
            && translation.rust.contains("BaseProblem")
            && translation.rust.contains("SpecificProblem"),
        "{}",
        translation.rust
    );
}

#[test]
fn documents_exception_base_catch_covers_subclass_gap() {
    let translation = translate_source(&idiom_gap("ExceptionBaseCatchCoversSubclass.java"));

    assert!(
        translation.rust.contains(
            "Err(ex) if ex.is_instance(&[\"MissingFileProblem\", \"RecoverableProblem\"])"
        ) && !translation
            .rust
            .contains("unsupported Java statement: try/catch/finally")
            && translation.rust.contains("RecoverableProblem")
            && translation.rust.contains("MissingFileProblem"),
        "{}",
        translation.rust
    );
}

#[test]
fn documents_exception_cause_chaining_gap() {
    let translation = translate_source(&idiom_gap("ExceptionCauseChaining.java"));

    assert!(
        translation
            .rust
            .contains("JavaException::with_cause(\"UserProblem\"")
            && translation.rust.contains("Result<(), JavaException>")
            && translation.rust.contains("UserProblem"),
        "{}",
        translation.rust
    );
}

#[test]
fn documents_exception_rethrow_specific_gap() {
    let translation = translate_source(&idiom_gap("ExceptionRethrowSpecific.java"));

    assert!(
        translation.rust.contains("return Err(ex);")
            && translation.rust.contains("Result<(), JavaException>")
            && translation.rust.contains("ValidationProblem"),
        "{}",
        translation.rust
    );
}

#[test]
fn external_throwing_call_in_try_uses_external_effect_facts() {
    let mut facts = FactSet::default();
    facts.external_api_effect_facts.push(ExternalApiEffectFact {
        method_name: "validate".to_string(),
        receiver_type: Some("Validator".to_string()),
        arg_count: Some(0),
        role: ExternalApiRole::SideEffect,
        receiver_mutability: ReceiverMutabilityFact::Unknown,
        return_type: Some(JavaType::Void),
        return_nullability: NullabilityState::DefinitelyNonNull,
        thrown: vec![JavaType::Unknown("ValidationProblem".to_string())],
        provenance: FactProvenance::UserOverride,
    });
    let translation =
        translate_source_with_facts(&idiom_gap("ExceptionRethrowSpecific.java"), &facts);

    assert!(
        translation.rust.contains("validator.validate()?;")
            && translation.rust.contains("return Err(ex);")
            && !translation
                .rust
                .contains("unsupported Java statement: try/catch/finally"),
        "{}",
        translation.rust
    );
}

#[test]
fn external_field_throwing_call_in_try_uses_external_field_facts() {
    let mut facts = FactSet::default();
    facts.external_field_facts.push(ExternalFieldFact {
        field: FieldKey {
            type_name: "Registry".to_string(),
            field_name: "DEFAULT_VALIDATOR".to_string(),
        },
        field_type: Some(JavaType::Named {
            name: "Validator".to_string(),
            args: Vec::new(),
        }),
        field_kind: ExternalFieldKind::StaticFinal,
        provenance: FactProvenance::UserOverride,
    });
    facts.external_api_effect_facts.push(ExternalApiEffectFact {
        method_name: "validate".to_string(),
        receiver_type: Some("Validator".to_string()),
        arg_count: Some(0),
        role: ExternalApiRole::SideEffect,
        receiver_mutability: ReceiverMutabilityFact::Unknown,
        return_type: Some(JavaType::Void),
        return_nullability: NullabilityState::DefinitelyNonNull,
        thrown: vec![JavaType::Unknown("ValidationProblem".to_string())],
        provenance: FactProvenance::UserOverride,
    });
    let translation =
        translate_source_with_facts(&idiom_gap("ExternalFieldThrowingCallInTry.java"), &facts);

    assert!(
        translation
            .rust
            .contains("Registry::DEFAULT_VALIDATOR.validate()?;")
            && translation.rust.contains("return Err(ex);")
            && !translation
                .rust
                .contains("unsupported Java statement: try/catch/finally"),
        "{}",
        translation.rust
    );
}

#[test]
fn external_nullable_getter_assignment_uses_option_carrier_recovery() {
    let mut facts = FactSet::default();
    facts.external_api_effect_facts.push(ExternalApiEffectFact {
        method_name: "getName".to_string(),
        receiver_type: Some("ExternalRecord".to_string()),
        arg_count: Some(0),
        role: ExternalApiRole::Getter,
        receiver_mutability: ReceiverMutabilityFact::ReadOnly,
        return_type: Some(JavaType::String),
        return_nullability: NullabilityState::MaybeNull,
        thrown: Vec::new(),
        provenance: FactProvenance::UserOverride,
    });
    let translation = translate_source_with_facts(
        r#"
class ExternalNullableGetterUse {
    String read(ExternalRecord record) {
        String name = record.getName();
        return name == null ? "missing" : name;
    }
}
"#,
        &facts,
    );

    assert!(
        translation
            .rust
            .contains("let name: Option<String> = record.get_name();")
            && translation.rust.contains("if name.is_none()")
            && translation.rust.contains("else { name }")
            && !translation.rust.contains("Some(record.get_name())"),
        "{}",
        translation.rust
    );
}

#[test]
fn documents_exception_runtime_checked_hierarchy_gap() {
    let translation = translate_source(&idiom_gap("ExceptionRuntimeCheckedHierarchy.java"));

    assert!(
        translation
            .rust
            .contains("Err(ex) if ex.is_instance(&[\"UncheckedProblem\"])")
            && translation
                .rust
                .contains("Err(ex) if ex.is_instance(&[\"CheckedProblem\"])")
            && !translation
                .rust
                .contains("unsupported Java statement: try/catch/finally")
            && translation.rust.contains("CheckedProblem")
            && translation.rust.contains("UncheckedProblem"),
        "{}",
        translation.rust
    );
}

#[test]
fn translates_cause_runtime_rethrow_without_nominal_downcast() {
    let translation = translate_source(&idiom_gap("CauseRuntimeRethrow.java"));

    assert!(
        translation
            .rust
            .contains("ce.cause_or_self_if_instance(&[\"RuntimeException\"])")
            && !translation.rust.contains("downcast::<RuntimeException>()")
            && !translation
                .rust
                .contains("unsupported Java statement: try/catch/finally"),
        "{}",
        translation.rust
    );
}

#[test]
fn simple_rethrow_catch_generated_rust_compiles() {
    let translation = translate_source(&idiom_gap("SimpleRethrowCatch.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    assert!(
        translation.rust.contains("return Ok(());")
            && translation.rust.contains("return Err(ex);")
            && !translation
                .rust
                .contains("unsupported Java statement: try/catch/finally"),
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
    fn rethrows_caught_exception_value() {
        let fixture = SimpleRethrowCatch {};
        assert_eq!(fixture.validate(false), Ok(()));
        let err = fixture.validate(true).unwrap_err();
        assert_eq!(err.class_name, "IllegalArgumentException");
        assert_eq!(err.message, "bad");
        assert!(err.cause.is_none());
    }
}
"#
    );
    let rust_path = Path::new("/tmp").join("new_java2rust_simple_rethrow_catch_test.rs");
    let bin_path = Path::new("/tmp").join("new_java2rust_simple_rethrow_catch_test");
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
        "simple rethrow catch test did not compile\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let output = Command::new(&bin_path).output().unwrap();
    assert!(
        output.status.success(),
        "simple rethrow catch behavior test failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn simple_cause_chaining_generated_rust_compiles() {
    let translation = translate_source(&idiom_gap("SimpleCauseChaining.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    assert!(
        translation
            .rust
            .contains("JavaException::with_cause(\"UserProblem\"")
            && !translation
                .rust
                .contains("unsupported Java statement: try/catch/finally"),
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
    fn preserves_chained_exception_cause() {
        let fixture = SimpleCauseChaining {};
        assert_eq!(fixture.copy(false), Ok(()));
        let err = fixture.copy(true).unwrap_err();
        assert_eq!(err.class_name, "UserProblem");
        assert_eq!(err.message, "copy failed");
        let cause = err.cause.as_ref().unwrap();
        assert_eq!(cause.class_name, "IOException");
        assert_eq!(cause.message, "io");
    }
}
"#
    );
    let rust_path = Path::new("/tmp").join("new_java2rust_simple_cause_chaining_test.rs");
    let bin_path = Path::new("/tmp").join("new_java2rust_simple_cause_chaining_test");
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
        "simple cause chaining test did not compile\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let output = Command::new(&bin_path).output().unwrap();
    assert!(
        output.status.success(),
        "simple cause chaining behavior test failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn internal_throwing_call_in_try_generated_rust_compiles() {
    let translation = translate_source(&idiom_gap("InternalThrowingCallInTry.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    assert!(
        translation.rust.contains("self.read(bad)?;")
            && translation
                .rust
                .contains("JavaException::with_cause(\"UserProblem\"")
            && !translation
                .rust
                .contains("unsupported Java statement: try/catch/finally"),
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
    fn catches_result_returning_internal_call() {
        let fixture = InternalThrowingCallInTry {};
        assert_eq!(fixture.copy(false), Ok(()));
        let err = fixture.copy(true).unwrap_err();
        assert_eq!(err.class_name, "UserProblem");
        let cause = err.cause.as_ref().unwrap();
        assert_eq!(cause.class_name, "IOException");
        assert_eq!(cause.message, "io");
    }
}
"#
    );
    let rust_path = Path::new("/tmp").join("new_java2rust_internal_throwing_call_in_try_test.rs");
    let bin_path = Path::new("/tmp").join("new_java2rust_internal_throwing_call_in_try_test");
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
        "internal throwing call in try test did not compile\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let output = Command::new(&bin_path).output().unwrap();
    assert!(
        output.status.success(),
        "internal throwing call in try behavior test failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn simple_try_finally_cleanup_generated_rust_compiles() {
    let translation = translate_source(&idiom_gap("SimpleTryFinallyCleanup.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}\n{}",
        translation.diagnostics,
        translation.rust
    );
    assert!(
        translation
            .rust
            .contains("let __java_try_result = (|| -> Result<(), JavaException>")
            && translation.rust.contains("resource.read()?;")
            && translation.rust.contains("resource.close();")
            && translation.rust.contains("__java_try_result?;")
            && !translation
                .rust
                .contains("unsupported Java statement: try/catch/finally"),
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
    fn try_finally_cleanup_typechecks() {
        let fixture = SimpleTryFinallyCleanup {};
        let resource = Resource {};
        assert_eq!(fixture.copy(resource), Ok(()));
    }
}
"#
    );
    let rust_path = Path::new("/tmp").join("new_java2rust_simple_try_finally_cleanup_test.rs");
    let bin_path = Path::new("/tmp").join("new_java2rust_simple_try_finally_cleanup_test");
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
        "simple try/finally cleanup test did not compile\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let output = Command::new(&bin_path).output().unwrap();
    assert!(
        output.status.success(),
        "simple try/finally cleanup behavior test failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn simple_try_catch_return_generated_rust_compiles() {
    let translation = translate_source(&idiom_gap("SimpleTryCatchReturn.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    assert!(
        translation
            .rust
            .contains("Err(ex) if ex.is_instance(&[\"IllegalArgumentException\"])")
            && !translation
                .rust
                .contains("unsupported Java statement: try/catch/finally"),
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
    fn catches_thrown_value() {
        let fixture = SimpleTryCatchReturn {};
        assert_eq!(fixture.parse(false).unwrap(), 7);
        assert_eq!(fixture.parse(true).unwrap(), 1);
    }
}
"#
    );
    let rust_path = Path::new("/tmp").join("new_java2rust_simple_try_catch_return_test.rs");
    let bin_path = Path::new("/tmp").join("new_java2rust_simple_try_catch_return_test");
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
        "simple try/catch return test did not compile\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let output = Command::new(&bin_path).output().unwrap();
    assert!(
        output.status.success(),
        "simple try/catch return behavior test failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn source_static_final_literal_fields_emit_associated_constants_not_storage() {
    let translation = translate_source(
        r#"
public final class Constants {
    public static final String NAME = "alpha";
    public static final int LIMIT = 7;
    private String value = "stored";
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("pub const NAME: &'static str = \"alpha\";")
            && translation.rust.contains("pub const LIMIT: i32 = 7;")
            && translation.rust.contains("value: String")
            && !translation.rust.contains("NAME: Option<String>")
            && !translation.rust.contains("LIMIT: i32,"),
        "{}",
        translation.rust
    );
}

#[test]
fn source_static_final_fields_are_referenced_as_associated_constants() {
    let translation = translate_source(
        r#"
public class UsesOwnConstants {
    public static final String FLAG = "flag";
    private static final int LIMIT = 7;

    public String flag() {
        return FLAG;
    }

    public static int limit() {
        return LIMIT;
    }
}
"#,
    );

    assert!(
        translation.rust.contains("return Self::FLAG.to_string();"),
        "{}",
        translation.rust
    );
    assert!(
        translation.rust.contains("return Self::LIMIT;"),
        "{}",
        translation.rust
    );
    assert!(!translation.rust.contains("return f_l_a_g;"));
    assert!(!translation.rust.contains("return l_i_m_i_t;"));
}

#[test]
fn context_translation_emits_sibling_static_final_constant_definition() {
    let target = r#"
public class UsesConstants {
    public String name() {
        return Constants.NAME;
    }
}
"#;
    let constants = r#"
public final class Constants {
    public static final String NAME = "alpha";
}
"#;

    let translation = translate_source_with_context_sources(
        target,
        &[constants.to_string()],
        &FactSet::default(),
    );

    assert!(
        translation
            .rust
            .contains("return Constants::NAME.to_string();"),
        "{}",
        translation.rust
    );
    assert!(!translation.rust.contains("downcast::<String>()"));
}

#[test]
fn static_string_constant_argument_to_owned_string_slot_uses_to_string_adapter() {
    let target = r#"
public class UsesConstants {
    public String name() {
        return accept(Constants.NAME);
    }

    private String accept(String value) {
        return value;
    }
}
"#;
    let constants = r#"
public final class Constants {
    public static final String NAME = "alpha";
}
"#;

    let translation = translate_source_with_context_sources(
        target,
        &[constants.to_string()],
        &FactSet::default(),
    );

    assert!(
        translation
            .rust
            .contains("self.accept(Constants::NAME.to_string())"),
        "{}",
        translation.rust
    );
    assert!(!translation.rust.contains("Constants::NAME.unwrap()"));
    assert!(!translation.rust.contains("downcast::<String>()"));
}

#[test]
fn unresolved_external_setter_static_string_argument_uses_owned_adapter() {
    let target = r#"
public class UsesExternalBuilder {
    public void configure(ExternalBuilder builder) {
        builder.setArrayName(Constants.NAME);
    }
}
"#;
    let constants = r#"
public final class Constants {
    public static final String NAME = "alpha";
}
"#;

    let translation = translate_source_with_context_sources(
        target,
        &[constants.to_string()],
        &FactSet::default(),
    );

    assert!(
        translation
            .rust
            .contains("builder.set_array_name(Constants::NAME.to_string());"),
        "{}",
        translation.rust
    );
    assert!(
        !translation
            .rust
            .contains("builder.set_array_name(Constants::NAME);")
    );
    assert!(!translation.rust.contains("downcast::<String>()"));
}

#[test]
fn inherited_source_summary_instance_fields_are_flattened_into_primary_struct() {
    let target = r#"
public class ChildCounter extends BaseCounter {
    private int localCount;

    public ChildCounter() {
        this.localCount = 3;
    }

    public int total() {
        return inheritedCount + localCount;
    }
}
"#;
    let summary = r#"
public class BaseCounter {
    protected int inheritedCount = 7;
}
"#;

    let translation = translate_source_with_context_summary_sources(
        target,
        &[summary.to_string()],
        &FactSet::default(),
    );

    assert!(
        translation.rust.contains("inherited_count: i32"),
        "{}",
        translation.rust
    );
    assert!(
        translation.rust.contains("inherited_count: 7"),
        "{}",
        translation.rust
    );
    assert!(
        translation
            .rust
            .contains("return (self.inherited_count + self.local_count);"),
        "{}",
        translation.rust
    );
}

#[test]
fn inherited_source_summary_field_types_emit_opaque_support_shells() {
    let target = r#"
public class ChildWithToolState extends BaseWithToolState {
    public void touch() {
        progressMeter.record();
    }
}
"#;
    let summary = r#"
public class BaseWithToolState {
    protected ProgressMeter progressMeter = new ProgressMeter();
}

public class ProgressMeter {
    public void record() {
    }
}
"#;

    let translation = translate_source_with_context_summary_sources(
        target,
        &[summary.to_string()],
        &FactSet::default(),
    );

    assert!(
        translation.rust.contains("progress_meter: ProgressMeter"),
        "{}",
        translation.rust
    );
    assert!(
        translation.rust.contains("pub struct ProgressMeter;"),
        "{}",
        translation.rust
    );
    assert!(
        translation.rust.contains("self.progress_meter.record();"),
        "{}",
        translation.rust
    );
}

#[test]
fn two_dimensional_numeric_arrays_lower_to_ndarray_array2() {
    let translation = translate_source(
        r#"
class NdarrayMatrixLowering {
    double[][] literal() {
        return new double[][] {{1.0, 2.0}, {3.0, 4.0}};
    }

    double[][] create(int rows, int cols) {
        double[][] values = new double[rows][cols];
        values[0][0] = 1.0;
        return values;
    }

    double read(double[][] values, int row, int col) {
        return values[row][col];
    }

    void write(double[][] values, int row, int col, double value) {
        values[row][col] = value;
    }

    int rows(double[][] values) {
        return values.length;
    }

    int cols(double[][] values, int row) {
        return values[row].length;
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("pub fn literal(&self) -> ndarray::Array2<f64>"),
        "{}",
        translation.rust
    );
    assert!(
        translation
            .rust
            .contains("return ndarray::arr2(&[[1f64, 2f64], [3f64, 4f64]]);"),
        "{}",
        translation.rust
    );
    assert!(
        translation.rust.contains(
            "let mut values: ndarray::Array2<f64> = ndarray::Array2::from_elem((rows as usize, cols as usize), 0.0);"
        ),
        "{}",
        translation.rust
    );
    assert!(
        translation
            .rust
            .contains("return values[[row as usize, col as usize]];"),
        "{}",
        translation.rust
    );
    assert!(
        translation
            .rust
            .contains("values: &mut ndarray::Array2<f64>"),
        "{}",
        translation.rust
    );
    assert!(
        translation
            .rust
            .contains("values[[row as usize, col as usize]] = value;"),
        "{}",
        translation.rust
    );
    assert!(
        translation.rust.contains("return values.nrows() as i32;"),
        "{}",
        translation.rust
    );
    assert!(
        translation
            .rust
            .contains("return values.row(row as usize).to_vec().len() as i32;"),
        "{}",
        translation.rust
    );
    assert!(
        !translation.rust.contains("Vec<Vec<f64>>"),
        "{}",
        translation.rust
    );
    assert!(
        !translation
            .rust
            .contains("Java array creation: Primitive(Double)"),
        "{}",
        translation.rust
    );
}

#[test]
fn two_dimensional_row_replacement_lowers_to_ndarray_row_mut_assign() {
    let translation = translate_source(
        r#"
class NdarrayRowReplacement {
    void replace(double[][] values, int row, double[] replacement) {
        values[row] = replacement;
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("values: &mut ndarray::Array2<f64>"),
        "{}",
        translation.rust
    );
    assert!(
        translation.rust.contains(
            "values.row_mut(row as usize).assign(&ndarray::ArrayView1::from(replacement));"
        ),
        "{}",
        translation.rust
    );
}
