use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use new_java2rust::facts::{
    ExternalApiArgumentEffectFact, ExternalApiEffectFact, ExternalApiRole,
    ExternalArgumentAliasPolicy, ExternalFfiHandoverFact, ExternalFfiHandoverSlot,
    ExternalFieldFact, ExternalFieldKind, FactProvenance, FactSet, FieldKey, NullabilityState,
    OwnedRecoveryCapability, OwnedRecoveryCapabilityFact, ReceiverMutabilityFact,
    TypeCapabilityOverride,
};
use new_java2rust::java_ir::{JavaType, PrimitiveType};
use new_java2rust::{
    external_packs::parse_external_pack_manifest, translate_source,
    translate_source_with_context_sources,
    translate_source_with_context_sources_and_summary_sources,
    translate_source_with_context_summary_sources, translate_source_with_facts,
};

fn idiom_gap(path: &str) -> String {
    fs::read_to_string(Path::new("examples/idiom-gaps").join(path)).unwrap()
}

#[test]
fn corpus_rerun_plan_tracks_every_real_corpus_as_backlog_or_regression_guard() {
    let corpora = fs::read_to_string("docs/corpora.tsv").unwrap();
    let plan = fs::read_to_string("docs/corpus-rerun-idiom-gap-plan.md").unwrap();
    let mut missing = Vec::new();

    for line in corpora.lines().skip(1) {
        let columns: Vec<&str> = line.split('\t').collect();
        if columns.len() < 2 {
            continue;
        }
        let name = columns[0];
        let kind = columns[1];
        if matches!(kind, "snippet" | "synthetic") {
            continue;
        }
        if !plan.contains(&format!("`{name}`")) {
            missing.push(name.to_string());
        }
    }

    assert!(
        missing.is_empty(),
        "real corpuses missing from docs/corpus-rerun-idiom-gap-plan.md: {missing:?}"
    );
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
        "{:?}\n{}",
        translation.diagnostics.iter().collect::<Vec<_>>(),
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
            .contains("left.to_lowercase() == right.to_lowercase()"),
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

    cargo_check_generated_modules(
        "appendable_fluent_chain",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn java_string_substring_and_hash_code_use_runtime_helpers() {
    let translation = translate_source(
        r#"
class JavaStringSubstringAndHashCode {
    String trimHeader(String header) {
        return header.substring(1, header.length());
    }

    int hash(String left, String right) {
        int result = 1;
        result = 31 * result + ((left == null) ? 0 : left.hashCode());
        result = 31 * result + ((right == null) ? 0 : right.hashCode());
        return result;
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("java_string_substring(header.as_str(), 1, header.len() as i32)")
            && translation
                .rust
                .contains("java_string_hash_code(left.as_ref().unwrap().as_str())")
            && translation
                .rust
                .contains("java_string_hash_code(right.as_ref().unwrap().as_str())")
            && !translation.rust.contains(".substring(")
            && !translation.rust.contains(".hash_code("),
        "{}",
        translation.rust
    );
}

#[test]
fn bare_final_instance_field_read_does_not_become_static_constant() {
    let translation = translate_source(
        r#"
class BareFinalInstanceFieldRead {
    private final String qualityHeader;

    BareFinalInstanceFieldRead(String qualityHeader) {
        if (qualityHeader != null && !qualityHeader.isEmpty()) {
            this.qualityHeader = qualityHeader;
        } else {
            this.qualityHeader = null;
        }
    }

    String getQualityHeader() {
        return qualityHeader;
    }

    int hash() {
        return qualityHeader == null ? 0 : qualityHeader.hashCode();
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("pub fn get_quality_header(&self) -> Option<String>")
            && translation
                .rust
                .contains("return self.quality_header.clone()")
            && translation
                .rust
                .contains("java_string_hash_code(self.quality_header.as_ref().unwrap().as_str())")
            && !translation.rust.contains("Self::quality_header"),
        "{}",
        translation.rust
    );
}

#[test]
fn same_class_nullable_string_field_hash_code_method_uses_runtime_helper() {
    let translation = translate_source(
        r#"
class SameClassNullableStringFieldHashCodeMethod {
    private final String qualityHeader;
    private final String bases;

    SameClassNullableStringFieldHashCodeMethod(String qualityHeader, String bases) {
        this.qualityHeader = qualityHeader;
        this.bases = bases;
    }

    int hashCode() {
        int result = 1;
        result = 31 * result + (qualityHeader == null ? 0 : qualityHeader.hashCode());
        result = 31 * result + (bases == null ? 0 : bases.hashCode());
        return result;
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("java_string_hash_code(self.quality_header.as_ref().unwrap().as_str())")
            && translation
                .rust
                .contains("java_string_hash_code(self.bases.as_str())")
            && !translation.rust.contains(".quality_header.hash_code()")
            && !translation.rust.contains(".bases.hash_code()"),
        "{}",
        translation.rust
    );
}

#[test]
fn external_receiver_generic_getter_projects_receiver_type_argument() {
    let translation = translate_source(
        r#"
import org.apache.spark.api.java.JavaSparkContext;
import org.apache.spark.broadcast.Broadcast;

class BroadcastGenericValue {
    void run(JavaSparkContext ctx, SAMRecord read, SAMFileHeader header) {
        Broadcast<SAMFileHeader> headerBroadcast = ctx.broadcast(header);
        read.setHeaderStrict(headerBroadcast.getValue());
    }
}

class SAMRecord {
    void setHeaderStrict(SAMFileHeader header) {}
}

class SAMFileHeader {}
"#,
    );

    assert!(
        translation
            .rust
            .contains("read.set_header_strict(header_broadcast.get_value())"),
        "{}",
        translation.rust
    );
    assert!(
        !translation.rust.contains("downcast::<"),
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
        translation.rust.contains("return (end as i64) < limit;"),
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
            "self.patch(values.clone()/* TODO(translation): validate added clone */, key)"
        ),
        "{}",
        translation.rust
    );
}

#[test]
fn casted_borrowed_map_argument_clones_for_owned_helper_slot() {
    let translation = translate_source(
        r#"
import java.util.Map;
import java.util.SortedMap;

class CastedBorrowedMapArgument {
    boolean parallel;

    Map<String, Reader> create(Map<String, URI> paths, int batch) {
        return parallel ? parallel((SortedMap<String, URI>) paths, batch)
                : serial(paths, batch);
    }

    SortedMap<String, Reader> parallel(SortedMap<String, URI> paths, int batch) {
        return null;
    }

    SortedMap<String, Reader> serial(Map<String, URI> paths, int batch) {
        return null;
    }
}
"#,
    );

    assert!(
        translation.rust.contains(
            "self.parallel(paths.clone()/* TODO(translation): validate added clone */, batch)"
        ) && translation.rust.contains("self.serial(paths, batch)"),
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
fn source_value_struct_derives_clone_and_partial_eq_when_fields_support_it() {
    let translation = translate_source(
        r#"
class SourceValueStructDerives {
    private String name;

    SourceValueStructDerives(String name) {
        this.name = name;
    }

    SourceValueStructDerives reuse(SourceValueStructDerives value) {
        return choose(value, value);
    }

    SourceValueStructDerives choose(SourceValueStructDerives left, SourceValueStructDerives right) {
        return left;
    }

    boolean same(SourceValueStructDerives left, SourceValueStructDerives right) {
        return left == right;
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("#[derive(Clone, PartialEq)]\npub struct SourceValueStructDerives")
            && translation.rust.contains("value.clone()")
            && translation.rust.contains("return left == right;"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "source_value_struct_derives",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn concrete_self_cast_flows_through_current_type_slot_without_downcast() {
    let translation = translate_source(
        r#"
class ConcreteSelfCast {
    private String name;

    ConcreteSelfCast(String name) {
        this.name = name;
    }

    String render() {
        return encode((ConcreteSelfCast)this);
    }

    static String encode(ConcreteSelfCast value) {
        return "ok";
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("#[derive(Clone, PartialEq)]\npub struct ConcreteSelfCast")
            && translation.rust.contains(
                "Self::encode(self.clone()/* TODO(translation): validate added clone */)"
            )
            && !translation
                .rust
                .contains("self.downcast::<ConcreteSelfCast>")
            && !translation.rust.contains("invalid Java cast"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "concrete_self_cast",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn nullable_string_format_and_concat_use_java_null_display_policy() {
    let translation = translate_source(&idiom_gap("NullableStringFormatAndConcat.java"));

    assert!(
        translation
            .rust
            .contains("pub fn describe(&self, value: Option<&String>) -> String")
            && translation.rust.contains(
                "value.as_ref().map(|value| value.to_string()).unwrap_or_else(|| \"null\".to_string())"
            )
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
        let value = "x".to_string();
        assert_eq!(fixture.describe(None), "value=null".to_string());
        assert_eq!(fixture.describe(Some(&value)), "value=x".to_string());
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
                .contains("java_string_split(line.as_str(), \"\\\\\\\\t\".to_string(), -1)")
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
                .contains("let sample: String = (parts[0 as usize]).clone()")
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
fn indexed_string_array_into_optional_borrowed_string_arg_borrows_element() {
    let translation = translate_source(
        r#"
class IndexedStringArrayOptionalBorrowedStringArg {
    boolean legal(String value) {
        return value != null && value.trim().equals(value);
    }

    boolean check(String line) {
        String[] parts = line.split("\\t", -1);
        return legal(parts[0]);
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("self.legal(Some(&parts[0 as usize]))")
            && !translation
                .rust
                .contains("self.legal(Some(parts[0 as usize]))"),
        "{}",
        translation.rust
    );
}

#[test]
fn stream_block_lambda_string_array_local_clones_owned_index() {
    let translation = translate_source(
        r#"
import java.util.List;
import java.util.stream.Collectors;

class StreamBlockLambdaStringArrayLocal {
    List<SimpleInterval> intervals(List<String> partitions) {
        return partitions.stream().map(partition -> {
            String[] partitionInfo = partition.split(":");
            if (partitionInfo.length != 3) {
                return null;
            }
            String contig = partitionInfo[0];
            int start = Integer.parseInt(partitionInfo[1]);
            int end = Integer.parseInt(partitionInfo[2]);
            return new SimpleInterval(contig, start, end);
        }).filter(interval -> interval != null).collect(Collectors.toList());
    }
}

class SimpleInterval {
    SimpleInterval(String contig, int start, int end) {
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("let contig = partition_info[0 as usize].clone();")
            && !translation
                .rust
                .contains("let contig = partition_info[0 as usize];"),
        "{}",
        translation.rust
    );
}

#[test]
fn parallel_stream_uses_shared_stream_lowering() {
    let translation = translate_source(
        r#"
import java.util.List;
import java.util.stream.Collectors;

class ParallelStreamSharedLowering {
    List<String> lower(List<String> names) {
        return names.parallelStream().map(name -> name.toLowerCase()).collect(Collectors.toList());
    }
}
"#,
    );

    assert!(
        translation.rust.contains("names.iter().map")
            && translation.rust.contains("to_lowercase()")
            && !translation.rust.contains("parallel_stream"),
        "{}",
        translation.rust
    );
}

#[test]
fn parallel_stream_reduce_uses_shared_stream_lowering() {
    let translation = translate_source(
        r#"
import java.util.List;

class ParallelStreamReduceSharedLowering {
    Rdd empty;

    Rdd one(String name, List<Interval> intervals) {
        return new Rdd();
    }

    Rdd many(List<String> names, List<Interval> intervals) {
        return names.parallelStream().map(name -> one(name, intervals)).reduce(empty, (result, rdd) -> result.union(rdd));
    }
}

class Rdd {
    Rdd union(Rdd other) {
        return this;
    }
}

class Interval {
}
"#,
    );

    assert!(
        translation.rust.contains("names.iter().map")
            && translation.rust.contains(".fold(self.empty")
            && translation.rust.contains("|result, rdd| result.union(rdd)")
            && translation.rust.contains("self.one(name")
            && translation.rust.contains("intervals.clone()")
            && !translation.rust.contains("parallel_stream")
            && !translation.rust.contains("map(|name| one("),
        "{}",
        translation.rust
    );
}

#[test]
fn optional_sequence_get_unwraps_receiver_for_concrete_slot() {
    let translation = translate_source(
        r#"
import java.util.List;

class OptionalSequenceGetConcreteSlot {
    private List<String> names;

    String first() {
        return names.get(0);
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("(self.names.as_ref().expect(\"Java null receiver\").get(0 as usize).expect(\"Java index out of bounds\")).clone()"),
        "{}",
        translation.rust
    );
}

#[test]
fn assignment_condition_loop_body_sees_shadowed_local_as_non_null() {
    let translation = translate_source(
        r#"
class AssignmentConditionLoopShadowReceiver {
    ReferenceSequence next() {
        return new ReferenceSequence();
    }

    void readAll() {
        ReferenceSequence seq;
        while ((seq = next()) != null) {
            String name = seq.getName();
            byte[] bases = seq.getBases();
        }
    }
}

class ReferenceSequence {
    String getName() {
        return "chr1";
    }

    byte[] getBases() {
        return new byte[0];
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("let seq = seq.as_ref().unwrap();")
            && translation
                .rust
                .contains("let name: String = seq.get_name();")
            && translation
                .rust
                .contains("let bases: Vec<i8> = seq.get_bases();")
            && !translation
                .rust
                .contains("seq.as_ref().unwrap().get_name()")
            && !translation
                .rust
                .contains("seq.as_ref().unwrap().get_bases()"),
        "{}",
        translation.rust
    );
}

#[test]
fn sequence_get_adapts_simple_interval_to_locatable_slot() {
    let translation = translate_source(
        r#"
import java.util.List;
import htsjdk.samtools.util.Locatable;
import org.broadinstitute.hellbender.utils.SimpleInterval;

class SequenceGetSimpleIntervalAsLocatable {
    private List<SimpleInterval> intervals;

    Locatable first() {
        return intervals.get(0);
    }
}
"#,
    );

    assert!(
        translation.rust.contains("Locatable::new_1(self.intervals.as_ref().expect(\"Java null receiver\").get(0 as usize).expect(\"Java index out of bounds\").clone())"),
        "{}",
        translation.rust
    );
}

#[test]
fn nullable_path_overload_argument_uses_option_param_and_non_optional_resolve_sibling() {
    let translation = translate_source(&idiom_gap("NullablePathOverloadArgument.java"));

    assert!(
        translation
            .rust
            .contains("pub fn check_path_path(&self, value: Path, optional_index: Option<&Path>)")
            && (translation
                .rust
                .contains("self.check_path_path(value.clone()")
                || translation
                    .rust
                    .contains("self.check_path_path(value, None)"))
            && translation.rust.contains(
                "optional_index.map(|value| value.clone()/* TODO(translation): validate added clone */)"
            )
            && translation
                .rust
                .contains("Some(value.resolve_sibling(\"index\".to_string()))")
            && !translation.rust.contains("Some(None)"),
        "{}",
        translation.rust
    );
}

#[test]
fn source_summary_nullable_string_field_hash_code_uses_runtime_helper() {
    let caller = r#"
class SourceSummaryNullableStringFieldHash {
    int hash(FastqRecord record) {
        return record.qualityHeader == null ? 0 : record.qualityHeader.hashCode();
    }
}
"#;
    let summary = r#"
class FastqRecord {
    String qualityHeader;
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
            .contains("java_string_hash_code(record.quality_header.as_ref().unwrap().as_str())")
            && !translation.rust.contains(".quality_header.hash_code()")
            && !translation.rust.contains("downcast::<String>")
            && !translation.rust.contains("invalid Java cast"),
        "{}",
        translation.rust
    );
}

#[test]
fn typed_map_get_redundant_cast_uses_generic_value_shape() {
    let translation = translate_source(
        r#"
import java.util.HashMap;

class TypedMapGetRedundantCast {
    private HashMap<String, Format> formats = new HashMap<String, Format>();

    Format getFormat(String id) {
        return (Format) formats.get(id);
    }
}

class Format {}
"#,
    );

    assert!(
        translation
            .rust
            .contains("self.formats.get(id).expect(\"Java null receiver\").clone()")
            && !translation.rust.contains("downcast::<Format>")
            && !translation.rust.contains("invalid Java cast"),
        "{}",
        translation.rust
    );
}

#[test]
fn casted_factory_result_into_concrete_optional_slot_uses_slot_shape() {
    let translation = translate_source(
        r#"
import java.net.JarURLConnection;
import java.net.URL;

class CastedFactoryResultIntoConcreteOptionalSlot {
    JarURLConnection open(URL url) throws Exception {
        JarURLConnection connection = null;
        try {
            connection = (JarURLConnection) url.openConnection();
        } catch (Exception e) {
            throw e;
        }
        return connection;
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("connection = Some(url.open_connection());")
            && !translation.rust.contains("downcast::<JarURLConnection>")
            && !translation.rust.contains("invalid Java cast"),
        "{}",
        translation.rust
    );
}

#[test]
fn static_owned_path_argument_does_not_borrow_owned_temporary() {
    let translation = translate_source(
        r#"
class StaticOwnedPathArgument {
    Path run(String vcfPath, String indexPath) throws Exception {
        return Helper.checkPath(IO.getPath(vcfPath), indexPath == null ? null : IO.getPath(indexPath));
    }
}

class Helper {
    static Path checkPath(Path path, Path optionalIndexPath) throws Exception {
        if (optionalIndexPath == null) {
            return path.resolveSibling("idx");
        }
        IO.consume(optionalIndexPath);
        return optionalIndexPath;
    }
}

class IO {
    static Path getPath(String value) {
        return new Path();
    }

    static void consume(Path value) {
    }
}

class Path {
    Path resolveSibling(String value) {
        return new Path();
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("pub fn check_path(path: Path, optional_index_path: Option<Path>)")
            && translation.rust.contains("Helper::check_path(")
            && translation.rust.contains("IO::get_path(vcf_path")
            && translation.rust.contains("Some(IO::get_path(")
            && !translation.rust.contains("&IO::get_path")
            && !translation.rust.contains("Some(&IO::get_path"),
        "{}",
        translation.rust
    );
}

#[test]
fn optional_borrowed_header_push_clones_element_not_reference() {
    let translation = translate_source(
        r#"
import java.util.ArrayList;
import java.util.List;
import htsjdk.variant.vcf.VCFHeader;

class OptionalBorrowedHeaderPush {
    VCFHeader getHeader() throws Exception {
        return null;
    }

    void run() throws Exception {
        List<VCFHeader> headers = new ArrayList<>();
        VCFHeader header = getHeader();
        headers.add(header);
        header.toString();
    }
}
"#,
    );

    assert!(
        translation.rust.contains("headers.push(")
            && (!translation
                .rust
                .contains("let mut header: Option<&VCFHeader>")
                || !translation
                    .rust
                    .contains("headers.push(header.as_ref().unwrap().clone()")),
        "{}",
        translation.rust
    );
}

#[test]
fn emitted_context_static_call_uses_solved_param_shape() {
    let caller = r#"
class EmittedContextStaticOwnedParamCaller {
    void run(boolean enabled, String raw) {
        Path path = raw == null ? null : IO.getPath(raw);
        if (enabled) {
            EmittedContextStaticOwnedParamHelper.check(path);
        }
    }
}

class IO {
    static Path getPath(String raw) {
        return new Path();
    }
}
"#;
    let helper = r#"
class EmittedContextStaticOwnedParamHelper {
    static void check(Path path) {
        check(path, null);
    }

    static void check(Path path, Path index) {
        path.toString();
    }
}

class Path {
}
"#;
    let translation =
        translate_source_with_context_sources(caller, &[helper.to_string()], &FactSet::default());

    assert!(
        translation.rust.contains(
            "EmittedContextStaticOwnedParamHelper::check_path(&path.as_ref().expect(\"Java null receiver\"))"
        ) && !translation.rust.contains(
            "EmittedContextStaticOwnedParamHelper::check_path(path.as_ref().unwrap().clone()"
        ),
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
fn solved_local_plan_inherits_nullable_source_call_initializer() {
    let translation = translate_source(
        r#"
class NullableSourceCallLocal {
    Path run(String sample) {
        Path path = lookup(sample);
        return path;
    }

    Path lookup(String sample) {
        return sample == null ? null : new Path();
    }
}

class Path {
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("let path: Option<Path> = self.lookup(sample);")
            && translation.rust.contains("return path;")
            && !translation
                .rust
                .contains("let mut path: Path = self.lookup"),
        "{}",
        translation.rust
    );
}

#[test]
fn nullable_source_call_through_field_receiver_stays_optional_without_double_wrap() {
    let translation = translate_source(
        r#"
class NullableFieldReceiverSourceCallLocal {
    Lookup lookup;

    Path run(String sample) {
        Path path = lookup.find(sample);
        return path;
    }
}

class Lookup {
    Path find(String sample) {
        return sample == null ? null : new Path();
    }
}

class Path {
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("let path: Option<Path> = self.lookup.as_ref().unwrap().find(sample);")
            && !translation
                .rust
                .contains("let path: Option<Path> = Some(self.lookup.as_ref().unwrap().find"),
        "{}",
        translation.rust
    );
}

#[test]
fn source_summary_nullable_return_into_optional_local_does_not_double_wrap() {
    let caller = r#"
class UsesNullableSourceSummaryReturn {
    Lookup lookup;

    Path run(String sample) {
        Path path = lookup.find(sample);
        return path;
    }
}
"#;
    let summary = r#"
class Lookup {
    Path find(String sample) {
        return sample == null ? null : new Path();
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
        translation
            .rust
            .contains("let path: Option<Path> = self.lookup.as_ref().unwrap().find(sample);")
            && !translation
                .rust
                .contains("let path: Option<Path> = Some(self.lookup.as_ref().unwrap().find"),
        "{}",
        translation.rust
    );
}

#[test]
fn source_summary_nullable_return_into_concrete_local_unwraps_once() {
    let caller = r#"
class UsesNullableSourceSummaryConcreteLocal {
    Lookup lookup;

    File run(String sample) {
        Path path = lookup.find(sample);
        return path.toFile();
    }
}
"#;
    let summary = r#"
class Lookup {
    Path find(String sample) {
        return sample == null ? null : new Path();
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
        translation
            .rust
            .contains("let path: Option<Path> = self.lookup.as_ref().unwrap().find(sample);")
            && translation
                .rust
                .contains("return path.as_ref().unwrap().to_file();")
            && !translation
                .rust
                .contains("Some(self.lookup.as_ref().unwrap().find"),
        "{}",
        translation.rust
    );
}

#[test]
fn source_static_call_uses_signature_shapes_for_nullable_args() {
    let translation = translate_source(
        r#"
class SourceStaticNullableArgCaller {
    String run(String workspace) {
        return SourceStaticNullableArgUtil.append(workspace, "vid.json");
    }
}

class SourceStaticNullableArgUtil {
    static String append(String parent, String path) {
        return parent == null ? path : parent + path;
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("SourceStaticNullableArgUtil::append(Some(")
            && !translation.rust.contains("append(workspace,"),
        "{}",
        translation.rust
    );
}

#[test]
fn overload_null_argument_makes_exact_callee_parameter_optional() {
    let translation = translate_source(
        r#"
class OverloadNullArgumentCalleeShape {
    static void write(String output) {
        write(output, null, true);
    }

    static void write(String output, String optionalParts, boolean enabled) {
        if (optionalParts != null) {
            optionalParts.trim();
        }
    }
}
"#,
    );

    assert!(
        translation.rust.contains("optional_parts: Option<")
            && translation
                .rust
                .contains("Self::write_string_string_boolean")
            && translation.rust.contains("None")
            && !translation.rust.contains("optional_parts: String"),
        "{}",
        translation.rust
    );
}

#[test]
fn source_static_throwing_call_uses_question_mark() {
    let translation = translate_source(
        r#"
import java.io.IOException;

class SourceStaticThrowingCall {
    String run() throws IOException {
        String value = SourceStaticThrowingUtil.read();
        return value;
    }
}

class SourceStaticThrowingUtil {
    static String read() throws IOException {
        return "x";
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("let value: String = SourceStaticThrowingUtil::read()?;")
            && !translation
                .rust
                .contains("let value: String = SourceStaticThrowingUtil::read();"),
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
fn sam_record_fastq_string_getters_emit_typed_facade_surfaces() {
    let translation = translate_source(
        r#"
import htsjdk.samtools.SAMRecord;

class SamRecordFastqStringGetters {
    String read(SAMRecord record) {
        return record.getReadString();
    }

    String qualities(SAMRecord record) {
        return record.getBaseQualityString();
    }
}
"#,
    );

    assert!(
        translation
            .diagnostics
            .iter()
            .all(|diagnostic| diagnostic.code != "external_pack_missing_api"),
        "{:?}\n{}",
        translation.diagnostics.iter().collect::<Vec<_>>(),
        translation.rust
    );
    assert!(
        translation.rust.contains("pub fn get_read_string(&self)")
            && translation
                .rust
                .contains("pub fn get_base_quality_string(&self)")
            && translation
                .rust
                .contains("-> String { Default::default() }")
            && translation
                .rust
                .contains("return record.get_read_string();")
            && translation
                .rust
                .contains("return record.get_base_quality_string();"),
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
fn object_return_from_nonnullable_external_getter_boxes_concrete_value() {
    let facts = parse_external_pack_manifest(
        r#"
external-pack dep dep.ExternalReader semantic
external-pack dep dep.Header semantic
external-pack-api dep dep.ExternalReader header semantic getter read-only definitely-non-null return=dep.Header args=0
"#,
    )
    .facts;
    let translation = translate_source_with_facts(
        r#"
import dep.ExternalReader;

class ObjectReturnFromNonnullableExternalGetter {
    Object header(ExternalReader reader) {
        return reader.header();
    }
}
"#,
        &facts,
    );

    assert!(
        translation.rust.contains("-> Box<dyn std::any::Any>")
            && translation
                .rust
                .contains("return Box::new(reader.header()) as Box<dyn std::any::Any>;")
            && !translation.rust.contains("downcast::<Header>()")
            && !translation.rust.contains("invalid Java cast"),
        "{}",
        translation.rust
    );
}

#[test]
fn object_return_from_primitive_boxes_autoboxed_value() {
    let translation = translate_source(
        r#"
class ObjectReturnFromPrimitive {
    Object success() {
        return true;
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("pub fn success(&self) -> Box<dyn std::any::Any>")
            && translation
                .rust
                .contains("return Box::new(true) as Box<dyn std::any::Any>;"),
        "{}",
        translation.rust
    );
}

#[test]
fn object_local_from_concrete_external_factory_boxes_concrete_value() {
    let facts = parse_external_pack_manifest(
        r#"
external-pack dep dep.ExternalFactory semantic
external-pack dep dep.Reader semantic
external-pack-api dep dep.ExternalFactory open semantic getter unknown definitely-non-null return=dep.Reader args=0
"#,
    )
    .facts;
    let translation = translate_source_with_facts(
        r#"
import dep.ExternalFactory;

class ObjectLocalFromConcreteExternalFactory {
    Object open() {
        Object reader = ExternalFactory.open();
        return reader;
    }
}
"#,
        &facts,
    );

    assert!(
        translation
            .rust
            .contains("let reader: Box<dyn std::any::Any> = Box::new(")
            && translation
                .rust
                .contains("::open()) as Box<dyn std::any::Any>;")
            && !translation.rust.contains("invalid Java cast"),
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
            .contains("pub fn keep_or_default(&self, value: Option<Box<dyn std::any::Any>>) -> Option<Box<dyn std::any::Any>>")
            && translation
                .rust
                .contains("let current: Option<Box<dyn std::any::Any>> = None::<Box<dyn std::any::Any>>;")
            && translation.rust.contains("return current;")
            && translation.rust.contains("return value;")
            && translation.rust.contains("if value.is_none()")
            && !translation.rust.contains("value == None")
            && !translation
                .rust
                .contains("let current: Box<dyn std::any::Any> = None")
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
            .contains("pub fn increment_or_one(&self, old: Option<&i32>) -> i32")
            && translation
                .rust
                .contains("if old.is_none() { 1 } else { old.unwrap() + 1 }")
            && translation.rust.contains(
                "if missing { None } else { Some(value.clone()/* TODO(translation): validate added clone */) }"
            )
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
            .contains("path.as_ref().unwrap().to_absolute_path().to_uri().to_string()")
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
        let _: String = fixture.label(SwitchReturnsEnumMatchType::SNP);
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
        assert!(fixture.same(&EnumEqualityComparisonKind::SNP, &EnumEqualityComparisonKind::SNP));
        assert!(fixture.different(&EnumEqualityComparisonKind::SNP, &EnumEqualityComparisonKind::INDEL));
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
        let _: i32 = fixture.parse(false);
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
        translation.rust.contains("values.fill(1);"),
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
            .contains("self.paths.insert(sample.clone()/* TODO(translation): validate added clone */, URI::parse(path)?)")
            && translation.rust.contains("format!(\"duplicate {}\", sample)")
            && !translation.rust.contains("self.paths.insert(sample, URI::")
            && !translation.rust.contains(".clone()/* TODO(translation): validate added clone */.clone()"),
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
            "self.seen.insert(name.as_ref().unwrap().clone()/* TODO(translation): validate added clone */, \"seen\".to_string())"
        ) && translation
            .rust
            .contains("self.valid(name.as_ref().map(|value| value.as_str()))")
            && translation.rust.contains("format!(\"bad {}\", name.as_ref().map")
            && !translation.rust.contains("self.valid(name.map")
            && !translation.rust.contains(".clone()/* TODO(translation): validate added clone */.clone()"),
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
            .contains("paths.get(sample).expect(\"Java null receiver\").to_string()")
            && !translation.rust.contains("paths.get(sample).to_string()")
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
            .contains("indexes.get(name).copied().expect(\"Java null receiver\")")
            && !translation.rust.contains("indexes.get(name) } else"),
        "{}",
        translation.rust
    );
}

#[test]
fn constructor_string_argument_reuse_clones_at_owned_boundary() {
    let translation = translate_source(&idiom_gap("ConstructorStringReuse.java"));

    assert!(
        translation.rust.contains(
            "let uri: URI = URI::parse(path.clone()/* TODO(translation): validate added clone */)?;"
        ) && translation
            .rust
            .contains("pub fn choose(&self, path: String) -> Result<URI, JavaException>")
            && translation.rust.contains("path.is_empty()")
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
            && translation
                .rust
                .contains("pub fn parse<P: JavaPathArg>(value: P) -> Result<URI, JavaException>")
            && translation.rust.contains(
                "pub fn parse(&self, path: Option<String>) -> Result<URI, JavaException>"
            )
            && translation.rust.contains("let uri: URI = URI::parse(")
            && translation.rust.contains("?;"),
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
                .contains("(keys.get(i as usize).expect(\"Java index out of bounds\")).clone()")
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
fn math_min_lowers_as_native_static_call() {
    let source = r#"
class MathMinCall {
    int clamp(int end, int limit) {
        return Math.min(end, limit);
    }
}
"#;
    let translation = translate_source(source);

    assert!(
        translation.rust.contains("std::cmp::min(end, limit)")
            && !translation.rust.contains("Math::min"),
        "{}",
        translation.rust
    );
}

#[test]
fn sam_sequence_record_get_sequence_length_has_int_surface() {
    let source = r#"
import htsjdk.samtools.SAMSequenceDictionary;
import htsjdk.samtools.SAMSequenceRecord;

class SamSequenceLengthSurface {
    int end(SAMSequenceDictionary dictionary, String contig) {
        SAMSequenceRecord record = dictionary.getSequence(contig);
        return Math.min(100, record.getSequenceLength());
    }
}
"#;
    let translation = translate_source(source);

    assert!(
        translation
            .rust
            .contains("pub fn get_sequence_length(&self) -> i32")
            && translation
                .rust
                .contains("std::cmp::min(100, record.get_sequence_length())"),
        "{}",
        translation.rust
    );
}

#[test]
fn two_bit_reference_get_reference_bases_returns_reference_sequence() {
    let source = r#"
import htsjdk.samtools.reference.ReferenceSequence;
import org.broadinstitute.hellbender.utils.SimpleInterval;
import org.broadinstitute.hellbender.utils.reference.ReferenceBases;
import org.broadinstitute.hellbender.utils.reference.TwoBitReference;

class TwoBitReferenceBasesSurface {
    ReferenceBases read(TwoBitReference reference, SimpleInterval interval) {
        ReferenceSequence sequence = reference.getReferenceBases(interval);
        return new ReferenceBases(sequence.getBases(), interval);
    }
}
"#;
    let translation = translate_source(source);

    assert!(
        translation
            .rust
            .contains("pub fn get_reference_bases<T0>(&self, _arg0: T0) -> ReferenceSequence")
            && translation.rust.contains(
                "let sequence: ReferenceSequence = reference.get_reference_bases(interval.clone()"
            ),
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
                .contains("pub fn sam_header(&self) -> Option<SAMFileHeader>")
            && translation
                .rust
                .contains("pub fn iterator(&self) -> Option<CloseableIterator>")
            && translation
                .rust
                .contains("return self.reader.as_ref().unwrap().header().sam_header();")
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
        let _: Option<SAMFileHeader> = fixture.read_header();
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
fn selected_opaque_support_type_gets_used_typed_void_surface() {
    let source = r#"
import htsjdk.samtools.reference.ReferenceSequenceFile;

class UsesReferenceCloseSurface {
    ReferenceSequenceFile reference;

    void closeReference() {
        reference.close();
    }
}
"#;
    let translation = translate_source(source);

    assert!(
        translation.rust.contains("pub fn close(&self) -> ()")
            && translation
                .rust
                .contains("self.reference.as_ref().unwrap().close();"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "typed_void_external_surface",
        vec![GeneratedCrateModule {
            name: "typed_void_external_surface",
            rust: translation.rust,
            tests: r#"
#[cfg(test)]
mod generated_tests {
    use super::*;

    #[test]
    fn typechecks_typed_void_surface() {
        let fixture = UsesReferenceCloseSurface { reference: Some(ReferenceSequenceFile) };
        fixture.close_reference();
    }
}
"#,
        }],
    );
}

#[test]
fn try_with_resources_external_type_gets_implicit_close_surface() {
    let source = r#"
import htsjdk.samtools.reference.ReferenceSequenceFile;
import htsjdk.samtools.reference.ReferenceSequenceFileFactory;
import htsjdk.samtools.SAMSequenceDictionary;
import java.nio.file.Path;

class UsesReferenceTryWithResourcesCloseSurface {
    Path path;

    SAMSequenceDictionary dictionary() {
        try (ReferenceSequenceFile reference = ReferenceSequenceFileFactory.getReferenceSequenceFile(path)) {
            return reference.getSequenceDictionary();
        }
    }
}
"#;
    let translation = translate_source(source);

    assert!(
        translation.rust.contains("pub fn close(&self) -> ()")
            && translation.rust.contains("reference.close();"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "try_with_resources_external_close_surface",
        vec![GeneratedCrateModule {
            name: "try_with_resources_external_close_surface",
            rust: translation.rust,
            tests: r#"
#[cfg(test)]
mod generated_tests {
    use super::*;

    #[test]
    fn typechecks_try_with_resources_close_surface() {
        let fixture = UsesReferenceTryWithResourcesCloseSurface { path: Some(Path::new("ref.fa")) };
        let _: Option<SAMSequenceDictionary> = fixture.dictionary();
    }
}
"#,
        }],
    );
}

#[test]
fn nullable_external_fluent_argument_wraps_owned_optional_branch() {
    let source = r#"
import org.broadinstitute.hellbender.engine.GATKPath;
import org.disq_bio.disq.HtsjdkReadsRddStorage;

class NullableDisqFluentArgument {
    void configure(HtsjdkReadsRddStorage storage, GATKPath reference, boolean missing) {
        storage.referenceSourcePath(missing ? null : reference.getRawInputString());
    }
}
"#;
    let translation = translate_source(source);

    assert!(
        translation
            .rust
            .contains("pub fn reference_source_path(&self, _arg0: Option<String>) -> Self")
            && translation.rust.contains(
                "storage.reference_source_path(if missing { None } else { Some(reference.get_raw_input_string()) })"
            )
            && !translation.rust.contains("Some(&reference.get_raw_input_string())"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "nullable_external_fluent_argument",
        vec![GeneratedCrateModule {
            name: "nullable_external_fluent_argument",
            rust: translation.rust,
            tests: r#"
#[cfg(test)]
mod generated_tests {
    use super::*;

    #[test]
    fn typechecks_nullable_fluent_argument() {
        let fixture = NullableDisqFluentArgument {};
        let mut storage = HtsjdkReadsRddStorage;
        let reference = GATKPath;
        fixture.configure(&mut storage, &reference, false);
    }
}
"#,
        }],
    );
}

#[test]
fn nullable_sam_reader_reference_sequence_argument_wraps_path_branch() {
    let source = r#"
import htsjdk.samtools.SamReaderFactory;
import java.nio.file.Path;

class NullableSamReaderReferenceSequenceArgument {
    void configure(SamReaderFactory factory, Path reference, boolean missing) {
        factory.referenceSequence(missing ? null : reference);
    }
}
"#;
    let translation = translate_source(source);

    assert!(
        translation
            .rust
            .contains("pub fn reference_sequence(&self, _arg0: Option<Path>) -> Self")
            && translation.rust.contains(
                "factory.reference_sequence(if missing { None } else { Some(reference.clone()/* TODO(translation): validate added clone */) })"
            )
            && !translation.rust.contains("Some(&reference"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "nullable_sam_reader_reference_sequence",
        vec![GeneratedCrateModule {
            name: "nullable_sam_reader_reference_sequence",
            rust: translation.rust,
            tests: r#"
#[cfg(test)]
mod generated_tests {
    use super::*;

    #[test]
    fn typechecks_nullable_reference_sequence_argument() {
        let fixture = NullableSamReaderReferenceSequenceArgument {};
        let mut factory = SamReaderFactory;
        let reference = Path::new("ref.fa");
        fixture.configure(&mut factory, &reference, false);
    }
}
"#,
        }],
    );
}

#[test]
fn optional_field_assignment_wraps_external_call_result() {
    let source = r#"
import org.broadinstitute.hellbender.engine.GATKPath;
import java.nio.file.Path;

class OptionalFieldAssignmentWrapsExternalCall {
    Path referencePath;

    Path getReferencePath(String raw) {
        if (referencePath == null) {
            referencePath = new GATKPath(raw).toPath();
        }
        return referencePath;
    }
}
"#;
    let translation = translate_source(source);

    assert!(
        translation
            .rust
            .contains("self.reference_path = Some(GATKPath::new(raw).to_path());"),
        "{}",
        translation.rust
    );
}

#[test]
fn optional_this_field_assignment_wraps_external_call_result() {
    let source = r#"
import org.broadinstitute.hellbender.engine.GATKPath;
import java.nio.file.Path;

class OptionalThisFieldAssignmentWrapsExternalCall {
    Path referencePath;

    Path getReferencePath(String raw) {
        if (this.referencePath == null) {
            this.referencePath = new GATKPath(raw).toPath();
        }
        return this.referencePath;
    }
}
"#;
    let translation = translate_source(source);

    assert!(
        translation
            .rust
            .contains("self.reference_path = Some(GATKPath::new(raw).to_path());"),
        "{}",
        translation.rust
    );
}

#[test]
fn file_extensions_fasta_lowers_as_iterable_static_slice() {
    let source = r#"
import htsjdk.samtools.util.FileExtensions;

class FileExtensionsFastaLoop {
    boolean matches(String value) {
        for (String ext : FileExtensions.FASTA) {
            if (value.endsWith(ext)) {
                return true;
            }
        }
        return false;
    }
}
"#;
    let translation = translate_source(source);

    assert!(
        translation
            .rust
            .contains("pub const FASTA: &'static [&'static str] = &[];"),
        "{}",
        translation.rust
    );
    assert!(
        translation
            .rust
            .contains("for ext in FileExtensions::FASTA.iter().copied()"),
        "{}",
        translation.rust
    );
}

#[test]
fn if_null_else_branch_refines_optional_display_segment() {
    let source = r#"
import org.broadinstitute.hellbender.engine.GATKPath;

class IfNullElseDisplayRefinement {
    String describe(GATKPath referencePathSpecifier) {
        if (referencePathSpecifier == null) {
            return "missing";
        } else {
            return "reference " + referencePathSpecifier;
        }
    }
}
"#;
    let translation = translate_source(source);

    assert!(
        translation
            .rust
            .contains("format!(\"reference {}\", (reference_path_specifier.unwrap()).to_string())")
            && !translation
                .rust
                .contains("(reference_path_specifier).to_string()"),
        "{}",
        translation.rust
    );
}

#[test]
fn null_guarded_ternary_display_branch_unwraps_optional_value() {
    let source = r#"
import java.io.File;

class NullGuardedTernaryDisplayBranch {
    private File file;

    String describe() {
        return "File[" + (this.file == null ? "" : this.file) + "]";
    }
}
"#;
    let translation = translate_source(source);

    assert!(
        translation
            .rust
            .contains("(self.file.as_ref().unwrap()).to_string()")
            && !translation.rust.contains("(self.file).to_string()")
            && !translation.rust.contains("self.file.to_string()"),
        "{}",
        translation.rust
    );
}

#[test]
fn null_guarded_ternary_display_branch_matches_unqualified_field_alias() {
    let source = r#"
import java.io.File;

class NullGuardedCamelFieldTernaryDisplayBranch {
    private File fastqFile;

    String describe() {
        return "File[" + (this.fastqFile == null ? "" : fastqFile) + "]";
    }
}
"#;
    let translation = translate_source(source);

    assert!(
        translation
            .rust
            .contains("(self.fastq_file.as_ref().unwrap()).to_string()")
            && !translation.rust.contains("(self.fastq_file).to_string()")
            && !translation.rust.contains("self.fastq_file.to_string()"),
        "{}",
        translation.rust
    );
}

#[test]
fn null_guarded_ternary_display_branch_matches_camel_case_this_field() {
    let source = r#"
import java.io.File;

class NullGuardedCamelThisFieldTernaryDisplayBranch {
    private File fastqFile;

    String describe() {
        return "File[" + (this.fastqFile == null ? "" : this.fastqFile) + "]";
    }
}
"#;
    let translation = translate_source(source);

    assert!(
        translation
            .rust
            .contains("(self.fastq_file.as_ref().unwrap()).to_string()")
            && !translation.rust.contains("(self.fastq_file).to_string()")
            && !translation.rust.contains("self.fastq_file.to_string()"),
        "{}",
        translation.rust
    );
}

#[test]
fn null_guarded_ternary_display_branch_uses_refined_local_once() {
    let source = r#"
class NullGuardedLocalTernaryDisplayBranch {
    String describe(String readName) {
        return "@" + (readName == null ? "" : readName) + "\n";
    }
}
"#;
    let translation = translate_source(source);

    assert!(
        translation
            .rust
            .contains("if read_name.is_none() { \"\".to_string() } else { read_name.unwrap() }")
            && !translation.rust.contains("read_name.unwrap().unwrap()")
            && !translation
                .rust
                .contains("read_name.as_ref().unwrap().unwrap()"),
        "{}",
        translation.rust
    );
}

#[test]
fn null_guarded_ternary_append_argument_uses_refined_local_once() {
    let source = r#"
import java.io.IOException;

class NullGuardedAppendArgument {
    static Appendable write(Appendable out, String readName) throws IOException {
        return out.append(readName == null ? "" : readName);
    }
}
"#;
    let translation = translate_source(source);

    assert!(
        translation
            .rust
            .contains("if read_name.is_none() { \"\".to_string() } else { read_name.unwrap() }")
            && !translation.rust.contains("read_name.unwrap().unwrap()")
            && !translation
                .rust
                .contains("read_name.as_ref().unwrap().unwrap()"),
        "{}",
        translation.rust
    );
}

#[test]
fn reused_optional_display_segment_borrows_before_later_use() {
    let source = r#"
import org.broadinstitute.hellbender.engine.GATKPath;

class ReusedOptionalDisplaySegment {
    GATKPath check(GATKPath referencePathSpecifier) {
        if (referencePathSpecifier == null) {
            return null;
        }
        String message = "reference " + referencePathSpecifier;
        referencePathSpecifier.toPath();
        return referencePathSpecifier;
    }
}
"#;
    let translation = translate_source(source);

    assert!(
        translation.rust.contains(
            "reference_path_specifier.as_ref().map(|value| value.to_string()).unwrap_or_else(|| \"null\".to_string())"
        ) && translation
            .rust
            .contains("reference_path_specifier.as_ref().unwrap().to_path()")
            && !translation
            .rust
            .contains("(reference_path_specifier.unwrap()).to_string()"),
        "{}",
        translation.rust
    );
}

#[test]
fn resolved_static_helper_call_uses_fallback_signature_arg_shapes() {
    let caller = r#"
import org.broadinstitute.hellbender.engine.GATKPath;

class SourceSummaryStaticBorrowedArgCaller {
    static void run(String output, GATKPath reference) {
        SourceSummaryStaticBorrowedArgHelper.check(new GATKPath(output), reference);
    }
}
"#;
    let helper = r#"
import org.broadinstitute.hellbender.engine.GATKPath;

class SourceSummaryStaticBorrowedArgHelper {
    static GATKPath check(final GATKPath filePathSpecifier, final GATKPath referencePathSpecifier) {
        filePathSpecifier.toPath();
        referencePathSpecifier.toPath();
        return referencePathSpecifier;
    }
}
"#;
    let translation =
        translate_source_with_context_sources(caller, &[helper.to_string()], &FactSet::default());

    assert!(
        translation.rust.contains(
            "SourceSummaryStaticBorrowedArgHelper::check(&GATKPath::new(output), &reference);"
        ) && !translation.rust.contains("reference.clone()"),
        "{}",
        translation.rust
    );
}

#[test]
fn constructed_external_argument_is_borrowed_for_borrowed_source_slot() {
    let caller = r#"
import org.broadinstitute.hellbender.engine.GATKPath;

class ConstructedBorrowedSourceSlotCaller {
    static void run(String output, GATKPath reference) {
        ConstructedBorrowedSourceSlotHelper.check(new GATKPath(output), reference);
    }
}
"#;
    let helper = r#"
import org.broadinstitute.hellbender.engine.GATKPath;

class ConstructedBorrowedSourceSlotHelper {
    static void check(final GATKPath outputPath, final GATKPath referencePath) {
        outputPath.toPath();
        referencePath.toPath();
    }
}
"#;
    let translation =
        translate_source_with_context_sources(caller, &[helper.to_string()], &FactSet::default());

    assert!(
        translation.rust.contains(
            "ConstructedBorrowedSourceSlotHelper::check(&GATKPath::new(output), &reference);"
        ),
        "{}",
        translation.rust
    );
}

#[test]
fn nullable_static_alias_return_emits_lifetime_for_borrowed_params() {
    let source = r#"
import org.broadinstitute.hellbender.engine.GATKPath;

class NullableStaticAliasReturn {
    static GATKPath check(final GATKPath filePathSpecifier, final GATKPath referencePathSpecifier) {
        filePathSpecifier.isCram();
        if (referencePathSpecifier == null) {
            return null;
        }
        if (Validator.isAllowed(referencePathSpecifier)) {
            return referencePathSpecifier;
        }
        return null;
    }
}

class Validator {
    static boolean isAllowed(GATKPath path) { return true; }
}
"#;
    let translation = translate_source(source);

    assert!(
        translation
            .rust
            .contains("pub fn check<'a>(file_path_specifier: &'a GATKPath, reference_path_specifier: Option<&'a GATKPath>) -> Option<&'a GATKPath>")
            && !translation.rust.contains("missing lifetime specifier"),
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
                .contains("pub fn get_default_tool_v_c_f_header_lines(&self) -> Option<std::collections::HashSet<VCFHeaderLine>>")
            && translation
                .rust
                .contains("self.get_default_tool_v_c_f_header_lines().unwrap()")
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
            .contains("let mut path: Option<Path> = self.maybe_path(")
            && translation.rust.contains(
                "self.maybe_path(name.clone()/* TODO(translation): validate added clone */)"
            )
            && translation
                .rust
                .contains("self.open_with_index(name, path.as_ref())")
            && !translation
                .rust
                .contains("let mut path: Path = self.maybe_path")
            && !translation.rust.contains("path.clone()")
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
        (translation
            .rust
            .contains("pub fn flatten(&self, groups: Vec<Vec<String>>) -> std::collections::HashSet<String>")
            || translation
                .rust
                .contains("pub fn flatten(&self, groups: &Vec<Vec<String>>) -> std::collections::HashSet<String>"))
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
            && (translation
                .rust
                .contains("map_partitions_to_pair(|iter| compute_scores")
                || translation
                    .rust
                    .contains("map_partitions_to_pair_1(|iter: Iterator| self.compute_scores"))
            || translation.rust.contains(
                "map_partitions_to_pair_1(|iter: std::vec::IntoIter<Hit>| self.compute_scores"
            ) && !translation
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
fn explicit_object_super_constructor_is_noop() {
    let translation = translate_source(
        r#"
class ExplicitObjectSuperConstructor {
    private final int value;

    ExplicitObjectSuperConstructor(int value) {
        super();
        this.value = value;
    }

    int getValue() {
        return value;
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("return ExplicitObjectSuperConstructor { value: value }")
            && !translation
                .rust
                .contains("super(...) requires inheritance lowering")
            && !translation.rust.contains("compile_error!"),
        "{}",
        translation.rust
    );
}

#[test]
fn no_arg_super_constructor_with_inherited_setter_trailing_call_lowers() {
    let subclass = r#"
class ChildFormat extends BaseFormat {
    ChildFormat() {
        super();
        setId("CHILD");
    }
}
"#;
    let superclass = r#"
class BaseFormat {
    private String id = null;

    void setId(String id) {
        this.id = id;
    }
}
"#;

    let translation = translate_source_with_context_sources(
        subclass,
        &[superclass.to_string()],
        &FactSet::default(),
    );

    assert!(
        translation.rust.contains("id: Option<String>")
            && translation
                .rust
                .contains("let mut __java_self = ChildFormat { id: None }")
            && translation
                .rust
                .contains("__java_self.set_id(\"CHILD\".to_string());")
            && translation.rust.contains("return __java_self;")
            && !translation
                .rust
                .contains("super(...) requires inheritance lowering")
            && !translation.rust.contains("compile_error!"),
        "{}",
        translation.rust
    );
}

#[test]
fn no_arg_super_constructor_with_source_summary_inherited_setter_lowers() {
    let subclass = r#"
class ChildSummaryFormat extends BaseSummaryFormat {
    ChildSummaryFormat() {
        super();
        setId("CHILD");
    }
}
"#;
    let superclass = r#"
class BaseSummaryFormat {
    private String id = null;

    void setId(String id) {
        this.id = id;
    }
}
"#;

    let translation = translate_source_with_context_summary_sources(
        subclass,
        &[superclass.to_string()],
        &FactSet::default(),
    );

    assert!(
        translation.rust.contains("id: Option<String>")
            && translation
                .rust
                .contains("let mut __java_self = ChildSummaryFormat { id: None }")
            && translation
                .rust
                .contains("__java_self.set_id(\"CHILD\".to_string());")
            && translation.rust.contains("return __java_self;")
            && !translation
                .rust
                .contains("super(...) requires inheritance lowering")
            && !translation.rust.contains("compile_error!"),
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
fn corpus_picard_imported_constants_need_source_summary_facts() {
    let receiver = new_java2rust::java_ir::Expr::Var("HelpConstants".to_string());
    let type_env = new_java2rust::types::TypeEnv::new();
    assert_eq!(
        new_java2rust::member_access_plan::java_member_access_policy_with_facts(
            &receiver,
            "DOC_CAT_BASE_CALLING",
            &type_env,
            &FactSet::default(),
        )
        .map(|policy| policy.java_ty),
        Some(JavaType::String)
    );

    let translation = translate_source(&idiom_gap("SourceSummaryConstantAcrossPackage.java"));

    assert!(
        translation
            .rust
            .contains("return HelpConstants::DOC_CAT_BASE_CALLING.to_string();")
            && !translation.rust.contains("downcast::<String>()")
            && !translation.rust.contains("invalid Java cast"),
        "{}",
        translation.rust
    );

    let constants = r#"
package picard.util.help;

public final class HelpConstants {
    public static final String DOC_CAT_BASE_CALLING = "Base Calling";
    public static final String DOC_CAT_BASE_CALLING_SUMMARY = "Tools that process sequencing machine data";
}
"#;
    let with_summary = translate_source_with_context_summary_sources(
        &idiom_gap("SourceSummaryConstantAcrossPackage.java"),
        &[constants.to_string()],
        &FactSet::default(),
    );

    assert!(
        with_summary
            .rust
            .contains("return HelpConstants::DOC_CAT_BASE_CALLING.to_string();")
            && !with_summary.rust.contains("downcast::<String>()"),
        "{}",
        with_summary.rust
    );
}

#[test]
fn corpus_genomicsdb_file_load_constructor_delegation_exposes_exception_type_gap() {
    let translation = translate_source(&idiom_gap("FileLoadConstructorDelegation.java"));

    assert!(
        translation
            .rust
            .contains("pub fn new_path(source: &Path) -> FileLoadConstructorDelegation")
            && translation
                .rust
                .contains("let mut __java_self = Self::new();")
            && translation.rust.contains("__java_self.load(source);")
            && translation.rust.contains("return __java_self;")
            && translation
                .rust
                .contains("JavaException::new(\"IllegalArgumentException\"")
            && translation
                .rust
                .contains("Err(e) if e.is_instance(&[\"URISyntaxException\"])")
            && translation
                .rust
                .contains("Err(e) if e.is_instance(&[\"IOException\"])")
            && translation
                .rust
                .contains("panic!(\"uncaught Java exception: {:?}\"")
            && !translation
                .rust
                .contains("unsupported Java statement: try/catch/finally"),
        "{}",
        translation.rust
    );
}

#[test]
fn corpus_genomicsdb_qualified_nested_builders_use_distinct_rust_type_names() {
    let translation = translate_source(&idiom_gap("QualifiedNestedBuilderChain.java"));

    assert!(
        translation
            .rust
            .contains("let mapping_builder: VidMappingBuilder = mapping.to_builder();")
            && translation.rust.contains(
                "let field_builder: FieldInfoBuilder = mapping_builder.get_fields_builder(index);"
            )
            && translation.rust.contains("pub struct VidMappingBuilder")
            && translation.rust.contains("pub struct FieldInfoBuilder")
            && !translation.rust.contains("pub struct Builder")
            && !translation.rust.contains("VidMapping.Builder")
            && !translation.rust.contains("FieldInfo.Builder")
            && translation.rust.contains("return mapping_builder.build();"),
        "{}",
        translation.rust
    );
}

#[test]
fn corpus_spark_rdd_pipeline_keeps_stream_lowering_at_noop_spark_boundary() {
    let translation = translate_source(&idiom_gap("SparkRddVariantPipeline.java"));

    assert!(
        (translation
            .rust
            .contains("variants.map_to_pair(|variant| Tuple2::new(variant, None))")
            || translation.rust.contains(
                "variants.map_to_pair(|variant: VariantContext| Tuple2::new(variant, None))"
            )
            || translation.rust.contains(
                "variants.map_to_pair(|variant: VariantContext| Tuple2::new(variant, None::<Void>))"
            ))
            && translation.rust.contains(
                "let comparator: Option<Comparator> = header.get_v_c_f_record_comparator();"
            )
            && translation.rust.contains("if comparator.is_none()")
            && translation.rust.contains(
                "pairs.sort_by_key_3(comparator.clone()/* TODO(translation): validate added clone */, true, reducers)"
            )
            && translation.rust.contains(
                "pairs.sort_by_key_1(comparator.clone()/* TODO(translation): validate added clone */)"
            )
            && translation
                .rust
                .contains("return sorted.map(|value: Tuple2| value._1());")
            && translation
                .diagnostics
                .has_code("external_pack_boundary_call")
            && !translation.rust.contains("if false")
            && !translation.rust.contains("sort_by_key_3(&comparator")
            && !translation.rust.contains("sort_by_key_1(&comparator")
            && !translation
                .rust
                .contains("unsupported Java expression: lambda"),
        "rust:\n{}\ndiagnostics:{:?}",
        translation.rust,
        translation.diagnostics
    );
}

#[test]
fn corpus_disq_external_chain_stays_unmodeled_boundary_without_self_recursion_warning() {
    let translation = translate_source(&idiom_gap("DisqExternalBoundaryChain.java"));

    assert!(
        translation.rust.contains(
            "HtsjdkVariantsRddStorage::make_default(ctx).read(input, intervals).get_variants()"
        ) && translation
            .rust
            .contains("TabixIndexWriteOption::from_boolean(true)")
            && translation
                .diagnostics
                .has_code("external_pack_boundary_call")
            && !translation.diagnostics.has_code("recursion_tco_fallback")
            && !translation
                .diagnostics
                .has_code("external_pack_missing_api"),
        "rust:
{}
diagnostics:{:?}",
        translation.rust,
        translation.diagnostics
    );
}

#[test]
fn wildcard_imported_headerless_read_adapter_has_gatkread_return_surface() {
    let translation = translate_source(
        r#"
import htsjdk.samtools.SAMRecord;
import org.broadinstitute.hellbender.utils.read.*;

class HeaderlessReadAdapterSurface {
    GATKRead adapt(SAMRecord record) {
        return SAMRecordToGATKReadAdapter.headerlessReadAdapter(record);
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("pub fn headerless_read_adapter<T0>(_arg0: T0) -> GATKRead")
            && !translation
                .rust
                .contains("headerless_read_adapter<R: Default"),
        "{}",
        translation.rust
    );
}

#[test]
fn variant_context_adapter_has_gatkvariant_return_surface() {
    let translation = translate_source(
        r#"
import htsjdk.variant.variantcontext.VariantContext;
import org.broadinstitute.hellbender.utils.variant.GATKVariant;
import org.broadinstitute.hellbender.utils.variant.VariantContextVariantAdapter;

class VariantContextAdapterSurface {
    GATKVariant adapt(VariantContext variant) {
        return VariantContextVariantAdapter.sparkVariantAdapter(variant);
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("pub fn spark_variant_adapter<T0>(_arg0: T0) -> GATKVariant")
            && !translation
                .rust
                .contains("spark_variant_adapter<R: Default"),
        "{}",
        translation.rust
    );
}

#[test]
fn objects_non_null_method_reference_in_callback_lowers_to_closure() {
    let translation = translate_source(
        r#"
import java.util.Objects;
import org.apache.spark.api.java.JavaRDD;

class ObjectsNonNullCallback {
    JavaRDD keep(JavaRDD values) {
        return values.filter(Objects::nonNull);
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("values.filter(|value| Objects::non_null(value))")
            && !translation.rust.contains("filter(Objects::non_null"),
        "{}",
        translation.rust
    );
}

#[test]
fn disq_reads_rdd_get_reads_shapes_spark_map_lambda_param() {
    let translation = translate_source(
        r#"
import htsjdk.samtools.SAMRecord;
import java.util.Objects;
import org.apache.spark.api.java.JavaRDD;
import org.broadinstitute.hellbender.utils.read.GATKRead;
import org.broadinstitute.hellbender.utils.read.SAMRecordToGATKReadAdapter;
import org.disq_bio.disq.HtsjdkReadsRdd;

class DisqReadsMapLambdaShape {
    JavaRDD adapt(HtsjdkReadsRdd rdd) {
        return rdd.getReads().map(read -> read.getContig());
    }

    JavaRDD filterAdapted(HtsjdkReadsRdd rdd) {
        return rdd.getReads().map(read -> SAMRecordToGATKReadAdapter.headerlessReadAdapter(read)).filter(value -> Objects.nonNull(value));
    }

    JavaRDD convert(JavaRDD reads) {
        return reads.map((GATKRead read) -> read.convertToSAMRecord(null));
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains(".map(|read: SAMRecord| read.get_contig())"),
        "{}",
        translation.rust
    );
    assert!(
        translation
            .rust
            .contains(".filter(|value: GATKRead| JavaUtilObjects::non_null(value))"),
        "{}",
        translation.rust
    );
    assert!(
        translation
            .rust
            .contains("read.convert_to_s_a_m_record(None"),
        "{}",
        translation.rust
    );
    assert!(
        translation
            .rust
            .contains("pub fn get_reads(&self) -> JavaRDD"),
        "{}",
        translation.rust
    );
    assert!(
        !translation
            .rust
            .contains("pub fn get_reads(&self) -> Vec<SAMRecord>"),
        "{}",
        translation.rust
    );
}

#[test]
fn disq_reads_rdd_get_reads_shapes_spark_method_reference_filter_param() {
    let translation = translate_source(
        r#"
import htsjdk.samtools.SAMRecord;
import java.util.Objects;
import org.apache.spark.api.java.JavaRDD;
import org.broadinstitute.hellbender.utils.read.GATKRead;
import org.broadinstitute.hellbender.utils.read.SAMRecordToGATKReadAdapter;
import org.disq_bio.disq.HtsjdkReadsRdd;

class DisqReadsMethodReferenceFilterShape {
    JavaRDD filterAdapted(HtsjdkReadsRdd rdd) {
        return rdd.getReads().map(read -> SAMRecordToGATKReadAdapter.headerlessReadAdapter(read)).filter(Objects::nonNull);
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains(".filter(|value: GATKRead| JavaUtilObjects::non_null(value))")
            || translation
                .rust
                .contains(".filter(|value: GATKRead| Objects::non_null(value))"),
        "{}",
        translation.rust
    );
}

#[test]
fn corpus_genomicsdb_callback_argument_external_fields_are_instance_fields() {
    let facts = parse_external_pack_manifest(
        r#"
external-pack genomicsdb BatchCompletionCallbackFunctionArgument semantic
external-pack-field genomicsdb BatchCompletionCallbackFunctionArgument batchCount semantic type=int const-kind=instance-field
external-pack-field genomicsdb BatchCompletionCallbackFunctionArgument totalBatchCount semantic type=int const-kind=instance-field
"#,
    )
    .facts;
    let translation = translate_source_with_facts(
        r#"
import org.genomicsdb.model.BatchCompletionCallbackFunctionArgument;

class GenomicsDbBatchCallbackFields {
    int next(BatchCompletionCallbackFunctionArgument arg, int size) {
        int startBatch = (arg.batchCount - 1) * size;
        int stopBatch = arg.batchCount * size;
        return startBatch + stopBatch + arg.totalBatchCount;
    }
}
"#,
        &facts,
    );

    assert!(
        translation.rust.contains("batch_count: i32")
            && translation.rust.contains("total_batch_count: i32")
            && translation.rust.contains("arg.batch_count")
            && translation.rust.contains("arg.total_batch_count")
            && !translation.rust.contains("pub const batch_count"),
        "{}",
        translation.rust
    );
}

#[test]
fn corpus_genomicsdb_external_instance_setter_keeps_receiver() {
    let translation = translate_source(
        r#"
import org.broadinstitute.hellbender.utils.progressmeter.ProgressMeter;

class GenomicsDbProgressMeterSetter {
    ProgressMeter progressMeter = new ProgressMeter();

    void configure() {
        progressMeter.setRecordsBetweenTimeChecks(1L);
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("pub fn set_records_between_time_checks<T0>(&self, _arg0: T0)")
            && translation
                .rust
                .contains("self.progress_meter.set_records_between_time_checks(1i64);"),
        "{}",
        translation.rust
    );
}

#[test]
fn inherited_external_field_setter_does_not_become_static_owner() {
    let target = r#"
import org.broadinstitute.hellbender.engine.GATKTool;

class InheritedProgressMeterSetter extends GATKTool {
    void configure() {
        progressMeter.setRecordsBetweenTimeChecks(1L);
    }
}
"#;
    let summary = r#"
package org.broadinstitute.hellbender.engine;

public class GATKTool {
    protected ProgressMeter progressMeter = new ProgressMeter();
}

public class ProgressMeter {
    public void setRecordsBetweenTimeChecks(long recordsBetweenTimeChecks) {
    }
}
"#;
    let translation = translate_source_with_context_summary_sources(
        target,
        &[summary.to_string()],
        &FactSet::default(),
    );

    assert!(
        translation
            .rust
            .contains("pub fn set_records_between_time_checks<T0>(&self, _arg0: T0)")
            && !translation
                .rust
                .contains("pub fn set_records_between_time_checks<T0>(_arg0: T0)")
            && translation
                .rust
                .contains("self.progress_meter.set_records_between_time_checks(1i64);"),
        "{}",
        translation.rust
    );
}

#[test]
fn corpus_genomicsdb_flatmap_owned_inner_stream_callback_arg() {
    let translation = translate_source(
        r#"
import java.util.List;
import java.util.stream.Collectors;

class GenomicsDbNestedFlatMapOwnedArg {
    List<SimpleInterval> intervals(List<String> partitions, VidMapping vidMap) {
        return partitions.stream().flatMap(partition -> {
            long[] bounds = GenomicsDBUtils.getArrayColumnBounds(partition);
            return vidMap.getContigsList().stream()
                .filter(x -> x.getTiledbColumnOffset() >= bounds[0] && x.getTiledbColumnOffset() <= bounds[1])
                .map(x -> new SimpleInterval(x.getName(), 1, x.getLength()));
        }).collect(Collectors.toList());
    }
}

class VidMapping {
    List<Chromosome> getContigsList() { return null; }
}

class Chromosome {
    long getTiledbColumnOffset() { return 0L; }
    String getName() { return ""; }
    long getLength() { return 0L; }
}

class SimpleInterval {
    SimpleInterval(String name, int start, long end) {}
}

class GenomicsDBUtils {
    static long[] getArrayColumnBounds(String partition) { return null; }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains(".map(|x: Chromosome| SimpleInterval::new")
            && !translation.rust.contains(".map(|x: &Chromosome|"),
        "{}",
        translation.rust
    );
}

#[test]
fn corpus_spark_casted_flatmap_lambda_is_target_typing_not_runtime_downcast() {
    let translation = translate_source(
        r#"
import htsjdk.variant.variantcontext.VariantContext;
import java.util.Iterator;
import org.apache.spark.api.java.JavaRDD;
import org.apache.spark.api.java.function.FlatMapFunction;

class SparkCastedFlatMapFunction {
    JavaRDD combine(JavaRDD variants, Header header) {
        return variants.mapPartitions((FlatMapFunction<Iterator<VariantContext>, VariantContext>) v -> new GVCFBlockCombiningIterator(v, header));
    }
}

class Header {}
class GVCFBlockCombiningIterator {
    GVCFBlockCombiningIterator(Iterator<VariantContext> variants, Header header) {}
}
"#,
    );

    assert!(
        (translation
            .rust
            .contains(".map_partitions(|v: std::vec::IntoIter<VariantContext>|")
            || translation
                .rust
                .contains(".map_partitions_1(|v: std::vec::IntoIter<VariantContext>|"))
            && translation.rust.contains("GVCFBlockCombiningIterator::new")
            && !translation.rust.contains("downcast::<FlatMapFunction>()"),
        "{}",
        translation.rust
    );
}

#[test]
fn corpus_spark_pair_rdd_map_callback_gets_tuple_param_type() {
    let translation = translate_source(
        r#"
import htsjdk.variant.variantcontext.VariantContext;
import org.apache.spark.api.java.JavaRDD;
import org.apache.spark.api.java.JavaPairRDD;
import scala.Tuple2;

class SparkPairRddMapTupleShape {
    JavaRDD<VariantContext> variants(JavaRDD<VariantContext> variants) {
        JavaPairRDD<VariantContext, Void> variantVoidPairs = variants.mapToPair(variant -> new Tuple2<>(variant, null));
        return variantVoidPairs.map(value -> value._1());
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains(".map(|value: Tuple2<VariantContext, Void>| value._1())")
            || translation
                .rust
                .contains(".map(|value: Tuple2<VariantContext, Option<Void>>| value._1())")
            || translation
                .rust
                .contains(".map(|value: Tuple2| value._1())"),
        "{}",
        translation.rust
    );
}

#[test]
fn corpus_spark_branch_assigned_pair_rdd_map_callback_gets_tuple_param_type() {
    let translation = translate_source(
        r#"
import htsjdk.variant.variantcontext.VariantContext;
import java.util.Comparator;
import org.apache.spark.api.java.JavaRDD;
import org.apache.spark.api.java.JavaPairRDD;
import scala.Tuple2;

class SparkPairRddBranchAssignedMapTupleShape {
    JavaRDD<VariantContext> variants(JavaRDD<VariantContext> variants, Comparator<VariantContext> comparator, int reducers) {
        JavaPairRDD<VariantContext, Void> pairs = variants.mapToPair(variant -> new Tuple2<>(variant, (Void) null));
        JavaPairRDD<VariantContext, Void> sorted;
        if (comparator == null) {
            sorted = pairs;
        } else if (reducers > 0) {
            sorted = pairs.sortByKey(comparator, true, reducers);
        } else {
            sorted = pairs.sortByKey(comparator);
        }
        return sorted.map(Tuple2::_1);
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains(".map(|value: Tuple2<VariantContext, Void>| value._1())")
            || translation
                .rust
                .contains(".map(|value: Tuple2<VariantContext, Option<Void>>| value._1())")
            || translation
                .rust
                .contains(".map(|value: Tuple2| value._1())"),
        "{}",
        translation.rust
    );
}

#[test]
fn corpus_spark_interface_receiver_delegation_borrows_trait_slot_arguments() {
    let translation = translate_source(
        r#"
interface ReferenceSource {
    Result getReferenceBases(SimpleInterval interval) throws IOException;
    SAMSequenceDictionary getReferenceSequenceDictionary(SAMSequenceDictionary dictionary) throws IOException;
    default byte[] getBases(SimpleInterval window) throws IOException {
        return getReferenceBases(window).getBases();
    }
}

class Result {
    byte[] getBases() {
        return null;
    }
}
class SimpleInterval {
    int getStart() {
        return 0;
    }
}
class SAMSequenceDictionary {
    int size() {
        return 0;
    }
}

class ReferenceMultiSource implements ReferenceSource {
    ReferenceSource referenceSource;

    public Result getReferenceBases(SimpleInterval interval) throws IOException {
        return referenceSource.getReferenceBases(interval);
    }

    public SAMSequenceDictionary getReferenceSequenceDictionary(SAMSequenceDictionary dictionary) throws IOException {
        return referenceSource.getReferenceSequenceDictionary(dictionary);
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("self.reference_source.as_ref().unwrap().get_reference_bases(&interval)?")
            && !translation
                .rust
                .contains("self.reference_source.as_ref().unwrap().get_reference_bases(interval)?")
            && translation.rust.contains(
                "self.reference_source.as_ref().unwrap().get_reference_sequence_dictionary(dictionary)?"
            )
            && !translation.rust.contains(
                "self.reference_source.as_ref().unwrap().get_reference_sequence_dictionary(&dictionary)?"
            ),
        "{}",
        translation.rust
    );
}

#[test]
fn boxed_dyn_clone_recovery_parenthesizes_cast_receiver() {
    let translation = translate_source(
        r#"
interface Writer {
}

class BasicWriter implements Writer {
}

class AsyncWriter implements Writer {
    Writer delegate;

    AsyncWriter(Writer writer) {
        this.delegate = writer;
    }
}

class InterfaceWriterFactory {
    Writer newWriter(boolean async) {
        Writer writer = new BasicWriter();
        if (async) {
            return new AsyncWriter(writer);
        }
        return writer;
    }
}
"#,
    );

    assert!(
        (translation
            .rust
            .contains("pub fn new_writer(&self, async_: bool) -> Box<dyn Writer>")
            || translation
                .rust
                .contains("pub fn new_writer(&self, async: bool) -> Box<dyn Writer>"))
            && translation.rust.contains(
                "let writer: Box<dyn Writer> = Box::new(BasicWriter::new()) as Box<dyn Writer>;"
            )
            && translation
                .rust
                .contains("return Box::new(AsyncWriter::new(writer")
            && !translation.rust.contains("writer.clone()")
            && !translation.rust.contains("-> Writer")
            && !translation.rust.contains("let writer: Writer")
            && !translation
                .rust
                .contains("Box::new(writer) as Box<dyn Writer>.clone()"),
        "{}",
        translation.rust
    );
}

#[test]
fn external_async_template_super_satisfies_interface_methods() {
    let translation = translate_source(
        r#"
import htsjdk.samtools.util.AbstractAsyncWriter;

interface MiniWriter {
    void write(MiniRecord record);
    void close();
}

class MiniRecord {
}

class MiniAsyncWriter extends AbstractAsyncWriter<MiniRecord> implements MiniWriter {
    private final MiniWriter writer;

    MiniAsyncWriter(MiniWriter writer, int queueSize) {
        super(queueSize);
        this.writer = writer;
    }

    protected void synchronouslyWrite(MiniRecord record) {
        writer.write(record);
    }

    protected void synchronouslyClose() {
        writer.close();
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("pub fn new(writer: Box<dyn MiniWriter>, queue_size: i32) -> MiniAsyncWriter")
            && translation
                .rust
                .contains("return MiniAsyncWriter { writer: writer };")
            && translation
                .rust
                .contains("fn write(&self, record: MiniRecord) { MiniAsyncWriter::synchronously_write(self, record); }")
            && translation
                .rust
                .contains("fn close(&self) { MiniAsyncWriter::synchronously_close(self); }")
            && !translation
                .rust
                .contains("unsupported Java constructor: super(...) requires inheritance lowering")
            && !translation
                .rust
                .contains("missing Java interface method implementation"),
        "{}",
        translation.rust
    );
}

#[test]
fn interface_return_this_does_not_emit_runtime_downcast() {
    let translation = translate_source(
        r#"
interface Cursor {
}

class Reader implements Cursor {
    public Cursor cursor() {
        return this;
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("pub fn cursor(&self) -> Box<dyn Cursor>")
            && !translation.rust.contains("-> Cursor")
            && !translation.rust.contains(".downcast::<Cursor>()")
            && !translation.rust.contains("invalid Java cast"),
        "{}",
        translation.rust
    );
}

#[test]
fn current_self_to_erased_implemented_interface_boxes_clone() {
    let translation = translate_source(
        r#"
interface Cursor {
}

class Reader implements Cursor {
    Cursor iterator() {
        return this;
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("pub fn iterator(&self) -> Box<dyn Cursor>")
            && translation.rust.contains(
                "return Box::new(self.clone()/* TODO(translation): validate added clone */) as Box<dyn Cursor>;"
            )
            && !translation.rust.contains("return self;")
            && !translation.rust.contains("invalid Java cast"),
        "{}",
        translation.rust
    );
}

#[test]
fn current_self_to_erased_platform_interface_boxes_clone() {
    let translation = translate_source(
        r#"
import java.util.Iterator;

class Reader implements Iterator<Record> {
    public boolean hasNext() { return false; }
    public Record next() { return null; }
    public Iterator<Record> iterator() {
        return this;
    }
}

class Record {
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("pub fn iterator(&self) -> Box<dyn std::any::Any>")
            && translation.rust.contains(
                "return Box::new(self.clone()/* TODO(translation): validate added clone */) as Box<dyn std::any::Any>;"
            )
            && !translation.rust.contains("return self;")
            && !translation.rust.contains("invalid Java cast"),
        "{}",
        translation.rust
    );
}

#[test]
fn current_self_concrete_static_argument_clones_without_downcast() {
    let translation = translate_source(
        r#"
class Encoder {
    static String encode(Record record) {
        return "x";
    }
}

class Record {
    String text() {
        return Encoder.encode(this);
    }
}
"#,
    );

    assert!(
        translation.rust.contains(
            "return Encoder::encode(self.clone()/* TODO(translation): validate added clone */);"
        ) && !translation.rust.contains("downcast::<Record>")
            && !translation.rust.contains("invalid Java cast"),
        "{}",
        translation.rust
    );
}

#[test]
fn appendable_fluent_chain_keeps_receiver_and_to_string_type() {
    let translation = translate_source(
        r#"
import java.io.IOException;

class AppendableFluentChain {
    static String encode(String value) throws IOException {
        return write(new StringBuilder(16), value).toString();
    }

    static Appendable write(Appendable out, String value) throws IOException {
        return out.append("a").append(value).append('\n');
    }
}
"#,
    );

    assert!(
        translation.rust.contains(
            "pub fn write(out: &mut Appendable, value: String) -> Result<Appendable, JavaException>"
        ) && (translation
            .rust
            .contains("Self::write(&mut StringBuilder::new(16).into(), value)")
            || translation
                .rust
                .contains("Self::write(StringBuilder::new(16).into(), value)"))
            && translation
                .rust
                .contains(".append(\"a\".to_string())?.append(value)?.append('\\n')?")
            && translation.rust.contains(")?.to_string()")
            && !translation
                .rust
                .contains("fn append<T0>(&self, _arg0: T0) -> ()")
            && !translation
                .rust
                .contains("pub fn append<T0>(&self, _arg0: T0) -> Self")
            && !translation.rust.contains("fn to_string(&self) -> ()"),
        "{}",
        translation.rust
    );
}

#[test]
fn appendable_field_conversion_clones_before_into() {
    let translation = translate_source(
        r#"
import java.io.IOException;
import java.io.PrintStream;

class AppendableFieldConversion {
    private final PrintStream writer;

    AppendableFieldConversion(PrintStream writer) {
        this.writer = writer;
    }

    void emit(String value) throws IOException {
        AppendableHelper.write(writer, value);
    }
}

class AppendableHelper {
    static Appendable write(Appendable out, String value) throws IOException {
        return out.append(value);
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("AppendableHelper::write(&mut self.writer.clone()/* TODO(translation): validate added clone */.into(), value)?;")
            && !translation.rust.contains("AppendableHelper::write(self.writer.into(), value)?;")
            && !translation.rust.contains(".into().clone()"),
        "{}",
        translation.rust
    );
}

#[test]
fn print_stream_check_error_is_boolean_getter() {
    let translation = translate_source(
        r#"
import java.io.PrintStream;

class PrintStreamCheckErrorUse {
    boolean failed(PrintStream writer) {
        return writer.checkError();
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("pub fn check_error(&self) -> bool")
            && translation.rust.contains("return writer.check_error();")
            && !translation.rust.contains("pub fn check_error(&self) -> ()"),
        "{}",
        translation.rust
    );
}

#[test]
fn print_stream_output_stream_constructor_has_typed_support_arg() {
    let translation = translate_source(
        r#"
import java.io.OutputStream;
import java.io.PrintStream;

class PrintStreamOutputStreamConstructorUse {
    PrintStream open(OutputStream out) {
        return new PrintStream(out);
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("pub fn new(_arg0: OutputStream) -> Self")
            && translation.rust.contains("return PrintStream::new(out);")
            && !translation
                .rust
                .contains("pub fn new<T0>(_arg0: T0) -> Self"),
        "{}",
        translation.rust
    );
}

#[test]
fn source_interface_trait_uses_fallible_implementation_signature() {
    let interface_source = r#"
interface Sink {
    void write(String value);
}
"#;
    let implementation_source = r#"
class FileSink implements Sink {
    public void write(String value) throws IOException {
        flush();
    }

    void flush() throws IOException {
    }
}

class IOException extends Exception {}
"#;
    let translation = translate_source(&format!("{interface_source}\n{implementation_source}"));

    assert!(
        translation
            .rust
            .contains("pub trait Sink {\n    fn write(&self, value: String) -> Result<(), JavaException>;")
            && translation
                .rust
                .contains("impl Sink for FileSink {\n    fn write(&self, value: String) -> Result<(), JavaException> { FileSink::write(self, value) }")
            && !translation
                .rust
                .contains("fn write(&self, value: String);"),
        "{}",
        translation.rust
    );

    let interface_translation = translate_source_with_context_sources_and_summary_sources(
        interface_source,
        &[implementation_source.to_string()],
        &[],
        true,
        &FactSet::default(),
    );
    assert!(
        interface_translation.rust.contains(
            "pub trait Sink {\n    fn write(&self, value: String) -> Result<(), JavaException>;"
        ),
        "{}",
        interface_translation.rust
    );

    let implementation_translation = translate_source_with_context_sources_and_summary_sources(
        implementation_source,
        &[interface_source.to_string()],
        &[],
        true,
        &FactSet::default(),
    );
    assert!(
        implementation_translation
            .rust
            .contains("impl Sink for FileSink {\n    fn write(&self, value: String) -> Result<(), JavaException> { FileSink::write(self, value) }"),
        "{}",
        implementation_translation.rust
    );
}

#[test]
fn source_varargs_pack_external_elements_with_explicit_adapter() {
    let facts = parse_external_pack_manifest(
        r#"
external-pack disq WriteOption semantic
external-pack disq ReadsFormatWriteOption semantic
external-pack disq FileCardinalityWriteOption semantic
external-pack-capability disq ReadsFormatWriteOption clone=true final=true implements=WriteOption
external-pack-capability disq FileCardinalityWriteOption clone=true final=true implements=WriteOption
external-pack-field disq ReadsFormatWriteOption BAM semantic type=ReadsFormatWriteOption const-kind=static-final
external-pack-field disq FileCardinalityWriteOption SINGLE semantic type=FileCardinalityWriteOption const-kind=static-final
external-pack-conversion disq ReadsFormatWriteOption WriteOption into
external-pack-conversion disq FileCardinalityWriteOption WriteOption into
"#,
    )
    .facts;
    let translation = translate_source_with_facts(
        r#"
class SourceVarargsWriteOptions {
    static void write(long granularity, WriteOption... options) {}
    static void run() {
        ReadsFormatWriteOption format = ReadsFormatWriteOption.BAM;
        FileCardinalityWriteOption cardinality = FileCardinalityWriteOption.SINGLE;
        write(128L, format, cardinality);
    }
}
"#,
        &facts,
    );

    assert!(
        translation
            .rust
            .contains("vec![format.into(), cardinality.into()]"),
        "{}",
        translation.rust
    );
}

#[test]
fn source_varargs_pack_borrowed_external_elements_clones_before_into() {
    let facts = parse_external_pack_manifest(
        r#"
external-pack disq WriteOption semantic
external-pack disq ReadsFormatWriteOption semantic
external-pack disq FileCardinalityWriteOption semantic
external-pack-capability disq ReadsFormatWriteOption clone=true final=true implements=WriteOption
external-pack-capability disq FileCardinalityWriteOption clone=true final=true implements=WriteOption
external-pack-conversion disq ReadsFormatWriteOption WriteOption into
external-pack-conversion disq FileCardinalityWriteOption WriteOption into
"#,
    )
    .facts;
    let translation = translate_source_with_facts(
        r#"
class SourceVarargsBorrowedWriteOptions {
    static void write(long granularity, WriteOption... options) {}
    static void run(ReadsFormatWriteOption format, FileCardinalityWriteOption cardinality) {
        write(128L, format, cardinality);
    }
}
"#,
        &facts,
    );

    assert!(
        translation
            .rust
            .contains("vec![format.clone().into(), cardinality.clone().into()]")
            && !translation
                .rust
                .contains("vec![format.into(), cardinality.into()]"),
        "{}",
        translation.rust
    );
}

#[test]
fn source_overload_prefers_expanded_varargs_over_same_arity_fixed_mismatch() {
    let facts = parse_external_pack_manifest(
        r#"
external-pack disq WriteOption semantic
external-pack disq ReadsFormatWriteOption semantic
external-pack disq FileCardinalityWriteOption semantic
external-pack disq ReadsWriteFormat semantic
external-pack-capability disq ReadsFormatWriteOption clone=true final=true implements=WriteOption
external-pack-capability disq FileCardinalityWriteOption clone=true final=true implements=WriteOption
external-pack-field disq ReadsFormatWriteOption BAM semantic type=ReadsFormatWriteOption const-kind=static-final
external-pack-field disq FileCardinalityWriteOption SINGLE semantic type=FileCardinalityWriteOption const-kind=static-final
external-pack-conversion disq ReadsFormatWriteOption WriteOption into
external-pack-conversion disq FileCardinalityWriteOption WriteOption into
"#,
    )
    .facts;
    let translation = translate_source_with_facts(
        r#"
class SourceOverloadExpandedVarargs {
    static void write(String output, ReadsWriteFormat format, int reducers, String parts, boolean sorted, long granularity) {}
    static void write(String output, long granularity, WriteOption... options) {}
    static void run(String output, long granularity) {
        ReadsFormatWriteOption format = ReadsFormatWriteOption.BAM;
        FileCardinalityWriteOption cardinality = FileCardinalityWriteOption.SINGLE;
        write(output, granularity, format, cardinality);
    }
}
"#,
        &facts,
    );

    assert!(
        translation
            .rust
            .contains("Self::write_string_long_write_option_array_varargs(output, granularity")
            && translation
                .rust
                .contains("vec![format.into(), cardinality.into()]")
            && !translation.rust.contains(
                "Self::write_string_reads_write_format_int_string_boolean_long(output, granularity"
            ),
        "{}",
        translation.rust
    );
}

#[test]
fn reused_optional_borrowed_source_argument_wraps_by_reference() {
    let facts = parse_external_pack_manifest(
        r#"
external-pack dep GATKPath semantic
external-pack dep JavaRDD semantic
external-pack dep WriteOption semantic
external-pack dep ConcreteWriteOption semantic
external-pack-capability dep GATKPath clone=true final=true
external-pack-capability dep JavaRDD clone=true final=true
external-pack-capability dep ConcreteWriteOption clone=true final=true implements=WriteOption
external-pack-conversion dep ConcreteWriteOption WriteOption into
"#,
    )
    .facts;
    let translation = translate_source_with_facts(
        r#"
class ReusedOptionalBorrowedSourceArgument {
    static void write(String output, GATKPath reference, JavaRDD reads, WriteOption... options) {}

    static void run(String output, GATKPath reference, JavaRDD reads, ConcreteWriteOption option) {
        write(output, null, reads, option);
        write(output, reference, reads, option);
        reference.toPath();
    }
}
"#,
        &facts,
    );

    assert!(
        translation
            .rust
            .contains("pub fn run(output: &String, reference: Option<&GATKPath>, reads: &JavaRDD")
            && translation
                .rust
                .contains("Self::write(output, reference, reads, &vec![option.clone().into()]);")
            && !translation
                .rust
                .contains("Self::write(output, reference, reads, &vec![option.into()]);")
            && !translation
                .rust
                .contains("Some(reference.clone()/* TODO(translation): validate added clone */)"),
        "{}",
        translation.rust
    );
}

#[test]
fn broad_reads_path_data_source_converts_to_reads_data_source() {
    let translation = translate_source(
        r#"
import htsjdk.samtools.SamReaderFactory;
import java.nio.file.Path;
import java.util.List;
import org.broadinstitute.hellbender.engine.ReadsDataSource;
import org.broadinstitute.hellbender.engine.ReadsPathDataSource;

class BroadReadsPathDataSourceConversion {
    ReadsDataSource open(List<Path> paths, SamReaderFactory factory) {
        ReadsDataSource source = new ReadsPathDataSource(paths, factory);
        return source;
    }
}
"#,
    );

    assert!(
        translation.rust.contains(
            "let source: ReadsDataSource = ReadsPathDataSource::new(paths, factory).into();"
        ) && translation
            .rust
            .contains("impl From<ReadsPathDataSource> for ReadsDataSource"),
        "rust:\n{}\ndiagnostics:{:?}",
        translation.rust,
        translation.diagnostics
    );
}

#[test]
fn broad_headerless_sam_comparator_converts_to_java_comparator() {
    let translation = translate_source(
        r#"
import htsjdk.samtools.SAMFileHeader;
import htsjdk.samtools.SAMRecord;
import java.util.Comparator;
import org.broadinstitute.hellbender.utils.read.HeaderlessSAMRecordCoordinateComparator;

class BroadHeaderlessSamComparatorConversion {
    Comparator<SAMRecord> comparator(SAMFileHeader header) {
        Comparator<SAMRecord> result = new HeaderlessSAMRecordCoordinateComparator(header);
        return result;
    }
}
"#,
    );

    assert!(
        translation.rust.contains(
            "let result: Comparator = HeaderlessSAMRecordCoordinateComparator::new(header).into();"
        ) && translation
            .rust
            .contains("impl From<HeaderlessSAMRecordCoordinateComparator> for Comparator"),
        "rust:\n{}\ndiagnostics:{:?}",
        translation.rust,
        translation.diagnostics
    );
}

#[test]
fn broad_spark_utils_hadoop_path_exists_has_bool_surface() {
    let translation = translate_source(
        r#"
import java.net.URI;
import org.apache.spark.api.java.JavaSparkContext;
import org.broadinstitute.hellbender.utils.spark.SparkUtils;

class BroadSparkUtilsHadoopPathExistsSurface {
    boolean exists(JavaSparkContext ctx, URI uri) {
        return SparkUtils.hadoopPathExists(ctx, uri);
    }
}
"#,
    );

    assert!(
        translation.rust.contains(
            "pub fn hadoop_path_exists<T0, T1>(_arg0: T0, _arg1: T1) -> bool"
        ) && (translation
            .rust
            .contains("return SparkUtils::hadoop_path_exists(ctx, uri);")
            || translation.rust.contains(
                "return OrgBroadinstituteHellbenderUtilsSparkSparkUtils::hadoop_path_exists(ctx, uri);"
            )),
        "rust:\n{}\ndiagnostics:{:?}",
        translation.rust,
        translation.diagnostics
    );
}

#[test]
fn seekable_stream_factory_get_instance_has_concrete_surface() {
    let translation = translate_source(
        r#"
import htsjdk.samtools.seekablestream.SeekableStreamFactory;
import java.nio.file.Path;

class SeekableStreamFactoryConcreteSurface {
    Object stream(Path path) {
        return SeekableStreamFactory.getInstance().getStreamFor(path);
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("pub fn get_instance() -> SeekableStreamFactory")
            && translation
                .rust
                .contains("pub fn get_stream_for<T0>(&self, _arg0: T0) -> Box<dyn std::any::Any>")
            && (translation
                .rust
                .contains("SeekableStreamFactory::get_instance().get_stream_for(path)")
                || translation.rust.contains(
                    "HtsjdkSamtoolsSeekablestreamSeekableStreamFactory::get_instance().get_stream_for(path)"
                )),
        "rust:\n{}\ndiagnostics:{:?}",
        translation.rust,
        translation.diagnostics
    );
}

#[test]
fn java_nio_files_exists_has_bool_surface() {
    let translation = translate_source(
        r#"
import java.nio.file.Files;
import java.nio.file.Path;

class JavaNioFilesExistsSurface {
    boolean exists(Path path) {
        return Files.exists(path);
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("pub fn exists<T0>(_arg0: T0) -> bool")
            && translation
                .rust
                .contains("return JavaNioFileFiles::exists(path);"),
        "rust:\n{}\ndiagnostics:{:?}",
        translation.rust,
        translation.diagnostics
    );
}

#[test]
fn raw_list_cast_over_typed_external_call_uses_target_slot_shape() {
    let translation = translate_source(
        r#"
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;

class RawListCastExternalCall {
    List<String> read(Path path) throws java.io.IOException {
        List<String> lines = (List) Files.readAllLines(path);
        return lines;
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("let lines: Vec<String> = JavaNioFileFiles::read_all_lines(path)?;")
            && !translation.rust.contains("downcast::<List>()")
            && !translation.rust.contains("invalid Java cast"),
        "rust:\n{}\ndiagnostics:{:?}",
        translation.rust,
        translation.diagnostics
    );
}

#[test]
fn typed_external_call_to_declared_generic_local_uses_external_return_shape() {
    let translation = translate_source(
        r#"
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;

class TypedExternalCallDeclaredGenericLocal {
    int count(Path path) throws java.io.IOException {
        final List<String> lines = Files.readAllLines(path);
        return lines.size();
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("let lines: Vec<String> = JavaNioFileFiles::read_all_lines(path)?;")
            && !translation.rust.contains("downcast::<List>()")
            && !translation.rust.contains("invalid Java cast"),
        "rust:\n{}\ndiagnostics:{:?}",
        translation.rust,
        translation.diagnostics
    );
}

#[test]
fn raw_list_cast_into_optional_slot_uses_optional_target_shape() {
    let translation = translate_source(
        r#"
import java.util.ArrayList;
import java.util.List;

class RawListCastOptionalSlot {
    private List<String> values;

    List<String> fallback() {
        return new ArrayList<>();
    }

    void choose(boolean merge, List<String> direct) {
        values = (List) (merge ? fallback() : direct);
    }
}
"#,
    );

    assert!(
        translation.rust.contains("self.values = Some(if merge")
            && !translation.rust.contains("downcast::<List>()")
            && !translation.rust.contains("invalid Java cast"),
        "rust:\n{}\ndiagnostics:{:?}",
        translation.rust,
        translation.diagnostics
    );
}

#[test]
fn static_external_call_to_generic_set_slot_uses_external_return_shape() {
    let translation = translate_source(
        r#"
import htsjdk.variant.vcf.VCFHeader;
import htsjdk.variant.vcf.VCFHeaderLine;
import htsjdk.variant.vcf.VCFUtils;
import java.util.List;
import java.util.Set;

class SmartMergeHeaderLinesSlot {
    Set<VCFHeaderLine> merge(List<VCFHeader> headers) {
        Set<VCFHeaderLine> lines = VCFUtils.smartMergeHeaders(headers, true);
        return lines;
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("let lines: std::collections::HashSet<VCFHeaderLine>")
            && translation
                .rust
                .contains("HtsjdkVariantVcfVCFUtils::smart_merge_headers")
            && !translation.rust.contains("downcast::<smartMergeHeaders>")
            && !translation.rust.contains("invalid Java cast"),
        "rust:\n{}\ndiagnostics:{:?}",
        translation.rust,
        translation.diagnostics
    );
}

#[test]
fn source_nullable_field_getter_returns_optional_string() {
    let translation = translate_source(
        r#"
class SourceNullableFieldGetter {
    private final String qualityHeader;

    SourceNullableFieldGetter(String qualityHeader) {
        if (qualityHeader != null && !qualityHeader.isEmpty()) {
            this.qualityHeader = qualityHeader;
        } else {
            this.qualityHeader = null;
        }
    }

    String getQualityHeader() {
        return qualityHeader;
    }

    String describe() {
        return "quality " + getQualityHeader();
    }

    String forwarded() {
        String value = getQualityHeader();
        return value;
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("pub fn get_quality_header(&self) -> Option<String>")
            && translation.rust.contains("return self.quality_header.clone()")
            && (translation.rust.contains(
                "format!(\"quality {}\", self.get_quality_header().as_ref().map(|value| value.to_string()).unwrap_or_else(|| \"null\".to_string()))"
            ) || translation
                .rust
                .contains("format!(\"quality {}\", self.quality_header.as_deref().unwrap_or(\"null\"))"))
            && translation
                .rust
                .contains("pub fn forwarded(&self) -> Option<String>")
            && (translation
                .rust
                .contains("let value: Option<String> = self.get_quality_header();")
                || translation
                    .rust
                    .contains("let value: Option<String> = self.quality_header.clone();"))
            && !translation
                .rust
                .contains("let value: Option<String> = Some(self.get_quality_header())")
            && !translation
                .rust
                .contains("pub fn get_quality_header(&self) -> String")
            && !translation.rust.contains("return self.quality_header;"),
        "{}",
        translation.rust
    );
}

#[test]
fn source_nullable_field_return_does_not_capture_shadowing_local() {
    let translation = translate_source(
        r#"
class SourceNullableFieldGetterShadow {
    private final String qualityHeader;

    SourceNullableFieldGetterShadow() {
        this.qualityHeader = null;
    }

    String getQualityHeader(String qualityHeader) {
        return qualityHeader;
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("pub fn get_quality_header(&self, quality_header: String) -> String")
            && translation.rust.contains("return quality_header;")
            && !translation.rust.contains(
                "pub fn get_quality_header(&self, quality_header: String) -> Option<String>"
            ),
        "{}",
        translation.rust
    );
}

#[test]
fn context_source_nullable_field_getter_keeps_optional_field_shape() {
    let source = r#"
package htsjdk.samtools.fastq;

import java.io.Serializable;

public class ContextSourceNullableFieldGetter implements Serializable {
    private final String qualityHeader;
    private final String bases;

    public ContextSourceNullableFieldGetter(final String qualityHeader, final String bases) {
        if (qualityHeader != null && !qualityHeader.isEmpty()) {
            this.qualityHeader = qualityHeader;
        } else {
            this.qualityHeader = null;
        }
        this.bases = bases;
    }

    public ContextSourceNullableFieldGetter(final ContextSourceNullableFieldGetter other) {
        if (other == null) {
            throw new IllegalArgumentException("new ContextSourceNullableFieldGetter(null)");
        }
        this.qualityHeader = other.qualityHeader;
        this.bases = other.bases;
    }

    public String getBaseQualityHeader() {
        return qualityHeader;
    }

    public int hashCode() {
        final int prime = 31;
        int result = 1;
        result = prime * result + ((qualityHeader == null) ? 0 : qualityHeader.hashCode());
        result = prime * result + ((bases == null) ? 0 : bases.hashCode());
        return result;
    }
}
"#;
    let constants = r#"
package htsjdk.samtools.fastq;

public class FastqConstants {
    public static final String QUALITY_HEADER = "+";
}
"#;
    let translation = translate_source_with_context_sources_and_summary_sources(
        source,
        &[constants.to_string(), source.to_string()],
        &[],
        true,
        &FactSet::default(),
    );

    assert!(
        translation
            .rust
            .contains("pub fn get_base_quality_header(&self) -> Option<String>")
            && translation.rust.contains("return self.quality_header.clone()")
            && translation
                .rust
                .contains("if self.quality_header.is_none() { 0 } else { java_string_hash_code(self.quality_header.as_ref().unwrap().as_str()) }")
            && !translation.rust.contains("return Some(self.quality_header)")
            && !translation
                .rust
                .contains("if false { 0 } else { self.quality_header.hash_code()"),
        "{}",
        translation.rust
    );
}

#[test]
fn static_external_call_to_optional_executor_slot_uses_external_return_shape() {
    let translation = translate_source(
        r#"
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.ThreadFactory;

class ExecutorServiceSlot {
    ExecutorService executor;

    void configure(int threads, ThreadFactory factory) {
        executor = Executors.newFixedThreadPool(threads, factory);
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("self.executor = Some(JavaUtilConcurrentExecutors::new_fixed_thread_pool")
            && !translation.rust.contains("downcast::<newFixedThreadPool>")
            && !translation.rust.contains("invalid Java cast"),
        "rust:\n{}\ndiagnostics:{:?}",
        translation.rust,
        translation.diagnostics
    );
}

#[test]
fn static_external_nullable_call_to_optional_field_does_not_double_wrap() {
    let mut facts = FactSet::default();
    facts.external_api_effect_facts.push(ExternalApiEffectFact {
        method_name: "readDictionary".to_string(),
        receiver_type: Some("example.DictionaryReader".to_string()),
        arg_count: Some(1),
        role: ExternalApiRole::Getter,
        receiver_mutability: ReceiverMutabilityFact::ReadOnly,
        return_type: Some(JavaType::Named {
            name: "Dictionary".to_string(),
            args: Vec::new(),
        }),
        return_nullability: NullabilityState::MaybeNull,
        thrown: Vec::new(),
        provenance: FactProvenance::UserOverride,
    });
    let translation = translate_source_with_facts(
        r#"
import example.DictionaryReader;

class StaticExternalNullableCallToOptionalField {
    Dictionary dictionary;

    void load(File file) {
        dictionary = DictionaryReader.readDictionary(file);
    }
}
"#,
        &facts,
    );

    assert!(
        translation
            .rust
            .contains("self.dictionary = ExampleDictionaryReader::read_dictionary(file);")
            && !translation
                .rust
                .contains("self.dictionary = Some(ExampleDictionaryReader::read_dictionary"),
        "{}",
        translation.rust
    );
}

#[test]
fn raw_list_cast_over_optional_ternary_uses_optional_slot_shape() {
    let translation = translate_source(
        r#"
import java.util.List;

class OptionalTernaryRawListCast {
    List<SimpleInterval> intervals;

    void configure(boolean merge, List<SimpleInterval> simpleIntervals, SAMSequenceDictionary dictionary) {
        intervals = (List)(merge ? IntervalUtils.getSpanningIntervals(simpleIntervals, dictionary) : simpleIntervals);
    }
}
"#,
    );

    assert!(
        translation.rust.contains("self.intervals = Some(if merge")
            && translation.rust.contains("get_spanning_intervals")
            && !translation.rust.contains("Some(Some(")
            && !translation.rust.contains("downcast::<List>")
            && !translation.rust.contains("invalid Java cast"),
        "rust:\n{}\ndiagnostics:{:?}",
        translation.rust,
        translation.diagnostics
    );
}

#[test]
fn optional_ternary_list_assignment_wraps_once_without_runtime_downcast() {
    let translation = translate_source(
        r#"
import java.util.List;

class OptionalTernaryListAssignment {
    List<SimpleInterval> intervals;

    void configure(boolean merge, List<SimpleInterval> simpleIntervals, SAMSequenceDictionary dictionary) {
        intervals = merge ? IntervalUtils.getSpanningIntervals(simpleIntervals, dictionary) : simpleIntervals;
    }
}
"#,
    );

    assert!(
        translation.rust.contains("self.intervals = Some(if merge")
            && translation.rust.contains("get_spanning_intervals")
            && !translation.rust.contains("{ Some(")
            && !translation.rust.contains("downcast::<List>")
            && !translation.rust.contains("invalid Java cast"),
        "rust:\n{}\ndiagnostics:{:?}",
        translation.rust,
        translation.diagnostics
    );
}

#[test]
fn corpus_ffi_boundary_manifest_shapes_native_handover_calls() {
    let manifest = parse_external_pack_manifest(
        r#"
        external-pack native.runtime native.runtime ffi-boundary conservative target=native_runtime
        external-pack-api native.runtime native.runtime.NativeLibrary openArray conservative getter unknown definitely-non-null return=native.runtime.NativeArray
        external-pack-api native.runtime native.runtime.NativeArray query conservative getter read-only definitely-non-null return=native.runtime.NativeQuery
        external-pack-api native.runtime native.runtime.NativeQuery count conservative getter read-only definitely-non-null return=int
        external-pack-api native.runtime native.runtime.NativeArray close conservative mutator mutates-receiver definitely-non-null return=void
        external-pack-owned-recovery native.runtime native.runtime.NativeArray ffi-owned-handle
        "#,
    );
    assert!(
        manifest.diagnostics.is_empty(),
        "manifest diagnostics: {:?}",
        manifest.diagnostics
    );

    let translation =
        translate_source_with_facts(&idiom_gap("FfiNativeHandoverCall.java"), &manifest.facts);

    assert!(
        translation
            .rust
            .contains("NativeLibrary::open_array(workspace)")
            && translation
                .rust
                .contains("pub type NativeLibrary = NativeRuntimeNativeLibrary;")
            && translation
                .rust
                .contains("let __java_try_result = (|| -> Result<i32, JavaException>")
            && translation.rust.contains("return Ok(query.count());")
            && translation.rust.contains("array.close();")
            && translation
                .rust
                .contains("return __java_try_result.unwrap_or_else")
            && translation
                .rust
                .contains("panic!(\"uncaught Java exception: {:?}\", ex)")
            && !translation
                .rust
                .contains("unsupported Java statement: try/catch/finally")
            && translation.diagnostics.has_code("recursion_tco_fallback"),
        "rust:\n{}\ndiagnostics:{:?}",
        translation.rust,
        translation.diagnostics
    );
}

#[test]
fn corpus_ffi_owned_handle_transfer_requires_explicit_handover() {
    let mut facts = FactSet::default();
    let native_array = JavaType::Named {
        name: "NativeArray".to_string(),
        args: Vec::new(),
    };
    facts
        .owned_recovery_capability_facts
        .push(OwnedRecoveryCapabilityFact {
            type_name: "NativeArray".to_string(),
            capability: OwnedRecoveryCapability::FfiOwnedHandle {
                java_ty: native_array.clone(),
            },
            provenance: FactProvenance::ExternalLibraryPack,
        });
    facts
        .external_api_argument_effect_facts
        .push(ExternalApiArgumentEffectFact {
            method_name: "consume".to_string(),
            receiver_type: Some("NativeConsumer".to_string()),
            parameter_index: 0,
            parameter_type: Some(JavaType::Named {
                name: "NativeArray".to_string(),
                args: Vec::new(),
            }),
            parameter_nullability: NullabilityState::DefinitelyNonNull,
            argument_mutability: ReceiverMutabilityFact::ReadOnly,
            alias_policy: ExternalArgumentAliasPolicy::Borrowed,
            provenance: FactProvenance::ExternalLibraryPack,
        });
    facts
        .type_capability_overrides
        .push(TypeCapabilityOverride {
            type_name: "NativeArray".to_string(),
            copy: Some(false),
            clone: Some(false),
            string_like: None,
            sequence_like: None,
            implements: Vec::new(),
            extends: None,
            final_type: None,
            object_safe_trait: None,
            provenance: FactProvenance::UserOverride,
        });
    let translation_without_handover =
        translate_source_with_facts(&idiom_gap("FfiOwnedHandleTransfer.java"), &facts);
    assert!(
        !translation_without_handover
            .diagnostics
            .has_code("signature_owned_recovery_requires_ffi_transfer")
            && translation_without_handover
                .rust
                .contains("consumer.consume(array);")
            && translation_without_handover.rust.contains("array.close();"),
        "rust:\n{}\ndiagnostics:{:?}",
        translation_without_handover.rust,
        translation_without_handover.diagnostics
    );

    facts
        .external_ffi_handover_facts
        .push(ExternalFfiHandoverFact {
            method_name: "consume".to_string(),
            receiver_type: Some("NativeConsumer".to_string()),
            slot: ExternalFfiHandoverSlot::Argument { parameter_index: 0 },
            parameter_type: Some(native_array.clone()),
            provenance: FactProvenance::ExternalLibraryPack,
        });
    facts
        .external_ffi_handover_facts
        .push(ExternalFfiHandoverFact {
            method_name: "close".to_string(),
            receiver_type: Some("NativeArray".to_string()),
            slot: ExternalFfiHandoverSlot::Receiver,
            parameter_type: Some(native_array),
            provenance: FactProvenance::ExternalLibraryPack,
        });
    let array_binding = new_java2rust::usage::BindingUseFacts {
        name: "array".to_string(),
        java_ty: JavaType::Named {
            name: "NativeArray".to_string(),
            args: Vec::new(),
        },
        uses: vec![new_java2rust::usage::BindingUse::PassedToCall {
            name: "consume".to_string(),
            receiver_type: Some(JavaType::Named {
                name: "NativeConsumer".to_string(),
                args: Vec::new(),
            }),
            arg_index: 0,
            boundary: new_java2rust::usage::CallArgumentBoundary::Unknown,
        }],
    };
    let _array_constraints =
        new_java2rust::signature::external_argument_binding_constraints(&array_binding, &facts);
    let translation =
        translate_source_with_facts(&idiom_gap("FfiOwnedHandleTransfer.java"), &facts);

    assert!(
        (translation.rust.contains("consumer.consume(&array);")
            || translation.rust.contains("consumer.consume(array);"))
            && translation.rust.contains("array.close();")
            && !translation
                .diagnostics
                .has_code("signature_owned_recovery_requires_ffi_transfer")
            && !translation
                .diagnostics
                .has_code("unsupported_borrowed_to_ffi_owned_handle"),
        "rust:\n{}\ndiagnostics:{:?}",
        translation.rust,
        translation.diagnostics
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
            && translation.rust.contains("input.read(")
            && translation.rust.contains(")?;")
            && translation.rust.contains("output.write(")
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
            && translation.rust.contains("reader.read_line()?")
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
fn buffered_reader_read_line_uses_reader_external_effect() {
    let translation = translate_source(
        r#"
import java.io.BufferedReader;
import java.io.IOException;

class BufferedReaderReadLineOptional {
    BufferedReader reader;

    String read() throws IOException {
        String line;
        do {
            line = reader.readLine();
            if (line == null) return line;
        } while (line.isEmpty());
        return line;
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("line = Some(self.reader.as_ref().unwrap().read_line()?);")
            && translation.rust.contains("return Ok(line);")
            && !translation.rust.contains("read_line() -> ()")
            && !translation
                .rust
                .contains("line = Some(self.reader.read_line());"),
        "{}",
        translation.rust
    );
}

#[test]
fn translates_exception_propagating_try_with_resources_without_catch() {
    let translation = translate_source(
        r#"
class ExceptionTryWithResourcesPropagation {
    void copy(Resource resource) throws IOException {
        try (Resource input = resource) {
            input.read();
        }
    }
}

class Resource {
    void read() throws IOException {}
    void close() throws IOException {}
}

class IOException extends Exception {}
"#,
    );

    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}\n{}",
        translation.diagnostics,
        translation.rust
    );
    assert!(
        translation.rust.contains("pub fn copy")
            && translation.rust.contains("let __java_twr_result =")
            && translation.rust.contains("input.read()?;")
            && translation.rust.contains("input.close()?;")
            && translation.rust.contains("__java_twr_result?;")
            && !translation
                .rust
                .contains("unsupported Java statement: try/catch/finally"),
        "{}",
        translation.rust
    );
}

#[test]
fn translates_jaligner_fallthrough_try_catch_result_without_placeholder() {
    let translation = translate_source(&idiom_gap("JalignerFallthroughTryCatch.java"));

    assert!(
        translation
            .rust
            .contains("match (|| -> Result<(), JavaException>")
            && translation
                .rust
                .contains("line = Some(reader.read_line()?);")
            && translation.rust.contains("return Ok(line);")
            && !translation
                .rust
                .contains("unsupported Java statement: try/catch/finally")
            && !translation.rust.contains("compile_error!"),
        "{}",
        translation.rust
    );
}

#[test]
fn try_return_with_fallthrough_catch_uses_outer_fallback_return() {
    let translation = translate_source(
        r#"
class TryReturnFallthroughCatchFallback {
    String manifestValue(URL url) {
        try {
            Connection connection = url.openConnection();
            Manifest manifest = connection.getManifest();
            return manifest.getValue("Build-Timestamp");
        } catch (Exception e) {
            String message = "failed: " + e.getMessage();
        }
        return null;
    }
}

class URL {
    Connection openConnection() throws Exception { return new Connection(); }
}

class Connection {
    Manifest getManifest() throws Exception { return new Manifest(); }
}

class Manifest {
    String getValue(String key) { return key; }
}

"#,
    );

    assert!(
        translation
            .rust
            .contains("match (|| -> Result<Option<String>, JavaException>")
            && translation.rust.contains("Ok(value) => return Ok(value),")
            && translation.rust.contains("message: String =")
            && translation.rust.contains("return Ok(None);")
            && !translation
                .rust
                .contains("unsupported Java statement: try/catch/finally")
            && !translation.rust.contains("compile_error!"),
        "{}",
        translation.rust
    );
}

#[test]
fn translates_jaligner_try_catch_finally_cleanup_without_placeholder() {
    let translation = translate_source(&idiom_gap("JalignerTryCatchFinallyCleanup.java"));

    assert!(
        translation
            .rust
            .contains("let __java_try_result = match (|| -> Result<(), JavaException>")
            && translation
                .rust
                .contains("sequence = Some(resource.read()?);")
            && translation.rust.contains("resource.close()?;")
            && translation.rust.contains("__java_try_result?;")
            && translation.rust.contains("return Ok(Sequence::new")
            && !translation
                .rust
                .contains("unsupported Java statement: try/catch/finally")
            && !translation.rust.contains("compile_error!"),
        "{}",
        translation.rust
    );
}

#[test]
fn jaligner_try_catch_finally_cleanup_gets_combined_exception_plan() {
    let parsed =
        new_java2rust::parser::parse_java_source(&idiom_gap("JalignerTryCatchFinallyCleanup.java"));
    let method = parsed.program.types[0]
        .members
        .iter()
        .find_map(|member| match member {
            new_java2rust::java_ir::Member::Method(method) if method.name == "parse" => {
                Some(method)
            }
            _ => None,
        })
        .unwrap();
    let stmt = method.body.as_ref().unwrap().statements[1].clone();
    let new_java2rust::java_ir::Stmt::Try {
        resources,
        body,
        catches,
        finally_block,
    } = stmt
    else {
        panic!("expected try statement");
    };

    assert!(
        matches!(
            new_java2rust::exception_plan::plan_try_stmt(
                &resources,
                &body,
                &catches,
                finally_block.as_ref()
            ),
            new_java2rust::exception_plan::TryStmtPlan::CatchFinally { .. }
        ),
        "{:?}",
        new_java2rust::exception_plan::plan_try_stmt(
            &resources,
            &body,
            &catches,
            finally_block.as_ref()
        )
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
        translation.rust.contains(
            "#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]\npub enum SwitchReturnsEnumMatchType"
        ) && translation
            .rust
            .contains("SwitchReturnsEnumMatchType::SNP =>")
            && translation
                .rust
                .contains("SwitchReturnsEnumMatchType::INDEL =>")
            && translation
                .rust
                .contains("SwitchReturnsEnumMatchType::SYMBOLIC =>"),
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
        assert_eq!(fixture.label(SwitchReturnsEnumMatchType::SNP), "snp".to_string());
        assert_eq!(fixture.label(SwitchReturnsEnumMatchType::INDEL), "indel".to_string());
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
fn fallthrough_switch_external_enum_labels_are_qualified() {
    let translation = translate_source(
        r#"
import htsjdk.samtools.SAMFileHeader;

class ExternalEnumFallthroughSwitch {
    int order(SAMFileHeader header) {
        switch (header.getSortOrder()) {
            case coordinate:
                return 1;
            case duplicate:
                return 2;
            case queryname:
            case unsorted:
                return 3;
            default:
                return 0;
        }
    }
}
"#,
    );

    assert!(
        translation.rust.contains("SortOrder::coordinate")
            && translation.rust.contains("SortOrder::duplicate")
            && translation.rust.contains("SortOrder::queryname")
            && translation.rust.contains("SortOrder::unsorted")
            && translation
                .rust
                .contains("pub const coordinate: SortOrder = SortOrder;"),
        "{}",
        translation.rust
    );
}

#[test]
fn enum_constant_with_constructor_args_lowers_as_unit_variant_access() {
    let translation = translate_source(
        r#"
class EnumConstantWithConstructorArgs {
    enum LineType {
        SequenceHeader("Sequence Header"),
        SequenceLine("Sequence Line");

        private String printable;

        LineType(String printable) {
            this.printable = printable;
        }
    }

    LineType next() {
        return LineType.SequenceLine;
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("return EnumConstantWithConstructorArgsLineType::SequenceLine;")
            && !translation.rust.contains("::LineType(")
            && !translation.rust.contains("line_type.sequence_line"),
        "{}",
        translation.rust
    );
}

#[test]
fn emitted_java_enum_does_not_get_opaque_facade_equality_impls() {
    let translation = translate_source(
        r#"
class EmittedEnumFacadeSupportSuppression {
    enum LineType {
        Header("Header"),
        Body("Body");

        private String printable;

        LineType(String printable) {
            this.printable = printable;
        }
    }

    boolean same(LineType left, LineType right) {
        return left == right;
    }

    LineType next() {
        return LineType.Body;
    }
}
"#,
    );

    assert!(
        translation.rust.contains(
            "#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]\npub enum EmittedEnumFacadeSupportSuppressionLineType"
        ) && translation
            .rust
            .contains("return EmittedEnumFacadeSupportSuppressionLineType::Body;")
            && translation
                .rust
                .contains("impl std::fmt::Display for EmittedEnumFacadeSupportSuppressionLineType")
            && translation.rust.contains("return left == right;")
            && !translation
                .rust
                .contains("impl PartialEq for EmittedEnumFacadeSupportSuppressionLineType")
            && !translation
                .rust
                .contains("impl Eq for EmittedEnumFacadeSupportSuppressionLineType"),
        "{}",
        translation.rust
    );
}

#[test]
fn fallback_switch_trailing_break_is_unit_typed() {
    let translation = translate_source(
        r#"
import htsjdk.samtools.SAMFileHeader;
import htsjdk.samtools.SAMRecord;
import java.util.Comparator;
import org.broadinstitute.hellbender.utils.read.HeaderlessSAMRecordCoordinateComparator;

class FallbackSwitchTrailingBreakIsUnitTyped {
    Comparator<SAMRecord> comparator(SAMFileHeader header) throws Exception {
        switch (header.getSortOrder()) {
            case coordinate:
                return new HeaderlessSAMRecordCoordinateComparator(header);
            case duplicate:
                throw new Exception("unsupported");
            case queryname:
            case unsorted:
                return null;
            default:
                return null;
        }
    }
}
"#,
    );

    assert!(
        translation.rust.contains("break 'java_switch ();")
            && !translation.rust.contains("break 'java_switch;"),
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
        (translation.rust.contains(
            "Files::read_all_lines(path.clone()/* TODO(translation): validate added clone */)"
        ) || translation.rust.contains("Files::read_all_lines(path)"))
            && translation.rust.contains("path.to_string()"),
        "{}",
        translation.rust
    );
}

#[test]
fn reused_owned_optional_source_argument_clones_before_later_read() {
    let translation = translate_source(
        r#"
class ReusedOwnedOptionalSourceArgument {
    static boolean valid(String value) {
        return value != null && value.trim().equals(value);
    }

    String describe(String value) {
        if (!valid(value)) {
            return "bad " + value;
        }
        return value;
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("Self::valid(value.clone()/* TODO(translation): validate added clone */)")
            && translation
                .rust
                .contains("format!(\"bad {}\", value.as_ref()")
            && translation.rust.contains("return value;"),
        "{}",
        translation.rust
    );
}

#[test]
fn current_self_equals_erased_object_uses_identity_downcast_ref() {
    let translation = translate_source(
        r#"
class CurrentSelfEqualsErasedObject {
    private String name;

    CurrentSelfEqualsErasedObject(String name) {
        this.name = name;
    }

    public boolean equals(Object obj) {
        if (this == obj) {
            return true;
        }
        return false;
    }
}
"#,
    );

    assert!(
        (translation.rust.contains(
            "obj.as_ref().and_then(|value| value.downcast_ref::<CurrentSelfEqualsErasedObject>()).is_some_and(|other| std::ptr::eq(self, other))"
        ) || translation.rust.contains(
            "obj.downcast_ref::<CurrentSelfEqualsErasedObject>().is_some_and(|other| std::ptr::eq(self, other))"
        )) && !translation.rust.contains("self == obj"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "current_self_equals_erased_object",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
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
fn current_get_class_vs_erased_object_get_class_uses_type_predicate() {
    let translation = translate_source(
        r#"
class CurrentGetClassVsErasedObjectGetClass {
    public boolean sameClass(Object obj) {
        if (obj == null) {
            return false;
        }
        if (getClass() != obj.getClass()) {
            return false;
        }
        return true;
    }
}
"#,
    );

    assert!(
        translation.rust.contains(
            "obj.as_ref().is_some_and(|value| value.is::<CurrentGetClassVsErasedObjectGetClass>())"
        ) && translation.rust.contains("if !(obj.as_ref().is_some_and")
            && !translation.rust.contains("get_class()")
            && !translation.rust.contains(".get_class()"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "current_get_class_vs_erased_object_get_class",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn exact_class_guard_casts_erased_object_to_borrowed_view() {
    let translation = translate_source(
        r#"
class ExactClassGuardCastsErasedObject {
    private int id;

    ExactClassGuardCastsErasedObject(int id) {
        this.id = id;
    }

    boolean sameId(int otherId) {
        return id == otherId;
    }

    public boolean equals(Object obj) {
        if (this == obj) {
            return true;
        }
        if (obj == null) {
            return false;
        }
        if (getClass() != obj.getClass()) {
            return false;
        }
        ExactClassGuardCastsErasedObject other = (ExactClassGuardCastsErasedObject)obj;
        return other.sameId(id);
    }
}
"#,
    );

    assert!(
        translation.rust.contains(
            "let other: &ExactClassGuardCastsErasedObject = obj.as_ref().and_then(|value| value.downcast_ref::<ExactClassGuardCastsErasedObject>()).expect(\"invalid Java cast\");"
        ) && !translation.rust.contains("obj.unwrap().downcast::<ExactClassGuardCastsErasedObject>")
            && !translation.rust.contains("*obj.unwrap().downcast")
            && !translation.rust.contains("get_class()"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "exact_class_guard_casts_erased_object",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn documents_equals_and_objects_gap() {
    let translation = translate_source(&idiom_gap("EqualsAndObjects.java"));

    assert!(
        translation.rust.contains("return left == right;")
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
            .contains("return left.as_ref() == right.as_ref();")
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
            .contains("Some(sample_name.as_ref().unwrap().trim()) == sample_name.as_ref().map(|value| value.as_str())")
            && !translation.rust.contains("Option<&&str>")
            && !translation
                .rust
                .contains("Some(&sample_name.as_ref().unwrap().trim()) == sample_name.as_ref()"),
        "{}",
        translation.rust
    );
}

#[test]
fn borrowed_nullable_trim_equals_original_uses_optional_string_view_policy() {
    let translation = translate_source(
        r#"
class BorrowedNullableTrimEqualsOriginal {
    boolean legal(String sampleName) {
        return sampleName != null && sampleName.trim().equals(sampleName);
    }

    boolean forward(String sampleName) {
        return legal(sampleName);
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("Some(sample_name.as_ref().unwrap().trim()) == sample_name.as_ref().map(|value| value.as_str())")
            && !translation.rust.contains("Some(sample_name.as_ref().unwrap().trim()) == sample_name.as_deref()")
            && !translation.rust.contains("Option<&&str>"),
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
        (translation.rust.contains("return left.equals(right);")
            || translation.rust.contains("return left.equals(&right);"))
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
            && (translation.rust.contains("left.equals(right)")
                || translation.rust.contains("left.equals(&right)"))
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
        assert!(!fixture.same(Some(&NullableValue::new(1)), None));
        assert!(!fixture.same(None, Some(&NullableValue::new(1))));
        assert!(fixture.same(Some(&NullableValue::new(7)), Some(&NullableValue::new(7))));
        assert!(!fixture.same(Some(&NullableValue::new(7)), Some(&NullableValue::new(8))));
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
        translation.rust.contains(
            "#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]\npub enum EnumEqualityComparisonKind"
        ) && translation.rust.contains("return left == right;")
            && translation.rust.contains("return left != right;")
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
            || translation.rust.contains(".get()")
            || translation.rust.contains("keep(")
            || translation.rust.contains("render(")
            || translation.rust.contains("supplier()"),
        "{}",
        translation.rust
    );
}

#[test]
fn nonescaping_biconsumer_parameter_lowers_to_callback_slot() {
    let translation = translate_source(&idiom_gap("FunctionalBiConsumerSlot.java"));

    assert!(translation.rust.contains("|s, n|"), "{}", translation.rust);
    assert!(
        translation
            .rust
            .contains("mut callback: impl FnMut(String, Integer)")
            || translation
                .rust
                .contains("mut callback: impl FnMut(String, i32)"),
        "{}",
        translation.rust
    );
    assert!(
        translation.rust.contains("callback(name") && translation.rust.contains(", count)"),
        "{}",
        translation.rust
    );
    assert!(
        !translation.rust.contains("callback: BiConsumer"),
        "{}",
        translation.rust
    );
    assert!(
        !translation.rust.contains("callback.accept("),
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
            && translation.rust.contains("let __java_tail_arg_0 = n - 1;")
            && translation
                .rust
                .contains("let __java_tail_arg_1 = acc * (n as i64);")
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
                .contains("pub fn first_missing(&self, cursor: &mut Cursor) -> Option<&Row>")
            && translation.rust.contains("return row;")
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
fn do_while_preserves_body_and_trailing_condition() {
    let translation = translate_source(
        r#"
class DoWhileAssignedReturn {
    String next() { return "x"; }
    boolean blank(String value) { return value.isEmpty(); }

    String read(boolean skipBlankLines) {
        String line;
        do {
            line = next();
            if (line == null) return line;
        } while (skipBlankLines && blank(line));
        return line;
    }
}
"#,
    );

    assert!(
        translation.rust.contains("loop {")
            && translation.rust.contains("line = Some(self.next());")
            && translation.rust.contains("if line.is_none()")
            && translation
                .rust
                .contains("if !(skip_blank_lines && self.blank(line.clone()")
            && translation.rust.contains("break;")
            && translation.rust.contains("return line;")
            && !translation
                .rust
                .contains("let line: String;\n        return line;"),
        "{}",
        translation.rust
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
        (translation.rust.contains(
            "Err(ex) if ex.is_instance(&[\"MissingFileProblem\", \"RecoverableProblem\"])",
        ) || translation.rust.contains(
            "Err(ex) if ex.is_instance(&[\"RecoverableProblem\", \"MissingFileProblem\"])",
        )) && !translation
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
fn exception_constructor_message_argument_is_stringified() {
    let translation = translate_source(
        r#"
class ExceptionConstructorMessageArgument {
    void load() throws Exception {}

    void wrap() throws RuntimeException {
        try {
            load();
        } catch (Exception e) {
            throw new RuntimeException(e);
        }
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("JavaException::new(\"RuntimeException\", e.to_string())"),
        "{}",
        translation.rust
    );
}

#[test]
fn caught_checked_exception_rethrown_as_known_unchecked_does_not_leak_result() {
    let translation = translate_source(
        r#"
import htsjdk.samtools.SAMException;
import java.io.IOException;

class CaughtCheckedRethrownAsKnownUnchecked {
    static String encode(String value) {
        return write(new StringBuilder(16), value).toString();
    }

    static Appendable write(Appendable out, String value) {
        try {
            return out.append(value);
        } catch (IOException e) {
            throw new SAMException(e);
        }
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("pub fn encode(value: String) -> String")
            && (translation
                .rust
                .contains("pub fn write(out: Appendable, value: String) -> Appendable")
                || translation
                    .rust
                    .contains("pub fn write(out: &mut Appendable, value: String) -> Appendable"))
            && !translation
                .rust
                .contains("pub fn write(out: Appendable, value: String) -> Result<Appendable, JavaException>")
            && !translation
                .rust
                .contains("pub fn write(out: &mut Appendable, value: String) -> Result<Appendable, JavaException>")
            && !translation
                .rust
                .contains("Self::write(StringBuilder::new(16).into(), value)?")
            && !translation
                .rust
                .contains("Self::write(&mut StringBuilder::new(16).into(), value)?"),
        "{}",
        translation.rust
    );
}

#[test]
fn unchecked_direct_throw_in_plain_method_panics_instead_of_returning_err() {
    let translation = translate_source(
        r#"
class UncheckedThrowOnly {
    Object unsupported() {
        throw new UnsupportedOperationException("no");
    }

    int bad(boolean fail) {
        if (fail) {
            throw new IllegalArgumentException("bad");
        }
        return 7;
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("pub fn unsupported(&self) -> Box<dyn std::any::Any>")
            && translation.rust.contains("pub fn bad(&self, fail: bool) -> i32")
            && translation.rust.contains(
                "panic!(\"uncaught Java exception: {:?}\", JavaException::new(\"UnsupportedOperationException\""
            )
            && translation.rust.contains(
                "panic!(\"uncaught Java exception: {:?}\", JavaException::new(\"IllegalArgumentException\""
            )
            && !translation
                .rust
                .contains("return Err(JavaException::new(\"UnsupportedOperationException\"")
            && !translation
                .rust
                .contains("return Err(JavaException::new(\"IllegalArgumentException\""),
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
fn sam_record_static_byte_array_constants_emit_typed_facade_surfaces() {
    let translation = translate_source(
        r#"
import htsjdk.samtools.SAMRecord;

class SamRecordStaticByteArrayConstants {
    byte[] nullSequence() {
        return SAMRecord.NULL_SEQUENCE;
    }

    byte[] nullQuals() {
        return SAMRecord.NULL_QUALS;
    }
}
"#,
    );

    assert!(
        translation
            .diagnostics
            .iter()
            .all(|diagnostic| diagnostic.code != "external_pack_missing_field"),
        "{:?}\n{}",
        translation.diagnostics.iter().collect::<Vec<_>>(),
        translation.rust
    );
    assert!(
        translation
            .rust
            .contains("pub const NULL_SEQUENCE: Vec<i8> = Vec::new();")
            && translation
                .rust
                .contains("pub const NULL_QUALS: Vec<i8> = Vec::new();")
            && translation
                .rust
                .contains("return SAMRecord::NULL_SEQUENCE;")
            && translation.rust.contains("return SAMRecord::NULL_QUALS;"),
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
            && translation
                .rust
                .contains("panic!(\"uncaught Java exception: {:?}\", ex);")
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
        assert_eq!(fixture.validate(false), ());
        let panic = std::panic::catch_unwind(|| fixture.validate(true));
        assert!(panic.is_err());
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
        assert_eq!(fixture.parse(false), 7);
        assert_eq!(fixture.parse(true), 1);
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
fn emitted_type_gets_source_summary_inherited_static_final_constant() {
    let target = r#"
class ChildWriter extends BaseWriter {
}

class UsesInheritedStaticConstant {
    int queueSize() {
        return ChildWriter.DEFAULT_QUEUE_SIZE;
    }
}
"#;
    let summary = r#"
class BaseWriter {
    public static final int DEFAULT_QUEUE_SIZE = 2000;
}
"#;

    let translation = translate_source_with_context_summary_sources(
        target,
        &[summary.to_string()],
        &FactSet::default(),
    );

    assert!(
        translation
            .rust
            .contains("pub const DEFAULT_QUEUE_SIZE: i32 = 2000;")
            && translation
                .rust
                .contains("return ChildWriter::DEFAULT_QUEUE_SIZE;"),
        "{}",
        translation.rust
    );
}

#[test]
fn emitted_type_gets_external_super_static_final_constant_surface() {
    let target = r#"
class ChildWriter extends ExternalBaseWriter {
}

class UsesInheritedExternalStaticConstant {
    int queueSize() {
        return ChildWriter.DEFAULT_QUEUE_SIZE;
    }
}
"#;
    let mut facts = FactSet::default();
    facts.external_field_facts.push(ExternalFieldFact {
        field: FieldKey {
            type_name: "ExternalBaseWriter".to_string(),
            field_name: "DEFAULT_QUEUE_SIZE".to_string(),
        },
        field_type: Some(JavaType::Primitive(PrimitiveType::Int)),
        field_kind: ExternalFieldKind::StaticFinal,
        provenance: FactProvenance::UserOverride,
    });

    let translation = translate_source_with_facts(target, &facts);

    assert!(
        translation
            .rust
            .contains("pub const DEFAULT_QUEUE_SIZE: i32 = 0;")
            && translation
                .rust
                .contains("return ChildWriter::DEFAULT_QUEUE_SIZE;"),
        "{}",
        translation.rust
    );
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
            .contains("return self.inherited_count + self.local_count;"),
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
