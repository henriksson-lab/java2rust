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
    cargo_verify_generated_modules(crate_name, modules, "check");
}

fn cargo_test_generated_modules(crate_name: &str, modules: Vec<GeneratedCrateModule>) {
    cargo_verify_generated_modules(crate_name, modules, "test");
}

fn cargo_verify_generated_modules(
    crate_name: &str,
    modules: Vec<GeneratedCrateModule>,
    cargo_command: &str,
) {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let crate_dir = std::env::temp_dir().join(format!("new_java2rust_{crate_name}_{unique}"));
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
        .arg(cargo_command)
        .arg("--tests")
        .arg("--quiet")
        .current_dir(&crate_dir)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "generated crate {cargo_command} failed for {crate_name}\nstdout:\n{}\nstderr:\n{}",
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
fn rust_keyword_local_names_render_as_raw_identifiers() {
    let translation = translate_source(
        r#"
class RustKeywordLocalNames {
    int echo(int in) {
        int match = in + 1;
        return match;
    }
}
"#,
    );

    assert!(
        translation.rust.contains("r#in: i32")
            && translation.rust.contains("let match_: i32 = r#in + 1;")
            && translation.rust.contains("return match_;")
            && !translation.rust.contains("let match:")
            && !translation.rust.contains("let in:"),
        "{}",
        translation.rust
    );
    cargo_check_generated_modules(
        "rust_keyword_local_names",
        vec![GeneratedCrateModule {
            name: "keyword_local_names",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn external_facade_keyword_method_names_render_as_raw_identifiers() {
    let translation = translate_source(
        r#"
import java.util.concurrent.CountDownLatch;

class ExternalFacadeKeywordMethodNames {
    void waitFor(CountDownLatch latch) throws InterruptedException {
        latch.await();
    }
}
"#,
    );

    assert!(
        translation.rust.contains("pub fn r#await(&self)")
            && translation.rust.contains("latch.r#await()")
            && !translation.rust.contains("pub fn await(")
            && !translation.rust.contains("latch.await()"),
        "{}",
        translation.rust
    );
    cargo_check_generated_modules(
        "external_facade_keyword_method_names",
        vec![GeneratedCrateModule {
            name: "external_facade_keyword_method_names",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn source_static_mutable_fields_use_generated_storage_accessors() {
    let translation = translate_source(
        r#"
class SourceStaticMutableFields {
    private static int count = 1;

    static int increment() {
        count = count + 1;
        return count;
    }
}
"#,
    );

    assert!(
        translation.rust.contains("std::thread_local!")
            && translation
                .rust
                .contains("static SOURCE_STATIC_MUTABLE_FIELDS_COUNT")
            && translation.rust.contains("fn __static_count() -> i32")
            && translation.rust.contains("fn __set_static_count(")
            && translation
                .rust
                .contains("Self::__set_static_count(Self::__static_count() + 1);")
            && translation.rust.contains("return Self::__static_count();")
            && !translation.rust.contains("count = count + 1;")
            && !translation.rust.contains("let count"),
        "{}",
        translation.rust
    );
    cargo_check_generated_modules(
        "source_static_mutable_fields",
        vec![GeneratedCrateModule {
            name: "source_static_mutable_fields",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn lowercase_static_owner_field_write_uses_static_storage_setter() {
    let translation = translate_source(
        r#"
class LowercaseStaticOwnerFieldWrite {
    private static LowercaseStaticOwnerFieldWrite holder = new LowercaseStaticOwnerFieldWrite();
    private static boolean enabled = false;

    static void setEnabled(boolean value) {
        holder.enabled = value;
    }

    static boolean isEnabled() {
        return enabled;
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("Self::__set_static_enabled(value);")
            && translation
                .rust
                .contains("return Self::__static_enabled();")
            && !translation.rust.contains("holder.enabled = value")
            && !translation.rust.contains("holder:"),
        "{}",
        translation.rust
    );
    cargo_check_generated_modules(
        "lowercase_static_owner_field_write",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn static_update_expressions_route_through_storage_setters() {
    let translation = translate_source(
        r#"
class OtherStaticCounter {
    static int c = 0;
}

class StaticUpdateExpressions {
    static int shared = 0;
    static byte narrow = 10;

    static int bumpAll() {
        shared = 0;
        OtherStaticCounter.c = 0;
        int old = shared++;
        int now = ++shared;
        shared--;
        narrow += 300;
        OtherStaticCounter.c++;
        return old + now + shared + narrow + OtherStaticCounter.c;
    }
}
"#,
    );

    assert!(
        translation.rust.contains("Self::__set_static_shared(")
            && translation.rust.contains("Self::__static_shared()")
            && translation
                .rust
                .contains("OtherStaticCounter::__set_static_c(")
            && translation
                .rust
                .contains("OtherStaticCounter::__static_c()")
            && !translation.rust.contains("Self::__static_shared() +=")
            && !translation
                .rust
                .contains("OtherStaticCounter::__static_c() +="),
        "{}",
        translation.rust
    );
    cargo_check_generated_modules(
        "static_update_expressions",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn public_readonly_string_param_uses_str_view_for_static_string_constants() {
    let translation = translate_source(
        r#"
class PublicReadonlyStringParamStaticConstant {
    private static final String RESOURCE = "example/resource.txt";

    public static int score(String path) {
        return path.isEmpty() ? 0 : path.length();
    }

    public static int call() {
        return score(RESOURCE);
    }
}
"#,
    );

    assert!(
        translation.rust.contains("pub fn score(path: &str) -> i32")
            && translation
                .rust
                .contains("return Self::score(Self::RESOURCE);")
            && !translation.rust.contains("path: &String")
            && !translation.rust.contains("Self::RESOURCE.to_string()"),
        "{}",
        translation.rust
    );
    cargo_check_generated_modules(
        "public_readonly_string_param_static_constant",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn java_util_logging_level_import_uses_shared_level_facade() {
    let translation = translate_source(
        r#"
import java.util.logging.Level;

class LoggingLevelImport {
    static Level warning() {
        return Level.WARNING;
    }
}
"#,
    );

    assert!(
        translation.rust.contains("-> Level")
            && translation.rust.contains("return Level::WARNING;")
            && !translation.rust.contains("JavaUtilLoggingLevel"),
        "{}",
        translation.rust
    );
    cargo_check_generated_modules(
        "logging_level_import",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn java_list_iterator_cursor_uses_platform_facade() {
    let translation = translate_source(
        r#"
import java.util.ArrayList;
import java.util.Iterator;
import java.util.List;

class JavaIteratorCursorOverList {
    static List<String> copy(List<String> input) {
        List<String> out = new ArrayList<String>();
        for (Iterator<String> i = input.iterator(); i.hasNext();) {
            out.add(i.next());
        }
        return out;
    }

    static List<String> copyRemaining(List<String> input) {
        List<String> out = new ArrayList<String>();
        Iterator<String> i = input.iterator();
        i.forEachRemaining(value -> {
            out.add(value);
        });
        return out;
    }
}
"#,
    );

    assert!(
        translation.rust.contains(
            "let mut i: JavaIterator<String> = JavaIterator::new(input.iter().cloned());"
        ) && translation.rust.contains("while i.has_next()")
            && translation.rust.contains("out.push(i.next());")
            && translation.rust.contains("i.for_each_remaining(|value|")
            && translation.rust.contains("out.push(value);")
            && !translation.rust.contains("let mut i: Iterator")
            && !translation.rust.contains(".iterator()"),
        "{}",
        translation.rust
    );
    cargo_check_generated_modules(
        "java_list_iterator_cursor",
        vec![GeneratedCrateModule {
            name: "java_list_iterator_cursor",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn loop_body_reuses_owned_value_with_back_edge_clone() {
    let translation = translate_source(
        r#"
class LoopMove {
    private String last = "";

    void add(String s) {
        last = s;
    }

    void run(String value, int n) {
        for (int i = 0; i < n; i++) {
            add(value);
        }
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("self.add(value.clone()/* TODO(translation): validate added clone */);"),
        "{}",
        translation.rust
    );
    cargo_check_generated_modules(
        "loop_body_reuses_owned_value_with_back_edge_clone",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn iterator_foreachremaining_bound_receiver_method_reference_compiles() {
    let translation = translate_source(
        r#"
import htsjdk.samtools.util.Interval;
import htsjdk.samtools.util.IntervalList;
import java.util.Iterator;

class BoundReceiverMethodReferenceCaller {
    void copyRemaining(Iterator<Interval> intervals, IntervalList intervalList) {
        intervals.forEachRemaining(intervalList::add);
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("intervals.for_each_remaining(|value| interval_list.add(value))")
            && !translation.rust.contains("intervalList::add")
            && !translation.rust.contains("interval_list::add"),
        "{}",
        translation.rust
    );
    cargo_check_generated_modules(
        "iterator_foreachremaining_bound_receiver_method_reference",
        vec![GeneratedCrateModule {
            name: "iterator_foreachremaining_bound_receiver_method_reference",
            rust: translation.rust,
            tests: "",
        }],
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
            .contains("left.to_lowercase().as_str() == right.to_lowercase().as_str()"),
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
fn java_string_uppercase_char_array_and_replace_use_rust_string_apis() {
    let translation = translate_source(
        r#"
class JavaStringUppercaseCharArrayReplace {
    String normalize(String value) {
        return value.toUpperCase().replace("X", "23");
    }

    int count(String value) {
        char[] chars = value.toCharArray();
        return chars.length;
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("value.to_uppercase().replace(\"X\", \"23\")")
            && translation
                .rust
                .contains("let chars: Vec<char> = value.chars().collect::<Vec<char>>();")
            && !translation.rust.contains(".to_upper_case(")
            && !translation.rust.contains(".to_char_array(")
            && !translation.rust.contains(".replace(\"X\".to_string()"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "java_string_uppercase_char_array_replace",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn java_string_char_at_lowers_to_rust_chars_nth() {
    let translation = translate_source(
        r#"
class JavaStringCharAtUse {
    char first(String value) {
        return value.charAt(0);
    }

    char at(String value, int index) {
        return value.charAt(index);
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("value.chars().nth(0 as usize).expect(\"Java string index out of bounds\")")
            && translation.rust.contains(
                "value.chars().nth(index as usize).expect(\"Java string index out of bounds\")"
            )
            && !translation.rust.contains(".char_at("),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "java_string_char_at",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn java_string_tokenizer_uses_facade_signature_facts() {
    let translation = translate_source(
        r#"
import java.util.StringTokenizer;

class JavaStringTokenizerUse {
    static int count(String text) {
        StringTokenizer tokenizer = new StringTokenizer(text, "|");
        int total = tokenizer.countTokens();
        while (tokenizer.hasMoreTokens()) {
            char first = tokenizer.nextToken().charAt(0);
            if (first == 'x') {
                total++;
            }
        }
        return total;
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("let mut tokenizer: StringTokenizer = StringTokenizer::new")
            && translation
                .rust
                .contains("let mut total: i32 = tokenizer.count_tokens();")
            && translation
                .rust
                .contains("while tokenizer.has_more_tokens()")
            && translation
                .rust
                .contains("let first: char = tokenizer.next_token().chars().nth(0 as usize)")
            && !translation.rust.contains("has_more_tokens() { ()")
            && !translation.rust.contains("next_token() -> ()"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "java_string_tokenizer_facade_signature",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn java_string_join_varargs_lowers_to_vec_join() {
    let translation = translate_source(
        r#"
class JavaStringJoinVarargs {
    static String classpath(String sep, String extra) {
        return String.join(sep, "bin", ".", extra, "htsjdk.jar");
    }
}
"#,
    );

    assert!(
        translation.rust.contains(
            "return vec![\"bin\".to_string(), \".\".to_string(), (extra).to_string(), \"htsjdk.jar\".to_string()].join(sep);"
        ) && !translation
            .rust
            .contains("String.join varargs are not supported yet"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "java_string_join_varargs",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
#[cfg(test)]
mod generated_tests {
    use super::*;

    #[test]
    fn joins_varargs() {
        let sep = ":".to_string();
        let extra = "classes".to_string();
        assert_eq!(
            JavaStringJoinVarargs::classpath(&sep, &extra),
            "bin:.:classes:htsjdk.jar".to_string()
        );
    }
}
"#,
        }],
    );
}

#[test]
fn jdk_numeric_and_decimal_format_facades_keep_precise_return_shapes() {
    let translation = translate_source(
        r#"
import java.text.DecimalFormat;
import static java.lang.Math.sqrt;

class JdkNumericAndFormatFacades {
    static double log2(double value) {
        return Math.log(value) / Math.log(2);
    }

    static double exponent(double value) {
        return Math.exp(value);
    }

    static double root(double value) {
        return Math.sqrt(value);
    }

    static double square(double value) {
        return Math.pow(value, 2.0);
    }

    static double log10(double value) {
        return Math.log10(value);
    }

    static double ceil(double value) {
        return Math.ceil(value);
    }

    static double floor(double value) {
        return Math.floor(value);
    }

    static double trig(double value) {
        return Math.sin(value) + Math.cos(value) + Math.tan(value);
    }

    static double inverse(double value) {
        return Math.asin(value) + Math.acos(value) + Math.atan(value);
    }

    static double hyperbolic(double value) {
        return Math.sinh(value) + Math.cosh(value) + Math.tanh(value);
    }

    static double shapedMore(double x, double y) {
        return Math.cbrt(x) + Math.rint(x) + Math.signum(x) + Math.atan2(x, y) + Math.hypot(x, y);
    }

    static double angle(double value) {
        return Math.toDegrees(value) + Math.toRadians(value);
    }

    static double importedSqrt(double value) {
        return sqrt(value);
    }

    static long rounded(double value) {
        return Math.round(value);
    }

    static long roundedEdge() {
        return Math.round(0.49999999999999994);
    }

    static int roundedFloat(float value) {
        return Math.round(value);
    }

    static int absInt(int value) {
        return Math.abs(value);
    }

    static long absLong(long value) {
        return Math.abs(value);
    }

    static int maxInt(int left, int right) {
        return Math.max(left, right);
    }

    static double maxDouble(double left, double right) {
        return Math.max(left, right);
    }

    static double minDouble(double left, double right) {
        return Math.min(left, right);
    }

    static boolean missing(double value) {
        return Double.isNaN(value);
    }

    static boolean missingFloat(float value) {
        return Float.isNaN(value);
    }

    static float parseFloat(String value) {
        return Float.parseFloat(value);
    }

    static int exact(long value) {
        return Math.toIntExact(value);
    }

    static int floorDiv(int left, int right) {
        return Math.floorDiv(left, right);
    }

    static int floorMod(int left, int right) {
        return Math.floorMod(left, right);
    }

    static long floorDivLong(long left, long right) {
        return Math.floorDiv(left, right);
    }

    static long floorModLong(long left, long right) {
        return Math.floorMod(left, right);
    }

    static void collect() {
        System.gc();
    }

    static String render(double value) {
        DecimalFormat df = new DecimalFormat("0.0");
        return "v=" + df.format(value);
    }
}
"#,
    );

    assert!(
        translation.rust.contains("(value as f64).ln()")
            && translation.rust.contains("(2 as f64).ln()")
            && translation.rust.contains("(value as f64).exp()")
            && translation.rust.contains("(value as f64).sqrt()")
            && translation.rust.contains(
                "pub fn imported_sqrt(value: f64) -> f64 {\n        return (value as f64).sqrt();"
            )
            && translation
                .rust
                .contains("(value as f64).powf(2f64 as f64)")
            && translation.rust.contains("(value as f64).log10()")
            && translation.rust.contains("(value as f64).ceil()")
            && translation.rust.contains("(value as f64).floor()")
            && translation.rust.contains("(value as f64).sin()")
            && translation.rust.contains("(value as f64).cos()")
            && translation.rust.contains("(value as f64).tan()")
            && translation.rust.contains("(value as f64).asin()")
            && translation.rust.contains("(value as f64).acos()")
            && translation.rust.contains("(value as f64).atan()")
            && translation.rust.contains("(value as f64).sinh()")
            && translation.rust.contains("(value as f64).cosh()")
            && translation.rust.contains("(value as f64).tanh()")
            && translation.rust.contains("(x as f64).cbrt()")
            && translation.rust.contains("(x as f64).round_ties_even()")
            && translation.rust.contains("(x as f64).signum()")
            && translation.rust.contains("(x as f64).atan2(y as f64)")
            && translation.rust.contains("(x as f64).hypot(y as f64)")
            && translation.rust.contains("(value as f64).to_degrees()")
            && translation.rust.contains("(value as f64).to_radians()")
            && translation.rust.contains("java_math_round_f64(value)")
            && translation.rust.contains("java_math_round_f32(value)")
            && translation.rust.contains("(value).wrapping_abs()")
            && translation.rust.contains("std::cmp::max(left, right)")
            && translation.rust.contains("java_math_max_f64(left, right)")
            && translation.rust.contains("java_math_min_f64(left, right)")
            && translation.rust.contains("value.is_nan()")
            && translation.rust.contains("value.parse::<f32>().unwrap()")
            && translation.rust.contains("return value as i32;")
            && translation
                .rust
                .contains("java_math_floor_div_i32(left, right)")
            && translation
                .rust
                .contains("java_math_floor_mod_i32(left, right)")
            && translation
                .rust
                .contains("java_math_floor_div_i64(left, right)")
            && translation
                .rust
                .contains("java_math_floor_mod_i64(left, right)")
            && translation.rust.contains("pub fn collect() {\n    }")
            && translation.rust.contains("value.to_string()")
            && !translation.rust.contains("Math::log")
            && !translation.rust.contains("Math::exp")
            && !translation.rust.contains("Math::sqrt")
            && !translation.rust.contains("Math::pow")
            && !translation.rust.contains("Math::log10")
            && !translation.rust.contains("Math::ceil")
            && !translation.rust.contains("Math::floor")
            && !translation.rust.contains("Math::sin")
            && !translation.rust.contains("Math::atan2")
            && !translation.rust.contains("Math::hypot")
            && !translation.rust.contains("Math::round")
            && !translation.rust.contains("Math::to_int_exact")
            && !translation.rust.contains("pub struct Math")
            && !translation.rust.contains("pub fn pow")
            && !translation.rust.contains("pub fn sqrt")
            && !translation.rust.contains("math.floor_div")
            && !translation.rust.contains("math.floor_mod")
            && !translation.rust.contains("Double::is_na_n")
            && !translation.rust.contains("Float::is_na_n")
            && !translation.rust.contains("float.parse_float")
            && !translation.rust.contains("system.gc")
            && !translation.rust.contains("fn log(")
            && !translation.rust.contains("fn exp("),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "jdk_numeric_and_decimal_format_facades",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
#[cfg(test)]
mod generated_tests {
    use super::*;

    #[test]
    fn numeric_facades_typecheck_and_run() {
        assert!((JdkNumericAndFormatFacades::log2(8.0) - 3.0).abs() < 1e-9);
        assert!((JdkNumericAndFormatFacades::exponent(1.0) - std::f64::consts::E).abs() < 1e-9);
        assert_eq!(JdkNumericAndFormatFacades::root(9.0), 3.0);
        assert_eq!(JdkNumericAndFormatFacades::square(4.0), 16.0);
        assert_eq!(JdkNumericAndFormatFacades::log10(1000.0), 3.0);
        assert_eq!(JdkNumericAndFormatFacades::ceil(1.25), 2.0);
        assert_eq!(JdkNumericAndFormatFacades::floor(1.75), 1.0);
        assert!((JdkNumericAndFormatFacades::trig(0.25)
            - (0.25f64.sin() + 0.25f64.cos() + 0.25f64.tan()))
        .abs()
            < 1e-9);
        assert!((JdkNumericAndFormatFacades::inverse(0.25)
            - (0.25f64.asin() + 0.25f64.acos() + 0.25f64.atan()))
        .abs()
            < 1e-9);
        assert!((JdkNumericAndFormatFacades::hyperbolic(0.25)
            - (0.25f64.sinh() + 0.25f64.cosh() + 0.25f64.tanh()))
        .abs()
            < 1e-9);
        assert!((JdkNumericAndFormatFacades::shaped_more(8.0, 6.0)
            - (8.0f64.cbrt()
                + 8.0f64.round_ties_even()
                + 8.0f64.signum()
                + 8.0f64.atan2(6.0)
                + 8.0f64.hypot(6.0)))
        .abs()
            < 1e-9);
        assert!((JdkNumericAndFormatFacades::angle(std::f64::consts::PI)
            - (std::f64::consts::PI.to_degrees() + std::f64::consts::PI.to_radians()))
        .abs()
            < 1e-9);
        assert_eq!(JdkNumericAndFormatFacades::imported_sqrt(16.0), 4.0);
        assert_eq!(JdkNumericAndFormatFacades::rounded(-2.5), -2);
        assert_eq!(JdkNumericAndFormatFacades::rounded(-0.5), 0);
        assert_eq!(JdkNumericAndFormatFacades::rounded_edge(), 0);
        assert_eq!(JdkNumericAndFormatFacades::rounded_float(-2.5), -2);
        assert_eq!(JdkNumericAndFormatFacades::abs_int(i32::MIN), i32::MIN);
        assert_eq!(JdkNumericAndFormatFacades::abs_long(i64::MIN), i64::MIN);
        assert_eq!(JdkNumericAndFormatFacades::max_int(1, 2), 2);
        assert!(JdkNumericAndFormatFacades::max_double(f64::NAN, 1.0).is_nan());
        assert!(JdkNumericAndFormatFacades::min_double(-0.0, 0.0).is_sign_negative());
        assert!(JdkNumericAndFormatFacades::missing(f64::NAN));
        assert_eq!(JdkNumericAndFormatFacades::floor_div(-7, 2), -4);
        assert_eq!(JdkNumericAndFormatFacades::floor_mod(-7, 2), 1);
        assert_eq!(JdkNumericAndFormatFacades::floor_mod(7, -2), -1);
        assert_eq!(JdkNumericAndFormatFacades::floor_div(i32::MIN, -1), i32::MIN);
        assert_eq!(JdkNumericAndFormatFacades::floor_mod(i32::MIN, -1), 0);
        assert_eq!(JdkNumericAndFormatFacades::floor_div_long(-7, 2), -4);
        assert_eq!(JdkNumericAndFormatFacades::floor_mod_long(-7, 2), 1);
        assert_eq!(JdkNumericAndFormatFacades::floor_div_long(i64::MIN, -1), i64::MIN);
        assert_eq!(JdkNumericAndFormatFacades::floor_mod_long(i64::MIN, -1), 0);
        assert!(JdkNumericAndFormatFacades::render(1.25).starts_with("v="));
    }
}
"#,
        }],
    );
}

#[test]
fn apache_fast_math_uses_javalibs_native_math_alias() {
    let translation = translate_source(
        r#"
import org.apache.commons.math3.util.FastMath;

class ApacheFastMathUse {
    static double log2(double value) {
        return FastMath.log(value) / FastMath.log(2);
    }

    static double clipped(double left, double right) {
        return FastMath.max(left, right);
    }

    static int rounded(float value) {
        return FastMath.round(value);
    }

    static double shaped(double value) {
        return FastMath.sqrt(value) + FastMath.pow(value, 2.0) + FastMath.ceil(value) + FastMath.floor(value);
    }

    static double trig(double value) {
        return FastMath.sin(value) + FastMath.cos(value) + FastMath.atan(value);
    }
}
"#,
    );

    assert!(
        translation.rust.contains("(value as f64).ln()")
            && translation.rust.contains("(2 as f64).ln()")
            && translation.rust.contains("java_math_max_f64(left, right)")
            && translation.rust.contains("java_math_round_f32(value)")
            && translation.rust.contains("(value as f64).sqrt()")
            && translation
                .rust
                .contains("(value as f64).powf(2f64 as f64)")
            && translation.rust.contains("(value as f64).ceil()")
            && translation.rust.contains("(value as f64).floor()")
            && translation.rust.contains("(value as f64).sin()")
            && translation.rust.contains("(value as f64).cos()")
            && translation.rust.contains("(value as f64).atan()")
            && !translation.rust.contains("FastMath::")
            && !translation.rust.contains("fast_math.")
            && !translation
                .rust
                .contains("OrgApacheCommonsMath3UtilFastMath::"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "apache_fast_math_use",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn jdk_decimal_and_integer_constants_are_typed_facade_facts() {
    let translation = translate_source(
        r#"
import java.math.BigDecimal;
import java.math.BigInteger;

class JdkDecimalAndIntegerConstants {
    static double rounded(double value, int scale) {
        return new BigDecimal(Double.toString(value))
            .setScale(scale, BigDecimal.ROUND_HALF_UP)
            .doubleValue();
    }

    static int roundingFlag(boolean up) {
        return up ? BigDecimal.ROUND_UP : BigDecimal.ROUND_DOWN;
    }

    static BigInteger one() {
        return BigInteger.ONE;
    }

    static boolean nonNegative(BigInteger value) {
        return value.compareTo(BigInteger.ZERO) >= 0;
    }

    static BigInteger squared(BigInteger value) {
        return value.multiply(value);
    }

    static BigInteger shifted(BigInteger value) {
        return value.shiftRight(1);
    }

    static BigInteger exponent(BigInteger value, int power) {
        return value.pow(power);
    }

    static boolean lowestBit(BigInteger value) {
        return value.testBit(0);
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("pub const ROUND_HALF_UP: i32 = 4;")
            && translation.rust.contains("pub const ROUND_UP: i32 = 0;")
            && translation.rust.contains("pub const ROUND_DOWN: i32 = 1;")
            && translation
                .rust
                .contains("pub const ONE: BigInteger = BigInteger;")
            && translation
                .rust
                .contains("pub const ZERO: BigInteger = BigInteger;")
            && translation
                .rust
                .contains(".set_scale(scale, BigDecimal::ROUND_HALF_UP)")
            && translation.rust.contains(".double_value()")
            && translation.rust.contains(".multiply(")
            && translation.rust.contains(".shift_right(1)")
            && translation.rust.contains(".pow(power)")
            && translation.rust.contains(".test_bit(0)")
            && !translation.rust.contains(".downcast::<BigInteger>()")
            && !translation
                .rust
                .contains("set_scale<T0, T1>(&self, _arg0: T0, _arg1: T1) -> ()")
            && !translation
                .rust
                .contains("multiply<T0>(&self, _arg0: T0) -> ()")
            && !translation
                .rust
                .contains("shift_right<T0>(&self, _arg0: T0) -> ()")
            && !translation.rust.contains("pow<T0>(&self, _arg0: T0) -> ()")
            && !translation
                .rust
                .contains("test_bit<T0>(&self, _arg0: T0) -> ()"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "jdk_decimal_and_integer_constants",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn boxed_primitive_constructors_lower_to_values() {
    let translation = translate_source(
        r#"
class BoxedPrimitiveConstructorsLowerToValues {
    static float keepFloat(float value) {
        return new Float(value).floatValue();
    }

    static double keepDouble(double value) {
        return new Double(value).doubleValue();
    }

    static double floatAsDouble(float value) {
        return new Float(value).doubleValue();
    }

    static float doubleAsFloat(double value) {
        return new Double(value).floatValue();
    }

    static int hashFloat(float value) {
        return new Float(value).hashCode();
    }

    static int hashDouble(double value) {
        return new Double(value).hashCode();
    }

    static int compareInts(int left, int right) {
        return new Integer(left).compareTo(new Integer(right));
    }

    static int compareParsedInts(String left, String right) {
        return new Integer(left).compareTo(new Integer(right));
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("pub fn keep_float(value: f32) -> f32 {\n        return value;")
            && translation
                .rust
                .contains("pub fn keep_double(value: f64) -> f64 {\n        return value;")
            && translation.rust.contains(
                "pub fn float_as_double(value: f32) -> f64 {\n        return value as f64;"
            )
            && translation.rust.contains(
                "pub fn double_as_float(value: f64) -> f32 {\n        return value as f32;"
            )
            && !translation.rust.contains("f32::new")
            && !translation.rust.contains("f64::new")
            && !translation.rust.contains("Float::new")
            && !translation.rust.contains("Double::new")
            && translation
                .rust
                .contains("java_primitive_compare_to(left, right)")
            && translation.rust.contains(
                "java_primitive_compare_to(java_parse_i32(&left), java_parse_i32(&right))"
            )
            && !translation.rust.contains(".hash_code()")
            && !translation.rust.contains(".compare_to(")
            && !translation.rust.contains("Integer::new"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "boxed_primitive_constructors_lower_to_values",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn explicit_primitive_casts_survive_numeric_binary_lowering() {
    let translation = translate_source(
        r#"
class ExplicitPrimitiveCastNumericBinary {
    float scale(double ratio, float value) {
        float adjusted = (float) ratio * (float) value;
        return adjusted;
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("let adjusted: f32 = (ratio as f32) * (value as f32);"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "explicit_primitive_casts_survive_numeric_binary_lowering",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn explicit_primitive_cast_applies_to_whole_shift_and_bitwise_expression() {
    let translation = translate_source(
        r#"
class ExplicitPrimitiveCastWholeShift {
    int exponent(long bits) {
        return (int)(bits >> 52) - 1023;
    }

    int masked(long bits) {
        return (int)((bits >> 52) & 2047L);
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("return ((bits >> 52) as i32) - 1023;")
            && translation
                .rust
                .contains("return (bits >> 52 & 2047i64) as i32;")
            && !translation.rust.contains("bits >> 52 as i32"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "explicit_primitive_cast_whole_shift",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn java_numeric_char_casts_use_valid_rust_char_conversions() {
    let translation = translate_source(
        r#"
class JavaNumericCharCasts {
    char fromInt(int value) {
        return (char) value;
    }

    double charDistance(int value, int offset) {
        return (double) ((char) (value - offset));
    }

    double inferredCharDistance(int value, int offset) {
        return (char) (value - offset);
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("char::from_u32(((value) as u32) & 0xffff).unwrap_or('\\0')")
            && translation.rust.contains(
                "((char::from_u32(((value - offset) as u32) & 0xffff).unwrap_or('\\0')) as u32) as f64"
            )
            && !translation.rust.contains("as char")
            && !translation.rust.contains("unwrap_or('\\0') as f64"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "java_numeric_char_casts_use_valid_rust_char_conversions",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn java_numeric_char_literals_adapt_to_char_slots_and_switch_labels() {
    let translation = translate_source(
        r#"
class NumericCharLiteralSlots {
    void clear(char[] values, int index) {
        values[index] = 0;
    }

    boolean whitespace(char value) {
        switch (value) {
            case 9:
            case 10:
            case 13:
            case 32:
                return true;
            default:
                return false;
        }
    }
}
"#,
    );

    assert!(
        translation.rust.contains("values[index as usize] = '\\0';")
            && translation.rust.contains("'\\t' | '\\n' | '\\r' | ' ' =>")
            && !translation.rust.contains("9 | 10 | 13 | 32 =>"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "java_numeric_char_literals_adapt_to_char_slots_and_switch_labels",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn decimal_format_argument_keeps_numeric_expression_before_string_concat() {
    let translation = translate_source(
        r#"
import java.text.DecimalFormat;

class DecimalFormatNumericExpressionConcat {
    String percent(double value) {
        DecimalFormat df = new DecimalFormat("0.0");
        return df.format(value * 100) + "%";
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("format!(\"{}%\", value * (100 as f64))")
            && !translation
                .rust
                .contains("value * (100 as f64).to_string()"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "decimal_format_argument_keeps_numeric_expression_before_string_concat",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn string_concat_preserves_parenthesized_numeric_addition() {
    let translation = translate_source(
        r#"
class StringConcatNestedArithmetic {
    String nested(int a, int b) {
        return "x" + (a + b);
    }

    String chain(int a, int b) {
        return "x" + a + b;
    }
}
"#,
    );

    assert!(
        translation.rust.contains("format!(\"x{}\", (a + b))")
            && translation
                .rust
                .contains("format!(\"x{}{}\", (a).to_string(), (b).to_string())")
            && !translation.rust.contains("format!(\"x{}{}\", a + b"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "string_concat_nested_arithmetic",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn labeled_statements_are_preserved_in_generated_rust() {
    let translation = translate_source(
        r#"
class LabeledStatementLowering {
    int first(int[] left, int[] right) {
        outer:
        for (int i = 0; i < left.length; i++) {
            for (int j = 0; j < right.length; j++) {
                if (left[i] != right[j]) {
                    continue outer;
                }
                return left[i];
            }
        }
        return -1;
    }

    int block(int[] values) {
        int found = -1;
        outer:
        {
            if (values.length == 0) {
                break outer;
            }
            found = 1;
        }
        return found;
    }
}
"#,
    );

    assert!(
        translation.rust.contains("'outer: loop")
            && translation.rust.contains("continue 'outer;")
            && translation
                .rust
                .contains("i += 1; __java_old_value };\n                    continue 'outer;")
            && translation.rust.contains("break 'outer;"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "labeled_statements_are_preserved",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn jdk_file_reader_and_system_facades_keep_control_flow_shapes() {
    let translation = translate_source(
        r#"
import java.io.BufferedReader;
import java.io.File;
import java.io.IOException;

class JdkFileReaderAndSystemFacades {
    static boolean filePresent(String path) {
        File file = new File(path);
        return file.exists();
    }

    static void cleanupOnExit(String path) {
        File file = new File(path);
        file.deleteOnExit();
    }

    static boolean readerReady(BufferedReader reader) throws IOException {
        return reader.ready();
    }

    static void stopIf(boolean stop) {
        if (stop) {
            System.exit(10);
        }
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("std::path::Path::new(file.get_path().as_str()).exists()")
            && translation.rust.contains("file.delete_on_exit();")
            && translation.rust.contains("return Ok(true);")
            && translation.rust.contains("std::process::exit(10 as i32)")
            && !translation.rust.contains("file.exists()")
            && !translation.rust.contains("file.deleteOnExit()")
            && !translation.rust.contains("reader.ready()")
            && !translation.rust.contains("System::exit(10"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "jdk_file_reader_and_system_facades",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn jdk_file_constructor_clones_borrowed_string_array_element() {
    let translation = translate_source(
        r#"
import java.io.File;

class JdkFileConstructorIndexedArg {
    static boolean present(String[] args) {
        File file = new File(args[1]);
        return file.exists();
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("File::new((args[1 as usize]).clone())")
            && !translation.rust.contains("File::new(args[1 as usize]);")
            && !translation.rust.contains("File::new((args[1 as usize]));"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "jdk_file_constructor_indexed_arg",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn jdk_file_constructor_overload_facade_supports_resolved_arity_names() {
    let translation = translate_source(
        r#"
import java.io.File;

class JdkFileConstructorOverloads {
    static File single(String path) {
        return new File(path);
    }

    static File child(String parent, String name) {
        return new File(parent, name);
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("pub fn new_1<P: JavaPathArg>(value: P) -> File")
            && translation.rust.contains(
                "pub fn new_2<P: JavaPathArg, C: JavaPathArg>(parent: P, child: C) -> File"
            )
            && (translation.rust.contains("File::new(path)")
                || translation.rust.contains("File::new_1(path)"))
            && translation.rust.contains("File::new_2(parent, name)"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "jdk_file_constructor_overload_facade",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn jdk_file_common_instance_facades_compile() {
    let translation = translate_source(
        r#"
import java.io.File;

class JdkFileCommonInstanceFacades {
    String describe(File file) {
        String parent = file.getParent();
        return file.getName() + ":" + parent + ":" + file.getAbsoluteFile().getPath();
    }

    boolean prepare(File file) {
        if (!file.isDirectory()) {
            return file.mkdir();
        }
        return true;
    }

    int readableChildren(File file) {
        int count = 0;
        String[] names = file.list();
        for (int i = 0; i < names.length; i++) {
            File child = new File(file.getPath() + "/" + names[i]);
            if (child.isFile() && child.canRead() && child.length() >= 0) {
                count++;
            }
        }
        return count;
    }
}
"#,
    );

    assert!(
        translation.rust.contains("file.get_name()")
            && translation.rust.contains("file.get_parent()")
            && translation
                .rust
                .contains("file.get_absolute_file().get_path()")
            && translation.rust.contains("file.is_directory()")
            && translation.rust.contains("file.mkdir()")
            && translation
                .rust
                .contains("let names: Vec<String> = file.list();")
            && translation
                .rust
                .contains("child.is_file() && child.can_read() && child.length() >= (0 as i64)"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "jdk_file_common_instance_facades",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn jdk_bitset_get_keeps_boolean_shape() {
    let translation = translate_source(
        r#"
import java.util.BitSet;

class JdkBitSetBooleanFacade {
    static boolean missing(BitSet bits, int index) {
        return !bits.get(index);
    }
}
"#,
    );

    assert!(
        translation.rust.contains("return !false;") && !translation.rust.contains(".get(index)"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "jdk_bitset_boolean_facade",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn java_string_regex_helpers_use_runtime_surfaces() {
    let translation = translate_source(
        r#"
class JavaStringRegexHelpers {
    boolean digit(String value) {
        return value.matches("[0-9]");
    }

    String stripPrefix(String value) {
        return value.replaceFirst("-", "");
    }

    String digits(String value) {
        return value.replaceAll("[^0-9]", "");
    }

    String[] parts(String value) {
        return value.replaceAll(".fast5$", "").split("_");
    }
}
"#,
    );

    assert!(
        translation.rust.contains("pub fn java_string_matches")
            && translation
                .rust
                .contains("java_string_matches(&value, \"[0-9]\")")
            && translation
                .rust
                .contains("java_string_replace_first(&value, \"-\", \"\")")
            && translation
                .rust
                .contains("java_string_replace_all(&value, \"[^0-9]\", \"\")")
            && translation.rust.contains(
                "java_string_split(&java_string_replace_all(&value, \".fast5$\", \"\"), \"_\", 0)"
            )
            && !translation.rust.contains(".replace_first(")
            && !translation.rust.contains(".replace_all(")
            && !translation.rust.contains(".matches(\"[0-9]\".to_string())")
            && !translation.rust.contains(".split(\"_\".to_string())"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "java_string_regex_helpers",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
#[cfg(test)]
mod generated_tests {
    use super::*;

    #[test]
    fn covers_simple_regex_helpers() {
        let fixture = JavaStringRegexHelpers {};
        assert!(fixture.digit("7".to_string()));
        assert!(!fixture.digit("77".to_string()));
        assert_eq!(fixture.strip_prefix("-abc"), "abc".to_string());
        assert_eq!(fixture.digits("a1-b2"), "12".to_string());
    }
}
"#,
        }],
    );
}

#[test]
fn java_string_get_bytes_and_index_of_use_native_helpers() {
    let translation = translate_source(
        r#"
class JavaStringBytesAndIndex {
    static byte[] bytes(String value) {
        return value.getBytes();
    }

    static byte[] bytesUtf8(String value) throws Exception {
        return value.getBytes("UTF-8");
    }

    static boolean hasAmpersand(String value) {
        return value.indexOf("&") >= 0;
    }

    static boolean hasEquals(String value) {
        return value.indexOf('=') >= 0;
    }

    static int nextTab(String value, int from) {
        return value.indexOf('\t', from);
    }

    static int lastDot(String value) {
        return value.lastIndexOf('.');
    }

    static int lastSlashBefore(String value, int from) {
        return value.lastIndexOf('/', from);
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("value.as_bytes().iter().map(|byte| *byte as i8).collect::<Vec<i8>>()")
            && translation
                .rust
                .contains("java_string_index_of(&value, \"&\")")
            && translation
                .rust
                .contains("java_string_index_of(&value, \"=\")")
            && translation
                .rust
                .contains("java_string_index_of_from(&value, \"\\t\", from)")
            && translation
                .rust
                .contains("java_string_last_index_of(&value, \".\")")
            && translation
                .rust
                .contains("java_string_last_index_of_from(&value, \"/\", from)")
            && !translation.rust.contains("&'='")
            && !translation.rust.contains("&'\\t'")
            && !translation.rust.contains("&'.'")
            && !translation.rust.contains("&'/'")
            && !translation.rust.contains(".get_bytes_1(")
            && !translation.rust.contains(".get_bytes()")
            && !translation.rust.contains(".index_of("),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "java_string_bytes_and_index",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn java_list_index_of_uses_position_projection() {
    let translation = translate_source(
        r#"
import java.util.Arrays;
import java.util.List;

class JavaListIndexOf {
    int find(List<String> values, String value) {
        return values.indexOf(value);
    }

    int findLiteral(String value) {
        return Arrays.asList("ID", "Number", "Type").indexOf(value);
    }
}
"#,
    );

    assert!(
        translation.rust.contains(
            "values.iter().position(|item| item == &value).map(|index| index as i32).unwrap_or(-1)"
        ) && translation
            .rust
            .contains(".iter().position(|item| item == &value)")
            && !translation.rust.contains(".index_of(")
            && !translation.rust.contains(".indexOf("),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "java_list_index_of",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn java_atomic_integer_uses_platform_facade_methods() {
    let translation = translate_source(
        r#"
import java.util.concurrent.atomic.AtomicInteger;

class JavaAtomicIntegerUse {
    int next() {
        AtomicInteger count = new AtomicInteger(0);
        return count.incrementAndGet();
    }

    int afterGetAndIncrement() {
        AtomicInteger count = new AtomicInteger(4);
        count.getAndIncrement();
        return count.get();
    }
}
"#,
    );

    assert!(
        translation.rust.contains("pub struct AtomicInteger")
            && translation.rust.contains("std::cell::Cell<i32>")
            && translation.rust.contains("AtomicInteger::new(0)")
            && translation.rust.contains("count.increment_and_get()")
            && translation.rust.contains("count.get_and_increment();")
            && translation.rust.contains("return count.get();")
            && !translation.rust.contains("pub struct AtomicInteger;")
            && !translation.rust.contains("increment_and_get<T0>"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "java_atomic_integer_use",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
#[cfg(test)]
mod generated_tests {
    use super::*;

    #[test]
    fn covers_atomic_integer_state() {
        let fixture = JavaAtomicIntegerUse {};
        assert_eq!(fixture.next(), 1);
        assert_eq!(fixture.after_get_and_increment(), 5);
    }
}
"#,
        }],
    );
}

#[test]
fn java_string_comparison_methods_use_signature_helpers() {
    let translation = translate_source(
        r#"
class JavaStringComparisonMethods {
    int compare(String left, String right) {
        return left.compareTo(right);
    }

    int compareIgnoreCase(String left, String right) {
        return left.compareToIgnoreCase(right);
    }

    boolean equalsIgnoreCase(String left, String right) {
        return left.equalsIgnoreCase(right);
    }

    String shorten(String value) {
        return value.substring(0, 2);
    }

    int compareTail(String left, String right, int index) {
        return left.substring(index, left.length()).compareTo(right.substring(index, right.length()));
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("java_string_compare_to(&left, right)")
            && translation
                .rust
                .contains("java_string_compare_to_ignore_case(&left, right.as_str())")
            && translation
                .rust
                .contains("java_string_equals_ignore_case(&left, right.as_str())")
            && translation
                .rust
                .contains("java_string_substring(&value, 0, 2)")
            && translation.rust.contains(
                "java_string_compare_to(&java_string_substring(&left, index, left.len() as i32), java_string_substring(&right, index, right.len() as i32).as_str())"
            )
            && !translation.rust.contains(".compare_to(")
            && !translation.rust.contains(".compare_to_ignore_case(")
            && !translation.rust.contains(".equals_ignore_case("),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "java_string_comparison_methods",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn string_pattern_methods_use_signature_borrowed_argument_slots() {
    let translation = translate_source(
        r#"
class StringPatternSignatureArgs {
    boolean has(String value, String needle) {
        return value.contains(needle);
    }

    boolean hasPrefix(String value) {
        return value.startsWith("chr");
    }

    boolean hasSuffix(String value, String suffix) {
        return value.endsWith(suffix);
    }

    String rewrite(String value, String replacement) {
        return value.replace("x", replacement);
    }
}
"#,
    );

    assert!(
        translation.rust.contains("return value.contains(needle);")
            && translation
                .rust
                .contains("return value.starts_with(\"chr\");")
            && translation.rust.contains("return value.ends_with(suffix);")
            && translation
                .rust
                .contains("return value.replace(\"x\", replacement);")
            && !translation.rust.contains("\"chr\".to_string().as_str()")
            && !translation.rust.contains("\"x\".to_string().as_str()"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "string_pattern_signature_args",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn numeric_parse_static_calls_use_signature_borrowed_string_args() {
    let translation = translate_source(
        r#"
class NumericParseStaticCalls {
    int count(String value) {
        return Integer.parseInt(value);
    }

    double ratio(String value) {
        return Double.parseDouble(value);
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("return value.parse::<i32>().unwrap();")
            && translation
                .rust
                .contains("return value.parse::<f64>().unwrap();")
            && !translation.rust.contains("value.as_str().parse::<i32>()")
            && !translation.rust.contains("value.as_str().parse::<f64>()"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "numeric_parse_static_calls",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn numeric_wrapper_static_to_string_keeps_primitive_argument_signature() {
    let translation = translate_source(
        r#"
class NumericWrapperStaticToString {
    int width(int value) {
        return Integer.toString(value).length();
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("Integer::to_string(value).len() as i32")
            && translation
                .rust
                .contains("pub fn to_string(_arg0: impl std::fmt::Display) -> String")
            && translation.rust.contains("_arg0.to_string()"),
        "{}",
        translation.rust
    );
}

#[test]
fn java_string_equality_normalizes_owned_and_borrowed_slots() {
    let translation = translate_source(
        r#"
class StringEqualitySlots {
    static boolean firstIs(String[] values, String expected) {
        return values[0] == expected;
    }

    static boolean firstEquals(String[] values, String expected) {
        return values[0].equals(expected);
    }

    static boolean literalIs(String value) {
        return value == "Chrom";
    }

    static boolean sortedSame(String[] sorted, String[] unsorted) {
        return sorted[0] == unsorted[0];
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("values[0 as usize].as_str() == expected")
            && translation
                .rust
                .matches("AUDIT(java-string-identity)")
                .count()
                == 3
            && translation
                .rust
                .contains("value == \"Chrom\".to_string().as_str()")
            && translation
                .rust
                .contains("sorted[0 as usize].as_str() == unsorted[0 as usize].as_str()")
            && !translation
                .rust
                .contains("return values[0 as usize] == expected;")
            && !translation
                .rust
                .contains("return value == \"Chrom\".to_string();"),
        "{}",
        translation.rust
    );
    cargo_check_generated_modules(
        "java_string_equality_slots",
        vec![GeneratedCrateModule {
            name: "string_equality_slots",
            rust: translation.rust,
            tests: r#"
#[test]
fn compares_string_slots() {
    let values = vec!["Chrom".to_string(), "Other".to_string()];
    assert!(StringEqualitySlots::first_is(&values, "Chrom"));
    assert!(StringEqualitySlots::first_equals(&values, "Chrom"));
    assert!(StringEqualitySlots::literal_is("Chrom"));
    assert!(StringEqualitySlots::sorted_same(&values, &values));
}
"#,
        }],
    );
}

#[test]
fn constructed_string_return_stays_owned_not_borrowed_alias() {
    let translation = translate_source(
        r#"
class ConstructedStringReturn {
    private static String render(String chrom, int start, int stop, boolean report) {
        String line = chrom + "\t" + start + "\t" + stop;
        if (report) {
            return line;
        }
        return "";
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("pub fn render(chrom: &str, start: i32, stop: i32, report: bool) -> String")
            && translation.rust.contains("let line: String = format!")
            && translation.rust.contains("return line;")
            && translation.rust.contains("return \"\".to_string();")
            && !translation.rust.contains("-> &str")
            && !translation.rust.contains(".to_string().as_str()")
            && !translation
                .rust
                .contains("format!(\"{}\\t{}\\t{}\", chrom, start, stop).as_str()"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "constructed_string_return",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
#[cfg(test)]
mod generated_tests {
    use super::*;

    #[test]
    fn returns_owned_constructed_strings() {
        assert_eq!(
            ConstructedStringReturn::render("chr1", 10, 20, true),
            "chr1\t10\t20".to_string()
        );
        assert_eq!(
            ConstructedStringReturn::render("chr1", 10, 20, false),
            "".to_string()
        );
    }
}
"#,
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
        (translation
            .rust
            .contains("java_string_substring(header.as_str(), 1, header.len() as i32)")
            || translation
                .rust
                .contains("java_string_substring(&header, 1, header.len() as i32)"))
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
            && (translation
                .rust
                .contains("return self.quality_header.as_ref().cloned()")
                || translation
                    .rust
                    .contains("return self.quality_header.clone()"))
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
                .contains("java_string_hash_code(&self.bases)")
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
fn jdk_numeric_limit_constants_emit_java_values_not_zero_stubs() {
    let translation = translate_source(
        r#"
class JdkNumericLimitConstants {
    static int maxInt() { return Integer.MAX_VALUE; }
    static int minInt() { return Integer.MIN_VALUE; }
    static long maxLong() { return Long.MAX_VALUE; }
    static int integerBytes() { return Integer.BYTES; }
    static char maxChar() { return Character.MAX_VALUE; }
    static float minFloat() { return Float.MIN_VALUE; }
    static float nanFloat() { return Float.NaN; }
    static double minDouble() { return Double.MIN_VALUE; }
    static double nan() { return Double.NaN; }
    static double posInf() { return Double.POSITIVE_INFINITY; }
    static double negInf() { return Double.NEGATIVE_INFINITY; }
    static double pi() { return Math.PI; }
    static double e() { return Math.E; }
    static boolean truth() { return Boolean.TRUE; }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("pub const MAX_VALUE: i32 = i32::MAX;")
            && translation
                .rust
                .contains("pub const MIN_VALUE: i32 = i32::MIN;")
            && translation
                .rust
                .contains("pub const MAX_VALUE: i64 = i64::MAX;")
            && translation.rust.contains("pub const BYTES: i32 = 4;")
            && translation
                .rust
                .contains("pub const MAX_VALUE: char = '\\u{ffff}';")
            && translation
                .rust
                .contains("pub const MIN_VALUE: f32 = f32::from_bits(1);")
            && translation.rust.contains("pub const NaN: f32 = f32::NAN;")
            && translation
                .rust
                .contains("pub const MIN_VALUE: f64 = f64::from_bits(1);")
            && translation.rust.contains("pub const NaN: f64 = f64::NAN;")
            && translation
                .rust
                .contains("pub const POSITIVE_INFINITY: f64 = f64::INFINITY;")
            && translation
                .rust
                .contains("pub const NEGATIVE_INFINITY: f64 = f64::NEG_INFINITY;")
            && translation
                .rust
                .contains("pub const PI: f64 = std::f64::consts::PI;")
            && translation
                .rust
                .contains("pub const E: f64 = std::f64::consts::E;")
            && translation.rust.contains("pub const TRUE: bool = true;"),
        "{}",
        translation.rust
    );
    assert!(
        !translation.rust.contains("pub const MAX_VALUE: i32 = 0;")
            && !translation.rust.contains("pub const MIN_VALUE: i32 = 0;")
            && !translation
                .rust
                .contains("pub const MAX_VALUE: i64 = 0i64;")
            && !translation
                .rust
                .contains("pub const MIN_VALUE: f64 = 0.0f64;"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "jdk_numeric_limit_constants",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
#[cfg(test)]
mod generated_tests {
    use super::*;

    #[test]
    fn constants_have_java_values() {
        assert_eq!(JdkNumericLimitConstants::max_int(), i32::MAX);
        assert_eq!(JdkNumericLimitConstants::min_int(), i32::MIN);
        assert_eq!(JdkNumericLimitConstants::max_long(), i64::MAX);
        assert_eq!(JdkNumericLimitConstants::integer_bytes(), 4);
        assert_eq!(JdkNumericLimitConstants::max_char(), '\u{ffff}');
        assert!(JdkNumericLimitConstants::min_float() > 0.0);
        assert!(JdkNumericLimitConstants::nan_float().is_nan());
        assert!(JdkNumericLimitConstants::min_double() > 0.0);
        assert!(JdkNumericLimitConstants::nan().is_nan());
        assert_eq!(JdkNumericLimitConstants::pos_inf(), f64::INFINITY);
        assert_eq!(JdkNumericLimitConstants::neg_inf(), f64::NEG_INFINITY);
        assert_eq!(JdkNumericLimitConstants::pi(), std::f64::consts::PI);
        assert_eq!(JdkNumericLimitConstants::e(), std::f64::consts::E);
        assert!(JdkNumericLimitConstants::truth());
    }
}
"#,
        }],
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

    boolean fileTooLarge(java.io.File file) {
        return file.length() > Integer.MAX_VALUE;
    }

    boolean zeroDouble(double x) {
        return x == 0;
    }

    boolean notZeroDouble(double x) {
        return x != 0;
    }

    boolean doubleLong(double y, long yi) {
        return y == yi;
    }

    boolean floatDouble(float f, double direction) {
        return f == direction;
    }
}
"#,
    );

    assert!(
        translation.rust.contains("return (end as i64) < limit;")
            && translation
                .rust
                .contains("return file.length() > (Integer::MAX_VALUE as i64);")
            && translation.rust.contains("return x == (0 as f64);")
            && translation.rust.contains("return x != (0 as f64);")
            && translation.rust.contains("return y == (yi as f64);")
            && translation.rust.contains("return (f as f64) == direction;"),
        "{}",
        translation.rust
    );
    assert!(
        !translation.rust.contains("return (end < limit);")
            && !translation.rust.contains("return x == 0;")
            && !translation.rust.contains("return y == yi;")
            && !translation.rust.contains("return f == direction;"),
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
            arg_count: None,
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
fn reused_string_parameters_clone_in_multiple_vec_initializers() {
    let translation = translate_source(
        r#"
import java.util.Arrays;

class ReusedStringParametersInVecs {
    boolean sorted(String input1, String input2) {
        String chrom1 = input1.trim();
        String chrom2 = input2.trim();
        String[] unsorted = new String[]{chrom1, chrom2};
        String[] sorted = new String[]{chrom1, chrom2};
        Arrays.sort(sorted);
        return sorted[0].equals(unsorted[0]);
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("vec![chrom1.clone()/* TODO(translation): validate added clone */, chrom2.clone()/* TODO(translation): validate added clone */]")
            && translation.rust.contains("let mut sorted: Vec<String> = vec![chrom1, chrom2];"),
        "{}",
        translation.rust
    );
}

#[test]
fn java_arrays_sort_lowers_whole_and_range_mutations() {
    let translation = translate_source(
        r#"
import java.util.Arrays;

class JavaArraysSortUse {
    static int[] whole(int[] values) {
        Arrays.sort(values);
        return values;
    }

    static int[] range(int[] values) {
        Arrays.sort(values, 1, 4);
        return values;
    }

    static String[] names(String[] values) {
        Arrays.sort(values, 1, 3);
        return values;
    }

    static int[] badRangeOrder(int[] values) {
        Arrays.sort(values, 4, 1);
        return values;
    }

    static int[] noopAtEnd(int[] values) {
        Arrays.sort(values, 3, 3);
        return values;
    }

    static int[] badRangeStart(int[] values) {
        Arrays.sort(values, -1, 2);
        return values;
    }

    static int[] badRangeEnd(int[] values) {
        Arrays.sort(values, 1, 5);
        return values;
    }
}
"#,
    );

    assert!(
        translation.rust.contains("values.sort()")
            && translation
                .rust
                .contains("__java_arrays_target[__java_arrays_from..__java_arrays_to].sort()")
            && !translation.rust.contains("Arrays::sort")
            && !translation.rust.contains("JavaUtilArrays::sort"),
        "{}",
        translation.rust
    );

    cargo_test_generated_modules(
        "java_arrays_sort_use",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
#[cfg(test)]
mod generated_tests {
    use super::*;

    #[test]
    fn sorts_whole_arrays_and_subranges() {
        assert_eq!(JavaArraysSortUse::whole(vec![3, 1, 2]), vec![1, 2, 3]);
        assert_eq!(JavaArraysSortUse::range(vec![9, 4, 3, 2, 1]), vec![9, 2, 3, 4, 1]);
        assert_eq!(
            JavaArraysSortUse::names(vec!["z".to_string(), "b".to_string(), "a".to_string(), "x".to_string()]),
            vec!["z".to_string(), "a".to_string(), "b".to_string(), "x".to_string()]
        );

        assert_eq!(JavaArraysSortUse::noop_at_end(vec![1, 2, 3]), vec![1, 2, 3]);
        assert!(std::panic::catch_unwind(|| JavaArraysSortUse::bad_range_order(vec![1, 2, 3])).is_err());
        assert!(std::panic::catch_unwind(|| JavaArraysSortUse::bad_range_start(vec![1, 2, 3])).is_err());
        assert!(std::panic::catch_unwind(|| JavaArraysSortUse::bad_range_end(vec![1, 2, 3])).is_err());
    }
}
"#,
        }],
    );
}

#[test]
fn reused_string_source_clones_for_multiple_owned_local_assignments() {
    let translation = translate_source(
        r#"
class ReusedStringSourceAssignments {
    String trio(String input, boolean replace) {
        String refBase = input.trim();
        String father = refBase;
        String mother = refBase;
        String child = refBase;
        if (replace) {
            father = refBase;
            mother = refBase;
            child = refBase;
        }
        return father + mother + child + refBase;
    }
}
"#,
    );

    assert!(
        translation.rust.contains(
            "let mut father: String = ref_base.clone()/* TODO(translation): validate added clone */;"
        ) && translation.rust.contains(
            "let mut mother: String = ref_base.clone()/* TODO(translation): validate added clone */;"
        ) && translation.rust.contains(
            "let mut child: String = ref_base.clone()/* TODO(translation): validate added clone */;"
        ) && translation
            .rust
            .contains("father = ref_base.clone()/* TODO(translation): validate added clone */;")
            && translation.rust.contains(
                "mother = ref_base.clone()/* TODO(translation): validate added clone */;"
            )
            && translation
                .rust
                .contains("child = ref_base.clone()/* TODO(translation): validate added clone */;"),
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
fn string_format_locale_overload_uses_template_argument() {
    let translation = translate_source(
        r#"
import java.util.Locale;

class StringFormatLocaleOverload {
    String literal(String name, int count) {
        return String.format(Locale.US, "%s:%d", name, count);
    }

    String dynamic(String template, int count) {
        return String.format(Locale.US, template, count);
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("return format!(\"{}:{}\", name, count);")
            && translation
                .rust
                .contains("java_string_format(template, vec![count.to_string()])")
            && !translation.rust.contains("java_string_format(Locale::US"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "string_format_locale_overload",
        vec![GeneratedCrateModule {
            name: "string_format_locale_overload",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn erased_object_boolean_instanceof_uses_rust_bool_runtime_target() {
    let translation = translate_source(
        r#"
class ErasedObjectBooleanInstanceof {
    String show(Object value) {
        if (value instanceof Boolean) {
            return ((Boolean)value) ? "true" : "false";
        }
        return "other";
    }
}
"#,
    );

    assert!(
        translation.rust.contains("value.is::<bool>()")
            && translation.rust.contains("downcast::<bool>()")
            && !translation.rust.contains("is::<Boolean>")
            && !translation.rust.contains("downcast::<Boolean>"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "erased_object_boolean_instanceof",
        vec![GeneratedCrateModule {
            name: "erased_object_boolean_instanceof",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn instanceof_pattern_variable_basic_block_compiles() {
    let translation = translate_source(
        r#"
class InstanceofPatternVariableBasic {
    int run(Object o) {
        int t = 1;
        if (o instanceof String s) {
            t = s.length();
        }
        return t;
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("if let Some(s) = o.downcast_ref::<String>()")
            && translation.rust.contains("t = s.len() as i32;")
            && !translation.rust.contains("s.length()"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "instanceof_pattern_variable_basic",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn instanceof_pattern_variable_negated_guard_binds_after_return() {
    let translation = translate_source(
        r#"
class InstanceofPatternVariableNegatedGuard {
    int run(Object o) {
        if (!(o instanceof String s)) {
            return -1;
        }
        return s.length();
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("let s = o.downcast_ref::<String>().expect(\"invalid Java cast\");")
            && translation.rust.contains("return s.len() as i32;")
            && !translation.rust.contains("s.length()"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "instanceof_pattern_variable_negated_guard",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn instanceof_pattern_variable_and_chain_binds_rhs_and_then_block() {
    let translation = translate_source(
        r#"
class InstanceofPatternVariableAndChain {
    int run(Object o) {
        if (o instanceof String s && s.length() > 2) {
            return s.length();
        }
        return 0;
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("if let Some(s) = o.downcast_ref::<String>()")
            && translation.rust.contains("if (s.len() as i32) > 2")
            && translation.rust.contains("return s.len() as i32;")
            && !translation.rust.contains("s.length()"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "instanceof_pattern_variable_and_chain",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn erased_object_raw_list_instanceof_uses_vec_erasure_runtime_target() {
    let translation = translate_source(
        r#"
import java.util.List;

class RawListInstanceofErasedObject {
    Object show(Object value) {
        if (value instanceof List) {
            return (List) value;
        }
        return null;
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("value.is::<Vec<Box<dyn std::any::Any>>>()")
            && translation
                .rust
                .contains("downcast::<Vec<Box<dyn std::any::Any>>>()")
            && !translation.rust.contains("unsupported Java instanceof")
            && !translation.rust.contains("downcast::<List>()"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "raw_list_instanceof_erased_object",
        vec![GeneratedCrateModule {
            name: "raw_list_instanceof_erased_object",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn raw_list_to_array_clones_erased_elements_with_runtime_helper() {
    let translation = translate_source(
        r#"
import java.util.List;

class RawListToArrayErasedObject {
    Object[] show(Object value) {
        if (value instanceof List) {
            return ((List) value).toArray();
        }
        return null;
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("value.is::<Vec<Box<dyn std::any::Any>>>()")
            && translation.rust.contains(
                ".iter().map(|value| java_clone_erased_object_ref(value)).collect::<Vec<_>>()"
            )
            && !translation
                .rust
                .contains(".iter().cloned().collect::<Vec<_>>()")
            && !translation.rust.contains("unsupported Java instanceof")
            && !translation.rust.contains("downcast::<List>()"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "raw_list_to_array_erased_object",
        vec![GeneratedCrateModule {
            name: "raw_list_to_array_erased_object",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn nullable_boolean_ternary_assignment_preserves_optional_string_slot() {
    let translation = translate_source(
        r#"
class NullableBooleanTernaryAssignment {
    String show(Object value) {
        String result;
        if (value instanceof Boolean) {
            result = (Boolean) value ? "" : null;
        } else {
            result = "other";
        }
        return result;
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("if *value.downcast::<bool>().expect(\"invalid Java cast\")")
            && translation
                .rust
                .contains("{ Some(\"\".to_string()) } else { None }")
            && !translation.rust.contains(".downcast::<String>()")
            && !translation.rust.contains("Some(if"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "nullable_boolean_ternary_assignment",
        vec![GeneratedCrateModule {
            name: "nullable_boolean_ternary_assignment",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn nullable_erased_object_instanceof_projects_optional_carrier() {
    let translation = translate_source(
        r#"
class NullableErasedObjectInstanceof {
    String show(Object value) {
        if (value == null) {
            return "null";
        }
        if (value instanceof Boolean) {
            return ((Boolean)value) ? "true" : "false";
        }
        return "other";
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("value.as_ref().is_some_and(|value| value.is::<bool>())")
            && !translation.rust.contains("if value.is::<bool>()")
            && translation.rust.contains("downcast::<bool>()")
            && !translation.rust.contains("is::<Boolean>")
            && !translation.rust.contains("downcast::<Boolean>"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "nullable_erased_object_instanceof",
        vec![GeneratedCrateModule {
            name: "nullable_erased_object_instanceof",
            rust: translation.rust,
            tests: "",
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
fn source_value_struct_with_java_hash_equality_derives_eq_and_hash() {
    let translation = translate_source(
        r#"
import java.util.HashSet;

class SourceHashValue {
    private String name;
    private String label;

    SourceHashValue(String name, String label) {
        this.name = name;
        this.label = label;
    }

    public boolean equals(Object other) {
        if (!(other instanceof SourceHashValue)) {
            return false;
        }
        SourceHashValue that = (SourceHashValue) other;
        return name.equals(that.name) && label.equals(that.label);
    }

    public int hashCode() {
        return name.hashCode() + label.hashCode();
    }

    boolean add(HashSet<SourceHashValue> values) {
        return values.add(this);
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("#[derive(Clone, PartialEq, Eq, Hash)]\npub struct SourceHashValue"),
        "{}",
        translation.rust
    );
}

#[test]
fn source_struct_with_dyn_trait_vec_field_derives_clone_when_adapter_exists() {
    let translation = translate_source(
        r#"
interface CloneableModule {
    String name();
}

class DynTraitVecCloneHolder {
    private CloneableModule[] modules;

    DynTraitVecCloneHolder(CloneableModule[] modules) {
        this.modules = modules;
    }

    DynTraitVecCloneHolder choose(DynTraitVecCloneHolder value) {
        return value;
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("#[derive(Clone)]\npub struct DynTraitVecCloneHolder")
            && translation
                .rust
                .contains("modules: Vec<Box<dyn CloneableModule>>")
            && translation
                .rust
                .contains("impl Clone for Box<dyn CloneableModule>"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "source_dyn_trait_vec_clone_holder",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn source_struct_with_erased_object_field_does_not_derive_clone() {
    let translation = translate_source(
        r#"
class ErasedObjectFieldNoCloneDerive {
    private Object value;

    ErasedObjectFieldNoCloneDerive(Object value) {
        this.value = value;
    }

    Object getValue() {
        return value;
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("pub struct ErasedObjectFieldNoCloneDerive"),
        "{}",
        translation.rust
    );
    assert!(
        !translation
            .rust
            .contains("#[derive(Clone)]\npub struct ErasedObjectFieldNoCloneDerive"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "erased_object_field_no_clone_derive",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn reused_optional_erased_object_cast_clones_with_erased_helper() {
    let translation = translate_source(
        r#"
class ReusedOptionalErasedObjectCastClone {
    void accept(String value) {}

    void run(Object maybe) {
        if (maybe != null) {
            accept((String) maybe);
            maybe.toString();
        }
    }
}
"#,
    );

    assert!(
        translation.rust.contains(
            "maybe.as_ref().and_then(|value| value.downcast_ref::<String>()).expect(\"invalid Java cast\").as_str()"
        ) && !translation.rust.contains("maybe.unwrap().downcast::<String>()")
            && !translation.rust.contains("maybe.as_ref().cloned()/* TODO(translation): validate added clone */.expect(\"Java null receiver\").downcast::<String>()"),
        "{}",
        translation.rust
    );
    cargo_check_generated_modules(
        "reused_optional_erased_object_cast_clone",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn source_struct_with_hash_set_of_source_values_derives_clone() {
    let translation = translate_source(
        r#"
import java.util.HashMap;
import java.util.HashSet;

class HashSetBackedCloneCarrier {
    private HashSet<HashSetBackedCloneLine> lines;
    private HashMap<String, HashSetBackedCloneLine> byId;
}

class HashSetBackedCloneLine {
    private String id;
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("#[derive(Clone)]\npub struct HashSetBackedCloneCarrier"),
        "{}",
        translation.rust
    );
    cargo_check_generated_modules(
        "hash_set_source_value_clone_derive",
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
            .contains("pub fn describe(&self, value: Option<&str>) -> String")
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
fn inherited_receiverless_nullable_getter_uses_java_display_policy() {
    let translation = translate_source(
        r#"
class HeaderBase {
    private String key = null;

    String getKey() {
        return key;
    }

    static String encodeMap() throws Exception {
        return "value";
    }
}

class HeaderChild extends HeaderBase {
    String ownKey = null;

    String getOwnKey() {
        return ownKey;
    }

    String encodeFormat() {
        return String.format("key=%s", getKey());
    }

    String encodeConcat() {
        return getKey() + "=" + "value";
    }

    String encodeOwnConcat() {
        return getOwnKey() + "=" + "value";
    }

    String encodeFallibleConcat() throws Exception {
        return getKey() + "=" + HeaderBase.encodeMap();
    }
}
"#,
    );

    assert!(
        translation.rust.contains(".as_deref().unwrap_or(\"null\")")
            && !translation
                .rust
                .contains("format!(\"key={}\", self.get_key())")
            && !translation.rust.contains("format!(\"key={}\", self.key)")
            && !translation
                .rust
                .contains("format!(\"{}={}\", self.get_key(), \"value\")")
            && !translation
                .rust
                .contains("format!(\"{}={}\", self.key, \"value\")")
            && !translation
                .rust
                .contains("format!(\"{}={}\", self.get_own_key(), \"value\")")
            && !translation
                .rust
                .contains("format!(\"{}={}\", self.own_key, \"value\")")
            && !translation
                .rust
                .contains("format!(\"{}={}\", self.get_key(), HeaderBase::encode_map()?)")
            && !translation
                .rust
                .contains("format!(\"{}={}\", self.key, HeaderBase::encode_map()?)"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "inherited_receiverless_nullable_getter_display",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn inherited_context_nullable_getter_display_uses_declaring_field_shape() {
    let base = r#"
class ContextNullableGetterBase {
    private String key = null;

    String getKey() {
        return key;
    }

}
"#;
    let child = r#"
class ContextNullableGetterChild extends ContextNullableGetterBase {
    String encode() {
        return getKey() + "=" + "value";
    }
}
"#;
    let translation =
        translate_source_with_context_sources(child, &[base.to_string()], &FactSet::default());

    assert!(
        translation.rust.contains(
            ".as_ref().map(|value| value.to_string()).unwrap_or_else(|| \"null\".to_string())"
        ) || translation.rust.contains(".as_deref().unwrap_or(\"null\")"),
        "{}",
        translation.rust
    );
    assert!(
        !translation
            .rust
            .contains("format!(\"{}={}\", self.get_key(), \"value\")"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "inherited_context_nullable_getter_display",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn receiverless_nullable_method_display_uses_current_signature_shape() {
    let translation = translate_source(
        r#"
class ReceiverlessNullableMethodDisplay {
    private String key = null;

    String getKey() {
        if (key == null) {
            return null;
        }
        return key;
    }

    String encode() {
        return getKey() + "=" + "value";
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("self.get_key().as_ref().map(|value| value.to_string()).unwrap_or_else(|| \"null\".to_string())")
            || translation
                .rust
                .contains("self.get_key().as_deref().unwrap_or(\"null\")"),
        "{}",
        translation.rust
    );
    assert!(
        !translation
            .rust
            .contains("format!(\"{}={}\", self.get_key(), \"value\")"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "receiverless_nullable_method_display",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn string_pattern_methods_borrow_owned_string_arguments() {
    let translation = translate_source(&idiom_gap("StringPatternBorrowing.java"));

    assert!(
        translation.rust.contains("value.contains(needle)")
            && translation.rust.contains("value.ends_with(suffix)")
            && !translation.rust.contains("contains(needle.as_str())")
            && !translation.rust.contains("ends_with(suffix.as_str())"),
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
            .contains("path.to_string().to_lowercase().ends_with(suffix)")
            && !translation.rust.contains("ends_with(suffix.as_str())"),
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
                .contains("java_string_split(&line, \"\\\\\\\\t\", -1)")
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
            .contains("self.legal(Some(parts[0 as usize].as_str()))")
            && !translation
                .rust
                .contains("self.legal(Some(parts[0 as usize]))"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "indexed-string-array-optional-borrowed",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn nested_indexed_string_array_append_borrows_inner_vec_element() {
    let translation = translate_source(
        r#"
class NestedIndexedStringArrayAppend {
    String flush(String[][] group) {
        StringBuilder sb = new StringBuilder();
        for (String[] seg : group) {
            sb.append(seg[0]).append(",").append(seg[1]);
        }
        return sb.toString();
    }
}
"#,
    );

    assert!(
        translation.rust.contains("sb.append(&seg[0 as usize])")
            && translation.rust.contains(".append(&seg[1 as usize])")
            && !translation.rust.contains("sb.append(seg[0 as usize])")
            && !translation.rust.contains(".append(seg[1 as usize])"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "nested_indexed_string_array_append",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn non_null_ternary_into_optional_borrowed_arg_wraps_planned_borrow() {
    let translation = translate_source(
        r#"
class NonNullTernaryOptionalBorrowedArg {
    boolean legal(String value) {
        return value != null && value.trim().equals(value);
    }

    boolean check(boolean chooseLeft, String line) {
        String[] parts = line.split("\\t", -1);
        return legal(chooseLeft ? parts[0] : parts[1]);
    }
}
"#,
    );

    assert!(
        translation.rust.contains(
            "self.legal(Some(if choose_left { parts[0 as usize].as_str() } else { parts[1 as usize].as_str() }))"
        ) && !translation
            .rust
            .contains("if choose_left { parts[0 as usize] } else { parts[1 as usize] }")
            && !translation.rust.contains("&parts[0 as usize].as_str()")
            && !translation.rust.contains("&parts[1 as usize].as_str()")
            && !translation.rust.contains("self.legal(&Some("),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "non-null-ternary-optional-borrowed-arg",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
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
            && (translation.rust.contains("to_lowercase()")
                || translation.rust.contains("to_lower_case()"))
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
            && translation.rust.contains("intervals.as_slice()")
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
fn integer_parse_int_unwraps_sequence_get_argument() {
    let translation = translate_source(
        r#"
import java.util.List;

class IntegerParseSequenceGet {
    private List<String> nullableValues;

    int parseAt(List<String> values, int index) {
        return Integer.parseInt(values.get(index));
    }

    int parseNullableAt(int index) {
        return Integer.parseInt(nullableValues.get(index));
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("values.get(index as usize).as_ref().expect(\"Java null receiver\").as_str().parse::<i32>().unwrap()")
            && translation
                .rust
                .contains("self.nullable_values.as_ref().unwrap().get(index as usize).as_ref().expect(\"Java null receiver\").as_str().parse::<i32>().unwrap()")
            && !translation.rust.contains(".get(index as usize).parse::<i32>()"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "integer_parse_sequence_get",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn collection_get_result_unwraps_before_array_indexing() {
    let translation = translate_source(
        r#"
import java.util.ArrayList;

class CollectionGetArrayIndexing {
    static int second(ArrayList<int[]> group) {
        if (group.isEmpty()) {
            return -1;
        }
        return group.get(0)[1];
    }
}
"#,
    );

    assert!(
        (translation
            .rust
            .contains("group.get(0 as usize).expect(\"Java index out of bounds\")[1 as usize]")
            || translation.rust.contains(
                "group.get(0 as usize).as_ref().expect(\"Java index out of bounds\")[1 as usize]"
            ))
            && !translation
                .rust
                .contains("group.get(0 as usize)[1 as usize]"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "collection_get_array_indexing",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
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
            .contains("connection = Some(url.open_connection()?.into());")
            && translation
                .rust
                .contains("pub fn open_connection(&self) -> Result<URLConnection, JavaException>")
            && !translation.rust.contains("open_connection<T: Default>")
            && !translation.rust.contains("downcast::<JarURLConnection>")
            && !translation.rust.contains("invalid Java cast"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "casted_factory_result_into_concrete_optional_slot",
        vec![GeneratedCrateModule {
            name: "casted_factory_result_into_concrete_optional_slot",
            rust: translation.rust,
            tests: r#"
#[cfg(test)]
mod generated_tests {
    use super::*;

    #[test]
    fn typechecks_canonical_url_open_connection_shape() {
        let fixture = CastedFactoryResultIntoConcreteOptionalSlot {};
        let _ = fixture.open(&URL::new("jar:file:///tmp/example.jar!/")).unwrap();
    }
}
"#,
        }],
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
            .contains("pub fn check_path(path: &Path, optional_index_path: Option<Path>)")
            && translation.rust.contains("Helper::check_path(")
            && translation.rust.contains("IO::get_path(vcf_path")
            && translation.rust.contains("Some(IO::get_path(")
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
            .contains("let path: Option<Path> = self.lookup.as_ref().unwrap().find(Some(sample));")
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
fn inferred_source_throwing_call_creates_caller_exception_fact() {
    let translation = translate_source(
        r#"
class InferredSourceThrowingCall {
    void caller() {
        callee();
    }

    void callee() {
        throw new CheckedProblem();
    }
}

class CheckedProblem extends Exception {}
"#,
    );

    assert!(
        translation
            .rust
            .contains("pub fn caller(&self) -> Result<(), JavaException>")
            && translation
                .rust
                .contains("pub fn callee(&self) -> Result<(), JavaException>")
            && translation.rust.contains("self.callee()?;"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "inferred_source_throwing_call",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn overloaded_static_throwing_call_creates_caller_exception_fact() {
    let translation = translate_source(
        r#"
class OverloadedStaticThrowingCall {
    static void caller(int value) {
        check(value);
    }

    static void check(int value) throws CheckedProblem {
        throw new CheckedProblem();
    }

    static void check(String value) {
    }
}

class CheckedProblem extends Exception {}
"#,
    );

    assert!(
        translation
            .rust
            .contains("pub fn caller(value: i32) -> Result<(), JavaException>")
            && translation
                .rust
                .contains("pub fn check_int(value: i32) -> Result<(), JavaException>")
            && translation.rust.contains("Self::check_int(value)?;")
            && !translation.rust.contains("pub fn caller(value: i32) {"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "overloaded_static_throwing_call",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn overloaded_static_caller_gets_exception_fact_on_emitted_rust_name() {
    let translation = translate_source(
        r#"
class OverloadedStaticCallerThrowingHelper {
    static void print(String name, int expected, double[] values) {
        checkLen(expected, values.length);
    }

    static void print(String name, int expected, double[][] values) {
        checkLen(expected, values.length);
    }

    static void checkLen(int expected, int actual) throws CheckedProblem {
        if (expected != actual) {
            throw new CheckedProblem();
        }
    }
}

class CheckedProblem extends Exception {}
"#,
    );

    assert!(
        translation.rust.contains(
            "pub fn print_string_int_double_array(name: &str, expected: i32, values: &[f64]) -> Result<(), JavaException>"
        ) && translation.rust.contains(
            "pub fn print_string_int_double_array_array(name: &str, expected: i32, values: &ndarray::Array2<f64>) -> Result<(), JavaException>"
        ) && translation.rust.contains("Self::check_len(expected, values.len() as i32)?;")
            && translation
                .rust
                .contains("Self::check_len(expected, values.nrows() as i32)?;")
            && !translation
                .rust
                .contains("pub fn print_string_int_double_array(name: &str, expected: i32, values: &[f64]) {"),
        "{}",
        translation.rust
    );
}

#[test]
fn collection_to_array_empty_string_array_lowers_to_owned_vec() {
    let translation = translate_source(
        r#"
import java.util.List;
import java.util.Map;

public class ToArrayEmptyStringArray {
    public String[] copy(List<String> names) {
        return names.toArray(new String[]{});
    }

    public String[] keys(Map<String, String> values) {
        return values.keySet().toArray(new String[0]);
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("names.iter().cloned().collect::<Vec<_>>()")
            && translation.rust.contains(
                "values.keys().cloned().collect::<Vec<_>>().iter().cloned().collect::<Vec<_>>()"
            )
            && !translation.rust.contains(".to_array("),
        "{}",
        translation.rust
    );
    assert!(
        !translation.rust.contains("Java array creation"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "collection_to_array_empty_string_array",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn java_int_arithmetic_array_index_casts_whole_expression_to_usize() {
    let translation = translate_source(
        r#"
class JavaIntArithmeticArrayIndex {
    double get(double[] values, int left, int right) {
        return values[left + right];
    }
}
"#,
    );

    assert!(
        translation.rust.contains("values[(left + right) as usize]")
            && !translation.rust.contains("values[left + right as usize]"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "java_int_arithmetic_array_index",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn java_shift_expression_casts_as_whole_expression_when_promoted() {
    let translation = translate_source(
        r#"
class JavaShiftPromotion {
    static final double SCALE = 4503599627370496.0;

    double scaled(long value) {
        return (value >> 12) / SCALE;
    }

    double mixed(long high, long low) {
        return (((high & 4095L) << 40) + (low >> 24)) / SCALE / SCALE;
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("((value).wrapping_shr((12) as u32) as f64) / Self::SCALE")
            && !translation.rust.contains("value >> 12 as f64"),
        "{}",
        translation.rust
    );
    assert!(
        translation
            .rust
            .contains("((((high & 4095i64).wrapping_shl((40) as u32)).wrapping_add((low).wrapping_shr((24) as u32)) as f64) / Self::SCALE")
            && !translation.rust.contains("+ (low >> 24) as f64"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "java_shift_promotion",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn java_unsigned_right_shift_lowers_with_unsigned_carrier_casts() {
    let translation = translate_source(
        r#"
class JavaUnsignedRightShift {
    int unsignedInt(int value) {
        return value >>> 8;
    }

    int arithmeticInt(int value) {
        return value >> 8;
    }

    long unsignedLong(long value) {
        return value >>> 8;
    }

    int compound(int value) {
        value >>>= 8;
        return value;
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("return ((value as u32).wrapping_shr((8) as u32)) as i32;")
            && translation
                .rust
                .contains("return (value).wrapping_shr((8) as u32);")
            && translation
                .rust
                .contains("return ((value as u64).wrapping_shr((8) as u32)) as i64;")
            && translation
                .rust
                .contains("value = ((value as u32).wrapping_shr((8) as u32)) as i32;")
            && !translation.rust.contains("value = 8;"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "java_unsigned_right_shift",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn java_integer_binary_ops_wrap_and_mask_shift_distance() {
    let translation = translate_source(
        r#"
class JavaIntegerBinarySemantics {
    int multiplyAdd(int a, int b) {
        return a * b + a;
    }

    int minDiv(int value) {
        return value / -1;
    }

    int shiftInt(int value, int count) {
        return value << count;
    }

    long shiftLong(long value, int count) {
        return value << count;
    }

    int unsignedShiftInt(int value, int count) {
        return value >>> count;
    }
}
"#,
    );

    assert!(
        translation.rust.contains(".wrapping_mul(")
            && translation.rust.contains(".wrapping_add(")
            && translation.rust.contains(".wrapping_div(")
            && translation.rust.contains(".wrapping_shl(")
            && translation.rust.contains(".wrapping_shr(")
            && !translation.rust.contains("value << count")
            && !translation.rust.contains("value >> count"),
        "{}",
        translation.rust
    );

    cargo_test_generated_modules(
        "java_integer_binary_semantics",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
#[cfg(test)]
mod generated_tests {
    use super::*;

    #[test]
    fn integer_binary_ops_match_java_wrapping_edges() {
        let fixture = JavaIntegerBinarySemantics {};
        assert_eq!(fixture.multiply_add(2_000_000_000, 2), 1_705_032_704);
        assert_eq!(fixture.min_div(i32::MIN), i32::MIN);
        assert_eq!(fixture.shift_int(1, 33), 2);
        assert_eq!(fixture.shift_long(1, 65), 2);
        assert_eq!(fixture.unsigned_shift_int(-1, 33), 2_147_483_647);
    }
}
"#,
        }],
    );
}

#[test]
fn java_local_update_expressions_wrap_for_integral_primitives() {
    let translation = translate_source(
        r#"
class JavaLocalUpdateSemantics {
    int preIncInt(int value) {
        int next = ++value;
        return next;
    }

    int postDecByte() {
        byte value = -128;
        byte old = value--;
        return old + value;
    }

    int preDecChar() {
        char value = '\0';
        --value;
        return value;
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("value = (value).wrapping_add(1);")
            && translation
                .rust
                .contains("value = (value).wrapping_sub(1);")
            && translation.rust.contains("char::from_u32")
            && !translation.rust.contains("value += 1")
            && !translation.rust.contains("value -= 1"),
        "{}",
        translation.rust
    );

    cargo_test_generated_modules(
        "java_local_update_semantics",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
#[cfg(test)]
mod generated_tests {
    use super::*;

    #[test]
    fn local_updates_match_java_wrapping_edges() {
        let fixture = JavaLocalUpdateSemantics {};
        assert_eq!(fixture.pre_inc_int(i32::MAX), i32::MIN);
        assert_eq!(fixture.post_dec_byte(), -1);
        assert_eq!(fixture.pre_dec_char(), 65535);
    }
}
"#,
        }],
    );
}

#[test]
fn optional_array_element_participates_in_numeric_promotion() {
    let translation = translate_source(
        r#"
class OptionalArrayNumericPromotion {
    static final long SCALE = 1073741824L;

    double scale(boolean present) {
        double values[] = present ? new double[2] : null;
        if (values != null) {
            final double tmp = values[0] * SCALE;
            return tmp;
        }
        return 0.0;
    }
}
"#,
    );

    assert!(
        translation.rust.contains(
            "values.as_ref().expect(\"Java null receiver\")[0 as usize] * (Self::SCALE as f64)"
        ) && !translation.rust.contains("* Self::SCALE) as f64"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "optional_array_numeric_promotion",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
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
            .contains("let name: String = r#box.get_name();"),
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
        translation
            .rust
            .contains("pub struct NonNullFieldFromConstructors {\n    name: String,")
            && !translation
                .rust
                .contains("pub struct NonNullFieldFromConstructors {\n    name: Option<String>,"),
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
fn collections_empty_set_and_unmodifiable_set_lower_to_native_sets() {
    let translation = translate_source(
        r#"
import java.util.Collections;
import java.util.HashSet;
import java.util.Set;

public class CollectionSetStatics {
    public Set<String> empty() {
        return Collections.<String>emptySet();
    }

    public Set<String> freeze(Set<String> input) {
        return Collections.unmodifiableSet(input);
    }

    public Set<String> copy() {
        Set<String> values = new HashSet<String>();
        values.add("a");
        return Collections.unmodifiableSet(values);
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("return std::collections::HashSet::new();")
            && translation.rust.contains("return input;")
            && translation.rust.contains("return values;")
            && !translation.rust.contains("JavaUtilCollections::empty_set")
            && !translation
                .rust
                .contains("JavaUtilCollections::unmodifiable_set"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "collections_empty_set_and_unmodifiable_set",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn arrays_as_list_single_string_array_spreads_for_add_all() {
    let translation = translate_source(&idiom_gap("ArraysAsListSplitAddAll.java"));

    assert!(
        translation
            .rust
            .contains("fields.extend(java_string_split(&filter_string, \",\", 0));")
            && !translation.rust.contains("extend(vec![java_string_split")
            && !translation.rust.contains(".add_all("),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "arrays_as_list_split_add_all",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
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
fn map_key_set_size_lowers_through_native_len() {
    let translation = translate_source(
        r#"
import java.util.HashMap;

class MapKeySetSize {
    int count(HashMap<String, Object> values) {
        return values.keySet().size();
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("values.keys().cloned().collect::<Vec<_>>().len() as i32")
            && !translation.rust.contains(".size()"),
        "{}",
        translation.rust
    );
}

#[test]
fn map_key_set_returning_set_collects_hashset_for_set_slot() {
    let translation = translate_source(
        r#"
import java.util.HashMap;
import java.util.Set;

class MapKeySetReturnSet {
    Set<String> keys(HashMap<String, Object> values) {
        return values.keySet();
    }
}
"#,
    );

    assert!(
        translation.rust.contains("pub fn keys(&self, values:")
            && translation
                .rust
                .contains(") -> std::collections::HashSet<String>")
            && translation.rust.contains(
                "return values.keys().cloned().collect::<std::collections::HashSet<_>>();"
            )
            && !translation
                .rust
                .contains("return values.keys().cloned().collect::<Vec<_>>();"),
        "{}",
        translation.rust
    );
}

#[test]
fn map_values_returning_collection_collects_vec() {
    let translation = translate_source(
        r#"
import java.util.Collection;
import java.util.HashMap;

class MapValuesReturnCollection {
    Collection<String> values(HashMap<String, String> values) {
        return values.values();
    }
}
"#,
    );

    assert!(
        translation.rust.contains(") -> Vec<String>")
            && translation
                .rust
                .contains("return values.values().cloned().collect::<Vec<_>>();")
            && !translation.rust.contains("return values.values();"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "map_values_return_collection",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn map_values_returning_set_collects_hashset_for_set_slot() {
    let translation = translate_source(
        r#"
import java.util.HashMap;
import java.util.Set;

class MapValuesReturnSet {
    Set<String> values(HashMap<String, String> values) {
        return values.values();
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains(") -> std::collections::HashSet<String>")
            && translation.rust.contains(
                "return values.values().cloned().collect::<std::collections::HashSet<_>>();"
            )
            && !translation
                .rust
                .contains("return values.values().cloned().collect::<Vec<_>>();")
            && !translation.rust.contains("return values.values();"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "map_values_return_set",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn map_values_returning_hashset_collects_hashset_for_concrete_set_slot() {
    let translation = translate_source(
        r#"
import java.util.HashMap;
import java.util.HashSet;
import java.util.LinkedHashSet;

class MapValuesReturnHashSet {
    HashSet<String> values(HashMap<String, String> values) {
        return values.values();
    }

    HashSet<String> localValues(String key, String value) {
        HashMap<String, String> values = new HashMap<String, String>();
        values.put(key, value);
        return values.values();
    }

    HashSet<String> linkedValues(HashMap<String, String> values) {
        return new LinkedHashSet<String>(values.values());
    }
}
"#,
    );

    let hashset_collect =
        "return values.values().cloned().collect::<std::collections::HashSet<_>>();";
    assert!(
        translation
            .rust
            .contains(") -> std::collections::HashSet<String>")
            && translation.rust.matches(hashset_collect).count() == 3
            && !translation
                .rust
                .contains("return values.values().cloned().collect::<Vec<_>>();")
            && !translation.rust.contains("return values.values();"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "map_values_return_hashset",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn map_put_all_lowers_to_hash_map_extend() {
    let translation = translate_source(
        r#"
import java.util.HashMap;

class MapPutAllJavaApi {
    private HashMap<String, String> values = new HashMap<String, String>();
    private HashMap<String, String> maybeValues;

    void merge(HashMap<String, String> other) {
        values.putAll(other);
    }

    void mergeMaybe(HashMap<String, String> other) {
        maybeValues.putAll(other);
    }

    HashMap<String, String> copy() {
        HashMap<String, String> out = new HashMap<String, String>();
        out.putAll(values);
        return out;
    }
}
"#,
    );

    assert!(
        translation.rust.contains(".extend(")
            && translation
                .rust
                .contains("self.maybe_values.as_mut().unwrap().extend(")
            && !translation.rust.contains(".put_all(")
            && !translation.rust.contains(".putAll("),
        "{}",
        translation.rust
    );
}

#[test]
fn map_put_all_boxes_values_for_erased_object_value_map() {
    let translation = translate_source(
        r#"
import java.util.HashMap;

class MapPutAllErasedValues {
    HashMap<String, Object> copy(HashMap<String, String> fields) {
        HashMap<String, Object> out = new HashMap<String, Object>();
        out.putAll(fields);
        return out;
    }
}
"#,
    );

    assert!(
        translation.rust.contains(
            "out.extend(fields.into_iter().map(|(k, v)| (k, Box::new(v) as Box<dyn std::any::Any>)));"
        ) && !translation.rust.contains("out.extend(fields);")
            && !translation.rust.contains(".put_all("),
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
fn source_array_setter_arguments_follow_owned_storage_signature() {
    let translation = translate_source(&idiom_gap("ArraySetterOwnedStorage.java"));

    assert!(
        translation
            .rust
            .contains("pub fn set_letters(&mut self, letters: Vec<char>)")
            && translation
                .rust
                .contains("let mut out: ArraySetterOwnedStorage = ArraySetterOwnedStorage::new();")
            && translation
                .rust
                .contains("out.set_letters(format!(\"{}{}\", left, right).chars().collect::<Vec<char>>())")
            && translation
                .rust
                .contains("out.set_letters(source.letters.as_ref().cloned()/* TODO(translation): validate added clone */.expect(\"Java null receiver\"))")
            && !translation.rust.contains(".set_letters(format!(\"{}{}\", left, right).chars().collect::<Vec<char>>().as_slice())")
            && !translation.rust.contains(".set_letters(source.letters.as_ref().expect(\"Java null receiver\").as_slice())"),
        "{}",
        translation.rust
    );
}

#[test]
fn shared_self_field_reads_recover_owned_values_without_moves() {
    let translation = translate_source(&idiom_gap("SharedSelfFieldOwnedRecovery.java"));

    assert!(
        translation
            .rust
            .contains("return self.name.clone();")
            && translation.rust.contains("return self.workspace.clone();")
            && translation.rust.contains(
                "return self.paths.clone()/* TODO(translation): validate added clone */;"
            )
            && translation
                .rust
                .contains("self.length(self.workspace.as_str())")
            && translation.rust.contains(
                "map.insert(\"ID\".to_string(), Box::new(self.name.clone()) as Box<dyn std::any::Any>);"
            )
            && translation.rust.contains(
                "map.insert(\"Workspace\".to_string(), Box::new(self.workspace.clone()) as Box<dyn std::any::Any>);"
            )
            && translation.rust.contains(
                "map.insert(\"Tag\".to_string(), self.tag.as_ref().map(|value| Box::new(value.clone()) as Box<dyn std::any::Any>).expect(\"Java null receiver\"));"
            )
            && translation.rust.contains(
                "map.insert(\"Paths\".to_string(), Box::new(self.paths.clone()/* TODO(translation): validate added clone */) as Box<dyn std::any::Any>);"
            )
            && !translation.rust.contains("return self.name;")
            && !translation.rust.contains("return self.workspace;")
            && !translation.rust.contains("return self.paths;")
            && !translation.rust.contains("Box::new(self.name) as Box<dyn std::any::Any>")
            && !translation
                .rust
                .contains("Box::new(self.workspace) as Box<dyn std::any::Any>")
            && !translation.rust.contains("Box::new(self.tag.unwrap())")
            && !translation.rust.contains("Box::new(self.paths) as Box<dyn std::any::Any>")
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
fn closed_enum_variant_borrows_as_dyn_trait_parameter() {
    let translation = translate_source(
        r#"
interface Localizable {
    String getSourceString();
}

enum LocalizedFormats implements Localizable {
    CONVERGENCE_FAILED("convergence failed");

    private final String source;

    LocalizedFormats(String source) {
        this.source = source;
    }

    public String getSourceString() {
        return source;
    }
}

class UseLocalized {
    String message() {
        return format(LocalizedFormats.CONVERGENCE_FAILED);
    }

    String format(Localizable pattern) {
        return pattern.getSourceString();
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("&LocalizedFormats::CONVERGENCE_FAILED")
            && translation.rust.contains("as &dyn Localizable")
            && translation
                .rust
                .contains("impl Localizable for LocalizedFormats")
            && translation.rust.contains("self.to_string()")
            && !translation
                .rust
                .contains("unsupported Java dispatch coercion")
            && !translation.rust.contains("compile_error!"),
        "{}",
        translation.rust
    );
}

#[test]
fn closed_enum_variant_constructor_delegation_supplies_empty_varargs_without_optional_as_ref() {
    let translation = translate_source(
        r#"
interface Localizable {
    String getSourceString();
}

enum LocalizedFormats implements Localizable {
    ILLEGAL_STATE("illegal state");

    private final String source;

    LocalizedFormats(String source) {
        this.source = source;
    }

    public String getSourceString() {
        return source;
    }
}

class LocalizedException {
    LocalizedException(Localizable pattern, Object... args) {
    }

    LocalizedException() {
        this(LocalizedFormats.ILLEGAL_STATE);
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("&LocalizedFormats::ILLEGAL_STATE as &dyn Localizable")
            && translation.rust.contains("Vec::new()")
            && !translation.rust.contains("ILLEGAL_STATE.as_ref()")
            && !translation.rust.contains("Java null receiver"),
        "{}",
        translation.rust
    );
}

#[test]
fn source_java_enum_constant_uses_slot_plan_not_optional_projection() {
    let translation = translate_source(
        r#"
class EnumConstantOptionalUse {
    enum HeaderVersion {
        V1;

        boolean isAtLeast(HeaderVersion other) {
            return true;
        }
    }

    private HeaderVersion version;

    EnumConstantOptionalUse() {
        version = null;
    }

    void set() {
        version = HeaderVersion.V1;
    }

    boolean check(HeaderVersion from) {
        HeaderVersion target = HeaderVersion.V1;
        if (from != null && from.isAtLeast(HeaderVersion.V1)) {
            return version == HeaderVersion.V1;
        }
        return target == HeaderVersion.V1;
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("self.version = Some(EnumConstantOptionalUseHeaderVersion::V1)")
            && translation
                .rust
                .contains(".is_at_least(EnumConstantOptionalUseHeaderVersion::V1)")
            && !translation
                .rust
                .contains("EnumConstantOptionalUseHeaderVersion::V1.unwrap()")
            && !translation
                .rust
                .contains("EnumConstantOptionalUseHeaderVersion::V1.as_ref()")
            && !translation
                .rust
                .contains("EnumConstantOptionalUseHeaderVersion::V1.map("),
        "{}",
        translation.rust
    );
}

#[test]
fn source_interface_collection_elements_use_boxed_dyn_trait() {
    let translation = translate_source(
        r#"
import java.util.ArrayList;
import java.util.List;

interface Localizable {
    String getSourceString();
}

class LocalizableStore {
    private List<Localizable> patterns;

    LocalizableStore() {
        patterns = new ArrayList<Localizable>();
    }

    void add(Localizable pattern) {
        patterns.add(pattern);
    }

    Localizable first(int index) {
        return patterns.get(index);
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("patterns: Vec<Box<dyn Localizable>>")
            && translation
                .rust
                .contains("self.patterns.push(pattern.clone_box());")
            && translation
                .rust
                .contains("return (self.patterns.get(index as usize).expect(\"Java index out of bounds\")).clone()")
            && !translation.rust.contains("Box::new(self.patterns.get")
            && !translation.rust.contains("patterns: Vec<Localizable>")
            && !translation.rust.contains("patterns: Vec<dyn Localizable>"),
        "{}",
        translation.rust
    );
}

#[test]
fn this_argument_can_borrow_inherited_interface_as_dyn_trait() {
    let translation = translate_source(
        r#"
interface Distribution {
    double value();
}

abstract class AbstractDistribution implements Distribution {
}

class RandomSampler {
    double sample(Distribution distribution) {
        return distribution.value();
    }
}

class ConcreteDistribution extends AbstractDistribution {
    private RandomSampler sampler = new RandomSampler();

    public double value() {
        return 1.0;
    }

    double sample() {
        return sampler.sample(this);
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("impl Distribution for ConcreteDistribution")
            && translation.rust.contains("sampler.sample(self)")
            && !translation.rust.contains(
                "unsupported Java erased object to borrowed dyn trait recovery for Distribution"
            ),
        "{}",
        translation.rust
    );
    cargo_check_generated_modules(
        "this_argument_inherited_interface_dyn_trait",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn source_interface_object_getter_returns_borrowed_field() {
    let translation = translate_source(
        r#"
interface ContextProvider {
    Context getContext();
}

class Context {
    Object value;
}

class HasContext implements ContextProvider {
    private final Context context;

    HasContext(Context context) {
        this.context = context;
    }

    public Context getContext() {
        return context;
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("fn get_context(&self) -> &Context;")
            && translation
                .rust
                .contains("pub fn get_context(&self) -> &Context")
            && translation.rust.contains("return &self.context;")
            && translation
                .rust
                .contains("fn get_context(&self) -> &Context { HasContext::get_context(self) }")
            && !translation.rust.contains("return self.context.clone();"),
        "{}",
        translation.rust
    );
}

#[test]
fn source_interface_trait_impl_unwraps_nullable_implementation_return() {
    let translation = translate_source(
        r#"
interface IdSource {
    String getID();
}

class HeaderLine implements IdSource {
    String id;

    String getID() {
        return id;
    }
}
"#,
    );

    assert!(
        translation.rust.contains("fn get_i_d(&self) -> String;")
            && translation
                .rust
                .contains("pub fn get_i_d(&self) -> Option<String>")
            && translation.rust.contains(
                "fn get_i_d(&self) -> String { HeaderLine::get_i_d(self).expect(\"Java null receiver\") }"
            ),
        "{}",
        translation.rust
    );
}

#[test]
fn nested_interface_type_names_are_valid_rust_identifiers() {
    let translation = translate_source(
        r#"
class Incrementor {
    interface MaxCountExceededCallback {
        void trigger(int maximalCount);
    }

    private final int maximalCount;
    private MaxCountExceededCallback callback;

    Incrementor(int maximalCount) {
        this.maximalCount = maximalCount;
    }

    Incrementor(int maximalCount, MaxCountExceededCallback callback) {
        this.maximalCount = maximalCount;
        this.callback = callback;
    }
}

"#,
    );

    assert!(
        translation
            .rust
            .contains("pub trait IncrementorMaxCountExceededCallback")
            && translation
                .rust
                .contains("Box<dyn IncrementorMaxCountExceededCallback>")
            && translation
                .rust
                .contains("new_int_incrementor_max_count_exceeded_callback")
            && !translation.rust.contains(
                "pub type MaxCountExceededCallback = IncrementorMaxCountExceededCallback;"
            )
            && !translation
                .rust
                .contains("Incrementor.MaxCountExceededCallback")
            && !translation.rust.contains("compile_error!"),
        "{}",
        translation.rust
    );
}

#[test]
fn nested_interface_implementor_forwarding_uses_flattened_rust_type_name() {
    let translation = translate_source(
        r#"
interface Headered {
    Header getHeader();
}

class Header {
}

class Builder {
    private static class Reader implements Headered {
        private Header header;

        public Header getHeader() {
            return this.header;
        }
    }
}
"#,
    );

    assert!(
        translation.rust.contains("impl Headered for BuilderReader")
            && translation
                .rust
                .contains("fn get_header(&self) -> &Header { BuilderReader::get_header(self) }")
            && !translation.rust.contains("Builder.Reader::get_header")
            && !translation.rust.contains("compile_error!"),
        "{}",
        translation.rust
    );
}

#[test]
fn anonymous_nested_interface_constructor_lowers_to_boxed_dyn_handoff() {
    let translation = translate_source(
        r#"
class Incrementor {
    interface MaxCountExceededCallback {
        void trigger(int maximalCount);
    }

    private MaxCountExceededCallback callback;

    Incrementor(int maximalCount) {
        this(maximalCount, new MaxCountExceededCallback() {
            public void trigger(int max) {
            }
        });
    }

    Incrementor(int maximalCount, MaxCountExceededCallback callback) {
        this.callback = callback;
    }
}
"#,
    );

    assert!(
        translation.rust.contains(
            "pub type MaxCountExceededCallback = dyn IncrementorMaxCountExceededCallback;"
        ) && translation
            .rust
            .contains("impl dyn IncrementorMaxCountExceededCallback")
            && translation
                .rust
                .contains("<dyn IncrementorMaxCountExceededCallback>::anonymous()")
            && !translation
                .rust
                .contains("&dyn IncrementorMaxCountExceededCallback::anonymous")
            && !translation.rust.contains("anonymous().clone_box()")
            && !translation.rust.contains("compile_error!"),
        "{}",
        translation.rust
    );
}

#[test]
fn lowercase_keyword_receiver_is_not_guessed_as_static_owner() {
    let translation = translate_source(
        r#"
class FilterInputStream {
}

class KeywordReceiverStream extends FilterInputStream {
    int read() {
        return in.read();
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("self.r#in.as_ref().expect(\"Java null receiver\").read_0()")
            && !translation.rust.contains("in::read")
            && !translation.rust.contains("return in.read()"),
        "{}",
        translation.rust
    );
}

#[test]
fn receiverless_overloaded_static_same_class_call_uses_self_and_arg_types() {
    let translation = translate_source(
        r#"
class ReceiverlessOverloadedStaticCall {
    static double round(double x, int scale) {
        return round(x, scale, 1);
    }

    static double round(double x, int scale, int mode) {
        return x + scale + mode;
    }

    static float round(float x, int scale) {
        return round(x, scale, 1);
    }

    static float round(float x, int scale, int mode) {
        return x + scale + mode;
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("return Self::round_double_int_int(x, scale, 1);")
            && translation
                .rust
                .contains("return Self::round_float_int_int(x, scale, 1);")
            && !translation.rust.contains("return round(x, scale, 1);"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "receiverless_overloaded_static_call",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn receiverless_instance_overload_delegation_uses_current_self_and_subtype_argument() {
    let translation = translate_source(
        r#"
class ReceiverlessInstanceOverloadDelegation {
    static class BaseStream {}
    static class LocalPath {}
    static class LocalFunction {}
    static class LocalSeekablePathStream extends BaseStream {
        LocalSeekablePathStream(LocalPath path, LocalFunction wrapper) {}
    }
    static class Iterator {}

    Iterator open(BaseStream in) {
        return new Iterator();
    }

    Iterator open(LocalPath path, LocalFunction wrapper) {
        return open(new LocalSeekablePathStream(path, wrapper));
    }
}
"#,
    );

    assert!(
        translation.rust.contains("return self.open_")
            && !translation.rust.contains("return open("),
        "{}",
        translation.rust
    );
}

#[test]
fn external_receiverless_instance_overload_delegation_uses_pack_constructor_and_conversion() {
    let translation = translate_source(
        r#"
import java.io.InputStream;
import java.nio.file.Path;
import java.util.Iterator;
import java.util.function.Function;
import htsjdk.samtools.seekablestream.SeekableByteChannel;
import htsjdk.samtools.seekablestream.SeekablePathStream;

class ExternalReceiverlessInstanceOverloadDelegation {
    Iterator open(InputStream in) {
        return null;
    }

    Iterator open(Path path, Function<SeekableByteChannel, SeekableByteChannel> wrapper) {
        return open(new SeekablePathStream(path, wrapper));
    }
}
"#,
    );

    assert!(
        translation.rust.contains("return self.open_input_stream(")
            && translation.rust.contains("SeekablePathStream::new_")
            && translation.rust.contains(".into())")
            && !translation.rust.contains("return open("),
        "{}",
        translation.rust
    );
}

#[test]
fn lowercase_jdk_static_owner_aliases_recover_canonical_static_calls() {
    let translation = translate_source(
        r#"
class LowercaseJdkStaticOwners {
    double normalize(double x, double y) {
        if (Double.isInfinite(x)) {
            return Math.sqrt(y);
        }
        return Math.pow(x, 2.0);
    }

    double remainder(double x, double y) {
        return StrictMath.IEEEremainder(x, y);
    }

    void copy(byte[] from, byte[] to, int n) {
        System.arraycopy(from, 0, to, 0, n);
    }
}
"#,
    );

    assert!(
        translation.rust.contains("Double::is_infinite")
            && translation.rust.contains("(y as f64).sqrt()")
            && translation.rust.contains("(x as f64).powf(2f64 as f64)")
            && translation.rust.contains("StrictMath::i_e_e_eremainder")
            && translation.rust.contains("System::arraycopy")
            && !translation.rust.contains("pub struct Math")
            && !translation.rust.contains("pub fn pow")
            && translation.rust.contains("pub struct StrictMath")
            && translation.rust.contains("pub fn i_e_e_eremainder")
            && translation.rust.contains("pub struct Double")
            && translation.rust.contains("pub fn is_infinite")
            && translation.rust.contains("pub struct System")
            && translation.rust.contains("pub fn arraycopy")
            && !translation.rust.contains("double.is_infinite")
            && !translation.rust.contains("math.sqrt")
            && !translation.rust.contains("math.pow")
            && !translation.rust.contains("strict_math.i_e_e_eremainder")
            && !translation.rust.contains("system.arraycopy"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "lowercase_jdk_static_owners",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn array_element_assignment_to_bare_field_requires_mutable_self() {
    let translation = translate_source(
        r#"
class ArrayElementAssignmentToBareField {
    private double[] values;

    double replace(double value) {
        double old = values[0];
        values[0] = value;
        return old;
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("pub fn replace(&mut self, value: f64) -> f64")
            && translation.rust.contains(
                "self.values.as_mut().expect(\"Java null receiver\")[0 as usize] = value;"
            )
            && !translation.rust.contains("pub fn replace(&self"),
        "{}",
        translation.rust
    );
}

#[test]
fn system_arraycopy_uses_signature_fact_slice_slots_for_optional_array_fields() {
    let translation = translate_source(
        r#"
class SystemArraycopyOptionalArrayFields {
    private double[] source;
    private double[] dest;

    void copy(int n) {
        System.arraycopy(source, 0, dest, 0, n);
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("pub fn copy(&mut self, n: i32)")
            && translation.rust.contains(
                "System::arraycopy(self.source.as_ref().expect(\"Java null receiver\").as_slice(), 0"
            )
            && translation.rust.contains(
                "self.dest.as_mut().expect(\"Java null receiver\").as_mut_slice(), 0"
            )
            && !translation.rust.contains("System::arraycopy(self.source, 0, self.dest,"),
        "{}",
        translation.rust
    );
}

#[test]
fn system_arraycopy_uses_signature_fact_slice_slots_for_parameter_fields() {
    let translation = translate_source(
        r#"
class SystemArraycopyParameterFields {
    double[] values;

    static void copy(SystemArraycopyParameterFields source,
                     SystemArraycopyParameterFields dest) {
        dest.values = new double[source.values.length];
        System.arraycopy(source.values, 0, dest.values, 0, dest.values.length);
    }
}
"#,
    );

    assert!(
        translation.rust.contains(
            "pub fn copy(source: &SystemArraycopyParameterFields, dest: &mut SystemArraycopyParameterFields)"
        ) && translation.rust.contains(
            "System::arraycopy(source.values.as_ref().expect(\"Java null receiver\").as_slice(), 0"
        ) && translation.rust.contains(
            "dest.values.as_mut().expect(\"Java null receiver\").as_mut_slice(), 0"
        ) && !translation.rust.contains("System::arraycopy(source.values, 0, dest.values,"),
        "{}",
        translation.rust
    );
}

#[test]
fn system_arraycopy_mutable_optional_local_dest_does_not_clone_slice() {
    let translation = translate_source(
        r#"
class SystemArraycopyOptionalLocalDest {
    byte[] copy(byte[] source, int n) {
        byte[] dest = null;
        dest = new byte[n];
        System.arraycopy(source, 0, dest, 0, n);
        return dest;
    }
}
"#,
    );

    assert!(
        translation.rust.contains("System::arraycopy")
            && translation
                .rust
                .contains("dest.as_mut().expect(\"Java null receiver\").as_mut_slice(), 0")
            && !translation.rust.contains("as_mut_slice().clone()"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "system_arraycopy_mutable_optional_local_dest_does_not_clone_slice",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn raw_cast_left_operand_is_parenthesized_before_ordered_comparison() {
    let translation = translate_source(
        r#"
class CastComparison {
    boolean shorter(String value, int limit) {
        return value.length() < limit;
    }
}
"#,
    );

    assert!(
        translation.rust.contains("(value.len() as i32) < limit")
            && !translation.rust.contains("value.len() as i32 < limit"),
        "{}",
        translation.rust
    );
}

#[test]
fn boolean_xor_of_comparisons_parenthesizes_operands() {
    let translation = translate_source(
        r#"
class BooleanXorComparison {
    boolean check(double direction, double d, long sign) {
        return (direction < d) ^ (sign == 0L);
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("return (direction < d) ^ (sign == 0i64);")
            && !translation.rust.contains("return direction < d ^ sign"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "boolean_xor_comparison",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn java_numeric_literal_suffixes_are_removed_or_rust_typed() {
    let translation = translate_source(
        r#"
class NumericSuffixes {
    float f() {
        return 2.5f;
    }

    double d() {
        return 10.0D;
    }

    long mask() {
        return 0x7ff0000000000000L;
    }
}
"#,
    );

    assert!(
        translation.rust.contains("return 2.5f32;")
            && translation.rust.contains("return 10f64;")
            && translation.rust.contains("9218868437227405312i64")
            && !translation.rust.contains("2.5f;")
            && !translation.rust.contains("10.0D")
            && !translation.rust.contains("0000L"),
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
fn collection_remove_all_statement_uses_retain_with_contains() {
    let translation = translate_source(
        r#"
import java.util.HashSet;
import java.util.Set;

class CollectionRemoveAllStatement {
    void remove(Set<String> lines, HashSet<String> toRemove) {
        lines.removeAll(toRemove);
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("lines.retain(|item| !__java_remove_all_values.contains(item));")
            && !translation.rust.contains(".remove_all(")
            && !translation.rust.contains(".removeAll("),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "collection_remove_all_statement",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
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
            && translation.rust.contains("self.emit(name.as_str());")
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
    let rust_path = std::env::temp_dir().join("new_java2rust_for_each_lambda_side_effect_test.rs");
    let bin_path = std::env::temp_dir().join("new_java2rust_for_each_lambda_side_effect_test");
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
        fixture.copy(&Resource {}).unwrap();
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
fn same_package_uppercase_utility_receiver_lowers_as_static_owner() {
    let translation = translate_source(
        r#"
class SamePackageUtilityCaller {
    Object[] flatten(Object[] values) {
        return ArgUtils.flatten(values);
    }
}
"#,
    );

    assert!(
        translation.rust.contains("ArgUtils::flatten")
            && translation.rust.contains("pub struct ArgUtils")
            && !translation.rust.contains("arg_utils.flatten"),
        "{}",
        translation.rust
    );
}

#[test]
fn uppercase_utility_receiver_shadowed_by_local_stays_instance_receiver() {
    let translation = translate_source(
        r#"
class ShadowedUppercaseUtilityCaller {
    String flatten(Object ArgUtils) {
        return ArgUtils.toString();
    }
}
"#,
    );

    assert!(
        translation.rust.contains("arg_utils.to_string()")
            && !translation.rust.contains("ArgUtils::to_string"),
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
            .contains("let mut values: Vec<i8> = vec![0; len as usize];"),
        "{}",
        translation.rust
    );
    assert!(
        !translation.rust.contains("Java array creation"),
        "{}",
        translation.rust
    );
    assert!(
        translation.rust.contains("values.fill(1 as i8);"),
        "{}",
        translation.rust
    );
}

#[test]
fn primitive_array_allocation_casts_compound_length_as_whole_expression() {
    let translation = translate_source(
        r#"
class PrimitiveArrayAllocationCompoundLength {
    float[] trim(long[] data, int offset) {
        float[] values = new float[data.length - offset];
        return values;
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("let values: Vec<f32> = vec![0.0; ((data.len() as i32) - offset) as usize];")
            && !translation
                .rust
                .contains("(data.len() as i32) - offset as usize"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "primitive_array_allocation_compound_length",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
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
            && !translation
                .rust
                .contains("writer.write_next(Vec::new(), false);"),
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
                .contains("pub fn replace(&self, values: &mut std::collections::HashMap<String, String>, key: String, value: String) -> Option<String>")
            && translation
                .rust
                .contains("return values.get(key.as_str()).cloned();")
            && !translation.rust.contains(".put(")
            && !translation.rust.contains("&mut Map"),
        "{}",
        translation.rust
    );
}

#[test]
fn typed_set_add_source_constructor_keeps_concrete_element_slot() {
    let translation = translate_source(
        r#"
import java.util.LinkedHashSet;
import java.util.Set;

class VCFHeaderLine {
    private String key;
    private String value;

    VCFHeaderLine(String key, String value) {
        this.key = key;
        this.value = value;
    }

    public boolean equals(Object other) {
        if (!(other instanceof VCFHeaderLine)) {
            return false;
        }
        VCFHeaderLine that = (VCFHeaderLine) other;
        return key.equals(that.key) && value.equals(that.value);
    }

    public int hashCode() {
        return key.hashCode() + value.hashCode();
    }
}

class TypedSetAddSourceConstructor {
    Set<VCFHeaderLine> make() {
        Set<VCFHeaderLine> metaData = new LinkedHashSet<VCFHeaderLine>();
        metaData.add(new VCFHeaderLine("source", "tool"));
        return metaData;
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("meta_data.insert(VCFHeaderLine::new(")
            && !translation
                .rust
                .contains("meta_data.insert(Box::new(VCFHeaderLine::new"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "typed_set_add_source_constructor",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn object_map_put_keeps_erased_object_value_without_downcast() {
    let translation = translate_source(
        r#"
import java.util.HashMap;

class ObjectMapPut {
    private HashMap<String, Object> context = new HashMap<String, Object>();

    void setValue(String key, Object value) {
        context.put(key, value);
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("self.context.insert(key, value);")
            && !translation.rust.contains("downcast::<Object>")
            && !translation.rust.contains("invalid Java cast"),
        "{}",
        translation.rust
    );
}

#[test]
fn hashtable_put_lowers_to_native_map_insert() {
    let translation = translate_source(
        r#"
import java.io.File;
import java.util.Hashtable;
import java.util.Vector;

class HashtablePutUse {
    static Hashtable<String, Vector<File>> groups(File[] files) {
        Hashtable<String, Vector<File>> fileBases = new Hashtable<String, Vector<File>>();
        Vector<File> group = new Vector<File>();
        group.add(files[0]);
        fileBases.put(files[0].getName(), group);
        return fileBases;
    }
}
"#,
    );

    assert!(
        translation.rust.contains("file_bases.insert(")
            && !translation.rust.contains("file_bases.put(")
            && !translation.rust.contains("let mut file_bases: Hashtable"),
        "{}",
        translation.rust
    );
}

#[test]
fn map_get_collection_mutation_uses_mutable_lookup() {
    let translation = translate_source(
        r#"
import java.io.File;
import java.util.Hashtable;
import java.util.Vector;

class MapGetCollectionMutation {
    static Hashtable<String, Vector<File>> groups(File[] files) {
        Hashtable<String, Vector<File>> fileBases = new Hashtable<String, Vector<File>>();
        String baseName = files[0].getName();
        if (!fileBases.containsKey(baseName)) {
            fileBases.put(baseName, new Vector<File>());
        }
        fileBases.get(baseName).add(files[0]);
        return fileBases;
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("file_bases.get_mut(&base_name).expect(\"Java null receiver\").push(")
            && !translation
                .rust
                .contains("file_bases.get(&base_name).push(")
            && !translation.rust.contains("Option<T>"),
        "{}",
        translation.rust
    );
    cargo_check_generated_modules(
        "map_get_collection_mutation",
        vec![GeneratedCrateModule {
            name: "map_get_collection_mutation",
            rust: translation.rust.clone(),
            tests: "",
        }],
    );
}

#[test]
fn optional_map_field_put_lowers_through_mutable_optional_receiver() {
    let translation = translate_source(
        r#"
import java.util.HashMap;

class OptionalMapFieldPut {
    private HashMap<String, Kmer> kmers;

    void reset() {
        kmers = new HashMap<String, Kmer>();
    }

    void add(String kmer, int index) {
        if (!kmers.containsKey(kmer)) {
            kmers.put(new String(kmer), new Kmer(kmer, index));
        }
    }
}

class Kmer {
    Kmer(String value, int index) {
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("self.kmers.as_mut().unwrap().insert(")
            && !translation
                .rust
                .contains("self.kmers.as_ref().unwrap().put(")
            && !translation.rust.contains(".put("),
        "{}",
        translation.rust
    );
}

#[test]
fn queue_put_lowers_to_native_push_for_vec_backed_queue() {
    let translation = translate_source(
        r#"
import java.util.concurrent.ArrayBlockingQueue;

class QueuePutUse {
    static void send(ArrayBlockingQueue<String> queue, String value) throws InterruptedException {
        queue.put(value);
    }
}
"#,
    );

    assert!(
        translation.rust.contains("queue.push(value);") && !translation.rust.contains(".put("),
        "{}",
        translation.rust
    );
}

#[test]
fn byte_and_char_buffer_put_are_modeled_statement_facade_calls() {
    let translation = translate_source(
        r#"
import java.nio.ByteBuffer;
import java.nio.CharBuffer;

class BufferPutUse {
    static int encode(ByteBuffer bytes, CharBuffer chars, byte[] encoded) {
        int rem = bytes.hasRemaining() ? bytes.remaining() : 0;
        bytes.put(encoded);
        chars.put((char)(encoded[0] & 0xFF));
        return rem;
    }
}
"#,
    );

    assert!(
        translation.rust.contains("{ let _ = encoded")
            && translation.rust.contains("let rem: i32 = if bytes.has_remaining() { bytes.remaining() } else { 0 };")
            && translation.rust.contains("pub fn has_remaining(&self) -> bool")
            && translation.rust.contains("pub fn remaining(&self) -> i32")
            && translation
                .rust
                .contains("{ let _ = char::from_u32(((encoded[0 as usize] as i32 & 255) as u32) & 0xffff).unwrap_or('\\0'); };")
            && !translation.rust.contains(".put("),
        "{}",
        translation.rust
    );
}

#[test]
fn byte_values_widen_for_jdk_output_stream_and_bitwise_int_arithmetic() {
    let translation = translate_source(
        r#"
import java.io.OutputStream;

class ByteIntWidenUse {
    static final byte NEW_LINE = 10;

    static void writeLine(OutputStream out) throws java.io.IOException {
        out.write(NEW_LINE);
    }

    static int head(byte[] bytes) {
        return bytes[0] & 255 | ((bytes[1] << 8) & 65280);
    }

    static int nullableHead(byte[] input) {
        byte[] bytes = null;
        bytes = input;
        if (bytes != null && bytes.length >= 2) {
            return ((int)bytes[0] & 0xff) | ((bytes[1] << 8) & 0xff00);
        }
        return 0;
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("out.write_int(Self::NEW_LINE as i32)?;")
            && translation.rust.contains("bytes[0 as usize] as i32 & 255")
            && translation.rust.contains("(bytes[1 as usize] as i32) << 8")
            && translation
                .rust
                .contains("bytes.as_ref().expect(\"Java null receiver\")[0 as usize] as i32 & 255")
            && !translation.rust.contains("i8 | i32")
            && !translation.rust.contains("Self::NEW_LINE)?"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "byte_values_widen_for_jdk_output_stream_and_bitwise_int_arithmetic",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn fully_qualified_jdk_static_constant_lowers_through_external_field_fact() {
    let translation = translate_source(
        r#"
class FullyQualifiedJdkStaticConstantUse {
    static boolean isGzip(int head) {
        return java.util.zip.GZIPInputStream.GZIP_MAGIC == head;
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("pub const GZIP_MAGIC: i32 = 0x8b1f;")
            && translation
                .rust
                .contains("return GZIPInputStream::GZIP_MAGIC == head;")
            && !translation
                .rust
                .contains("java.util.zip.GZIPInputStream.GZIP_MAGIC"),
        "{}",
        translation.rust
    );

    cargo_test_generated_modules(
        "fully_qualified_jdk_static_constant_lowers_through_external_field_fact",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
#[cfg(test)]
mod generated_tests {
    use super::*;

    #[test]
    fn gzip_magic_has_java_value() {
        assert!(FullyQualifiedJdkStaticConstantUse::is_gzip(0x8b1f));
        assert!(!FullyQualifiedJdkStaticConstantUse::is_gzip(0));
    }
}
"#,
        }],
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
fn map_put_else_branch_reused_key_clones_before_later_block_read() {
    let translation = translate_source(
        r#"
import java.util.HashMap;

class MapPutElseBranchKeyReuse {
    static int count(HashMap<String, Integer> counts, HashMap<String, Integer> plus, String key) {
        if (counts.containsKey(key)) {
            counts.put(key, counts.get(key) + 1);
        } else {
            counts.put(key, 1);
        }
        if (plus.containsKey(key)) {
            return plus.get(key);
        }
        return 0;
    }
}
"#,
    );

    assert!(
        translation.rust.contains(
            "counts.insert(key.clone()/* TODO(translation): validate added clone */, 1);"
        ) && translation.rust.contains("plus.contains_key(key.as_str())"),
        "{}",
        translation.rust
    );
}

#[test]
fn map_put_else_branch_reassigned_key_clones_before_later_block_read() {
    let translation = translate_source(
        r#"
import java.util.HashMap;

class MapPutElseBranchReassignedKeyReuse {
    static int count(HashMap<String, Integer> counts, HashMap<String, Integer> plus, String raw) {
        String key = "";
        key = raw.toUpperCase();
        if (counts.containsKey(key)) {
            counts.put(key, counts.get(key) + 1);
        } else {
            counts.put(key, 1);
        }
        if (plus.containsKey(key)) {
            plus.put(key, plus.get(key) + 1);
        } else {
            plus.put(key, 1);
        }
        return counts.get(key);
    }
}
"#,
    );

    assert!(
        translation.rust.contains(
            "counts.insert(key.clone()/* TODO(translation): validate added clone */, 1);"
        ) && translation
            .rust
            .contains("plus.insert(key.clone()/* TODO(translation): validate added clone */, 1);")
            && translation.rust.contains("counts.get(key.as_str())"),
        "{}",
        translation.rust
    );
}

#[test]
fn optional_map_put_reused_value_borrows_before_clone_unwrap() {
    let translation = translate_source(&idiom_gap("OptionalMapPutValueReuse.java"));

    assert!(
        translation.rust.contains(
            "path.as_ref().cloned()/* TODO(translation): validate added clone */.expect(\"Java null receiver\")"
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
            "self.seen.insert(name.as_ref().cloned()/* TODO(translation): validate added clone */.expect(\"Java null receiver\"), \"seen\".to_string())"
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
fn map_get_into_optional_borrowed_arg_uses_optional_carrier_plan() {
    let translation = translate_source(
        r#"
import java.util.HashMap;

class MapGetOptionalBorrowedArg {
    boolean valid(String value) {
        return value != null && value.trim().equals(value);
    }

    boolean check(HashMap<String, String> values, String key) {
        return valid(values.get(key));
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("self.valid(values.get(key).as_ref().map(|value| value.as_str()))")
            && !translation.rust.contains("self.valid(Some(")
            && !translation
                .rust
                .contains(".get(key).expect(\"Java null receiver\")"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "map-get-optional-borrowed-arg",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn map_get_return_uses_nullable_signature_and_optional_carrier() {
    let translation = translate_source(
        r#"
import java.util.HashMap;

class MapGetNullableReturn {
    private HashMap<String, String> fields = new HashMap<String, String>();

    String fromParam(HashMap<String, String> values, String key) {
        return values.get(key);
    }

    String fromField(String key) {
        return fields.get(key);
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("pub fn from_param(&self, values: &std::collections::HashMap<String, String>, key: &str) -> Option<String>")
            && translation.rust.contains("return values.get(key).cloned();")
            && translation
                .rust
                .contains("pub fn from_field(&self, key: &str) -> Option<String>")
            && translation
                .rust
                .contains("return self.fields.get(key).cloned();")
            && !translation.rust.contains("-> String {\n        return values.get(key);")
            && !translation.rust.contains("-> String {\n        return self.fields.get(key);"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "map_get_nullable_return",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn map_get_optional_receiver_to_string_uses_java_null_receiver_policy() {
    let translation = translate_source(&idiom_gap("MapGetOptionalReceiverToString.java"));

    assert!(
        translation
            .rust
            .contains("paths.get(sample).expect(\"Java null receiver\").to_string()")
            && !translation
                .rust
                .contains("paths.get(sample.as_str()).to_string()")
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
fn map_get_primitive_value_in_arithmetic_unboxes_optional_borrow() {
    let translation = translate_source(
        r#"
import java.util.HashMap;

class MapGetPrimitiveArithmetic {
    int average(HashMap<String, Integer> totals, String key, int count) {
        return totals.get(key) / count;
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("totals.get(key).copied().expect(\"Java null receiver\") / count")
            && !translation
                .rust
                .contains("totals.get(key.as_str()) / count"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "map_get_primitive_arithmetic",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn compound_assignment_expression_argument_returns_assigned_value() {
    let translation = translate_source(
        r#"
class CompoundAssignmentArgument {
    int combine(int a, int b) {
        return take(a += 1, b -= 1);
    }

    int take(int a, int b) {
        return a + b;
    }
}
"#,
    );

    assert!(
        translation.rust.contains("let mut a = a;")
            && translation.rust.contains("let mut b = b;")
            && translation
                .rust
                .contains("self.take({ a = a + 1; a }, { b = b - 1; b })")
            && !translation.rust.contains("self.take(a += 1, b -= 1)"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "compound_assignment_argument",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn compound_assignment_on_narrow_primitives_casts_back_to_target_type() {
    let translation = translate_source(
        r#"
class CompoundAssignmentNarrowing {
    int byteAdd() {
        byte b = 10;
        b += 300;
        return b;
    }

    int shortMul() {
        short s = 1;
        s *= 100000;
        return s;
    }

    int charAdd() {
        char c = 'a';
        c += 1;
        return c;
    }

    int byteArg(byte b) {
        return take(b += 300);
    }

    int take(int value) {
        return value;
    }
}
"#,
    );

    assert!(
        translation.rust.contains("b = (b as i32).wrapping_add(300) as i8;")
            && translation
                .rust
                .contains("s = (s as i32).wrapping_mul(100000) as i16;")
            && translation.rust.contains(
                "c = char::from_u32(((((c as u32) as i32).wrapping_add(1)) as u32) & 0xffff).unwrap_or('\\0');"
            )
            && translation
                .rust
                .contains("self.take({ b = (b as i32).wrapping_add(300) as i8; b } as i32)"),
        "{}",
        translation.rust
    );

    cargo_test_generated_modules(
        "compound_assignment_narrowing",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
#[cfg(test)]
mod generated_tests {
    use super::*;

    #[test]
    fn narrow_compound_assignments_match_java_casts() {
        let fixture = CompoundAssignmentNarrowing {};
        assert_eq!(fixture.byte_add(), 54);
        assert_eq!(fixture.short_mul(), -31072);
        assert_eq!(fixture.char_add(), 98);
        assert_eq!(fixture.byte_arg(10), 54);
    }
}
"#,
        }],
    );
}

#[test]
fn array_assignment_with_side_effecting_index_preserves_java_order() {
    let translation = translate_source(
        r#"
class AssignmentOrderArrayIndex {
    int arr(int[] a) {
        int k = 0;
        a[k++] = k;
        return a[0];
    }

    int expr(int[] a) {
        int k = 0;
        return (a[k++] = k);
    }

    void plain(int[] a, int i, int v) {
        a[i] = v;
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("let __java_assignment_index = { let __java_old_value = k; k = (k).wrapping_add(1); __java_old_value } as usize;")
            && translation.rust.contains("a[__java_assignment_index] = k;")
            && translation.rust.contains("{ let __java_assignment_index = { let __java_old_value = k; k = (k).wrapping_add(1); __java_old_value } as usize; a[__java_assignment_index] = k; a[__java_assignment_index] }")
            && translation.rust.contains("a[i as usize] = v;")
            && !translation
                .rust
                .contains("a[{ let __java_old_value = k;"),
        "{}",
        translation.rust
    );

    cargo_test_generated_modules(
        "assignment_order_array_index",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
#[cfg(test)]
mod generated_tests {
    use super::*;

    #[test]
    fn array_assignment_index_runs_before_rhs() {
        let fixture = AssignmentOrderArrayIndex {};
        let mut values = vec![0, 0];
        assert_eq!(fixture.arr(&mut values), 1);
        assert_eq!(values[0], 1);
        values = vec![0, 0];
        assert_eq!(fixture.expr(&mut values), 1);
        assert_eq!(values[0], 1);
        assert_eq!(values[1], 0);
        fixture.plain(&mut values, 1, 7);
        assert_eq!(values[1], 7);
    }
}
"#,
        }],
    );
}

#[test]
fn compound_array_assignment_with_side_effecting_index_preserves_java_order() {
    let translation = translate_source(
        r#"
class CompoundAssignmentOrderArrayIndex {
    int stmt(int[] a) {
        int k = 0;
        a[k++] += k;
        return a[0];
    }

    int expr(int[] a) {
        int k = 0;
        return (a[k++] += k);
    }
}
"#,
    );

    assert!(
        translation.rust.contains("let __java_assignment_index = { let __java_old_value = k; k = (k).wrapping_add(1); __java_old_value } as usize;")
            && translation.rust.contains("let __java_assignment_old = a[__java_assignment_index];")
            && translation
                .rust
                .contains("a[__java_assignment_index] = (__java_assignment_old).wrapping_add(k);")
            && !translation.rust.contains("a[{ let __java_old_value = k;"),
        "{}",
        translation.rust
    );

    cargo_test_generated_modules(
        "compound_assignment_order_array_index",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
#[cfg(test)]
mod generated_tests {
    use super::*;

    #[test]
    fn compound_array_assignment_index_runs_once() {
        let fixture = CompoundAssignmentOrderArrayIndex {};
        let mut values = vec![10, 20];
        assert_eq!(fixture.stmt(&mut values), 11);
        assert_eq!(values, vec![11, 20]);
        values = vec![10, 20];
        assert_eq!(fixture.expr(&mut values), 11);
        assert_eq!(values, vec![11, 20]);
    }
}
"#,
        }],
    );
}

#[test]
fn chained_array_assignment_expression_returns_assigned_value() {
    let translation = translate_source(
        r#"
class ChainedArrayAssignmentExpression {
    static double reset(double values[]) {
        values[0] = values[1] = 0.0;
        return values[0];
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("values[0 as usize] = { values[1 as usize] = 0f64; values[1 as usize] };")
            && !translation
                .rust
                .contains("values[0 as usize] = values[1 as usize] = 0f64;"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "chained_array_assignment_expression",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn prefix_update_expression_argument_returns_updated_value() {
    let translation = translate_source(
        r#"
class PrefixUpdateArgument {
    int combine(int a, int b) {
        return take(++a, --b);
    }

    int take(int a, int b) {
        return a + b;
    }
}
"#,
    );

    assert!(
        translation.rust.contains("let mut a = a;")
            && translation.rust.contains("let mut b = b;")
            && translation
                .rust
                .contains("self.take({ a += 1; a }, { b -= 1; b })")
            && !translation.rust.contains("self.take(a += 1, b -= 1)"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "prefix_update_argument",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
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
            .contains("self.holder.as_mut().unwrap().add_string_int(name.to_string(), value);")
            && !translation.rust.contains(".add(name, value);"),
        "{}",
        translation.rust
    );
}

#[test]
fn inherited_receiverless_instance_call_uses_current_override_signature_name() {
    let source = r#"
import java.util.Map;

class HeaderBase {
    public String toString() {
        return toStringEncoding();
    }

    protected String toStringEncoding() {
        return "base";
    }

    static String toStringEncoding(Map<String, Object> values) {
        return "map";
    }
}

class HeaderChild extends HeaderBase {
    protected String toStringEncoding() {
        return "child";
    }
}
"#;
    let translation = translate_source(source);
    let child_impl = translation
        .rust
        .split("impl HeaderChild {")
        .nth(1)
        .and_then(|tail| tail.split("impl From<HeaderChild>").next())
        .unwrap_or(&translation.rust);

    assert!(
        translation
            .rust
            .contains("pub fn to_string(&self) -> String")
            && child_impl.contains("return self.to_string_encoding();")
            && translation
                .rust
                .contains("pub fn to_string_encoding(&self) -> String")
            && !child_impl.contains("self.to_string_encoding_()"),
        "{}",
        translation.rust
    );
}

#[test]
fn source_generic_class_preserves_type_params_and_interface_bounds() {
    let source = r#"
interface Named {
    String id();
}

class Box<T extends Named> {
    private T value;

    Box(T value) {
        this.value = value;
    }

    String id() {
        return value.id();
    }

    T get() {
        return value;
    }
}
"#;
    let translation = translate_source(source);

    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    assert!(
        translation.rust.contains("pub struct Box<T>")
            && translation.rust.contains("impl<T: Named + Clone> Box<T>")
            && translation.rust.contains("pub fn new(value: T) -> Box<T>")
            && translation.rust.contains("return Box::<T> {")
            && translation.rust.contains("return self.value.id();"),
        "{}",
        translation.rust
    );
}

#[test]
fn source_generic_method_preserves_type_params() {
    let translation = translate_source(
        r#"
class SourceGenericMethod {
    interface Named {
        String name();
    }

    static <U> U identity(U value) {
        return value;
    }

    <T> T instanceIdentity(T value) {
        return value;
    }

    static <N extends Named> String callName(N value) {
        return value.name();
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("pub fn identity<U>(value: U) -> U")
            && translation
                .rust
                .contains("pub fn instance_identity<T>(&self, value: T) -> T")
            && translation
                .rust
                .contains("pub fn call_name<N: SourceGenericMethodNamed>(value: N) -> String")
            && translation.rust.contains("return value.name();")
            && !translation.rust.contains("identity(value: U) -> U")
            && !translation
                .rust
                .contains("instance_identity(&self, value: T) -> T"),
        "{}",
        translation.rust
    );

    cargo_test_generated_modules(
        "source_generic_method",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
#[cfg(test)]
mod generated_tests {
    use super::*;

    #[test]
    fn generic_methods_round_trip_values() {
        let fixture = SourceGenericMethod {};
        assert_eq!(SourceGenericMethod::identity::<i32>(3), 3);
        assert_eq!(fixture.instance_identity::<String>("x".to_string()), "x");
    }
}
"#,
        }],
    );
}

#[test]
fn source_class_bound_trait_generic_methods_preserve_type_params() {
    let translation = translate_source(
        r#"
class Base {
    <T> T id(T value) {
        return value;
    }
}

class Holder<N extends Base> {
    private N base;

    Holder(N base) {
        this.base = base;
    }

    int use(int value) {
        return base.id(value);
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("pub trait BaseBound {\n    fn id<T>(&self, value: T) -> T where Self: Sized;")
            && translation
                .rust
                .contains("impl BaseBound for Base {\n    fn id<T>(&self, value: T) -> T where Self: Sized { Base::id(self, value) }")
            && translation.rust.contains("impl<N: BaseBound> Holder<N>")
            && translation.rust.contains("return self.base.id(value);"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "source_class_bound_trait_generic_methods",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn source_generic_constructor_preserves_type_param_bounds() {
    let translation = translate_source(
        r#"
interface Named {
    String name();
}

class GenericConstructor {
    private String name;

    <N extends Named> GenericConstructor(N value) {
        this.name = value.name();
    }

    String name() {
        return name;
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("pub fn new<N: Named>(value: N) -> GenericConstructor")
            && translation.rust.contains("name: value.name()")
            && !translation
                .rust
                .contains("pub fn new<N>(value: N) -> GenericConstructor"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "source_generic_constructor_bounds",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn context_source_generic_constructor_preserves_type_param_bounds() {
    let support = r#"
interface ContextNamed {
    String name();
}
"#;
    let source = r#"
class ContextGenericConstructor {
    private String name;

    <N extends ContextNamed> ContextGenericConstructor(N value) {
        this.name = value.name();
    }

    String name() {
        return name;
    }
}
"#;
    let translation = translate_source_with_context_sources_and_summary_sources(
        source,
        &[support.to_string()],
        &[],
        true,
        &FactSet::default(),
    );

    assert!(
        translation
            .rust
            .contains("pub fn new<N: ContextNamed>(value: N) -> ContextGenericConstructor")
            && translation.rust.contains("name: value.name()")
            && !translation
                .rust
                .contains("pub fn new<N>(value: N) -> ContextGenericConstructor"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "context_source_generic_constructor_bounds",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: format!(
                "pub trait ContextNamed {{ fn name(&self) -> String; }}\n{}",
                translation.rust
            ),
            tests: "",
        }],
    );
}

#[test]
fn non_emitted_context_generic_constructor_does_not_emit_missing_bound() {
    let support = r#"
interface ContextOnlyNamed {
    String name();
}
"#;
    let source = r#"
class ContextOnlyGenericConstructor {
    private String name;

    <N extends ContextOnlyNamed> ContextOnlyGenericConstructor(N value) {
        this.name = value.name();
    }
}
"#;
    let translation =
        translate_source_with_context_sources(source, &[support.to_string()], &FactSet::default());

    assert!(
        translation
            .rust
            .contains("pub fn new<N>(value: N) -> ContextOnlyGenericConstructor")
            && !translation.rust.contains("N: ContextOnlyNamed"),
        "{}",
        translation.rust
    );
}

#[test]
fn context_source_generic_method_preserves_type_param_bounds() {
    let support = r#"
interface ContextMethodNamed {
    String name();
}
"#;
    let source = r#"
class ContextGenericMethod {
    <N extends ContextMethodNamed> String encode(N value) {
        return value.name();
    }
}
"#;
    let translation = translate_source_with_context_sources_and_summary_sources(
        source,
        &[support.to_string()],
        &[],
        true,
        &FactSet::default(),
    );

    assert!(
        translation
            .rust
            .contains("pub fn encode<N: ContextMethodNamed>(&self, value: N) -> String")
            && translation.rust.contains("value.name()")
            && !translation
                .rust
                .contains("pub fn encode<N>(&self, value: N) -> String"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "context_source_generic_method_bounds",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: format!(
                "pub trait ContextMethodNamed {{ fn name(&self) -> String; }}\n{}",
                translation.rust
            ),
            tests: "",
        }],
    );
}

#[test]
fn generic_source_map_storage_return_adds_clone_bound() {
    let source = r#"
import java.util.HashMap;
import java.util.Map;

interface Named {
    String id();
}

class Standards<T extends Named> {
    private Map<String, T> standards = new HashMap<String, T>();

    void add(T line) {
        standards.put(line.id(), line);
    }

    T get(String id) {
        return standards.get(id);
    }
}
"#;
    let translation = translate_source(source);

    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    assert!(
        translation
            .rust
            .contains("impl<T: Named + Clone> Standards<T>")
            && translation.rust.contains(".cloned()"),
        "{}",
        translation.rust
    );
}

#[test]
fn source_generic_class_bound_method_call_uses_bound_surface() {
    let source = r#"
class HeaderLine {
    String id;

    String getID() {
        return id;
    }
}

class FormatLine extends HeaderLine {
}

class Box<T extends HeaderLine> {
    private T value;

    Box(T value) {
        this.value = value;
    }

    String id() {
        return value.getID();
    }
}
"#;
    let translation = translate_source(source);

    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    assert!(
        translation.rust.contains("pub struct Box<T>")
            && translation.rust.contains("pub trait HeaderLineBound")
            && translation.rust.contains("fn get_i_d(&self) -> String;")
            && translation
                .rust
                .contains("impl HeaderLineBound for HeaderLine")
            && translation
                .rust
                .contains("impl HeaderLineBound for FormatLine")
            && translation.rust.contains("impl<T: HeaderLineBound> Box<T>")
            && translation
                .rust
                .contains("HeaderLine::get_i_d(self).expect(\"Java null receiver\")")
            && translation.rust.contains("pub fn id(&self) -> String")
            && translation.rust.contains("return self.value.get_i_d();"),
        "{}",
        translation.rust
    );
}

#[test]
fn source_class_bound_inherited_mutator_forwards_to_child_method() {
    let source = r#"
class HeaderLine {
    String source;

    void setSource(String source) {
        this.source = source;
    }
}

class FormatLine extends HeaderLine {
}

class Box<T extends HeaderLine> {
    private T value;

    Box(T value) {
        this.value = value;
    }

    void update(String source) {
        value.setSource(source);
    }
}
"#;
    let translation = translate_source(source);

    assert!(
        translation.rust.contains("pub trait HeaderLineBound")
            && translation
                .rust
                .contains("fn set_source(&mut self, source: String);")
            && translation
                .rust
                .contains("impl HeaderLineBound for FormatLine")
            && translation.rust.contains(
                "fn set_source(&mut self, source: String) { FormatLine::set_source(self, source); }"
            )
            && !translation.rust.contains("HeaderLine::set_source(&{")
            && !translation.rust.contains("HeaderLine::set_source(&mut {"),
        "{}",
        translation.rust
    );
}

#[test]
fn source_generic_class_bound_overload_call_uses_bound_rust_name() {
    let source = r#"
class HeaderLine {
    int getCount() {
        return 1;
    }

    int getCount(String value) {
        return 2;
    }
}

class Box<T extends HeaderLine> {
    private T value;

    Box(T value) {
        this.value = value;
    }

    int count() {
        return value.getCount();
    }
}
"#;
    let translation = translate_source(source);

    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    assert!(
        translation.rust.contains("fn get_count_(&self) -> i32;")
            && translation.rust.contains("impl<T: HeaderLineBound> Box<T>")
            && translation.rust.contains("return self.value.get_count_();")
            && !translation.rust.contains("return self.value.get_count();"),
        "{}",
        translation.rust
    );
}

#[test]
fn optional_type_param_bound_call_display_uses_bound_return_shape() {
    let source = r#"
class HeaderLine {
    String id;

    String getID() {
        return id;
    }
}

class Box<T extends HeaderLine> {
    String describe(T line) {
        if (line == null) {
            return "missing";
        }
        return String.format("%s", line.getID());
    }
}
"#;
    let translation = translate_source(source);

    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    assert!(
        translation
            .rust
            .contains("line.as_ref().unwrap().get_i_d()")
            && !translation
                .rust
                .contains("line.as_ref().unwrap().get_i_d().as_deref()")
            && !translation
                .rust
                .contains("line.as_ref().unwrap().get_i_d().unwrap()"),
        "{}",
        translation.rust
    );
}

#[test]
fn type_param_bound_enum_return_equality_uses_bound_return_shape() {
    let source = r#"
class BoundEnumReturnEquality {
    enum CountType {
        INTEGER,
        A
    }

    static class Line {
        CountType getCountType() {
            return CountType.INTEGER;
        }
    }

    static class Standards<T extends Line> {
        boolean bad(T line, T standard) {
            return line.getCountType() != standard.getCountType();
        }
    }
}
"#;
    let translation = translate_source(source);

    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    assert!(
        translation
            .rust
            .contains("line.get_count_type() != standard.get_count_type()")
            && !translation.rust.contains("get_count_type().as_ref()"),
        "{}",
        translation.rust
    );
}

#[test]
fn type_param_bound_source_supertype_assignment_uses_into_bound() {
    let source = r#"
class HeaderLine {
    String id;
}

class CompoundLine extends HeaderLine {
}

class FormatLine extends CompoundLine {
}

class Standards<T extends CompoundLine> {
    private T value;

    Standards(T value) {
        this.value = value;
    }

    HeaderLine upcast(T line) {
        HeaderLine headerLine = line;
        return headerLine;
    }

    void addFound(java.util.List<HeaderLine> lines, boolean present, T line) {
        T value = present ? line : null;
        if (value != null) {
            lines.add(value);
        }
    }
}
"#;
    let translation = translate_source(source);

    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    assert!(
        translation
            .rust
            .contains("impl<T: CompoundLineBound + Clone + Into<HeaderLine>> Standards<T>")
            && translation
                .rust
                .contains("let header_line: HeaderLine = line.into();")
            && translation
                .rust
                .contains("lines.push(value.unwrap().into());")
            && translation
                .rust
                .contains("impl From<FormatLine> for HeaderLine"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "type_param_bound_source_supertype_assignment",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn source_receiver_generic_result_upcasts_to_supertype_assignment_slot() {
    let source = r#"
class HeaderLine {
    String id;
}

class FormatLine extends HeaderLine {
}

class Standards<T extends HeaderLine> {
    T last;

    T repair(T line) {
        return line;
    }
}

class StandardLineRepair {
    Standards<FormatLine> formats;

    HeaderLine repairOne(FormatLine line) {
        HeaderLine repaired = formats.repair(line);
        return repaired;
    }
}
"#;
    let translation = translate_source(source);

    assert!(
        translation.rust.contains(
            "let repaired: HeaderLine = self.formats.as_ref().unwrap().repair(line.clone()/* TODO(translation): validate added clone */).into();"
        ) && translation.rust.contains("return repaired;"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "source_receiver_generic_result_upcast",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn sealed_source_hierarchy_instanceof_repair_uses_closed_enum_runtime_carrier() {
    let source = r#"
sealed class HeaderLine permits FormatLine, InfoLine {
    String id;
}

final class FormatLine extends HeaderLine {
}

final class InfoLine extends HeaderLine {
}

class Standards<T> {
    T last;

    T repair(T line) {
        return line;
    }
}

class StandardLineRepair {
    Standards<FormatLine> formats;
    Standards<InfoLine> infos;

    void repairAll(java.util.Set<HeaderLine> lines) {
        for (HeaderLine line : lines) {
            if (line instanceof FormatLine) {
                line = formats.repair((FormatLine) line);
            } else if (line instanceof InfoLine) {
                line = infos.repair((InfoLine) line);
            }
        }
    }
}
"#;
    let translation = translate_source(source);

    assert!(
        !translation
            .rust
            .contains("unsupported Java instanceof shape")
            && translation
                .rust
                .contains("matches!(line, HeaderLineValue::FormatLine(_))")
            && translation
                .rust
                .contains("matches!(line, HeaderLineValue::InfoLine(_))"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "sealed_source_hierarchy_instanceof_repair",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
#[ignore = "tracks remaining VCF source hierarchy instanceof/downcast repair slot gap"]
fn source_hierarchy_instanceof_repair_keeps_supertype_slot_adapted() {
    let source = r#"
class HeaderLine {
    String id;
}

class FormatLine extends HeaderLine {
}

class InfoLine extends HeaderLine {
}

class Standards<T extends HeaderLine> {
    T repair(T line) {
        return line;
    }
}

class StandardLineRepair {
    Standards<FormatLine> formats;
    Standards<InfoLine> infos;

    void repairAll(java.util.Set<HeaderLine> lines) {
        for (HeaderLine line : lines) {
            if (line instanceof FormatLine) {
                line = formats.repair((FormatLine) line);
            } else if (line instanceof InfoLine) {
                line = infos.repair((InfoLine) line);
            }
        }
    }
}
"#;
    let translation = translate_source(source);

    assert!(
        !translation
            .rust
            .contains("unsupported Java instanceof shape: actual=Concrete, target=Concrete")
            && !translation.rust.contains("repair(Some(line))")
            && !translation.rust.contains("repair(line)")
            && !translation.rust.contains("line = formats")
            && !translation.rust.contains("line = infos"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "source_hierarchy_instanceof_repair",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn nested_generic_source_alias_preserves_type_params() {
    let source = r#"
class Outer {
    static class Inner<T> {
        T value;

        Inner(T value) {
            this.value = value;
        }
    }

    Inner<String> make(String value) {
        return new Inner<String>(value);
    }
}
"#;
    let translation = translate_source(source);

    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    assert!(
        translation
            .rust
            .contains("pub type Inner<T> = OuterInner<T>;")
            && translation.rust.contains("pub struct OuterInner<T>")
            && translation
                .rust
                .contains("pub fn make(&self, value: String) -> OuterInner<String>"),
        "{}",
        translation.rust
    );
}

#[test]
fn plain_returning_method_recovers_fallible_helper_call_without_question_mark() {
    let translation = translate_source(
        r#"
import java.io.IOException;
import java.util.Map;

class PlainReturnFallibleHelper {
    String render() {
        return encode();
    }

    String encode() {
        return "value";
    }

    static String encode(Map<String, Object> values) throws IOException {
        return "map";
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("pub fn render(&self) -> String")
            && translation.rust.contains(
                "return self.encode_().unwrap_or_else(|ex| panic!(\"uncaught Java exception: {:?}\", ex));"
            )
            && !translation.rust.contains("return self.encode_()?;"),
        "{}",
        translation.rust
    );
}

#[test]
fn nested_unchecked_exception_throw_does_not_force_result_return_shell() {
    let translation = translate_source(
        r#"
import htsjdk.tribble.TribbleException;
import java.util.Map;

class NestedUncheckedReturnShell {
    public String render() {
        return encode(null);
    }

    static String encode(Map<String, ? extends Object> values) {
        if (values == null) {
            throw new TribbleException.InternalCodecException("bad");
        }
        return "map";
    }
}
"#,
    );

    assert!(
        translation.rust.contains("pub fn render(&self) -> String")
            && translation.rust.contains("pub fn encode")
            && !translation.rust.contains("pub fn render(&self) -> Result")
            && !translation.rust.contains("Result<String, JavaException>"),
        "{}",
        translation.rust
    );
}

#[test]
fn constructor_deferred_field_recovers_casted_fallible_helper_without_question_mark() {
    let translation = translate_source(
        r#"
import java.io.IOException;

class ConstructorFallibleCast {
    Object source;
    String value;

    ConstructorFallibleCast(Object input) {
        this.source = input;
        this.value = (String) read(this.source);
    }

    static Object read(Object input) throws IOException {
        return "value";
    }
}
"#,
    );

    assert!(
        translation.rust.contains(
            "Self::read(java_clone_erased_object_ref(&__java_self.source)).unwrap_or_else(|ex| panic!(\"uncaught Java exception: {:?}\", ex))"
        ) && !translation.rust.contains("Self::read(java_clone_erased_object_ref(&__java_self.source))?"),
        "{}",
        translation.rust
    );
}

#[test]
fn constructor_self_optional_field_argument_uses_field_slot_shape() {
    let translation = translate_source(
        r#"
import java.io.IOException;

class ConstructorOptionalFieldArgument {
    Helper helper = new Helper();
    Token token = null;
    Token copied;

    ConstructorOptionalFieldArgument(InputStream input) {
        this.token = this.helper.make(input);
        this.copied = this.helper.echo(this.token);
    }
}

class Helper {
    Token make(InputStream input) {
        return new Token();
    }

    Token echo(Token token) throws IOException {
        return token;
    }
}

class Token {
}
"#,
    );

    assert!(
        (translation.rust.contains(
            "__java_self.token.as_ref().expect(\"Java null receiver\").clone()"
        ) || translation.rust.contains(
            "__java_self.token.as_ref().cloned()/* TODO(translation): validate added clone */.expect(\"Java null receiver\")"
        ))
            && !translation.rust.contains("echo(__java_self.token)"),
        "{}",
        translation.rust
    );
}

#[test]
fn borrowed_enum_constructor_param_stores_owned_optional_field_element() {
    let translation = translate_source(
        r#"
class BorrowedEnumConstructorParamStoreBase {
    String key;

    BorrowedEnumConstructorParamStoreBase(String key, String value) {
        this.key = key;
    }
}

class BorrowedEnumConstructorParamStore extends BorrowedEnumConstructorParamStoreBase {
    enum Mode { FORMAT, INFO }
    Mode mode = null;

    BorrowedEnumConstructorParamStore(Object mode) {
        this((Mode) mode, "");
    }

    BorrowedEnumConstructorParamStore(Mode mode, String ignored) {
        super(mode.toString(), "");
        this.mode = mode;
    }
}
"#,
    );

    assert!(
        !translation.rust.contains("mode: Some(mode)")
            && !translation.rust.contains("self.mode = Some(mode)")
            && (translation.rust.contains("mode: Some(*mode)")
                || translation.rust.contains("mode: Some(mode.clone()")
                || translation.rust.contains("self.mode = Some(*mode)")
                || translation.rust.contains("self.mode = Some(mode.clone()")),
        "borrowed enum constructor parameter should be recovered before optional field storage:\n{}",
        translation.rust
    );
}

#[test]
fn stale_constructor_optional_enum_field_shape_uses_final_storage_slot() {
    let translation = translate_source(&idiom_gap("StaleConstructorOptionalEnumFieldShape.java"));

    assert!(
        translation
            .rust
            .contains("count_type: Some(StaleConstructorOptionalEnumFieldShapeCountType::INTEGER)")
            && translation.rust.contains(
                "self.count_type = Some(StaleConstructorOptionalEnumFieldShapeCountType::UNBOUNDED)"
            )
            && !translation
                .rust
                .contains("count_type: StaleConstructorOptionalEnumFieldShapeCountType::INTEGER")
            && !translation.rust.contains(
                "self.count_type = StaleConstructorOptionalEnumFieldShapeCountType::UNBOUNDED"
            )
            && !translation.rust.contains("line_type: Some(line_type)")
            && (translation.rust.contains("line_type: Some(*line_type)")
                || translation
                    .rust
                    .contains("line_type: Some(line_type.clone()")),
        "constructor field lowering should use final optional enum field storage shape:\n{}",
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
            && translation.rust.contains(
                "unimplemented!(\"source-summary method GATKPath.toPath is not translated\")"
            )
            && translation
                .rust
                .contains("self.header_override.as_ref().unwrap().to_path()"),
        "{}",
        translation.rust
    );
}

#[test]
fn source_summary_object_to_string_in_print_uses_java_object_stringification() {
    let caller = r#"
class UsesSummaryObjectToStringInPrint {
    void report(Alignment alignment) {
        System.err.println(alignment.toString());
    }
}
"#;
    let summary = r#"
class Alignment {
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
            .contains("java_erased_object_to_string(&alignment)")
            && !translation.rust.contains("alignment.to_string()"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "source_summary_object_to_string_print",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn emitted_context_object_to_string_in_print_uses_java_object_stringification_without_declared_method()
 {
    let caller = r#"
class UsesEmittedContextObjectToStringInPrint {
    void report(Alignment alignment) {
        System.err.println(alignment.toString());
    }
}
"#;
    let context = r#"
class Alignment {
    String getSummary() {
        return "summary";
    }
}
"#;
    let translation = translate_source_with_context_sources_and_summary_sources(
        caller,
        &[context.to_string()],
        &[],
        true,
        &FactSet::default(),
    );

    assert!(
        translation
            .rust
            .contains("java_erased_object_to_string(&alignment)")
            && !translation.rust.contains("alignment.to_string()"),
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
            && translation.rust.contains(
                "unimplemented!(\"source-summary method Header.dictionary is not translated\")"
            )
            && translation
                .rust
                .contains("pub fn sam_header(&self) -> Option<SAMFileHeader>")
            && translation
                .rust
                .contains("pub fn iterator(&self) -> Option<CloseableIterator>")
            && translation
                .rust
                .contains("return self.reader.as_ref().unwrap().header().sam_header().unwrap();")
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
        let _: SAMSequenceDictionary = fixture.dictionary();
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
            && !translation.rust.contains("Some(reference.get_raw_input_string().as_str())")
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
            && !translation
                .rust
                .contains("factory.reference_sequence(if missing { None } else { Some(&reference) })"),
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
        translation.rust.contains(
            "if read_name.is_none() { \"\".to_string() } else { (read_name.unwrap()).to_string() }"
        ) && !translation.rust.contains("read_name.unwrap().unwrap()")
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
        translation.rust.contains(
            "if read_name.is_none() { \"\".to_string() } else { (read_name.unwrap()).to_string() }"
        ) && !translation.rust.contains("read_name.unwrap().unwrap()")
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
fn map_get_string_display_segment_unwraps_optional_borrow() {
    let translation = translate_source(
        r#"
import java.util.HashMap;

class MapGetStringDisplaySegment {
    String append(HashMap<String, String> values, String key, String suffix) {
        if (values.containsKey(key)) {
            return values.get(key) + suffix;
        }
        return suffix;
    }
}
"#,
    );

    assert!(
        translation.rust.contains(
            "values.get(key).map(|value| value.to_string()).unwrap_or_else(|| \"null\".to_string())"
        ) && translation.rust.contains("format!(\"{}{}\"")
            && !translation
                .rust
                .contains("format!(\"{}{}\", values.get(&key), suffix)")
            && !translation.rust.contains("values.get(&key.as_str())"),
        "{}",
        translation.rust
    );
}

#[test]
fn map_get_keyset_loop_does_not_double_borrow_key() {
    let translation = translate_source(
        r#"
import java.util.HashMap;

class MapGetKeySetLoop {
    Object first(HashMap<String, Object> context) {
        for (String key : context.keySet()) {
            return context.get(key);
        }
        return null;
    }
}
"#,
    );

    assert!(
        translation.rust.contains("context.get(key.as_str())")
            && !translation.rust.contains("context.get(&key)")
            && !translation.rust.contains("context.get(&key.as_str())"),
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
            .contains("pub struct Future<T = Box<dyn std::any::Any>>"),
        "{}",
        translation.rust
    );
    assert!(
        translation
            .rust
            .contains("impl<T> Future<T> {\n    pub fn get(&self) -> T"),
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
        !translation
            .rust
            .contains("pub fn get<T: Default>(&self) -> T")
            && !translation.rust.contains("pub fn get<R: Default>() -> R")
            && !translation.rust.contains("FutureGetResult"),
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
            && translation.rust.contains(
                "unimplemented!(\"source-summary method GatkTool.getDefaultToolVCFHeaderLines is not translated\")"
            )
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
fn chained_optional_external_receiver_getter_is_unwrapped_before_call() {
    let source = r#"
import htsjdk.samtools.SAMSequenceDictionary;
import htsjdk.variant.vcf.VCFHeader;

class ChainedOptionalExternalReceiverGetterUse {
    VCFHeader maybeHeader() {
        return null;
    }

    SAMSequenceDictionary dictionary() {
        return maybeHeader().getSequenceDictionary();
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
            .contains("self.maybe_header().as_ref().unwrap().get_sequence_dictionary()")
            && !translation
                .rust
                .contains("self.maybe_header().get_sequence_dictionary()"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "chained_optional_external_receiver_getter",
        vec![GeneratedCrateModule {
            name: "chained_optional_external_receiver_getter",
            rust: translation.rust,
            tests: r#"
#[cfg(test)]
mod generated_tests {
    use super::*;

    #[test]
    fn typechecks_chained_optional_receiver_getter() {
        let fixture = ChainedOptionalExternalReceiverGetterUse {};
        let _: SAMSequenceDictionary = fixture.dictionary();
    }
}
"#,
        }],
    );
}

#[test]
fn sibling_optional_source_return_receiver_getter_is_unwrapped_before_call() {
    let source = r#"
import htsjdk.samtools.SAMSequenceDictionary;

class SiblingOptionalSourceReturnReceiverGetterUse {
    SAMSequenceDictionary dictionary(SourceHeaderReader reader) {
        return reader.getFileHeader().getSequenceDictionary();
    }
}
"#;
    let sibling = r#"
import htsjdk.variant.vcf.VCFHeader;

class SourceHeaderReader {
    VCFHeader header;

    VCFHeader getHeader() {
        return header;
    }

    VCFHeader getFileHeader() {
        return getHeader();
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
    let translation = translate_source_with_context_sources(source, &[sibling.to_string()], &facts);

    assert!(
        translation
            .rust
            .contains("reader.get_file_header().as_ref().unwrap().get_sequence_dictionary()")
            && !translation
                .rust
                .contains("reader.get_file_header().get_sequence_dictionary()"),
        "{}",
        translation.rust
    );
}

#[test]
fn constructor_initialized_local_receiver_does_not_get_optional_adapter() {
    let translation = translate_source(
        r#"
import java.io.File;
import java.io.InputStream;
import java.util.Enumeration;
import java.util.zip.ZipEntry;
import java.util.zip.ZipFile;

class ConstructorInitializedLocalReceiver {
    ZipFile zipFile;

    void use(File file) throws Exception {
        ZipFile zipFile = new ZipFile(file);
        Enumeration entries = zipFile.entries();
        InputStream stream = zipFile.getInputStream(new ZipEntry());
        zipFile.close();
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("let zip_file: ZipFile = ZipFile::new(file")
            && translation
                .rust
                .contains("let entries: Enumeration = zip_file.entries()?;")
            && translation
                .rust
                .contains("let stream: InputStream = zip_file.get_input_stream(ZipEntry::new())?;")
            && translation.rust.contains("zip_file.close()?;")
            && translation
                .rust
                .contains("pub fn entries(&self) -> Result<Enumeration, JavaException>")
            && translation
                .rust
                .contains("pub fn get_input_stream<T0>(&self, _arg0: T0) -> Result<InputStream, JavaException>")
            && !translation.rust.contains("zip_file.as_ref().unwrap()"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "constructor_initialized_local_receiver",
        vec![GeneratedCrateModule {
            name: "constructor_initialized_local_receiver",
            rust: translation.rust,
            tests: r#"
#[cfg(test)]
mod generated_tests {
    use super::*;

    #[test]
    fn typechecks_constructor_initialized_local_receiver() {
        let fixture = ConstructorInitializedLocalReceiver { zip_file: Some(ZipFile) };
        fixture.r#use(File::new("archive.zip")).unwrap();
    }
}
"#,
        }],
    );
}

#[test]
fn jdk_facade_signature_facts_type_io_zip_xml_awt_and_base64_calls() {
    let translation = translate_source(
        r#"
import java.awt.Image;
import java.awt.image.ImageObserver;
import java.io.BufferedInputStream;
import java.io.ByteArrayOutputStream;
import java.io.InputStream;
import java.util.Base64;
import java.util.Enumeration;
import java.util.zip.ZipEntry;
import javax.swing.JComponent;
import javax.swing.RepaintManager;
import javax.xml.stream.XMLOutputFactory;
import javax.xml.stream.XMLStreamWriter;

class JdkFacadeSignatureUse {
    XMLStreamWriter writer(XMLOutputFactory factory, Object out) throws Exception {
        return factory.createXMLStreamWriter(out);
    }

    int copy(InputStream raw) throws Exception {
        BufferedInputStream in = new BufferedInputStream(raw);
        byte[] buffer = new byte[16];
        return in.read(buffer, 0, buffer.length);
    }

    String zip(Enumeration entries) {
        if (!entries.hasMoreElements()) {
            return "";
        }
        ZipEntry entry = (ZipEntry) entries.nextElement();
        return entry.isDirectory() ? entry.getName() : entry.getName();
    }

    int imageSize(Image image, ImageObserver observer) {
        return image.getWidth(observer) + image.getHeight(observer);
    }

    boolean repaint(JComponent component) {
        RepaintManager manager = RepaintManager.currentManager(component);
        return manager.isDoubleBufferingEnabled();
    }

    String encoded(byte[] bytes, ByteArrayOutputStream out) throws Exception {
        return Base64.getEncoder().encodeToString(bytes) + out.toString("UTF-8");
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("pub fn create_x_m_l_stream_writer<T0>(&self, _arg0: T0) -> XMLStreamWriter")
            && (translation
                .rust
                .contains("pub fn read_3(&self, _arg0: &[i8], _arg1: i32, _arg2: i32) -> Result<i32, JavaException>")
                || translation
                    .rust
                    .contains("pub fn read_3<T0: ?Sized>(\n        &self,\n        _arg0: &T0,\n        _arg1: i32,\n        _arg2: i32,\n    ) -> Result<i32, JavaException>"))
            && translation
                .rust
                .contains("pub fn has_more_elements(&self) -> Result<bool, JavaException>")
            && translation
                .rust
                .contains("pub fn next_element(&self) -> Result<ZipEntry, JavaException>")
            && !translation.rust.contains("next_element<T: Default>")
            && translation
                .rust
                .contains("pub fn is_directory(&self) -> Result<bool, JavaException>")
            && translation
                .rust
                .contains("pub fn get_name(&self) -> Result<String, JavaException>")
            && translation
                .rust
                .contains("pub fn get_width<T0>(&self, _arg0: T0) -> i32")
            && translation
                .rust
                .contains("pub fn current_manager<T0>(_arg0: T0) -> RepaintManager")
            && translation
                .rust
                .contains("pub fn get_encoder() -> Base64Encoder")
            && translation
                .rust
                .contains("pub fn encode_to_string<T0>(&self, _arg0: T0) -> String")
            && (translation
                .rust
                .contains("pub fn to_string<T0>(&self, _arg0: T0) -> Result<String, JavaException>")
                || translation
                    .rust
                    .contains("pub fn to_string_1<T0>(&self, _arg0: T0) -> Result<String, JavaException>"))
            && !translation.rust.contains("has_more_elements(&self) -> ()")
            && !translation.rust.contains("current_manager<R: Default"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "jdk_facade_signature_facts",
        vec![GeneratedCrateModule {
            name: "jdk_facade_signature_facts",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn jdk_mutable_buffer_and_random_numeric_methods_have_typed_facades() {
    let translation = translate_source(
        r#"
import java.util.Random;

class JdkMutableBufferAndRandomNumericMethods {
    int width(String value) {
        StringBuffer buffer = new StringBuffer();
        buffer.append(value);
        return buffer.length();
    }

    int random(Random random, int bound) {
        return random.nextInt(bound);
    }
}
"#,
    );

    assert!(
        translation.rust.contains("return buffer.length();")
            && translation.rust.contains("pub fn length(&self) -> i32")
            && translation.rust.contains("return random.next_int(bound);")
            && translation
                .rust
                .contains("pub fn next_int<T0>(&self, _arg0: T0) -> i32")
            && !translation.rust.contains("length(&self) -> ()")
            && !translation
                .rust
                .contains("next_int<T0>(&self, _arg0: T0) -> ()"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "jdk_mutable_buffer_and_random_numeric_methods",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn omitted_context_receiver_method_used_in_display_sink_gets_string_stub() {
    let translation = translate_source(
        r#"
class OmittedContextReceiverMethodDisplaySink {
    private SequenceFile sequenceFile;

    void write(StringBuffer summaryText) {
        summaryText.append(sequenceFile.name());
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("impl SequenceFile {\n    pub fn name(&self) -> String")
            && !translation
                .rust
                .contains("impl SequenceFile {\n    pub fn name(&self) -> ()"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "omitted_context_display_sink",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn omitted_context_name_getter_feeds_typed_constructor_argument() {
    let translation = translate_source(
        r#"
class OmittedContextNameGetterConstructorArg {
    Matrix load(NamedInputStream nis) {
        return new Matrix(nis.getName());
    }
}

class Matrix {
    Matrix(String name) {
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
            .contains("impl NamedInputStream {\n    pub fn get_name(&self) -> String")
            && translation
                .rust
                .contains("return Matrix::new(nis.get_name())"),
        "{}",
        translation.rust
    );
    assert!(
        !translation.rust.contains("pub fn get_name(&self) -> ()"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "omitted_context_name_getter_constructor_arg",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
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
            && translation.rust.contains("self.maybe_path(name.as_str())")
            && translation
                .rust
                .contains("self.open_with_index(name.as_str(), path.as_ref())")
            && !translation
                .rust
                .contains("let mut path: Path = self.maybe_path")
            && !translation.rust.contains("path.clone()")
            && !translation.rust.contains("Some(path"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "nullable-callback-local-slot",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
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
            "pub fn run(&self, futures: &std::collections::HashMap<String, Future<Reader>>) -> Result<std::collections::HashMap<String, Reader>, JavaException>"
        ) && translation
            .rust
            .contains("Err(e) if e.is_instance(&[\"ExecutionException\"])")
            && translation.rust.contains("return Err(JavaException::with_cause(\"UserException\"")
            && translation.rust.contains("readers.insert(name.clone(), reader);")
            && translation.rust.contains("return Ok(readers);"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "foreach_lambda_throw_wraps_enclosing_method_in_result",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
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

    boolean checkReady() throws IOException {
        super.ready();
        return true;
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

    boolean ready() throws IOException {
        return false;
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
        translation.rust.contains("Ok::<(), JavaException>(())?;")
            && translation.rust.contains(
                "return unimplemented!(\"source-summary method SummaryBase.label is not translated\");"
            ),
        "{}",
        translation.rust
    );
    assert!(
        !translation.rust.contains("Default::default()?"),
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
        let _: Result<bool, JavaException> = fixture.check_ready();
        let _: String = fixture.inherited_label();
    }
}
"#,
        }],
    );
}

#[test]
fn negated_boolean_super_boundary_call_uses_typed_default() {
    let caller = r#"
class NegatedSuperEqualsCaller extends SummaryEqualsBase {
    boolean same(Object o) {
        if (!super.equals(o)) {
            return false;
        }
        return true;
    }
}
"#;
    let summary = r#"
class SummaryEqualsBase {
    public boolean equals(Object o) {
        return true;
    }
}
"#;
    let translation = translate_source_with_context_summary_sources(
        caller,
        &[summary.to_string()],
        &FactSet::default(),
    );

    assert!(
        translation.rust.contains("if !false {")
            && !translation.rust.contains("!Default::default()")
            && !translation.rust.contains("super."),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "negated_super_equals_boundary",
        vec![GeneratedCrateModule {
            name: "negated_super_equals_boundary",
            rust: translation.rust,
            tests: "",
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
                .contains("pub fn flatten(&self, groups: &Vec<Vec<String>>) -> std::collections::HashSet<String>")
            || translation
                .rust
                .contains("pub fn flatten(&self, groups: &[Vec<String>]) -> std::collections::HashSet<String>"))
            && translation
                .rust
                .contains("groups.iter().flat_map(|item| item.iter()).cloned().collect::<std::collections::HashSet<_>>()")
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
    let rust_path = std::env::temp_dir().join("new_java2rust_method_reference_flat_map_test.rs");
    let bin_path = std::env::temp_dir().join("new_java2rust_method_reference_flat_map_test");
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
fn htsjdk_log_static_field_warn_uses_pack_support() {
    let translation = translate_source(
        r#"
import htsjdk.samtools.util.Log;

class HtsjdkLogStaticFieldWarnUse {
    private static final Log log = Log.getInstance(HtsjdkLogStaticFieldWarnUse.class);

    void run(String sample) {
        log.warn("slow " + sample);
    }
}
"#,
    );

    assert!(
        translation.rust.contains("pub struct Log")
            && translation.rust.contains("pub fn warn")
            && translation.rust.contains("Self::log.warn(")
            && !translation.rust.contains("no method named `warn`"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "htsjdk_log_static_field_warn",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
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
fn unsupported_instanceof_placeholder_escapes_debug_type_quotes() {
    let translation = translate_source(
        r#"
class UnsupportedInstanceofPlaceholderEscapesDebugTypeQuotes {
    boolean isObjectArray(Object value) {
        return value instanceof Object[];
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains(r#"compile_error!("unsupported Java instanceof check: target type Array(Named { name: \"Object\", args: [] }) has no concrete runtime type")"#)
            && !translation.rust.contains(r#"name: "Object""#),
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
fn source_super_constructor_with_args_initializes_flattened_inherited_fields() {
    let subclass = r#"
class ChildWriter extends ParentWriter {
    private final State state;

    ChildWriter(Writer writer, String sampleName, State state) {
        super(writer, sampleName);
        this.state = state;
    }
}
"#;
    let superclass = r#"
class ParentWriter {
    private final Writer writer;
    private final String sampleName;

    ParentWriter(Writer writer, String sampleName) {
        this.writer = writer;
        this.sampleName = sampleName;
    }
}
"#;

    let translation = translate_source_with_context_sources(
        subclass,
        &[superclass.to_string()],
        &FactSet::default(),
    );

    assert!(
        translation.rust.contains("writer: Writer")
            && translation.rust.contains("sample_name: Option<String>")
            && translation.rust.contains("state: State")
            && translation.rust.contains("writer: writer")
            && translation.rust.contains("sample_name: Some(sample_name")
            && translation.rust.contains("state: state")
            && !translation.rust.contains("__java_self.state = state;")
            && !translation
                .rust
                .contains("super(...) requires inheritance lowering")
            && !translation.rust.contains("compile_error!"),
        "{}",
        translation.rust
    );
}

#[test]
fn source_super_constructor_with_trailing_throw_guard_lowers() {
    let subclass = r#"
class CheckedChild extends CheckedBase {
    CheckedChild(String name, int count, String kind) {
        super(name, count, kind, "CHILD");
        if (count < 0)
            throw new IllegalArgumentException("bad count");
    }
}
"#;
    let superclass = r#"
class CheckedBase {
    private final String name;
    private final int count;
    private final String kind;
    private final String target;

    CheckedBase(String name, int count, String kind, String target) {
        this.name = name;
        this.count = count;
        this.kind = kind;
        this.target = target;
    }
}
"#;

    let translation = translate_source_with_context_sources(
        subclass,
        &[superclass.to_string()],
        &FactSet::default(),
    );

    assert!(
        translation.rust.contains("name: name")
            && translation.rust.contains("count: count")
            && translation.rust.contains("kind: kind")
            && translation.rust.contains("target: \"CHILD\".to_string()")
            && translation
                .rust
                .contains("-> Result<CheckedChild, JavaException>")
            && translation.rust.contains("if count < 0")
            && translation
                .rust
                .contains("return Err(JavaException::new(\"IllegalArgumentException\"")
            && !translation
                .rust
                .contains("super(...) requires inheritance lowering")
            && !translation.rust.contains("compile_error!"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "source_super_constructor_with_trailing_throw_guard",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn source_super_constructor_with_child_locals_and_nested_field_assignment_lowers() {
    let subclass = r#"
class ParsedChild extends ParsedBase {
    private String name;
    private String source;

    ParsedChild(String line, boolean modern) {
        super("KEY", "");
        String parsed = line.trim();
        name = parsed;
        if (modern) {
            this.source = parsed;
        }
    }
}
"#;
    let superclass = r#"
class ParsedBase {
    private final String key;
    private final String value;

    ParsedBase(String key, String value) {
        this.key = key;
        this.value = value;
    }
}
"#;

    let translation = translate_source_with_context_sources(
        subclass,
        &[superclass.to_string()],
        &FactSet::default(),
    );

    let parsed_pos = translation.rust.find("let parsed").unwrap_or_else(|| {
        panic!(
            "child constructor local should be emitted before self construction:\n{}",
            translation.rust
        )
    });
    let self_pos = translation
        .rust
        .find("let mut __java_self = ParsedChild")
        .unwrap_or_else(|| {
            panic!(
                "child self temporary should be emitted:\n{}",
                translation.rust
            )
        });
    assert!(
        parsed_pos < self_pos
            && translation.rust.contains("key: \"KEY\".to_string()")
            && translation.rust.contains("value: \"\".to_string()")
            && translation.rust.contains("name: Some(parsed.clone()")
            && translation.rust.contains("__java_self.source = parsed")
            && !translation
                .rust
                .contains("super(...) requires inheritance lowering")
            && !translation.rust.contains("compile_error!"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "source_super_constructor_with_child_locals",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn optional_map_string_get_flows_into_string_value_contexts() {
    let source = r#"
import java.util.Map;

class OptionalMapStringValueUse {
    enum Kind { Text, Number }

    static final String NUMBER = "Number";

    int parse(Map<String, String> mapping) {
        String numberStr = mapping.get("Number");
        if (numberStr == NUMBER) {
            return 0;
        }
        return Integer.parseInt(numberStr);
    }

    Kind kind(Map<String, String> mapping) {
        return Kind.valueOf(mapping.get("Type"));
    }
}
"#;

    let translation = translate_source(source);

    assert!(
        translation
            .rust
            .contains("number_str.as_ref().map(|value| value.as_str()) == Some(Self::NUMBER)")
            && translation
                .rust
                .contains("number_str.as_ref().expect(\"Java null receiver\").as_str().parse::<i32>().unwrap()")
            && translation
                .rust
                .contains("OptionalMapStringValueUseKind::value_of(mapping.get(\"Type\").expect(\"Java null receiver\").as_str())")
            && !translation.rust.contains("Option<&String>: AsRef<str>")
            && !translation.rust.contains("compile_error!"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "optional_map_string_get_value_contexts",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn optional_map_string_get_compares_to_cross_class_static_string_constant() {
    let use_source = r#"
import java.util.Map;

class OptionalCrossClassStaticStringUse {
    int parse(Map<String, String> mapping) {
        String numberStr = mapping.get("Number");
        if (numberStr == CrossConstants.NUMBER) {
            return 0;
        }
        return Integer.parseInt(numberStr);
    }
}
"#;
    let constants_source = r#"
class CrossConstants {
    public static final String NUMBER = "Number";
}
"#;

    let translation = translate_source_with_context_sources(
        use_source,
        &[constants_source.to_string()],
        &FactSet::default(),
    );

    assert!(
        translation.rust.contains(
            "number_str.as_ref().map(|value| value.as_str()) == Some(CrossConstants::NUMBER)"
        ) && translation.rust.contains(
            "number_str.as_ref().expect(\"Java null receiver\").as_str().parse::<i32>().unwrap()"
        ) && !translation
            .rust
            .contains("number_str == CrossConstants::NUMBER")
            && !translation.rust.contains("number_str.parse"),
        "{}",
        translation.rust
    );

    // This assertion checks primary-file rendering. Context sources are not emitted
    // by this helper, so a cargo check here would fail on the absent constants type.
}

#[test]
fn optional_map_string_get_flows_into_source_enum_value_of_inside_try_assignment() {
    let source = r#"
import java.util.Map;

class OptionalMapStringEnumTryUse {
    enum Kind { Text, Number }
    private Kind kind;

    void read(Map<String, String> mapping) {
        try {
            kind = Kind.valueOf(mapping.get("Type"));
        } catch (Exception e) {
            throw new IllegalArgumentException(mapping.get("Type"));
        }
    }
}
"#;

    let translation = translate_source(source);

    assert!(
        translation
            .rust
            .contains("OptionalMapStringEnumTryUseKind::value_of(mapping.get(\"Type\").expect(\"Java null receiver\").as_str())")
            && !translation.rust.contains("value_of(mapping.get(\"Type\"))")
            && !translation.rust.contains("Option<&String>: AsRef<str>"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "optional_map_string_enum_try_assignment",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn optional_map_string_get_flows_into_top_level_source_enum_value_of() {
    let use_source = r#"
import java.util.Map;

class OptionalTopLevelEnumValueOfUse {
    private TopLevelKind kind;

    void read(Map<String, String> mapping) {
        try {
            kind = TopLevelKind.valueOf(mapping.get("Type"));
        } catch (Exception e) {
            throw new IllegalArgumentException(mapping.get("Type"));
        }
    }
}
"#;
    let enum_source = r#"
enum TopLevelKind { Text, Number }
"#;

    let translation = translate_source_with_context_sources(
        use_source,
        &[enum_source.to_string()],
        &FactSet::default(),
    );

    assert!(
        translation.rust.contains(
            "TopLevelKind::value_of(mapping.get(\"Type\").expect(\"Java null receiver\").as_str())"
        ) && !translation
            .rust
            .contains("TopLevelKind::value_of(mapping.get(\"Type\"))")
            && !translation.rust.contains("Option<&String>: AsRef<str>"),
        "{}",
        translation.rust
    );
}

#[test]
fn source_super_constructor_composes_delegated_parameter_bindings() {
    let translation = translate_source(&idiom_gap("SourceSuperDelegatedParamSubstitution.java"));

    assert!(
        translation
            .rust
            .contains("kind: Some(SourceSuperDelegatedParamSubstitutionBaseKind::FORMAT)")
            && !translation.rust.contains("self.kind")
            && !translation
                .rust
                .contains("super(...) requires inheritance lowering")
            && !translation.rust.contains("compile_error!"),
        "source super constructor delegation should substitute original super args:\n{}",
        translation.rust
    );
}

#[test]
fn source_super_constructor_delegated_null_marks_flattened_field_optional() {
    let superclass = r#"
class HeaderBase {
    private String source;

    HeaderBase(String name) {
        this(name, null);
    }

    HeaderBase(String name, String source) {
        this.source = source;
    }
}
"#;
    let subclass = r#"
class HeaderChild extends HeaderBase {
    HeaderChild(String name) {
        super(name);
    }
}
"#;

    let translation = translate_source_with_context_sources(
        subclass,
        &[superclass.to_string()],
        &FactSet::default(),
    );

    assert!(
        translation.rust.contains("source: Option<String>")
            && !translation.rust.contains("source: String")
            && !translation
                .rust
                .contains("super(...) requires inheritance lowering")
            && !translation.rust.contains("compile_error!"),
        "{}",
        translation.rust
    );
}

#[test]
fn source_super_constructor_lowers_locals_needed_by_inherited_fields() {
    let superclass = r#"
class HeaderBase {
    private String name;
    private String description;

    HeaderBase(String line) {
        String parsed = parse(line);
        name = parsed;
        description = parsed.trim();
    }

    static String parse(String line) {
        return line.trim();
    }
}
"#;
    let subclass = r#"
class HeaderChild extends HeaderBase {
    HeaderChild(String line) {
        super(line);
    }
}
"#;

    let translation = translate_source_with_context_sources(
        subclass,
        &[superclass.to_string()],
        &FactSet::default(),
    );

    let parsed_pos = translation.rust.find("let mut parsed").unwrap_or_else(|| {
        panic!(
            "super constructor local should be emitted before child self construction:\n{}",
            translation.rust
        )
    });
    let self_pos = translation
        .rust
        .find("let mut __java_self = HeaderChild")
        .unwrap_or_else(|| {
            panic!(
                "child constructor should build a self temporary:\n{}",
                translation.rust
            )
        });
    assert!(
        parsed_pos < self_pos
            && translation.rust.contains("name: Some(parsed.clone()")
            && translation.rust.contains("description: Some(parsed.trim()")
            && !translation.rust.contains("compile_error!")
            && !translation.rust.contains("requires inheritance lowering"),
        "{}",
        translation.rust
    );
}

#[test]
fn source_super_constructor_uses_resolved_overload_not_first_same_arity() {
    let superclass = r#"
enum CountKind {
    INTEGER,
    UNBOUNDED
}

enum ValueType {
    STRING
}

class HeaderBase {
    private int count;
    private CountKind countType;
    private ValueType type;

    HeaderBase(String name, int count, ValueType type) {
        this.countType = CountKind.INTEGER;
        this.count = count;
        this.type = type;
    }

    HeaderBase(String name, CountKind count, ValueType type) {
        this.count = -1;
        this.countType = count;
        this.type = type;
    }
}
"#;
    let subclass = r#"
class HeaderChild extends HeaderBase {
    HeaderChild(String name, CountKind count, ValueType type) {
        super(name, count, type);
    }
}
"#;

    let translation = translate_source_with_context_sources(
        subclass,
        &[superclass.to_string()],
        &FactSet::default(),
    );

    assert!(
        translation.rust.contains("count: -1")
            && translation.rust.contains("count_type: Some(count")
            && !translation.rust.contains("count: count")
            && !translation
                .rust
                .contains("count_type: Some(CountKind::INTEGER)")
            && !translation.rust.contains("compile_error!"),
        "{}",
        translation.rust
    );
}

#[test]
fn external_pack_static_utility_call_uses_static_owner() {
    let translation = translate_source(
        r#"
class ExternalPackStaticUtilityUse {
    Map<String, String> parse(VCFHeaderVersion version, String line, List<String> expected, List<String> recommended) {
        return VCFHeaderLineTranslator.parseLine(version, line, expected, recommended);
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("VCFHeaderLineTranslator::parse_line(")
            && !translation
                .rust
                .contains("v_c_f_header_line_translator.parse_line")
            && !translation.rust.contains("compile_error!"),
        "{}",
        translation.rust
    );
}

#[test]
fn constructor_trailing_instance_call_rewrites_self_arguments() {
    let translation = translate_source(&idiom_gap("ConstructorTrailingCallReadsSeededField.java"));

    assert!(
        translation.rust.contains("__java_self.initialize(")
            && !translation.rust.contains("initialize(self.name")
            && !translation.rust.contains("self.name,")
            && !translation
                .rust
                .contains("expected value, found module `self`")
            && !translation.rust.contains("compile_error!"),
        "trailing constructor calls should rewrite self-field arguments through __java_self:\n{}",
        translation.rust
    );
}

#[test]
fn generic_source_super_constructor_initializes_flattened_inherited_fields() {
    let subclass = r#"
interface Function {}

abstract class ChildSolver extends BaseSolver<Function> {
    ChildSolver(double absoluteAccuracy) {
        super(absoluteAccuracy);
    }

    ChildSolver(double relativeAccuracy, double absoluteAccuracy) {
        super(relativeAccuracy, absoluteAccuracy);
    }

    ChildSolver(double relativeAccuracy, double absoluteAccuracy, double valueAccuracy) {
        super(relativeAccuracy, absoluteAccuracy, valueAccuracy);
    }
}
"#;
    let superclass = r#"
interface Function {}

abstract class BaseSolver<FUNC extends Function> {
    private static final double DEFAULT_RELATIVE_ACCURACY = 1e-14;
    private static final double DEFAULT_VALUE_ACCURACY = 1e-15;
    private final double valueAccuracy;
    private final double absoluteAccuracy;
    private final double relativeAccuracy;

    BaseSolver(double absoluteAccuracy) {
        this(DEFAULT_RELATIVE_ACCURACY, absoluteAccuracy, DEFAULT_VALUE_ACCURACY);
    }

    BaseSolver(double relativeAccuracy, double absoluteAccuracy) {
        this(relativeAccuracy, absoluteAccuracy, DEFAULT_VALUE_ACCURACY);
    }

    BaseSolver(double relativeAccuracy, double absoluteAccuracy, double valueAccuracy) {
        this.absoluteAccuracy = absoluteAccuracy;
        this.relativeAccuracy = relativeAccuracy;
        this.valueAccuracy = valueAccuracy;
    }
}
"#;

    let translation = translate_source_with_context_sources(
        subclass,
        &[superclass.to_string()],
        &FactSet::default(),
    );

    assert!(
        translation.rust.contains("pub struct ChildSolver")
            && translation.rust.contains("value_accuracy: f64")
            && translation.rust.contains("absolute_accuracy: f64")
            && translation.rust.contains("relative_accuracy: f64")
            && translation
                .rust
                .contains("absolute_accuracy: absolute_accuracy")
            && translation
                .rust
                .contains("relative_accuracy: relative_accuracy")
            && translation.rust.contains("value_accuracy: value_accuracy")
            && !translation
                .rust
                .contains("super(...) requires inheritance lowering")
            && !translation.rust.contains("compile_error!"),
        "{}",
        translation.rust
    );
}

#[test]
fn source_super_constructor_preserves_trailing_method_result_call() {
    let subclass = r#"
class TooManyMini extends MaxExceededMini {
    TooManyMini(Number max) {
        super(max);
        getContext().addMessage("evaluations");
    }
}
"#;
    let superclass = r#"
class MaxExceededMini extends MathIllegalStateMini {
    private final Number max;

    MaxExceededMini(Number max) {
        this.max = max;
        this.context = new ExceptionContext();
    }

    Number getMax() {
        return max;
    }
}

class MathIllegalStateMini {
    protected ExceptionContext context;

    ExceptionContext getContext() {
        return context;
    }
}

class ExceptionContext {
    ExceptionContext() {
    }

    void addMessage(Object pattern, Object... args) {
    }
}

"#;

    let translation = translate_source_with_context_sources(
        subclass,
        &[superclass.to_string()],
        &FactSet::default(),
    );

    assert!(
        translation.rust.contains("pub struct TooManyMini")
            && translation.rust.contains("max: Number")
            && translation
                .rust
                .contains("context: Option<ExceptionContext>")
            && translation
                .rust
                .contains("let mut __java_self = TooManyMini")
            && translation.rust.contains("max: max")
            && translation.rust.contains("__java_self.context")
            && translation.rust.contains(".add_message")
            && !translation
                .rust
                .contains("super(...) requires inheritance lowering")
            && !translation.rust.contains("compile_error!"),
        "{}",
        translation.rust
    );
}

#[test]
fn source_super_constructor_matches_varargs_super_arity() {
    let subclass = r#"
class NumberProblem extends BaseProblem {
    private final Number argument;

    NumberProblem(String pattern, Number wrong, Object[] arguments) {
        super(pattern, wrong, arguments);
        argument = wrong;
    }
}
"#;
    let superclass = r#"
class BaseProblem {
    private final ExceptionContext context;

    BaseProblem(String pattern, Object... args) {
        context = new ExceptionContext();
    }
}

class ExceptionContext {
    ExceptionContext() {
    }
}
"#;

    let translation = translate_source_with_context_sources(
        subclass,
        &[superclass.to_string()],
        &FactSet::default(),
    );

    assert!(
        translation.rust.contains("pub struct NumberProblem")
            && translation.rust.contains("argument: Option<Number>")
            && translation.rust.contains("context: ExceptionContext")
            && translation
                .rust
                .contains("let mut __java_self = NumberProblem")
            && translation.rust.contains("argument: Some(wrong.clone()")
            && !translation
                .rust
                .contains("super(...) requires inheritance lowering")
            && !translation.rust.contains("compile_error!"),
        "{}",
        translation.rust
    );
}

#[test]
fn implicit_source_super_constructor_initializes_inherited_fields() {
    let subclass = r#"
class ChildProblem extends BaseProblem {
    private final Number max;

    ChildProblem(Number max) {
        getContext().addMessage("child");
        this.max = max;
    }
}
"#;
    let superclass = r#"
class BaseProblem {
    protected ExceptionContext context;

    BaseProblem() {
        context = new ExceptionContext();
    }

    ExceptionContext getContext() {
        return context;
    }
}

class ExceptionContext {
    ExceptionContext() {
    }

    void addMessage(Object pattern, Object... args) {
    }
}
"#;

    let translation = translate_source_with_context_sources(
        subclass,
        &[superclass.to_string()],
        &FactSet::default(),
    );

    assert!(
        translation.rust.contains("pub struct ChildProblem")
            && translation.rust.contains("context: ExceptionContext")
            && translation
                .rust
                .contains("let mut __java_self = ChildProblem")
            && !translation.rust.contains("context: Default::default()")
            && translation
                .rust
                .contains("context: ExceptionContext::new()")
            && translation.rust.contains("__java_self.context")
            && !translation.rust.contains("__java_self.get_context()")
            && translation.rust.contains(".add_message")
            && translation.rust.contains("max: max")
            && !translation
                .rust
                .contains("super(...) requires inheritance lowering")
            && !translation.rust.contains("compile_error!"),
        "{}",
        translation.rust
    );
}

#[test]
fn source_object_varargs_forward_and_append_erased_arguments() {
    let translation = translate_source(
        r#"
class ObjectVarargsCaller {
    private ExceptionContext context;

    void run(Object pattern, Number max, Object... args) {
        context.addMessage(pattern, args);
        context.addMessage(pattern, max, args);
    }
}

class ExceptionContext {
    private Object[][] stored;

    ExceptionContext() {
    }

    public void addMessage(Object pattern, Object... arguments) {
        stored[0] = arguments;
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains(".add_message(pattern, args.clone()")
            && translation
                .rust
                .contains("let mut __java_varargs = vec![Box::new(max")
            && translation.rust.contains("__java_varargs.extend(args)")
            && !translation.rust.contains("args.as_slice()")
            && !translation.rust.contains("vec![max, args]")
            && !translation.rust.contains("compile_error!"),
        "{}",
        translation.rust
    );
}

#[test]
fn inherited_context_trailing_call_uses_context_field_and_shared_varargs_plan() {
    let subclass = r#"
class ChildProblem extends BaseProblem {
    private final Number max;

    ChildProblem(Number max, Object... args) {
        getContext().addMessage("max", max, args);
        this.max = max;
    }
}
"#;
    let superclass = r#"
class BaseProblem {
    protected ExceptionContext context;

    BaseProblem() {
        context = new ExceptionContext();
    }

    ExceptionContext getContext() {
        return context;
    }
}

class ExceptionContext {
    private Object[][] stored;

    ExceptionContext() {
    }

    public void addMessage(Object pattern, Object... arguments) {
        stored[0] = arguments;
    }
}
"#;

    let translation = translate_source_with_context_sources(
        subclass,
        &[superclass.to_string()],
        &FactSet::default(),
    );

    assert!(
        translation.rust.contains("__java_self.context.add_message")
            && translation.rust.contains("max: max")
            && translation
                .rust
                .contains("let mut __java_varargs = vec![Box::new(max.clone()")
            && translation.rust.contains("__java_varargs.extend(args)")
            && !translation.rust.contains("__java_self.get_context()")
            && !translation.rust.contains("vec![max, args]")
            && !translation.rust.contains("compile_error!"),
        "{}",
        translation.rust
    );
}

#[test]
fn external_standard_exception_super_with_trailing_body_lowers_subclass_state() {
    let translation = translate_source(
        r#"
class CauseStateException extends IllegalStateException {
    private final ExceptionContext context;

    CauseStateException(Throwable cause, String message) {
        super(cause);
        context = new ExceptionContext(this);
        getContext().addMessage(message);
    }

    ExceptionContext getContext() {
        return context;
    }
}

class ExceptionContext {
    ExceptionContext(CauseStateException owner) {
    }

    void addMessage(String message) {
    }
}
"#,
    );

    assert!(
        translation.rust.contains("pub struct CauseStateException")
            && translation.rust.contains("context: ExceptionContext")
            && translation
                .rust
                .contains("let mut __java_self = CauseStateException")
            && translation
                .rust
                .contains("__java_self.context = ExceptionContext::new(__java_self.clone()")
            && translation.rust.contains(".add_message(message")
            && !translation
                .rust
                .contains("super(...) requires inheritance lowering")
            && !translation.rust.contains("compile_error!"),
        "{}",
        translation.rust
    );
}

#[test]
fn exception_self_as_throwable_context_argument_uses_facade_boundary() {
    let translation = translate_source(
        r#"
class FacadeContextException extends IllegalArgumentException {
    private final ExceptionContext context;

    FacadeContextException(String message) {
        context = new ExceptionContext(this);
    }
}

class ExceptionContext {
    ExceptionContext(Throwable owner) {
    }
}
"#,
    );

    assert!(
        translation.rust.contains("context: ExceptionContext")
            && !translation
                .rust
                .contains("context: Default::default()")
            && translation.rust.contains(
                "ExceptionContext::new(Default::default()/* TODO(translation): Java exception self-as-supertype facade boundary */)"
            )
            && !translation.rust.contains("downcast::<Throwable>")
            && !translation.rust.contains("invalid Java cast"),
        "{}",
        translation.rust
    );
}

#[test]
fn jdk_file_reader_super_constructor_is_no_state_facade() {
    let translation = translate_source(
        r#"
import java.io.File;
import java.io.FileReader;

class MiniSmartFileReader extends FileReader {
    MiniSmartFileReader(File file) {
        super(file);
    }
}
"#,
    );

    assert!(
        translation.rust.contains("pub struct MiniSmartFileReader")
            && translation
                .rust
                .contains("pub fn new(file: File) -> MiniSmartFileReader")
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
fn jdk_stream_wrapper_super_constructors_preserve_platform_state_fields() {
    let translation = translate_source(
        r#"
import java.io.FilterInputStream;
import java.io.FilterOutputStream;
import java.io.InputStream;
import java.io.OutputStream;
import java.util.zip.GZIPInputStream;

class MiniGzipStream extends GZIPInputStream {
    private final int size;

    MiniGzipStream(InputStream input, int size) {
        super(input, size);
        this.size = size;
    }
}

class MiniFilterInput extends FilterInputStream {
    private final boolean enabled;

    MiniFilterInput(InputStream input) {
        super(input);
        this.enabled = true;
    }
}

class MiniFilterOutput extends FilterOutputStream {
    private final boolean enabled;

    MiniFilterOutput(OutputStream output) {
        super(output);
        this.enabled = true;
    }
}
"#,
    );

    assert!(
        translation.rust.contains("pub struct MiniGzipStream")
            && translation.rust.contains("size: i32")
            && translation
                .rust
                .contains("let mut __java_self = MiniGzipStream")
            && translation.rust.contains("size: size")
            && translation.rust.contains("r#in: Option<JavaIoInputStream>")
            && translation
                .rust
                .contains("__java_self.r#in = Some(input.into())")
            && translation
                .rust
                .contains("let mut __java_self = MiniFilterInput")
            && translation.rust.contains("enabled: true")
            && translation.rust.contains("out: Option<JavaIoOutputStream>")
            && translation
                .rust
                .contains("__java_self.out = Some(output.into())")
            && translation
                .rust
                .contains("let mut __java_self = MiniFilterOutput")
            && translation.rust.contains("enabled: true")
            && !translation
                .rust
                .contains("super(...) requires inheritance lowering")
            && !translation.rust.contains("compile_error!"),
        "{}",
        translation.rust
    );
}

#[test]
fn emitted_jdk_stream_subclass_gets_query_backed_inherited_close_surface() {
    let translation = translate_source(
        r#"
import java.io.*;

class MiniCloseableFilterInput extends FilterInputStream {
    MiniCloseableFilterInput(InputStream input) {
        super(input);
    }

    void finish() throws IOException {
        this.close();
    }
}
"#,
    );

    assert!(
        translation.rust.contains("impl MiniCloseableFilterInput")
            && translation
                .rust
                .contains("pub fn close(&self) -> Result<(), JavaException>")
            && translation.rust.contains("self.close();")
            && !translation.rust.contains("no method named")
            && !translation.rust.contains("compile_error!"),
        "{}",
        translation.rust
    );
}

#[test]
fn local_filter_input_stream_shadow_does_not_get_jdk_inherited_close_surface() {
    let translation = translate_source(
        r#"
class FilterInputStream {
}

class LocalFilterInputShadow extends FilterInputStream {
    void finish() {
        this.close();
    }
}
"#,
    );

    assert!(
        !translation
            .rust
            .contains("impl LocalFilterInputShadow {\n    pub fn close"),
        "{}",
        translation.rust
    );
}

#[test]
fn emitted_jdk_stream_subclass_converts_to_exact_platform_super_slot() {
    let translation = translate_source(
        r#"
import java.io.*;

class LocalFilterOutputAdapter extends FilterOutputStream {
    LocalFilterOutputAdapter(OutputStream output) {
        super(output);
    }
}

class UsesLocalFilterOutputAdapter {
    java.io.OutputStream wrap(OutputStream output) {
        return new LocalFilterOutputAdapter(output);
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("return LocalFilterOutputAdapter::new(output).into();")
            && translation
                .rust
                .contains("impl From<LocalFilterOutputAdapter> for JavaIoOutputStream"),
        "{}",
        translation.rust
    );
}

#[test]
fn modeled_external_super_constructor_field_read_defers_until_self_temporary_exists() {
    let translation = translate_source(
        r#"
import java.io.FilterInputStream;
import java.io.InputStream;

class ExternalSuperConstructorFieldRead extends FilterInputStream {
    private boolean enabled;
    private int bufferLength;
    private byte[] buffer;

    ExternalSuperConstructorFieldRead(InputStream input, int options) {
        super(input);
        this.enabled = (options & 1) != 0;
        this.bufferLength = enabled ? 4 : 3;
        this.buffer = new byte[bufferLength];
    }
}
"#,
    );

    let length_assignment = translation
        .rust
        .find("__java_self.buffer_length = if __java_self.enabled { 4 } else { 3 };")
        .unwrap_or(usize::MAX);
    let buffer_assignment = translation
        .rust
        .find("__java_self.buffer = vec![0; __java_self.buffer_length as usize];")
        .unwrap_or(usize::MAX);
    assert!(
        translation
            .rust
            .contains("let mut __java_self = ExternalSuperConstructorFieldRead")
            && length_assignment < buffer_assignment
            && !translation
                .rust
                .contains("buffer: vec![0; self.buffer_length")
            && !translation.rust.contains("buffer_length: if self.enabled"),
        "{}",
        translation.rust
    );
}

#[test]
fn jdk_stream_wrapper_inherits_typed_protected_stream_fields() {
    let translation = translate_source(
        r#"
import java.io.InputStream;
import java.io.IOException;
import java.util.zip.GZIPInputStream;

class MiniGzipState extends GZIPInputStream {
    MiniGzipState(InputStream input) {
        super(input);
    }

    int copiedLen() {
        return len;
    }

    byte[] buffer() {
        return buf;
    }

    int remaining() {
        return inf.getRemaining();
    }

    int probe() throws IOException {
        byte[] b = new byte[1];
        int ret = in.read(b, 0, 1);
        return ret;
    }

    int superProbe(byte[] b) throws IOException {
        return super.read(b, 0, 1);
    }
}
"#,
    );

    assert!(
        translation.rust.contains("r#in: InputStream")
            && !translation.rust.contains("pub in: InputStream")
            && translation.rust.contains("inf: Inflater")
            && translation.rust.contains("buf: Vec<i8>")
            && translation.rust.contains("len: i32")
            && translation.rust.contains("return self.len;")
            && translation
                .rust
                .contains("return self.buf.to_vec()/* TODO(translation): validate added clone */;")
            && translation
                .rust
                .contains("return self.inf.get_remaining();")
            && translation
                .rust
                .contains("let mut b: Vec<i8> = vec![0; 1 as usize];")
            && translation
                .rust
                .contains("let ret: i32 = self.r#in.read_3(b.as_mut_slice(), 0, 1)?;")
            && translation
                .rust
                .contains("pub fn super_probe(&self, b: &mut [i8]) -> Result<i32, JavaException>")
            && translation
                .rust
                .contains("return Ok(self.r#in.read_3(b, 0, 1)?);")
            && !translation.rust.contains("Default::default()?"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "jdk_stream_wrapper_typed_protected_stream_fields",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn source_self_referential_instance_fields_use_boxed_optional_storage() {
    let translation = translate_source(
        r#"
class SelfLinkedNode {
    private SelfLinkedNode parent;
    private SelfLinkedNode child;
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("parent: Option<Box<SelfLinkedNode>>")
            && translation
                .rust
                .contains("child: Option<Box<SelfLinkedNode>>")
            && !translation.rust.contains("parent: Option<SelfLinkedNode>")
            && !translation.rust.contains("child: Option<SelfLinkedNode>"),
        "{}",
        translation.rust
    );
}

#[test]
fn jdk_stream_wrapper_super_with_self_linking_trailing_mutation_is_precise_gap() {
    let translation = translate_source(
        r#"
import java.io.InputStream;
import java.util.zip.GZIPInputStream;

class ParentLinkedGzipStream extends GZIPInputStream {
    private ParentLinkedGzipStream parent;
    private ParentLinkedGzipStream child;
    private int size;

    ParentLinkedGzipStream(InputStream input) {
        super(input);
        this.size = -1;
    }

    private ParentLinkedGzipStream(ParentLinkedGzipStream parent) {
        super(parent.in);
        this.size = -1;
        this.parent = parent.parent == null ? parent : parent.parent;
        this.parent.child = this;
    }

    ParentLinkedGzipStream next() {
        return new ParentLinkedGzipStream(this);
    }
}
"#,
    );

    assert!(
        translation.rust.contains(
            "unsupported Java constructor: super(...) followed by unsupported object graph mutation"
        ) && !translation
            .rust
            .contains("unsupported Java constructor: super(...) requires inheritance lowering")
            && translation
                .rust
                .contains("panic!(\"unsupported Java constructor: super(...) followed by unsupported object graph mutation\")"),
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
            && translation.rust.contains("return __java_self;"),
        "{}",
        translation.rust
    );
    cargo_check_generated_modules(
        "no_arg_super_constructor_with_inherited_setter_trailing_call",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn source_subclass_constructor_flows_to_flattened_superclass_slot() {
    let translation = translate_source(
        r#"
class BaseOutputFormat {
    private String id = null;

    void setId(String id) {
        this.id = id;
    }
}

class PairOutputFormat extends BaseOutputFormat {
    PairOutputFormat() {
        super();
        setId("PAIR");
    }
}

class UsesOutputFormat {
    BaseOutputFormat create() {
        BaseOutputFormat format = new PairOutputFormat();
        return format;
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("let format: BaseOutputFormat = PairOutputFormat::new().into();")
            && translation
                .rust
                .contains("impl From<PairOutputFormat> for BaseOutputFormat")
            && !translation
                .rust
                .contains("let format: BaseOutputFormat = PairOutputFormat::new();"),
        "{}",
        translation.rust
    );
    cargo_check_generated_modules(
        "source_subclass_constructor_flows_to_flattened_superclass_slot",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn emitted_context_source_subclass_constructor_flows_to_superclass_slot() {
    let caller = r#"
class PairOutputFormat extends BaseOutputFormat {
    PairOutputFormat() {
        super();
        setId("PAIR");
    }
}

class UsesOutputFormat {
    BaseOutputFormat create() {
        BaseOutputFormat format = new PairOutputFormat();
        return format;
    }
}
"#;
    let base = r#"
class BaseOutputFormat {
    private String id = null;

    void setId(String id) {
        this.id = id;
    }
}
"#;
    let translation = translate_source_with_context_sources_and_summary_sources(
        caller,
        &[base.to_string()],
        &[],
        true,
        &FactSet::default(),
    );

    assert!(
        translation
            .rust
            .contains("let format: BaseOutputFormat = PairOutputFormat::new().into();")
            && translation
                .rust
                .contains("impl From<PairOutputFormat> for BaseOutputFormat")
            && !translation
                .rust
                .contains("let format: BaseOutputFormat = PairOutputFormat::new();"),
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
            && translation.rust.contains("__java_self.set_id(\"CHILD\");")
            && translation.rust.contains("return __java_self;"),
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
            && !translation.rust.contains("pub type Builder")
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
            )
            || translation.rust.contains(
                "variants.map_to_pair(|variant: VariantContext| Box::new((variant, None::<Void>)) as Box<dyn std::any::Any>)"
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
        translation.rust.contains("values.filter(|_value| true)")
            && !translation.rust.contains("Objects::non_null"),
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
        translation.rust.contains(".filter(|value: GATKRead| true)"),
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
        translation.rust.contains(".filter(|value: GATKRead| true)"),
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
fn corpus_spark_interface_receiver_delegation_widens_owned_return_slots() {
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
            .contains("fn get_reference_bases(&self, interval: SimpleInterval) -> Result<Result, JavaException>")
            && translation
                .rust
                .contains("self.reference_source.as_ref().unwrap().get_reference_bases(interval)?")
            && !translation
                .rust
                .contains("fn get_reference_bases(&self, interval: SimpleInterval) -> Result<&Result, JavaException>")
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
                .contains("pub fn new_writer(&self, r#async: bool) -> Box<dyn Writer>")
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
                .contains("let mut __java_self = MiniAsyncWriter { writer: writer };")
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
fn abstract_class_gets_inherent_stubs_for_unimplemented_interface_methods() {
    let translation = translate_source(
        r#"
interface Module {
    Panel getPanel();
    String name();
}

class Panel {
}

abstract class AbstractModule implements Module {
    public Panel helper() {
        return getPanel();
    }

    public String name() {
        return "abstract";
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("pub fn get_panel(&self) -> &Panel")
            && translation.rust.contains(
                "panic!(\"abstract interface method Module.getPanel has no implementation\")"
            )
            && translation.rust.contains("return self.get_panel().clone()")
            && translation.rust.contains("impl Module for AbstractModule")
            && translation.rust.contains(
                "panic!(\"missing Java interface method implementation: AbstractModule.getPanel\")"
            )
            && !translation.rust.contains("compile_error!"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "abstract_interface_method_stub",
        vec![GeneratedCrateModule {
            name: "abstract_interface_method_stub",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn interface_default_method_body_is_emitted_and_inherited() {
    let translation = translate_source(
        r#"
interface Greeter {
    String name();
    default String greet() {
        return "hi " + name();
    }
}

class DefaultGreeter implements Greeter {
    public String name() {
        return "default";
    }

    public String run() {
        return greet();
    }
}

class OverrideGreeter implements Greeter {
    public String name() {
        return "override";
    }

    public String greet() {
        return "yo";
    }
}
"#,
    );

    assert!(
        translation.rust.contains(
            "fn greet(&self) -> String {\n        return format!(\"hi {}\", self.name());\n    }"
        ) && !translation.rust.contains(
            "panic!(\"missing Java interface method implementation: DefaultGreeter.greet\")"
        ) && translation
            .rust
            .contains("fn greet(&self) -> String { OverrideGreeter::greet(self) }"),
        "{}",
        translation.rust
    );

    cargo_test_generated_modules(
        "interface_default_method_body",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
#[cfg(test)]
mod generated_tests {
    use super::*;

    #[test]
    fn defaults_are_inherited_and_overrides_win() {
        let inherited = DefaultGreeter {};
        let overridden = OverrideGreeter {};
        assert_eq!(Greeter::greet(&inherited), "hi default");
        assert_eq!(Greeter::greet(&overridden), "yo");
    }
}
"#,
        }],
    );
}

#[test]
fn interface_default_generic_method_preserves_type_params() {
    let translation = translate_source(
        r#"
interface DefaultMapper {
    default <T> T id(T value) {
        return value;
    }
}

class DefaultMapperImpl implements DefaultMapper {
}
"#,
    );

    assert!(
        translation.rust.contains(
            "fn id<T>(&self, value: T) -> T where Self: Sized {\n        return value;\n    }"
        ) && !translation.rust.contains("fn id(&self, value: T) -> T"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "interface_default_generic_method",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn abstract_class_methods_with_nonvoid_returns_emit_diverging_placeholders() {
    let translation = translate_source(
        r#"
abstract class ContinuedFractionTemplate {
    protected abstract double getA(int n, double x);

    protected abstract double getB(int n, double x);

    double sum(int n, double x) {
        return getA(n, x) + getB(n, x);
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("pub fn get_a(&self, n: i32, x: f64) -> f64")
            && translation.rust.contains(
                "panic!(\"abstract method ContinuedFractionTemplate.getA has no implementation\")"
            )
            && translation
                .rust
                .contains("pub fn get_b(&self, n: i32, x: f64) -> f64")
            && translation.rust.contains(
                "panic!(\"abstract method ContinuedFractionTemplate.getB has no implementation\")"
            ),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "abstract_class_nonvoid_method_placeholders",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn swing_panel_converts_to_component_facade_slots() {
    let translation = translate_source(
        r#"
import java.awt.Component;
import java.io.OutputStream;
import javax.swing.JPanel;

class SVGImageSaver {
    static String saveImage(Component component, OutputStream out) {
        return "";
    }
}

class PanelImageWriter {
    String write(JPanel panel, OutputStream out) {
        return SVGImageSaver.saveImage(panel, out);
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("SVGImageSaver::save_image(panel.into(), out)")
            && translation.rust.contains("impl From<JPanel> for Component"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "swing_panel_component_conversion",
        vec![GeneratedCrateModule {
            name: "swing_panel_component_conversion",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn reassigned_string_param_return_renders_owned_signature() {
    let translation = translate_source(
        r#"
class SvgOptimizer {
    private static String optimize(String svg) {
        svg = merge(svg);
        svg = trim(svg);
        return svg;
    }

    private static String merge(String svg) {
        return svg + "!";
    }

    private static String trim(String svg) {
        return svg.trim();
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("pub fn optimize(mut svg: String) -> String"),
        "{}",
        translation.rust
    );
    assert!(
        !translation
            .rust
            .contains("pub fn optimize<'a>(svg: &'a String) -> &'a String"),
        "{}",
        translation.rust
    );
    assert!(
        !translation.rust.contains("svg = &Self::") && translation.rust.contains("return svg;"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "reassigned_string_param_return_renders_owned_signature",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
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
            .contains("Self::write(&mut StringBuilder::new_1(16).into(), value)")
            || translation
                .rust
                .contains("Self::write(StringBuilder::new_1(16).into(), value)"))
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
fn direct_string_builder_append_is_modeled_as_fluent_jdk_call() {
    let translation = translate_source(
        r#"
class DirectStringBuilderAppend {
    String build(String value) {
        StringBuilder sb = new StringBuilder();
        sb.append(value);
        sb.append("!");
        return sb.toString();
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("let mut sb: StringBuilder = StringBuilder::new_0();")
            && translation.rust.contains("sb.append(value);")
            && translation.rust.contains("sb.append(&\"!\".to_string());")
            && translation.rust.contains(
                "pub struct StringBuilder {\n    value: std::rc::Rc<std::cell::RefCell<String>>,"
            )
            && translation
                .rust
                .contains("pub fn append<T: JavaAppendValue>(&self, value: T) -> Self")
            && translation
                .rust
                .contains("self.value.borrow_mut().push_str(&value.into_java_append_string());")
            && !translation.rust.contains("sb.append(Some(")
            && !translation
                .rust
                .contains("fn append<T0>(&self, _arg0: T0) -> ()")
            && !translation
                .rust
                .contains("pub fn append<T0>(&self, _arg0: T0) -> ()"),
        "{}",
        translation.rust
    );

    cargo_test_generated_modules(
        "direct_string_builder_append_runtime",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
#[test]
fn generated_string_builder_appends_at_runtime() {
    let owner = DirectStringBuilderAppend {};
    assert_eq!(owner.build("ok"), "ok!");
}
"#,
        }],
    );
}

#[test]
fn non_static_inner_class_outer_fields_lower_through_outer_capture_handle() {
    let translation = translate_source(
        r#"
class InnerOuterFieldCapture {
    StringBuilder sb = new StringBuilder();
    int limit = 10;

    class Inner {
        int clamp(int value) {
            if (value > limit) {
                value = limit;
            }
            sb.append(value);
            return value;
        }
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
            .contains("__outer: std::rc::Rc<std::cell::RefCell<InnerOuterFieldCapture>>"),
        "{}",
        translation.rust
    );
    assert!(
        translation.rust.contains("self.__outer.borrow().limit")
            && translation.rust.contains("self.__outer.borrow().sb")
            && translation
                .rust
                .contains("pub fn append<T: JavaAppendValue>(&self, value: T) -> Self")
            && !translation.rust.contains("if value > limit")
            && !translation.rust.contains("value = limit;")
            && !translation.rust.contains("\n        sb.append"),
        "{}",
        translation.rust
    );
    cargo_check_generated_modules(
        "inner_outer_field_capture",
        vec![GeneratedCrateModule {
            name: "inner_outer_field_capture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn string_buffer_append_reused_object_argument_is_borrowed() {
    let translation = translate_source(
        r#"
class StringBufferAppendReusableObject {
    String build() {
        StringBuffer out = new StringBuffer();
        StringBuffer prefix = new StringBuffer();
        out.append(prefix);
        out.append(prefix);
        return out.toString();
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("out.append(&prefix);\n        out.append(&prefix);")
            && !translation.rust.contains("out.append(prefix);"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "string_buffer_append_reused_object_argument",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn string_buffer_append_ternary_argument_stays_owned_value() {
    let translation = translate_source(
        r#"
class StringBufferAppendTernaryValue {
    String build(String[] names, int i) {
        StringBuffer out = new StringBuffer();
        out.append(names[i].length() > 2 ? names[i].substring(0, 2) : names[i]);
        return out.toString();
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("out.append(if (names[i as usize].len() as i32) > 2")
            && !translation.rust.contains("{ &java_string_substring")
            && !translation.rust.contains("} else { names[i as usize] })"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "string_buffer_append_ternary_value",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn string_buffer_append_support_accepts_char_arrays_and_object_values() {
    let translation = translate_source(
        r#"
class AppendWideJavaValues {
    String build(char[] letters, Object other) {
        StringBuffer buffer = new StringBuffer();
        buffer.append(letters);
        buffer.append(other);
        return buffer.toString();
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("pub fn append<T: JavaAppendValue>(&self, value: T) -> Self")
            && translation.rust.contains("buffer.append(letters")
            && translation
                .rust
                .contains("buffer.append(java_erased_object_to_string(&other))")
            && !translation
                .rust
                .contains("pub fn append(&self, _arg0: impl std::fmt::Display) -> Self"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "string_buffer_append_wide_java_values",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn source_object_to_string_uses_java_object_stringification_fallback() {
    let translation = translate_source(
        r#"
class SourceObjectToStringFallback {
    String describe(SourceObjectToStringFallback other) {
        return other.toString();
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("return java_erased_object_to_string(&other);")
            && !translation.rust.contains("return other.to_string();")
            && !translation
                .rust
                .contains("impl std::fmt::Display for SourceObjectToStringFallback"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "source_object_to_string_fallback",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn source_object_to_string_fallback_applies_across_source_classes() {
    let translation = translate_source(
        r#"
class SourceObjectToStringCallee {
}

class SourceObjectToStringCaller {
    String describe(SourceObjectToStringCallee other) {
        return other.toString();
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("return java_erased_object_to_string(&other);")
            && !translation.rust.contains("return other.to_string();"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "source_object_to_string_cross_source",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn inherited_source_to_string_suppresses_object_fallback() {
    let translation = translate_source(
        r#"
class SourceToStringBase {
    public String toString() {
        return "base";
    }
}

class SourceToStringChild extends SourceToStringBase {
    String describe() {
        return this.toString();
    }
}
"#,
    );

    assert!(
        translation.rust.contains("impl SourceToStringChild")
            && translation
                .rust
                .contains("pub fn to_string(&self) -> String")
            && !translation.rust.contains(
                "pub fn to_string(&self) -> String { java_erased_object_to_string(self) }"
            ),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "inherited_source_to_string_no_object_fallback",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn string_buffer_to_string_stays_jdk_facade_method() {
    let translation = translate_source(
        r#"
class StringBufferToStringFacade {
    String build() {
        StringBuffer buffer = new StringBuffer();
        buffer.append("ok");
        return buffer.toString();
    }
}
"#,
    );

    assert!(
        translation.rust.contains("return buffer.to_string();")
            && !translation
                .rust
                .contains("return java_erased_object_to_string(&buffer);"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "string_buffer_to_string_facade",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn source_object_string_concat_uses_java_object_stringification_fallback() {
    let translation = translate_source(
        r#"
class SourceObjectStringConcatFallback {
    String describe(SourceObjectStringConcatFallback other) {
        return "value=" + other;
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("java_erased_object_to_string(&other)")
            && !translation.rust.contains("format!(\"value={}\", other)"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "source_object_string_concat_fallback",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn explicit_source_to_string_method_wins_over_object_fallback() {
    let translation = translate_source(
        r#"
class ExplicitSourceToString {
    public String toString() {
        return "explicit";
    }

    String describe(ExplicitSourceToString other) {
        return other.toString();
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("pub fn to_string(&self) -> String")
            && translation.rust.contains("return other.to_string();")
            && !translation
                .rust
                .contains("return java_erased_object_to_string(&other);"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "explicit_source_to_string_method",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
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
        translation.rust.contains("pub struct OutputStream;")
            && translation
                .rust
                .contains("pub fn new(_arg0: JavaIoOutputStream) -> Self")
            && translation
                .rust
                .contains("return PrintStream::new(out.into());")
            && !translation
                .rust
                .contains("pub fn new<T0>(_arg0: T0) -> Self"),
        "{}",
        translation.rust
    );
}

#[test]
fn optional_print_stream_field_println_uses_arity_named_support() {
    let effect = new_java2rust::external_packs::java_platform_api_effect_for_receiver_with_arity(
        "PrintStream",
        "println",
        Some(1),
    )
    .expect("PrintStream.println(String) should have a platform ABI fact");
    assert_eq!(effect.role, ExternalApiRole::SideEffect);
    assert_eq!(effect.return_type, Some(JavaType::Void));

    let translation = translate_source(
        r#"
import java.io.PrintStream;

class OptionalPrintStreamFieldPrintln {
    private PrintStream outputStream = null;

    void emit(String value) {
        outputStream.println(value);
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("self.output_stream.as_ref().unwrap().println_1(value);")
            && translation
                .rust
                .contains("pub fn println_1<T0>(&self, _arg0: T0) -> ()")
            && translation
                .rust
                .contains("pub fn println<T0>(&self, _arg0: T0) -> ()")
            && !translation
                .rust
                .contains("self.output_stream.as_ref().unwrap().println(value);"),
        "{}",
        translation.rust
    );
}

#[test]
fn optional_print_stream_field_println_with_throwing_arg_keeps_arity_name() {
    let translation = translate_source(
        r#"
import java.io.IOException;
import java.io.PrintStream;

class OptionalPrintStreamThrowingArg {
    private PrintStream outputStream = null;
    private Encoder encoder = null;

    void emit(String value) throws IOException {
        outputStream.println(encoder.encode(value));
    }
}

class Encoder {
    String encode(String value) throws IOException {
        return value;
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("self.output_stream.as_ref().unwrap().println_1(")
            && !translation
                .rust
                .contains("self.output_stream.as_ref().unwrap().println("),
        "{}",
        translation.rust
    );
}

#[test]
fn optional_print_stream_field_println_with_mutating_optional_arg_uses_mut_receiver() {
    let translation = translate_source(
        r#"
import java.io.IOException;
import java.io.PrintStream;

class OptionalPrintStreamMutatingArg {
    private PrintStream outputStream = null;
    private Encoder encoder = null;

    void emit(String value) throws IOException {
        outputStream.println(encoder.encode(value));
    }
}

class Encoder {
    private int count = 0;

    String encode(String value) throws IOException {
        count++;
        return value;
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("pub fn emit(&mut self, value: &str) -> Result<(), JavaException>")
            && translation
                .rust
                .contains("self.output_stream.as_ref().unwrap().println_1(")
            && translation
                .rust
                .contains("self.encoder.as_mut().unwrap().encode(value.to_string())?"),
        "{}",
        translation.rust
    );
}

#[test]
fn file_output_stream_constructor_selects_file_and_string_overload_slots() {
    let translation = translate_source(
        r#"
	import java.io.File;
	import java.io.FileOutputStream;

	class FileOutputStreamOverloadSlot {
	    private File file = null;

	    FileOutputStreamOverloadSlot(File input) {
	        this.file = input;
	    }

	    FileOutputStream fromFileField() throws Exception {
	        return new FileOutputStream(file);
	    }

	    FileOutputStream fromStringPath(String path) throws Exception {
	        return new FileOutputStream(path);
	    }
	}
	"#,
    );

    assert!(
        (translation.rust.contains(
            "return Ok(FileOutputStream::new(self.file.as_ref().expect(\"Java null receiver\").clone()"
            ) || translation.rust.contains(
            "return Ok(FileOutputStream::new(self.file.as_ref().cloned()/* TODO(translation): validate added clone */.expect(\"Java null receiver\")"
            )) && !translation.rust.contains("FileOutputStream::new(self.file.clone()")
            && translation
                .rust
                .contains("return Ok(FileOutputStream::new(path)")
            && !translation.rust.contains("FileOutputStream::new(File::new(path"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "file_output_stream_overload_slots",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn zip_output_stream_converts_to_output_stream_slot() {
    let translation = translate_source(
        r#"
	import java.io.FileOutputStream;
	import java.io.OutputStream;
	import java.util.zip.ZipOutputStream;

	class ZipOutputStreamSlot {
	    static String save(OutputStream out) {
	        return "ok";
	    }

	    static String write(FileOutputStream fileOut) {
	        ZipOutputStream zip = new ZipOutputStream(fileOut);
	        return save(zip);
	    }
	}
	"#,
    );

    assert!(
        translation.rust.contains("return Self::save(zip.into());")
            && translation
                .rust
                .contains("impl From<ZipOutputStream> for OutputStream"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "zip_output_stream_slot",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn base64_output_stream_converts_to_output_stream_slot() {
    let translation = translate_source(
        r#"
	import java.io.ByteArrayOutputStream;
	import java.io.OutputStream;
	import org.apache.commons.codec.binary.Base64OutputStream;
	import net.sourceforge.iharder.base64.Base64;

	class Base64OutputStreamSlot {
	    static OutputStream encode(ByteArrayOutputStream out) {
	        OutputStream stream = new Base64OutputStream(out);
	        return stream;
	    }

	    static String encodeAndReuse(ByteArrayOutputStream out) {
	        OutputStream stream = new Base64OutputStream(out);
	        return out.toString() + stream.toString();
	    }

	    static String wrapOutput(OutputStream out) {
	        OutputStream stream = new Base64OutputStream(out);
	        return out.toString() + stream.toString();
	    }

	    static String wrapIHarder(ByteArrayOutputStream out) {
	        OutputStream stream = new Base64.OutputStream(out);
	        return out.toString() + stream.toString();
	    }
	}
	"#,
    );

    assert!(
        translation
            .rust
            .contains("let stream: OutputStream = Base64OutputStream::new(out).into();")
            && translation.rust.contains(
                "let stream: OutputStream = Base64OutputStream::new(out.clone()/* TODO(translation): validate added clone */).into();"
            )
            && translation.rust.contains(
                "let stream: OutputStream = Base64OutputStream::new(out.clone()/* TODO(translation): validate added clone */).into();"
            )
            && translation
                .rust
                .contains("impl From<Base64OutputStream> for OutputStream"),
        "{}",
        translation.rust
    );
    assert!(
        !translation.rust.contains("invalid Java cast"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "base64_output_stream_slot",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn system_in_static_field_and_thread_sleep_use_jdk_facades() {
    let translation = translate_source(
        r#"
import java.io.InputStreamReader;

class SystemInAndThreadSleepUse {
    InputStreamReader open() throws Exception {
        Thread.sleep(1000);
        return new InputStreamReader(System.in);
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("std::thread::sleep(std::time::Duration::from_millis(1000 as u64));")
            && translation
                .rust
                .contains("return Ok(InputStreamReader::new(System::r#in));")
            && translation.rust.contains("pub struct InputStream;")
            && translation
                .rust
                .contains("pub const r#in: InputStream = InputStream;")
            && !translation.rust.contains("\n        thread::sleep")
            && !translation.rust.contains("system.r#in"),
        "{}",
        translation.rust
    );
    cargo_check_generated_modules(
        "system_in_and_thread_sleep_use_jdk_facades",
        vec![GeneratedCrateModule {
            name: "system_in_and_thread_sleep_use_jdk_facades",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn system_out_and_err_print_stream_statements_lower_to_rust_macros() {
    let translation = translate_source(
        r#"
class SystemPrintStreamStatements {
    void report(String value) {
        System.out.println(value);
        System.err.print("warn:");
        System.err.println(value);
        System.out.println();
    }
}
"#,
    );

    assert!(
        translation.rust.contains("println!(\"{}\", value);")
            && translation
                .rust
                .contains("eprint!(\"{}\", \"warn:\".to_string());")
            && translation.rust.contains("eprintln!(\"{}\", value);")
            && translation.rust.contains("println!();")
            && !translation.rust.contains("Out::")
            && !translation.rust.contains("Err::"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "system_print_stream_statements",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn java_exception_logging_surface_compiles_without_system_err_value() {
    let translation = translate_source(
        r#"
class JavaExceptionLoggingSurface {
    void run(boolean fail) {
        try {
            if (fail) {
                throw new RuntimeException("boom");
            }
        } catch (Exception e) {
            System.err.println(e.getLocalizedMessage());
            e.printStackTrace(System.err);
            e.printStackTrace();
        }
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("pub fn get_localized_message(&self) -> Option<String>")
            && translation.rust.contains(
                "eprintln!(\"{}\", e.get_localized_message().as_deref().unwrap_or(\"null\"));"
            )
            && translation
                .rust
                .contains("Java printStackTrace omitted by translation runtime")
            && !translation.rust.contains("System::err")
            && !translation.rust.contains(".print_stack_trace("),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "java_exception_logging_surface",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
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
            .contains("pub trait Sink {\n    fn write(&self, value: &str) -> Result<(), JavaException>;")
            && translation
                .rust
                .contains("impl Sink for FileSink {\n    fn write(&self, value: &str) -> Result<(), JavaException> { FileSink::write(self, value.to_string()) }")
            && !translation
                .rust
                .contains("fn write(&self, value: &str);"),
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
            "pub trait Sink {\n    fn write(&self, value: &str) -> Result<(), JavaException>;"
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
            .contains("impl Sink for FileSink {\n    fn write(&self, value: &str) -> Result<(), JavaException> { FileSink::write(self, value.to_string()) }"),
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
fn erased_object_sequence_push_moves_when_no_later_read() {
    let translation = translate_source(
        r#"
class ErasedObjectSequencePush {
    private Object[][] stored;

    void remember(Object... args) {
        stored[0] = args;
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("self.stored.as_mut().expect(\"Java null receiver\")[0 as usize] = args;")
            && !translation
                .rust
                .contains("args.clone()/* TODO(translation): validate added clone */"),
        "{}",
        translation.rust
    );
}

#[test]
fn erased_object_sequence_list_push_moves_when_no_later_read() {
    let translation = translate_source(
        r#"
import java.util.List;

class ErasedObjectSequenceListPush {
    private List<Object[]> stored;

    void remember(Object... args) {
        stored.add(args);
    }
}
"#,
    );

    assert!(
        translation.rust.contains(".push(args);")
            && !translation
                .rust
                .contains("args.clone()/* TODO(translation): validate added clone */"),
        "{}",
        translation.rust
    );
}

#[test]
fn erased_object_sequence_list_push_inside_loop_moves_block_local() {
    let translation = translate_source(
        r#"
import java.util.List;

class ErasedObjectSequenceListPushInsideLoop {
    private List<Object[]> stored;

    void remember(int len) {
        stored = new java.util.ArrayList<Object[]>(len);
        int i = 0;
        while (i < len) {
            int aLen = 1;
            final Object[] args = new Object[aLen];
            int j = 0;
            while (j < aLen) {
                args[j] = Integer.valueOf(j);
                j++;
            }
            stored.add(args);
            i++;
        }
    }
}
"#,
    );

    assert!(
        translation.rust.contains(".push(args);")
            && !translation
                .rust
                .contains("args.clone()/* TODO(translation): validate added clone */"),
        "{}",
        translation.rust
    );
}

#[test]
fn object_stream_erased_object_sequence_list_push_moves_block_local() {
    let translation = translate_source(
        r#"
import java.io.ObjectInputStream;
import java.util.List;

class ObjectStreamErasedObjectSequenceListPush {
    private List<Object[]> msgArguments;

    void read(ObjectInputStream in) throws Exception {
        int len = in.readInt();
        msgArguments = new java.util.ArrayList<Object[]>(len);
        int i = 0;
        while (i < len) {
            int aLen = in.readInt();
            final Object[] args = new Object[aLen];
            int j = 0;
            while (j < aLen) {
                args[j] = in.readObject();
                j++;
            }
            msgArguments.add(args);
            i++;
        }
    }
}
"#,
    );

    assert!(
        translation.rust.contains(".push(args);")
            && translation
                .rust
                .contains("args[j as usize] = r#in.read_object()?;")
            && !translation
                .rust
                .contains("args.clone()/* TODO(translation): validate added clone */"),
        "{}",
        translation.rust
    );
}

#[test]
fn object_stream_erased_object_sequence_for_loop_field_push_moves_block_local() {
    let translation = translate_source(
        r#"
import java.io.ObjectInputStream;
import java.util.List;

class ObjectStreamErasedObjectSequenceForLoopFieldPush {
    private List<Object[]> msgArguments;

    void read(ObjectInputStream in) throws Exception {
        final int len = in.readInt();
        msgArguments = new java.util.ArrayList<Object[]>(len);
        for (int i = 0; i < len; i++) {
            final int aLen = in.readInt();
            final Object[] args = new Object[aLen];
            for (int j = 0; j < aLen; j++) {
                args[j] = in.readObject();
            }
            msgArguments.add(args);
        }
    }
}
"#,
    );

    assert!(
        translation.rust.contains(".push(args);")
            && translation
                .rust
                .contains("args[j as usize] = r#in.read_object()?;")
            && !translation
                .rust
                .contains("args.clone()/* TODO(translation): validate added clone */"),
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
            .contains("vec![format.clone()/* TODO(translation): validate added clone */.into(), cardinality.into()]")
            && !translation.rust.contains(
                "vec![format.clone()/* TODO(translation): validate added clone */.into(), cardinality.clone()/* TODO(translation): validate added clone */.into()]"
            ),
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
external-pack dep Path semantic
external-pack dep JavaRDD semantic
external-pack dep WriteOption semantic
external-pack dep ConcreteWriteOption semantic
external-pack-capability dep GATKPath clone=true final=true
external-pack-capability dep Path clone=true final=true
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
            .contains("pub fn run(output: &str, reference: Option<&GATKPath>, reads: JavaRDD")
            && translation.rust.contains("Self::write(output, None, reads.clone()/* TODO(translation): validate added clone */, &vec![option.clone()/* TODO(translation): validate added clone */.into()]);")
            && translation
                .rust
                .contains("Self::write(output, reference, reads, &vec![option.clone()/* TODO(translation): validate added clone */.into()]);")
            && !translation
                .rust
                .contains("output.clone()/* TODO(translation): validate added clone */")
            && !translation
                .rust
                .contains("Some(reference.clone()/* TODO(translation): validate added clone */)"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "reused-optional-borrowed-source-argument",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
#[derive(Clone, PartialEq, Default)]
pub struct Path;
"#,
        }],
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
    private final String mKey;

    SourceNullableFieldGetter(String qualityHeader) {
        this.mKey = qualityHeader;
        if (qualityHeader != null && !qualityHeader.isEmpty()) {
            this.qualityHeader = qualityHeader;
        } else {
            this.qualityHeader = null;
        }
    }

    String getQualityHeader() {
        return qualityHeader;
    }

    String getKey() {
        return mKey;
    }

    String describe() {
        return "quality " + getQualityHeader();
    }

    String keyDescribe() {
        return "key " + getKey();
    }

    String forwarded() {
        String value = getQualityHeader();
        return value;
    }
}

class SourceNullableFieldGetterChild extends SourceNullableFieldGetter {
    SourceNullableFieldGetterChild(String qualityHeader) {
        super(qualityHeader);
    }

    String childDescribe() {
        return "quality " + getQualityHeader();
    }

    String explicitChildDescribe() {
        return "quality " + this.getQualityHeader();
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("pub fn get_quality_header(&self) -> Option<String>")
            && (translation.rust.contains("return self.quality_header.as_ref().cloned()")
                || translation.rust.contains("return self.quality_header.clone()"))
            && (translation.rust.contains(
                "format!(\"quality {}\", self.get_quality_header().as_ref().map(|value| value.to_string()).unwrap_or_else(|| \"null\".to_string()))"
            ) || translation
                .rust
                .contains("format!(\"quality {}\", self.quality_header.as_deref().unwrap_or(\"null\"))"))
            && (translation.rust.contains(
                "format!(\"key {}\", self.get_key().as_ref().map(|value| value.to_string()).unwrap_or_else(|| \"null\".to_string()))"
            ) || translation
                .rust
                .contains("format!(\"key {}\", self.m_key.as_deref().unwrap_or(\"null\"))"))
            && !translation.rust.contains("format!(\"key {}\", self.get_key())")
            && translation
                .rust
                .contains("pub fn child_describe(&self) -> String")
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
                    .contains("let value: Option<String> = self.quality_header.clone();")
                || translation
                    .rust
                    .contains("let value: Option<String> = self.quality_header.as_ref().cloned();"))
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
fn public_source_instance_fields_remain_public_in_generated_struct() {
    let translation = translate_source(
        r#"
class PublicSourceFieldConsumer {
    boolean enabled(Config config) {
        return config.svgOutput;
    }

    String separator(Config config) {
        return config.lineSeparator;
    }
}

class Config {
    public boolean svgOutput = true;
    public String lineSeparator = "\n";
    private String privateValue = "hidden";
}
"#,
    );

    assert!(
        translation.rust.contains("pub svg_output: bool")
            && translation.rust.contains("pub line_separator: String")
            && translation.rust.contains("private_value: String")
            && !translation.rust.contains("pub private_value: String")
            && translation.rust.contains("return config.svg_output;")
            && translation
                .rust
                .contains("return config.line_separator.clone()"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "public_source_instance_fields",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn private_static_singleton_getter_lowers_to_owned_source_value() {
    let translation = translate_source(
        r#"
class StaticSingletonGetterUse {
    boolean enabled() {
        return Config.getInstance().svgOutput;
    }
}

class Config {
    private static Config instance = new Config();
    public boolean svgOutput = true;

    private Config() {
        svgOutput = true;
    }

    public static Config getInstance() {
        return instance;
    }
}
"#,
    );

    assert!(
        translation.rust.contains("pub fn get_instance() -> Config")
            && translation.rust.contains("return Config::new();")
            && translation
                .rust
                .contains("return Config::get_instance().svg_output;")
            && !translation.rust.contains("return Some(&instance);")
            && !translation.rust.contains("Option<&Config>"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "private_static_singleton_getter",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn private_static_singleton_with_hashmap_field_derives_clone_when_value_is_cloneable() {
    let translation = translate_source(
        r#"
import java.util.HashMap;

class StaticSingletonHashMapGetterUse {
    Format get(String id) {
        return Factory.getInstance().getFormat(id);
    }
}

class Factory {
    private static Factory instance = null;
    private HashMap<String, Format> formats = new HashMap<String, Format>();

    static Factory getInstance() {
        if (instance == null) {
            instance = new Factory();
        }
        return instance;
    }

    Format getFormat(String id) {
        return (Format) formats.get(id);
    }
}

class Format {}
"#,
    );

    assert!(
        translation.rust.contains("#[derive(Clone")
            && translation.rust.contains(
                "pub struct Factory {\n    formats: std::collections::HashMap<String, Format>"
            )
            && translation.rust.contains("cell.borrow().clone()")
            && !translation.rust.contains("downcast::<Format>")
            && !translation.rust.contains("invalid Java cast"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "private_static_singleton_hashmap_getter",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
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
            && (translation.rust.contains("return self.quality_header.as_ref().cloned()")
                || translation.rust.contains("return self.quality_header.clone()"))
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
fn source_fastq_constants_owner_is_not_erased_by_builtin_external_pack() {
    let constants = r#"
package htsjdk.samtools.fastq;

public class FastqConstants {
    public static final String SEQUENCE_HEADER = "@";
    public static final String QUALITY_HEADER = "+";
}
"#;
    let translation = translate_source_with_context_sources_and_summary_sources(
        constants,
        &[constants.to_string()],
        &[],
        true,
        &FactSet::default(),
    );

    assert!(
        translation
            .rust
            .contains("pub const SEQUENCE_HEADER: &'static str = \"@\";")
            && translation
                .rust
                .contains("pub const QUALITY_HEADER: &'static str = \"+\";")
            && !translation
                .rust
                .contains("pub type FastqConstants = Box<dyn std::any::Any>;"),
        "{}",
        translation.rust
    );
}

#[test]
fn source_vcf_enum_return_is_not_erased_by_builtin_external_pack() {
    let enum_source = r#"
package htsjdk.variant.vcf;

public enum VCFHeaderLineType {
    Integer,
    String
}
"#;
    let getter_source = r#"
package htsjdk.variant.vcf;

public class VCFSourceEnumGetter {
    private VCFHeaderLineType type;

    public VCFHeaderLineType getType() {
        return type;
    }
}
"#;
    let translation = translate_source_with_context_sources_and_summary_sources(
        getter_source,
        &[enum_source.to_string(), getter_source.to_string()],
        &[],
        true,
        &FactSet::default(),
    );

    assert!(
        translation
            .rust
            .contains("pub fn get_type(&self) -> VCFHeaderLineType")
            && !translation
                .rust
                .contains("pub fn get_type(&self) -> Option<Box<dyn std::any::Any>>")
            && !translation
                .rust
                .contains("Box::new(value.clone()) as Box<dyn std::any::Any>"),
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
fn static_external_nullable_constructor_field_initializer_does_not_double_wrap() {
    let translation = translate_source(
        r#"
class StaticExternalNullableConstructorFieldInitializer {
    String lineSeparator;

    StaticExternalNullableConstructorFieldInitializer() {
        lineSeparator = System.getProperty("line.separator");
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("line_separator: System::get_property(\"line.separator\".to_string())")
            && !translation
                .rust
                .contains("line_separator: Some(System::get_property"),
        "{}",
        translation.rust
    );
    cargo_check_generated_modules(
        "static_external_nullable_constructor_field_initializer",
        vec![GeneratedCrateModule {
            name: "static_external_nullable_constructor_field_initializer",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn static_string_storage_initializer_preserves_lazy_default_value() {
    let translation = translate_source(
        r#"
class StaticOptionalStringStorageInitializer {
    static String name = "alpha";

    static String get() {
        return name;
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("cell.borrow().clone().unwrap_or_else(|| \"alpha\".to_string())")
            && !translation.rust.contains("Some(Some("),
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
            arg_count: None,
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
        translation.rust.contains("value.is_some()")
            && translation.rust.contains(
                "value.as_ref().map(|value| (*value).clone()).expect(\"Java null receiver\")"
            )
            && !translation.rust.contains(".is_present()")
            && !translation.rust.contains(".get()"),
        "{}",
        translation.rust
    );
}

#[test]
fn java_optional_or_else_lowers_to_rust_option_unwrap_or() {
    let translation = translate_source(
        r#"
import java.util.Optional;

class JavaOptionalOrElseLowering {
    String fallback(Optional<String> value, String fallback) {
        return value.orElse(fallback);
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("value.as_ref().map(|value| (*value).clone()).unwrap_or(fallback)")
            && !translation.rust.contains(".or_else(")
            && !translation.rust.contains(".or_else_"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "java_optional_or_else_lowering",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn java_objects_null_predicates_lower_from_rust_option_shape() {
    let translation = translate_source(
        r#"
import java.util.Objects;

class JavaObjectsNullPredicateLowering {
    boolean keep() {
        String value = null;
        return Objects.nonNull(value);
    }

    boolean drop() {
        String value = null;
        return Objects.isNull(value);
    }

    boolean concrete(String value) {
        return Objects.nonNull(value);
    }
}
"#,
    );

    assert!(
        translation.rust.contains("value.is_some()")
            && translation.rust.contains("value.is_none()")
            && translation
                .rust
                .contains("pub fn concrete(&self, value: &str) -> bool")
            && translation.rust.contains("return true;")
            && !translation.rust.contains("Objects::non_null(")
            && !translation.rust.contains("JavaUtilObjects::non_null("),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "java_objects_null_predicate_lowering",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn documents_try_with_resources_copy_gap() {
    let translation = translate_source(&idiom_gap("TryWithResourcesCopy.java"));

    assert!(
        translation.rust.contains("pub fn copy")
            && translation.rust.contains("new_input_stream")
            && translation.rust.contains("input.read_3(")
            && translation.rust.contains(")?;")
            && translation.rust.contains("output.write_3(")
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
            .contains("line = self.reader.as_ref().unwrap().read_line()?;")
            && translation.rust.contains("return Ok(line);")
            && !translation.rust.contains("read_line() -> ()")
            && !translation
                .rust
                .contains("line = Some(self.reader.as_ref().unwrap().read_line()?);"),
        "{}",
        translation.rust
    );
}

#[test]
fn buffered_reader_read_line_assignment_to_initialized_optional_wraps_result_value() {
    let translation = translate_source(
        r#"
import java.io.BufferedReader;
import java.io.IOException;

class BufferedReaderReadLineInitializedOptional {
    String load(BufferedReader reader) throws IOException {
        String line = "";
        while (true) {
            line = reader.readLine();
            if (line == null) {
                break;
            }
        }
        return line;
    }
}
"#,
    );

    assert!(
        translation.rust.contains("line = reader.read_line()?;")
            && !translation
                .rust
                .contains("line = Some(reader.read_line()?);"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "buffered_reader_read_line_initialized_optional",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn buffered_reader_read_line_initializer_is_optional_when_rechecked_for_null() {
    let translation = translate_source(
        r#"
import java.io.BufferedReader;
import java.io.IOException;

class BufferedReaderReadLineInitializerOptional {
    String load(BufferedReader reader) throws IOException {
        String line = reader.readLine();
        while (true) {
            line = reader.readLine();
            if (line == null) {
                break;
            }
            line.trim();
        }
        return line;
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("let mut line: Option<String> = reader.read_line()?;")
            && translation.rust.contains("line = reader.read_line()?;")
            && translation.rust.contains("if line.is_none()")
            && !translation
                .rust
                .contains("let mut line: String = reader.read_line()?;"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "buffered_reader_read_line_initializer_optional",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn buffered_reader_read_line_assignment_condition_wraps_result_value() {
    let translation = translate_source(
        r#"
import java.io.BufferedReader;
import java.io.IOException;

class BufferedReaderReadLineAssignmentCondition {
    int count(BufferedReader reader) throws IOException {
        String line = "";
        int count = 0;
        while ((line = reader.readLine()) != null) {
            count++;
        }
        return count;
    }
}
"#,
    );

    assert!(
        translation.rust.contains("line = reader.read_line()?;")
            && !translation
                .rust
                .contains("line = Some(reader.read_line()?);"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "buffered_reader_read_line_assignment_condition",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn buffered_reader_read_line_assignment_condition_with_and_does_not_move_optional() {
    let translation = translate_source(
        r#"
import java.io.BufferedReader;
import java.io.IOException;

class BufferedReaderReadLineAssignmentConditionAnd {
    int count(BufferedReader reader) throws IOException {
        String line = "";
        int count = 0;
        while ((line = reader.readLine()) != null && line.trim().length() > 0) {
            count++;
        }
        return count;
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("{ line = reader.read_line()?; line.is_some() }")
            && !translation
                .rust
                .contains("{ line = reader.read_line()?; line }.is_some()"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "buffered_reader_read_line_assignment_condition_and",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn while_true_loop_lowers_to_loop_for_definite_assignment() {
    let translation = translate_source(
        r#"
class WhileTrueDefiniteAssignment {
    double run(double x) {
        double remA;
        double remB;
        int k = 1;
        while (true) {
            remA = x + k;
            remB = remA - x;
            if (remA > 10.0) {
                break;
            }
            k++;
        }
        return remA + remB;
    }
}
"#,
    );

    assert!(
        translation.rust.contains("loop {")
            && !translation.rust.contains("while true")
            && translation.rust.contains("return rem_a + rem_b;"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "while_true_definite_assignment",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
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
fn plain_fallthrough_try_with_value_returns_uses_optional_result_carrier() {
    let translation = translate_source(
        r#"
class PlainFallthroughTryWithValueReturns {
    String describe(boolean first, boolean second) {
        try {
            if (first) {
                return "first";
            } else if (second) {
                return "second";
            }
        } catch (Exception e) {
            System.err.println(e.getMessage());
        }
        return "";
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("match (|| -> Result<Option<String>, JavaException>")
            && translation
                .rust
                .contains("return Ok(Some(\"first\".to_string()));")
            && translation
                .rust
                .contains("return Ok(Some(\"second\".to_string()));")
            && translation.rust.contains("return Ok(None);")
            && translation
                .rust
                .contains("Ok(Some(value)) => return value,")
            && translation.rust.contains("Ok(None) => {}")
            && translation.rust.contains("return \"\".to_string();"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "plain_fallthrough_try_with_value_returns_uses_optional_result_carrier",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn caught_exception_message_display_uses_nullable_facade_signature() {
    let translation = translate_source(
        r#"
class CaughtExceptionMessageDisplay {
    String run() {
        try {
            throw new RuntimeException("bad");
        } catch (Exception e) {
            return "Failed: " + e.getMessage();
        }
        return "";
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("pub fn get_message(&self) -> Option<String>")
            && translation
                .rust
                .contains("e.get_message().as_deref().unwrap_or(\"null\")")
            && !translation
                .rust
                .contains("pub fn get_message(&self) -> String"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "caught_exception_message_display",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn input_stream_no_arg_read_uses_result_facade_overload() {
    let translation = translate_source(
        r#"
import java.io.InputStream;

class InputStreamNoArgReadFacade {
    int first(InputStream input) throws Exception {
        return input.read();
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("pub fn read_0(&self) -> Result<i32, JavaException>")
            && translation.rust.contains("return Ok(input.read_0()?);")
            && !translation.rust.contains("return input.read_0();"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "input_stream_no_arg_read_facade",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
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
    let rust_path = std::env::temp_dir().join("new_java2rust_switch_returns_enum_match_test.rs");
    let bin_path = std::env::temp_dir().join("new_java2rust_switch_returns_enum_match_test");
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
fn source_enum_constructor_payload_fields_emit_match_backed_accessors() {
    let translation = translate_source(
        r#"
class EnumPayloadMethodUse {
    enum PayloadVersion {
        V1("VCFv4.1", "fileformat"),
        V2("VCFv4.2", "fileformat");

        private final String versionString;
        private final String formatString;

        PayloadVersion(String versionString, String formatString) {
            this.versionString = versionString;
            this.formatString = formatString;
        }

        boolean isAtLeastAsRecentAs(PayloadVersion target) {
            return this.ordinal() >= target.ordinal();
        }

        String getVersionString() {
            return versionString;
        }

        String getFormatString() {
            return this.formatString;
        }

        static PayloadVersion toPayloadVersion(String version) {
            for (PayloadVersion hv : PayloadVersion.values()) {
                if (hv.versionString.equals(version)) {
                    return hv;
                }
            }
            return null;
        }

        static boolean isFormatString(String format) {
            for (PayloadVersion hv : PayloadVersion.values()) {
                if (hv.formatString.equals(format)) {
                    return true;
                }
            }
            return false;
        }
    }

    String latest() {
        PayloadVersion version = PayloadVersion.V2;
        return version.getVersionString();
    }

    boolean recent() {
        return PayloadVersion.V2.isAtLeastAsRecentAs(PayloadVersion.V1);
    }

    boolean fromPayloadFieldLoop() {
        return PayloadVersion.toPayloadVersion("VCFv4.2") == PayloadVersion.V2
            && PayloadVersion.isFormatString("fileformat");
    }
}
"#,
    );

    assert!(
        translation.rust.contains("pub fn ordinal(&self) -> i32")
            && translation
                .rust
                .contains("pub fn version_string(&self) -> String")
            && translation
                .rust
                .contains("Self::V2 => \"VCFv4.2\".to_string()")
            && translation
                .rust
                .contains("return Some(self.version_string());")
            && translation
                .rust
                .contains("return Some(self.format_string());")
            && translation.rust.contains("if hv.version_string()")
            && translation.rust.contains("if hv.format_string()")
            && !translation.rust.contains("hv.version_string.as_ref()")
            && !translation.rust.contains("hv.format_string.as_ref()")
            && !translation.rust.contains("self.version_string.clone()")
            && !translation.rust.contains("self.format_string.clone()"),
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
    fn enum_payload_methods_match_java_values() {
        let fixture = EnumPayloadMethodUse {};
        assert_eq!(fixture.latest(), Some("VCFv4.2".to_string()));
        assert!(fixture.recent());
        assert!(fixture.from_payload_field_loop());
        assert!(EnumPayloadMethodUsePayloadVersion::V2.is_at_least_as_recent_as(&EnumPayloadMethodUsePayloadVersion::V1));
        assert_eq!(EnumPayloadMethodUsePayloadVersion::V1.get_format_string(), Some("fileformat".to_string()));
    }
}
"#
    );
    let rust_path = std::env::temp_dir().join("new_java2rust_enum_payload_method_test.rs");
    let bin_path = std::env::temp_dir().join("new_java2rust_enum_payload_method_test");
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
        "enum payload method test did not compile\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let output = Command::new(&bin_path).output().unwrap();
    assert!(
        output.status.success(),
        "enum payload method behavior test failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
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
fn source_java_enum_emits_default_for_support_placeholders() {
    let translation = translate_source(
        r#"
class EnumDefaultSupport {
    enum LineType {
        Header,
        Body
    }

    LineType fallback() {
        return LineType.Body;
    }
}
"#,
    );

    assert!(
        translation.rust.contains(
            "impl Default for EnumDefaultSupportLineType {\n    fn default() -> Self { Self::Header }\n}"
        ) && translation
            .rust
            .contains("return EnumDefaultSupportLineType::Body;"),
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
fn switch_assignment_with_default_fallthrough_lowers_to_match_assignment() {
    let translation = translate_source(
        r#"
class SwitchAssignmentWithDefaultFallthrough {
    enum CountType {
        A,
        R,
        INTEGER
    }

    int encode(CountType countType, int count) {
        int number;
        switch (countType) {
            case A:
                number = 1;
                break;
            case R:
                number = 2;
                break;
            case INTEGER:
            default:
                number = count;
        }
        return number;
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
        translation.rust.contains("number = match count_type {")
            && translation
                .rust
                .contains("SwitchAssignmentWithDefaultFallthroughCountType::A => 1")
            && translation
                .rust
                .contains("SwitchAssignmentWithDefaultFallthroughCountType::INTEGER => count")
            && translation.rust.contains("_ => count")
            && !translation.rust.contains("__java_switch_matched"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "switch_assignment_with_default_fallthrough",
        vec![GeneratedCrateModule {
            name: "switch_assignment_with_default_fallthrough",
            rust: translation.rust.clone(),
            tests: "",
        }],
    );
}

#[test]
fn nullable_source_enum_field_switch_keeps_concrete_enum_storage() {
    let translation = translate_source(
        r#"
public class NullableEnumSwitchOwner {
    public enum CountType {
        INTEGER,
        UNBOUNDED
    }

    private CountType countType;
    private int count;

    public NullableEnumSwitchOwner() {
    }

    public NullableEnumSwitchOwner(CountType countType) {
        this.countType = countType;
    }

    public void setFixed() {
        countType = CountType.INTEGER;
    }

    public boolean isFixedCount() {
        return countType == CountType.INTEGER;
    }

    public int getCount() {
        switch (countType) {
            case INTEGER:
                return count;
            case UNBOUNDED:
                return -1;
            default:
                return 0;
        }
    }
}

class NullableEnumSwitchChild extends NullableEnumSwitchOwner {
    public NullableEnumSwitchChild() {
        super(NullableEnumSwitchOwner.CountType.INTEGER);
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("count_type: Option<NullableEnumSwitchOwnerCountType>")
            && translation.rust.contains(
                "self.count_type.as_ref() == Some(&NullableEnumSwitchOwnerCountType::INTEGER)"
            )
            && translation
                .rust
                .contains("Some(NullableEnumSwitchOwnerCountType::INTEGER) =>")
            && translation
                .rust
                .contains("self.count_type = Some(NullableEnumSwitchOwnerCountType::INTEGER)")
            && translation
                .rust
                .contains("count_type: Some(NullableEnumSwitchOwnerCountType::INTEGER)")
            && !translation.rust.contains("count_type: Option<Box<dyn")
            && !translation.rust.contains("__static_i_n_t_e_g_e_r"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "nullable_enum_switch_owner",
        vec![GeneratedCrateModule {
            name: "nullable_enum_switch_owner",
            rust: translation.rust,
            tests: "",
        }],
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
fn raw_map_return_refines_to_consistent_returned_local_generic_type() {
    let translation = translate_source(
        r#"
import java.util.HashMap;

class Params {
    static void run(String[] args) {
        HashMap<String, String> params = getParams(args);
        sink(params);
    }

    static HashMap getParams(String[] args) {
        HashMap<String, String> params = new HashMap<String, String>();
        params.put("enabled", "true");
        if (args.length > 0) {
            params.put("first", args[0]);
        }
        return params;
    }

    static void sink(HashMap<String, String> params) {
    }
}
"#,
    );

    assert!(
        translation.rust.contains(
            "pub fn get_params(args: &[String]) -> std::collections::HashMap<String, String>"
        ) && translation.rust.contains(
            "let params: std::collections::HashMap<String, String> = Self::get_params(args);"
        ) && !translation
            .rust
            .contains("HashMap<String, Box<dyn std::any::Any>>"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "raw_map_return_refinement",
        vec![GeneratedCrateModule {
            name: "raw_map_return_refinement",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn map_hash_code_lowers_to_java_runtime_helper() {
    let translation = translate_source(
        r#"
import java.util.HashMap;

class MapHashCodeJavaApi {
    int hash(HashMap<String, String> values) {
        return values.hashCode();
    }
}
"#,
    );

    assert!(
        translation.rust.contains("java_map_hash_code(&values)")
            && !translation.rust.contains(".hash_code("),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "map_hash_code_java_api",
        vec![GeneratedCrateModule {
            name: "map_hash_code_java_api",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn source_enum_hash_code_lowers_to_java_value_hash_helper() {
    let translation = translate_source(
        r#"
class SourceEnumHashCodeJavaApi {
    enum Kind {
        ALPHA,
        BETA
    }

    private Kind kind;

    int hash() {
        return kind.hashCode();
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("java_value_hash_code(&self.kind.as_ref().unwrap())")
            && !translation
                .rust
                .contains(".kind.as_ref().unwrap().hash_code("),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "source_enum_hash_code_java_api",
        vec![GeneratedCrateModule {
            name: "source_enum_hash_code_java_api",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn raw_map_extends_object_argument_borrows_local_not_map_type() {
    let translation = translate_source(
        r#"
import java.util.LinkedHashMap;
import java.util.Map;

class RawMapEncodingTarget {
    static String encode(Map<String, ? extends Object> values) {
        if (values.isEmpty()) {
            return "empty";
        }
        return "encoded";
    }
}

class RawMapExtendsObjectArgument {
    String run(String name) {
        Map<String, Object> map = new LinkedHashMap<String, Object>();
        map.put("ID", name);
        return RawMapEncodingTarget.encode(map);
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("RawMapEncodingTarget::encode(&map)")
            && !translation.rust.contains("&&map")
            && !translation.rust.contains("&Map")
            && !translation.rust.contains("let _java_value = map"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "raw_map_extends_object_argument",
        vec![GeneratedCrateModule {
            name: "raw_map_extends_object_argument",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn map_entry_enhanced_for_uses_planned_iterator_and_tuple_fields() {
    let translation = translate_source(
        r#"
import java.util.LinkedHashMap;
import java.util.Map;

class MapEntryEnhancedFor {
    String join(Map<String, ? extends Object> values) {
        StringBuilder builder = new StringBuilder();
        for (Map.Entry<String, ?> entry : values.entrySet()) {
            builder.append(entry.getKey());
        }
        return builder.toString();
    }

    String run(String name) {
        Map<String, Object> map = new LinkedHashMap<String, Object>();
        map.put("ID", name);
        return join(map);
    }
}
"#,
    );

    assert!(
        translation.rust.contains("for entry in values.iter()")
            && translation.rust.contains("entry.0")
            && !translation.rust.contains("values.iter().iter()")
            && !translation.rust.contains(".entry_set()")
            && !translation.rust.contains(".get_key()"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "map_entry_enhanced_for",
        vec![GeneratedCrateModule {
            name: "map_entry_enhanced_for",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn map_get_static_string_constant_key_does_not_double_borrow_str() {
    let translation = translate_source(
        r#"
import java.util.HashMap;
import java.util.Map;

class MapGetStaticStringKey {
    static final String ID = "ID";

    String read(Map<String, String> values) {
        return values.get(ID);
    }

    String run(String name) {
        Map<String, String> values = new HashMap<String, String>();
        values.put(ID, name);
        return read(values);
    }
}
"#,
    );

    assert!(
        translation.rust.contains(".get(Self::ID)")
            && !translation.rust.contains(".get(&Self::ID)")
            && !translation.rust.contains("Borrow<&str>"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "map_get_static_string_key",
        vec![GeneratedCrateModule {
            name: "map_get_static_string_key",
            rust: translation.rust,
            tests: "",
        }],
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
fn reassigned_string_enhanced_for_variable_iterates_owned_mutable_values() {
    let translation = translate_source(
        r#"
class ReassignedStringEnhancedFor {
    static String shorten(String value) {
        return value.substring(1);
    }

    static String join(String[] alleles) {
        String out = "";
        for (String allele : alleles) {
            if (allele.startsWith("+")) {
                allele = shorten(allele);
            }
            out = out + allele + allele;
        }
        return out;
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("for mut allele in alleles.iter().cloned()")
            && translation.rust.contains("allele = Self::shorten(")
            && !translation.rust.contains("allele = &Self::shorten")
            && !translation.rust.contains("for allele in alleles.iter()"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "reassigned_string_enhanced_for",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
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
        translation.rust.contains("Self::valid(value.as_ref())")
            && translation
                .rust
                .contains("format!(\"bad {}\", value.as_ref()")
            && translation
                .rust
                .contains("pub fn describe(&self, value: Option<String>) -> Option<String>")
            && translation.rust.contains("return value;")
            && !translation.rust.contains("Self::valid(value.clone()"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "reused_owned_optional_source_argument",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
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
fn erased_object_get_class_name_has_typed_facade_return() {
    let translation = translate_source(
        r#"
class ErasedObjectGetClassName {
    String className(Object obj) {
        if (obj == null) {
            return "";
        }
        return obj.getClass().getName();
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("fn get_class(&self) -> JavaClass<()>")
            && translation
                .rust
                .contains("fn get_class(&self) -> JavaClass<()> { JavaClass::<()>::for_name(\"java.lang.Object\") }")
            && translation.rust.contains(".get_class().get_name()")
            && !translation.rust.contains("fn get_class(&self) -> ()")
            && !translation.rust.contains("get_name() found for unit"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "erased_object_get_class_name",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn erased_object_get_class_is_array_has_class_facade_support() {
    let translation = translate_source(
        r#"
class ErasedObjectGetClassIsArray {
    boolean array(Object obj) {
        if (obj == null) {
            return false;
        }
        return obj.getClass().isArray();
    }
}
"#,
    );

    assert!(
        translation.rust.contains("pub fn is_array(&self) -> bool")
            && translation.rust.contains(".get_class().is_array()")
            && !translation.rust.contains("method `is_array` not found"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "erased_object_get_class_is_array",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn source_object_get_class_name_uses_inherent_java_class_fallback() {
    let translation = translate_source(
        r#"
class SourceObjectGetClassNameFallback {
    private String id = null;

    String name() {
        return id == null ? this.getClass().getName() : id;
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("pub fn get_class(&self) -> JavaClass<SourceObjectGetClassNameFallback>")
            && translation.rust.contains(
                "JavaClass::<SourceObjectGetClassNameFallback>::literal(\"SourceObjectGetClassNameFallback\")"
            )
            && translation.rust.contains("self.get_class().get_name()"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "source_object_get_class_name_fallback",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn receiverless_get_class_resource_stream_uses_current_receiver() {
    let translation = translate_source(
        r#"
import java.io.InputStream;

class ReceiverlessGetClassResourceStream {
    boolean present() {
        InputStream in = getClass().getResourceAsStream("/Templates/header_template.html");
        return in != null;
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("JavaClass::<ReceiverlessGetClassResourceStream>::for_name(\"ReceiverlessGetClassResourceStream\").get_resource_as_stream")
            && !translation.rust.contains("Some(get_class()")
            && !translation.rust.contains(" get_class()."),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "receiverless_get_class_resource_stream",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn class_loader_static_resource_call_uses_platform_facade() {
    let translation = translate_source(
        r#"
import java.awt.image.BufferedImage;
import javax.imageio.ImageIO;

class ClassLoaderStaticResourceImage {
    BufferedImage image(String path) throws Exception {
        return ImageIO.read(ClassLoader.getSystemResource("Templates/" + path));
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("JavaxImageioImageIO::read(ClassLoader::get_system_resource(format!(\"Templates/{}\", path)))")
            && translation.rust.contains("pub struct ClassLoader")
            && translation.rust.contains("pub struct URL")
            && !translation.rust.contains("class_loader.get_system_resource"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "class_loader_static_resource_image",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn inherited_context_method_body_sees_declaring_static_imports() {
    let source = r#"
class AdapterContent extends AbstractQCModule {
}
"#;
    let context = r#"
import java.awt.image.BufferedImage;
import java.io.OutputStream;
import javax.imageio.ImageIO;

class AbstractQCModule {
    void writeDefaultImage(OutputStream out, int width, int height) throws Exception {
        BufferedImage image = new BufferedImage(width, height, BufferedImage.TYPE_INT_RGB);
        ImageIO.write(image, "PNG", out);
    }
}
"#;
    let translation = translate_source_with_context_sources_and_summary_sources(
        source,
        &[context.to_string()],
        &[],
        true,
        &FactSet::default(),
    );

    assert!(
        translation.rust.contains("JavaxImageioImageIO::write")
            && !translation.rust.contains("image_i_o::write"),
        "{}",
        translation.rust
    );
    assert!(
        translation.rust.contains(
            "pub fn write_default_image(&self, out: OutputStream, width: i32, height: i32) -> Result<(), JavaException>"
        ),
        "{}",
        translation.rust
    );

    // The minimal fixture does not assemble context-only support types into a
    // crate; the regression here is the inherited body's static-owner spelling.
}

#[test]
fn class_literal_get_class_loader_uses_platform_facade() {
    let translation = translate_source(
        r#"
import java.io.InputStream;
import java.net.URL;

class ClassLiteralGetClassLoaderFacade {
    InputStream stream(String path) {
        return ClassLiteralGetClassLoaderFacade.class.getClassLoader().getResourceAsStream(path);
    }

    URL resource(String path) {
        return ClassLiteralGetClassLoaderFacade.class.getClassLoader().getResource(path);
    }
}
"#,
    );

    let class_loader = "JavaClass::<ClassLiteralGetClassLoaderFacade>::literal(\"ClassLiteralGetClassLoaderFacade\").get_class_loader()";
    let borrowed_class_loader = format!("(&{class_loader})");
    assert!(
        ([class_loader, borrowed_class_loader.as_str()]
            .iter()
            .any(|receiver| translation.rust.contains(&format!(
                "{receiver}.get_resource_as_stream(path).expect(\"Java null receiver\")"
            )) || translation.rust.contains(&format!(
                "{receiver}.get_resource_as_stream(path.to_string()).expect(\"Java null receiver\")"
            ))))
            && ([class_loader, borrowed_class_loader.as_str()]
                .iter()
                .any(|receiver| translation
                    .rust
                    .contains(&format!("{receiver}.get_resource(path)"))
                    || translation
                        .rust
                        .contains(&format!("{receiver}.get_resource(path.to_string())"))))
            && translation
                .rust
                .contains("pub fn get_class_loader(&self) -> ClassLoader")
            && translation.rust.contains("pub fn get_resource_as_stream")
            && translation.rust.contains("pub fn get_resource")
            && !translation.rust.contains("impl Class {")
            && !translation
                .rust
                .contains("method `get_class_loader` not found"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "class_literal_get_class_loader_facade",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn optional_class_loader_resource_stream_assignment_keeps_optional_return_shape() {
    let translation = translate_source(
        r#"
import java.io.InputStream;

class OptionalClassLoaderResourceStreamAssignment {
    InputStream stream(String path) {
        InputStream in = null;
        in = OptionalClassLoaderResourceStreamAssignment.class.getClassLoader().getResourceAsStream(path);
        return in;
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("let mut r#in: Option<InputStream> = None;")
            && (translation.rust.contains(
                "r#in = JavaClass::<OptionalClassLoaderResourceStreamAssignment>::literal(\"OptionalClassLoaderResourceStreamAssignment\").get_class_loader().get_resource_as_stream(path);"
            ) || translation.rust.contains(
                "r#in = JavaClass::<OptionalClassLoaderResourceStreamAssignment>::literal(\"OptionalClassLoaderResourceStreamAssignment\").get_class_loader().get_resource_as_stream(path.to_string());"
            ))
            && !translation.rust.contains("r#in = Some(JavaClass")
            && !translation.rust.contains("Some(Some("),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "optional_class_loader_resource_stream_assignment",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn java_platform_input_stream_upcast_support_emits_from_impl() {
    assert!(
        new_java2rust::external_packs::external_conversion_adapters()
            .iter()
            .any(|fact| fact.from_type == "FileInputStream" && fact.to_type == "InputStream"),
        "java platform conversion adapters should publish FileInputStream -> InputStream"
    );
    assert!(
        new_java2rust::external_packs::external_conversion_adapters()
            .iter()
            .any(|fact| fact.from_type == "InputStream" && fact.to_type == "java.io.InputStream"),
        "java platform conversion adapters should publish InputStream -> java.io.InputStream"
    );
    assert!(
        new_java2rust::external_packs::external_conversion_adapters()
            .iter()
            .any(|fact| fact.from_type == "OutputStream" && fact.to_type == "java.io.OutputStream"),
        "java platform conversion adapters should publish OutputStream -> java.io.OutputStream"
    );
    assert!(
        new_java2rust::external_packs::external_conversion_adapters()
            .iter()
            .any(|fact| fact.from_type == "java.io.ByteArrayOutputStream"
                && fact.to_type == "java.io.OutputStream"),
        "java platform conversion adapters should publish java.io.ByteArrayOutputStream -> java.io.OutputStream"
    );
    let items = new_java2rust::stdlib::external_api_facade_support_items(
        &new_java2rust::java_ir::Program::default(),
        &FactSet::default(),
    );
    let rendered = items
        .iter()
        .map(|item| format!("{item:?}"))
        .collect::<Vec<_>>()
        .join("\n");

    assert!(
        rendered.contains("impl From<FileInputStream> for InputStream"),
        "{rendered}"
    );
    assert!(
        rendered.contains("impl From<InputStream> for JavaIoInputStream"),
        "{rendered}"
    );
    assert!(
        rendered.contains("impl From<JavaIoInputStream> for InputStream"),
        "{rendered}"
    );
    assert!(
        !rendered.contains("impl From<OutputStream> for JavaIoOutputStream"),
        "{rendered}"
    );
    assert!(
        rendered.contains("impl From<JavaIoByteArrayOutputStream> for JavaIoOutputStream"),
        "{rendered}"
    );

    let local_stream_program = new_java2rust::parser::parse_java_source(
        r#"
class Base64 {
    static class InputStream {}
    static class UsesExactJavaIoStream {
        UsesExactJavaIoStream(java.io.InputStream in) {}
    }
}
"#,
    )
    .program;
    let local_items = new_java2rust::stdlib::external_api_facade_support_items(
        &local_stream_program,
        &FactSet::default(),
    );
    let local_rendered = local_items
        .iter()
        .map(|item| format!("{item:?}"))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        local_rendered.contains("impl From<JavaIoInputStream> for InputStream"),
        "{local_rendered}"
    );

    let exact_byte_array_program = new_java2rust::parser::parse_java_source(
        r#"
class UsesExactByteArrayOutputStream {
    byte[] bytes(java.io.ByteArrayOutputStream out) {
        return out.toByteArray();
    }
}
"#,
    )
    .program;
    let byte_array_items = new_java2rust::stdlib::external_api_facade_support_items(
        &exact_byte_array_program,
        &FactSet::default(),
    );
    let byte_array_rendered = byte_array_items
        .iter()
        .map(|item| format!("{item:?}"))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        byte_array_rendered.contains("impl JavaIoByteArrayOutputStream"),
        "{byte_array_rendered}"
    );
    assert!(
        byte_array_rendered.contains("pub fn to_byte_array(&self) -> Vec<i8>"),
        "{byte_array_rendered}"
    );
}

#[test]
fn file_input_stream_constructor_upcasts_into_optional_input_stream_slot() {
    let translation = translate_source(
        r#"
import java.io.FileInputStream;
import java.io.InputStream;

class FileInputStreamOptionalInputSlot {
    InputStream open(String path) throws Exception {
        InputStream in = null;
        in = new FileInputStream(path);
        return in;
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("let mut r#in: Option<InputStream> = None;")
            && translation
                .rust
                .contains("r#in = Some(FileInputStream::new(path).into());")
            && translation
                .rust
                .contains("impl From<FileInputStream> for InputStream"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "file_input_stream_optional_input_slot",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn emitted_source_call_uses_authoritative_owned_param_signature() {
    let translation = translate_source(
        r#"
class SourceOwnedParamCallUsesSolvedSignature {
    static class Alignment {
        int score() { return 1; }
    }

    static class Pair {
        public String format(Alignment alignment) {
            return "" + alignment.score();
        }
    }

    static String run() {
        Alignment alignment = new Alignment();
        return new Pair().format(alignment);
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("pub fn format(&self, alignment: SourceOwnedParamCallUsesSolvedSignatureAlignment) -> String")
            && translation
                .rust
                .contains("return SourceOwnedParamCallUsesSolvedSignaturePair::new().format(alignment);")
            && !translation.rust.contains("format(&alignment)"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "emitted_source_call_uses_authoritative_owned_param_signature",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn resource_input_stream_read_and_close_have_typed_facade_support() {
    let translation = translate_source(
        r#"
import java.io.InputStream;

class ResourceInputStreamReadClose {
    int readAll() throws Exception {
        InputStream in = getClass().getResourceAsStream("/Templates/header_template.html");
        if (in == null) {
            return 0;
        }
        byte[] buffer = new byte[128];
        int total = 0;
        int len;
        while ((len = in.read(buffer)) != -1) {
            total += len;
        }
        in.close();
        return total;
    }
}
"#,
    );

    assert!(
        translation.rust.contains(".read_3(")
            && translation.rust.contains("buffer.as_mut_slice()")
            && translation.rust.contains(".close()")
            && translation.rust.contains("pub fn read_3")
            && translation.rust.contains("pub fn close")
            && !translation.rust.contains("no method named `read`")
            && !translation.rust.contains("no method named `close`"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "resource_input_stream_read_close",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn byte_array_local_with_spaced_dimensions_feeds_string_range_constructor() {
    let translation = translate_source(
        r#"
class ByteArrayStringRangeConstructor {
    String allBytes() {
        byte [] array = new byte[128];
        return new String(array);
    }

    String allBytesUtf8() throws Exception {
        byte [] array = new byte[128];
        return new String(array, "UTF-8");
    }

    String read(int n) {
        byte [] array = new byte[128];
        return new String(array, 0, n);
    }

    String readUtf8(int n) throws Exception {
        byte [] array = new byte[128];
        return new String(array, 0, n, "UTF-8");
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("let array: Vec<i8> = vec![0; 128 as usize];")
            && translation.rust.contains("java_string_from_bytes(&array)")
            && translation
                .rust
                .contains("java_string_from_byte_range(&array, 0, n)")
            && !translation.rust.contains("String::new(array)")
            && !translation.rust.contains("String::new(array, \"UTF-8\")")
            && !translation.rust.contains("String::new(array, 0, n)")
            && !translation
                .rust
                .contains("String::new(array, 0, n, \"UTF-8\")"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "byte_array_string_range_constructor",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn char_array_feeds_java_string_constructor() {
    let translation = translate_source(
        r#"
class CharArrayStringConstructor {
    private char[] letters;

    char[] getLetters() {
        return letters;
    }

    String fromLocal(String value) {
        char[] chars = value.toCharArray();
        return new String(chars);
    }

    String fromField() {
        return new String(letters);
    }

    String fromGetter() {
        return new String(getLetters());
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("return chars.as_slice().iter().collect::<String>();")
            && translation.rust.contains(
                "return self.letters.as_ref().expect(\"Java null receiver\").as_slice().iter().collect::<String>();"
            )
            && translation
                .rust
                .matches("return self.letters.as_ref().expect(\"Java null receiver\").as_slice().iter().collect::<String>();")
                .count()
                >= 2
            && !translation.rust.contains("String::new(chars)")
            && !translation.rust.contains("String::new(self.letters"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "char_array_string_constructor",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn output_stream_write_byte_range_borrows_reused_buffer() {
    let translation = translate_source(
        r#"
import java.io.BufferedOutputStream;
import java.io.InputStream;

class OutputStreamWriteByteRangeBorrow {
    int copy(InputStream input, BufferedOutputStream output) throws Exception {
        byte[] buffer = new byte[128];
        int size = input.read(buffer, 0, buffer.length);
        output.write(buffer, 0, size);
        return buffer.length;
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("let __java_arg2 = buffer.len() as i32; input.read_3(buffer.as_mut_slice(), 0, __java_arg2) }?;")
            && translation
                .rust
                .contains("output.write_3(buffer.as_slice(), 0, size)?;")
            && (translation.rust.contains("return buffer.len() as i32;")
                || translation.rust.contains("return Ok(buffer.len() as i32);"))
            && !translation.rust.contains("output.write(buffer, 0, size)?;"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "output_stream_write_byte_range_borrow",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn source_filter_output_stream_write_byte_array_uses_three_arg_override() {
    let translation = translate_source(
        r#"
import java.io.FilterOutputStream;
import java.io.ByteArrayOutputStream;
import java.io.OutputStream;

class SourceFilterOutputStreamWriteBytes {
    static class EncodingOutput extends FilterOutputStream {
        EncodingOutput(ByteArrayOutputStream out) {
            super(out);
        }

        @Override
        public void write(int value) throws java.io.IOException {
            out.write(value);
        }

        @Override
        public void write(byte[] bytes, int off, int len) throws java.io.IOException {
            for (int i = 0; i < len; i++) {
                write(bytes[off + i]);
            }
        }
    }

    void save(EncodingOutput out, byte[] bytes) throws java.io.IOException {
        out.write(bytes);
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("write_byte_array_int_int(__java_write_bytes, 0, __java_write_len)")
            && !translation.rust.contains("write_int(bytes.as_slice()")
            && !translation.rust.contains("write_int(__java_write_bytes"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "source_filter_output_stream_write_byte_array",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn input_stream_read_byte_array_uses_three_arg_overload_with_full_buffer() {
    let translation = translate_source(
        r#"
import java.io.InputStream;
import java.util.zip.GZIPInputStream;

class SourceGzipReadBytesUse {
    static class LocalGzipInputStream extends GZIPInputStream {
        LocalGzipInputStream(InputStream input) throws java.io.IOException {
            super(input);
        }

        @Override
        public int read(byte[] bytes, int off, int len) throws java.io.IOException {
            return super.read(bytes, off, len);
        }
    }

    int read(LocalGzipInputStream input, byte[] buffer) throws java.io.IOException {
        return input.read(buffer);
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("(__java_read_bytes, 0, __java_read_len)")
            && translation.rust.contains("let __java_read_bytes = buffer;")
            && !translation.rust.contains(".read(buffer.as_mut_slice())"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "input_stream_read_byte_array_uses_three_arg_overload_with_full_buffer",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn optional_resource_constructor_argument_clones_before_later_cleanup_read() {
    let translation = translate_source(
        r#"
import java.io.InputStream;
import java.util.zip.GZIPInputStream;

class OptionalResourceConstructorMoveUse {
    void wrap(InputStream input) throws java.io.IOException {
        InputStream raw = null;
        GZIPInputStream gzip = null;
        try {
            raw = input;
            gzip = new GZIPInputStream(raw);
        } finally {
            if (raw != null) {
                raw.close();
            }
            if (gzip != null) {
                gzip.close();
            }
        }
    }
}
"#,
    );

    assert!(
        translation.rust.contains(
            "GZIPInputStream::new(raw.clone()/* TODO(translation): validate added clone */)"
        ) && translation.rust.contains("raw.as_ref().unwrap().close()?"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "optional_resource_constructor_argument_clones_before_later_cleanup_read",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn optional_resource_facade_upcast_clones_wrapper_before_consuming_unwrap() {
    let translation = translate_source(
        r#"
import java.io.ByteArrayOutputStream;
import java.util.zip.GZIPOutputStream;

class OptionalResourceFacadeUpcastMoveUse {
    void wrap() throws java.io.IOException {
        ByteArrayOutputStream baos = null;
        GZIPOutputStream gzip = null;
        try {
            baos = new ByteArrayOutputStream();
            gzip = new GZIPOutputStream(baos);
        } finally {
            if (baos != null) {
                baos.close();
            }
            if (gzip != null) {
                gzip.close();
            }
        }
    }
}
"#,
    );

    assert!(
        translation.rust.contains(
            "baos.as_ref().cloned()/* TODO(translation): validate added clone */.expect(\"Java null receiver\").into()"
        )
            && !translation
                .rust
                .contains("GZIPOutputStream::new(baos.unwrap()")
            && !translation.rust.contains("baos.clone().unwrap()")
            && translation.rust.contains("baos.as_ref().unwrap().close()?"),
        "{}",
        translation.rust
    );
}

#[test]
fn optional_source_receiver_uses_mut_projection_for_mutating_context_fallback() {
    let translation = translate_source(
        r#"
class OptionalSourceReceiverMutClose {
    static class LocalStream {
        boolean closed;

        void close() throws java.io.IOException {
            closed = true;
        }
    }

    void run() {
        LocalStream stream = null;
        try {
            stream = new LocalStream();
        } finally {
            if (stream != null) {
                stream.close();
            }
        }
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("stream.as_mut().unwrap().close()?;")
            && !translation
                .rust
                .contains("stream.as_ref().unwrap().close();"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "optional_source_receiver_uses_mut_projection_for_mutating_context_fallback",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );

    let nested_translation = translate_source(
        r#"
class Base64 {
    static class InputStream {
        boolean closed;

        void close() {
            closed = true;
        }
    }

    void run() throws java.io.IOException {
        InputStream stream = null;
        try {
            stream = new InputStream();
        } finally {
            if (stream != null) {
                stream.close();
            }
        }
    }
}
"#,
    );

    assert!(
        nested_translation
            .rust
            .contains("stream.as_mut().unwrap().close()?;")
            && !nested_translation
                .rust
                .contains("stream.as_ref().unwrap().close();"),
        "{}",
        nested_translation.rust
    );
}

#[test]
fn buffered_image_graphics_facades_are_typed() {
    let translation = translate_source(
        r#"
import java.awt.Graphics;
import java.awt.image.BufferedImage;

class BufferedImageGraphicsFacades {
    void draw(int width, int height) {
        BufferedImage image = new BufferedImage(width, height, BufferedImage.TYPE_INT_RGB);
        Graphics graphics = image.createGraphics();
        graphics.dispose();
    }
}
"#,
    );

    assert!(
        translation.rust.contains("BufferedImage::TYPE_INT_RGB")
            && (translation
                .rust
                .contains("let graphics: Graphics = image.create_graphics();")
                || translation
                    .rust
                    .contains("let mut graphics: Graphics = image.create_graphics();")
                || translation
                    .rust
                    .contains("let graphics: Graphics = image.create_graphics().into();")
                || translation
                    .rust
                    .contains("let mut graphics: Graphics = image.create_graphics().into();"))
            && translation.rust.contains("graphics.dispose();")
            && !translation
                .rust
                .contains("let graphics: Box<dyn std::any::Any>")
            && !translation
                .rust
                .contains("let mut graphics: Box<dyn std::any::Any>")
            && !translation
                .rust
                .contains("create_graphics(&self) -> Box<dyn std::any::Any>")
            && !translation.rust.contains("create_graphics(&self) -> ()")
            && !translation
                .rust
                .contains("impl Graphics {\n    pub fn dispose(&self) -> ()"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "buffered_image_graphics_facades",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn buffered_image_create_graphics_returns_graphics2d_without_downcast() {
    let translation = translate_source(
        r#"
import java.awt.Graphics2D;
import java.awt.image.BufferedImage;

class BufferedImageGraphics2DCreate {
    void draw(int width, int height) {
        BufferedImage image = new BufferedImage(width, height, BufferedImage.TYPE_INT_ARGB);
        Graphics2D graphics = image.createGraphics();
        graphics.dispose();
    }
}
"#,
    );

    assert!(
        translation.rust.contains("BufferedImage::TYPE_INT_ARGB")
            && (translation
                .rust
                .contains("let graphics: Graphics2D = image.create_graphics();")
                || translation
                    .rust
                    .contains("let mut graphics: Graphics2D = image.create_graphics();"))
            && translation.rust.contains("graphics.dispose();")
            && !translation.rust.contains("downcast::<Graphics2D>")
            && !translation
                .rust
                .contains("let graphics: Box<dyn std::any::Any>")
            && !translation
                .rust
                .contains("create_graphics(&self) -> Graphics {"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "buffered_image_graphics2d_create",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn buffered_image_graphics_passed_to_jpanel_print_stays_typed() {
    let translation = translate_source(
        r#"
import java.awt.Graphics;
import java.awt.image.BufferedImage;
import javax.swing.JPanel;

class BufferedImageGraphicsPanelPrint {
    void draw(JPanel panel, int width, int height) {
        BufferedImage image = new BufferedImage(width, height, BufferedImage.TYPE_INT_RGB);
        Graphics graphics = image.createGraphics();
        panel.print(graphics);
        graphics.dispose();
    }
}
"#,
    );

    assert!(
        translation.rust.contains("panel.print")
            && (translation
                .rust
                .contains("let graphics: Graphics = image.create_graphics();")
                || translation
                    .rust
                    .contains("let mut graphics: Graphics = image.create_graphics();")
                || translation
                    .rust
                    .contains("let graphics: Graphics = image.create_graphics().into();")
                || translation
                    .rust
                    .contains("let mut graphics: Graphics = image.create_graphics().into();"))
            && translation.rust.contains("graphics.dispose();")
            && !translation
                .rust
                .contains("let graphics: Box<dyn std::any::Any>")
            && !translation
                .rust
                .contains("let mut graphics: Box<dyn std::any::Any>"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "buffered_image_graphics_panel_print",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn buffered_image_graphics_from_source_panel_getter_stays_typed() {
    let translation = translate_source(
        r#"
import java.awt.Graphics;
import java.awt.image.BufferedImage;
import javax.swing.JPanel;

class BufferedImageGraphicsSourcePanelGetter {
    private JPanel panel;

    BufferedImageGraphicsSourcePanelGetter(JPanel panel) {
        this.panel = panel;
    }

    JPanel getPanel() {
        return panel;
    }

    void draw(int width, int height) {
        JPanel panel = getPanel();
        BufferedImage image = new BufferedImage(width, height, BufferedImage.TYPE_INT_RGB);
        Graphics graphics = image.createGraphics();
        panel.print(graphics);
        graphics.dispose();
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("let mut graphics: Graphics = image.create_graphics();")
            || translation
                .rust
                .contains("let graphics: Graphics = image.create_graphics();")
            || translation
                .rust
                .contains("let mut graphics: Graphics = image.create_graphics().into();")
            || translation
                .rust
                .contains("let graphics: Graphics = image.create_graphics().into();"),
        "{}",
        translation.rust
    );
    assert!(
        translation.rust.contains("panel.print(&graphics")
            || translation.rust.contains("panel.print(graphics"),
        "{}",
        translation.rust
    );
    assert!(
        !translation.rust.contains("graphics.downcast::<Graphics>")
            && !translation
                .rust
                .contains("let mut graphics: Box<dyn std::any::Any>")
            && !translation
                .rust
                .contains("let graphics: Box<dyn std::any::Any>"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "buffered_image_graphics_source_panel_getter",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn buffered_image_graphics_in_fallible_panel_workflow_stays_typed() {
    let translation = translate_source(
        r#"
import java.awt.Graphics;
import java.awt.image.BufferedImage;
import javax.swing.JPanel;

class BufferedImageGraphicsFalliblePanelWorkflow {
    private JPanel panel;

    BufferedImageGraphicsFalliblePanelWorkflow(JPanel panel) {
        this.panel = panel;
    }

    JPanel getPanel() {
        return panel;
    }

    void draw(int width, int height) throws Exception {
        JPanel resultsPanel = getPanel();
        resultsPanel.setSize(width, height);
        resultsPanel.validate();
        BufferedImage image = new BufferedImage(width, height, BufferedImage.TYPE_INT_RGB);
        Graphics graphics = image.createGraphics();
        resultsPanel.setDoubleBuffered(false);
        resultsPanel.addNotify();
        resultsPanel.validate();
        resultsPanel.print(graphics);
        graphics.dispose();
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("let mut graphics: Graphics = image.create_graphics();")
            || translation
                .rust
                .contains("let graphics: Graphics = image.create_graphics();")
            || translation
                .rust
                .contains("let mut graphics: Graphics = image.create_graphics().into();")
            || translation
                .rust
                .contains("let graphics: Graphics = image.create_graphics().into();"),
        "{}",
        translation.rust
    );
    assert!(
        !translation.rust.contains("graphics.downcast::<Graphics>")
            && !translation
                .rust
                .contains("let mut graphics: Box<dyn std::any::Any>")
            && !translation
                .rust
                .contains("let graphics: Box<dyn std::any::Any>"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "buffered_image_graphics_fallible_panel_workflow",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn nullable_outer_graphics_field_call_uses_concrete_optional_facade_receiver() {
    let translation = translate_source(
        r#"
import java.awt.Component;
import java.awt.Graphics;
import java.awt.Shape;

class NullableOuterGraphicsFieldCall {
    private Graphics graphics;

    NullableOuterGraphicsFieldCall(Component component) {
        graphics = component.getGraphics();
    }

    class Inner {
        Shape clip() {
            return graphics.getClip();
        }

        void close() {
            graphics.dispose();
        }
    }
}
"#,
    );

    assert!(
        translation.rust.contains("graphics: Option<Graphics>")
            && translation
                .rust
                .contains("graphics: component.get_graphics()")
            && translation
                .rust
                .contains("self.__outer.borrow().graphics.as_ref().unwrap().get_clip()")
            && translation
                .rust
                .contains("self.__outer.borrow().graphics.as_ref().unwrap().dispose()")
            && !translation
                .rust
                .contains("graphics: Option<Box<dyn std::any::Any>>")
            && !translation.rust.contains("graphics.get_clip()"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "nullable_outer_graphics_field_call",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn constructor_field_assignment_from_local_keeps_local_and_typed_field() {
    let translation = translate_source(
        r#"
import java.awt.Graphics;
import java.awt.image.BufferedImage;

class ConstructorFieldAssignmentFromLocalGraphics {
    private Graphics graphics;

    ConstructorFieldAssignmentFromLocalGraphics(int width, int height) {
        BufferedImage image = new BufferedImage(width, height, BufferedImage.TYPE_INT_RGB);
        graphics = image.getGraphics();
    }
}
"#,
    );

    assert!(
        translation.rust.contains("graphics: Option<Graphics>")
            && translation
                .rust
                .contains("let image: BufferedImage = BufferedImage::new")
            && translation
                .rust
                .contains("__java_self.graphics = Some(image.get_graphics());")
            && !translation.rust.contains("graphics: image.get_graphics()")
            && !translation
                .rust
                .contains("graphics: Some(image.get_graphics())")
            && !translation
                .rust
                .contains("graphics: Option<Box<dyn std::any::Any>>")
            && !translation.rust.contains("cannot find value `image`"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "constructor_field_assignment_from_local_graphics",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn constructor_assigned_field_read_defers_until_self_temporary_exists() {
    let translation = translate_source(
        r#"
class ConstructorAssignedFieldRead {
    private int seed;
    private int doubled;

    ConstructorAssignedFieldRead(int input) {
        this.seed = input;
        doubled = seed + 1;
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("let mut __java_self = ConstructorAssignedFieldRead")
            && translation.rust.contains("seed: input")
            && translation
                .rust
                .contains("__java_self.doubled = __java_self.seed + 1;")
            && !translation.rust.contains("doubled: self.seed + 1")
            && !translation
                .rust
                .contains("expected value, found module `self`"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "constructor_assigned_field_read",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn source_super_constructor_field_read_defers_until_self_temporary_exists() {
    let translation = translate_source(
        r#"
class SuperConstructorFieldReadBase {
    SuperConstructorFieldReadBase(int input) {
    }
}

class SuperConstructorFieldReadChild extends SuperConstructorFieldReadBase {
    private boolean enabled;
    private int width;
    private int[] values;

    SuperConstructorFieldReadChild(int input, int options) {
        super(input);
        this.enabled = (options & 1) != 0;
        this.width = enabled ? 4 : 3;
        this.values = new int[width];
    }
}
"#,
    );

    let width_assignment = translation
        .rust
        .find("__java_self.width = if __java_self.enabled { 4 } else { 3 };")
        .unwrap_or(usize::MAX);
    let values_assignment = translation
        .rust
        .find("__java_self.values = vec![0; __java_self.width as usize];")
        .unwrap_or(usize::MAX);
    assert!(
        translation
            .rust
            .contains("let mut __java_self = SuperConstructorFieldReadChild")
            && width_assignment < values_assignment
            && !translation.rust.contains("values: vec![0; self.width")
            && !translation.rust.contains("width: if self.enabled"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "source_super_constructor_field_read",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn constructor_nested_arg_reads_assigned_field_without_cloning_self() {
    let translation = translate_source(
        r#"
class ConstructorNestedArgFieldRead {
    private Payload payload = null;
    private Wrapper wrapper = null;

    ConstructorNestedArgFieldRead(Payload input) {
        this.payload = input;
        wrapper = new Wrapper(new Holder(payload));
    }
}

class Payload {
}

class Holder {
    Holder(Payload payload) {
    }
}

class Wrapper {
    Wrapper(Holder holder) {
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("let mut __java_self = ConstructorNestedArgFieldRead")
            && (translation.rust.contains(
                "__java_self.wrapper = Some(Wrapper::new(Holder::new(__java_self.payload.as_ref().expect(\"Java null receiver\").clone()"
            ) || translation.rust.contains(
                "__java_self.wrapper = Some(Wrapper::new(Holder::new(__java_self.payload.as_ref().cloned()/* TODO(translation): validate added clone */.expect(\"Java null receiver\")"
            ))
            && !translation.rust.contains("__java_self.clone().payload"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "constructor_nested_arg_field_read",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn static_owner_name_does_not_collapse_to_same_named_instance_field() {
    let translation = translate_source(
        r#"
import java.awt.Color;
import java.awt.Font;

class StaticOwnerFieldNameCollision {
    private Color color = Color.BLACK;
    private Font font = new Font("Default", Font.PLAIN, 12);

    StaticOwnerFieldNameCollision() {
    }

    int plain() {
        return Font.PLAIN;
    }

    int boldItalic() {
        return Font.BOLD + Font.ITALIC;
    }
}
"#,
    );

    assert!(
        translation.rust.contains("color: Some(Color::BLACK")
            && translation.rust.contains("Font::PLAIN")
            && translation.rust.contains("pub const PLAIN: i32 = 0;")
            && translation.rust.contains("pub const BOLD: i32 = 1;")
            && translation.rust.contains("pub const ITALIC: i32 = 2;")
            && !translation.rust.contains("self.color")
            && !translation.rust.contains("self.font"),
        "{}",
        translation.rust
    );

    cargo_test_generated_modules(
        "static_owner_field_name_collision",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
#[cfg(test)]
mod generated_tests {
    use super::*;

    #[test]
    fn font_style_constants_have_java_values() {
        let fixture = StaticOwnerFieldNameCollision::new();
        assert_eq!(fixture.plain(), 0);
        assert_eq!(fixture.bold_italic(), 3);
    }
}
"#,
        }],
    );
}

#[test]
fn platform_gui_facade_getters_have_precise_return_shapes() {
    let translation = translate_source(
        r#"
import java.awt.Color;
import java.awt.Component;
import java.awt.Font;
import javax.swing.table.TableModel;

class PlatformGuiFacadeGetterShapes {
    int componentWidth(Component component) {
        return component.getWidth();
    }

    int red(Color color) {
        return color.getRed();
    }

    int fontSize(Font font) {
        return font.getSize();
    }

    boolean bold(Font font) {
        return font.isBold();
    }

    int columns(TableModel table) {
        return table.getColumnCount();
    }

    String columnName(TableModel table, int column) {
        return table.getColumnName(column);
    }

    Object value(TableModel table, int row, int column) {
        return table.getValueAt(row, column);
    }

    String stringValue(TableModel table, int row, int column) {
        return String.valueOf(table.getValueAt(row, column));
    }

    void appendValue(StringBuffer buffer, TableModel table, int row, int column) {
        buffer.append(table.getValueAt(row, column));
    }
}
"#,
    );

    assert!(
        translation.rust.contains("pub fn get_width(&self) -> i32")
            && translation.rust.contains("pub fn get_red(&self) -> i32")
            && translation.rust.contains("pub fn get_size(&self) -> i32")
            && translation.rust.contains("pub fn is_bold(&self) -> bool")
            && translation
                .rust
                .contains("pub fn get_column_count(&self) -> i32")
            && translation
                .rust
                .contains("pub fn get_column_name<T0>(&self, _arg0: T0) -> String")
            && translation.rust.contains(
                "pub fn get_value_at<T0, T1>(&self, _arg0: T0, _arg1: T1) -> Box<dyn std::any::Any>"
            )
            && !translation.rust.contains("get_width(&self) -> ()")
            && !translation.rust.contains("get_red(&self) -> ()")
            && !translation.rust.contains("get_size(&self) -> ()")
            && !translation.rust.contains("is_bold(&self) -> ()")
            && !translation.rust.contains("get_column_count(&self) -> ()")
            && !translation
                .rust
                .contains("get_column_name<T0>(&self, _arg0: T0) -> ()")
            && !translation
                .rust
                .contains("get_value_at<T0, T1>(&self, _arg0: T0, _arg1: T1) -> ()")
            && translation
                .rust
                .contains("java_erased_object_to_string(&table.get_value_at(row, column))")
            && !translation.rust.contains("downcast::<String>()")
            && !translation.rust.contains("invalid Java cast"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "platform_gui_facade_getter_shapes",
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
fn string_value_of_static_owner_uses_native_jdk_projection() {
    let translation = translate_source(
        r#"
import java.util.HashMap;
import java.util.Map;

class StringValueOfStaticOwner {
    String start(int value) {
        return String.valueOf(value);
    }

    Map<String, String> labels(int value) {
        Map<String, String> map = new HashMap<>();
        map.put("length", String.valueOf(value));
        return map;
    }
}
"#,
    );

    assert!(
        translation.rust.contains("value.to_string()")
            && !translation.rust.contains("__static_string")
            && !translation.rust.contains(".value_of("),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "string_value_of_static_owner",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn rejected_instanceof_guard_casts_erased_object_to_borrowed_view() {
    let translation = translate_source(
        r#"
class RejectedInstanceofGuardCastsErasedObject {
    private int id;

    boolean same(Object obj) {
        if (obj instanceof RejectedInstanceofGuardCastsErasedObject == false) {
            return false;
        }
        RejectedInstanceofGuardCastsErasedObject other =
            (RejectedInstanceofGuardCastsErasedObject) obj;
        return other.id == id;
    }
}
"#,
    );

    assert!(
        translation.rust.contains(
            "let other: &RejectedInstanceofGuardCastsErasedObject = obj.downcast_ref::<RejectedInstanceofGuardCastsErasedObject>().expect(\"invalid Java cast\");"
        ) && !translation
            .rust
            .contains("*obj.downcast::<RejectedInstanceofGuardCastsErasedObject>")
            && !translation.rust.contains("obj.downcast::<RejectedInstanceofGuardCastsErasedObject>"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "rejected_instanceof_guard_casts_erased_object",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn java_arrays_equals_lowers_as_static_platform_equality() {
    let translation = translate_source(
        r#"
import java.util.Arrays;

class JavaArraysEqualsUse {
    boolean same(double[] left, double[] right) {
        return Arrays.equals(left, right);
    }
}
"#,
    );

    assert!(
        translation.rust.contains("return left == right;")
            && !translation.rust.contains("arrays ==")
            && !translation.rust.contains("Arrays::equals")
            && !translation.rust.contains("arrays.equals"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "java_arrays_equals_use",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn java_arrays_copy_lowers_as_static_platform_copy_projection() {
    let translation = translate_source(
        r#"
import java.util.Arrays;

class JavaArraysCopyUse {
    byte[] prefix(byte[] values) {
        return Arrays.copyOf(values, 2);
    }

    byte[] padded(byte[] values) {
        return Arrays.copyOf(values, 5);
    }

    String[] names(String[] values) {
        return Arrays.copyOfRange(values, 1, 4);
    }

    byte[] padFromEnd(byte[] values) {
        return Arrays.copyOfRange(values, 3, 5);
    }

    byte[] badLength(byte[] values) {
        return Arrays.copyOf(values, -1);
    }

    byte[] badRangeOrder(byte[] values) {
        return Arrays.copyOfRange(values, 2, 1);
    }

    byte[] badRangeStart(byte[] values) {
        return Arrays.copyOfRange(values, -1, 1);
    }

    byte[] badRangePastEnd(byte[] values) {
        return Arrays.copyOfRange(values, 4, 5);
    }
}
"#,
    );

    assert!(
        translation.rust.contains("__java_arrays_out.resize")
            && !translation.rust.contains("Arrays::copy")
            && !translation.rust.contains("JavaUtilArrays::copy"),
        "{}",
        translation.rust
    );

    cargo_test_generated_modules(
        "java_arrays_copy_use",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
#[cfg(test)]
mod generated_tests {
    use super::*;

    #[test]
    fn copies_and_pads_arrays() {
        let fixture = JavaArraysCopyUse {};
        assert_eq!(fixture.prefix(&[1, 2, 3]), vec![1, 2]);
        assert_eq!(fixture.padded(&[7, 8]), vec![7, 8, 0, 0, 0]);
        assert_eq!(
            fixture.names(&["a".to_string(), "b".to_string()]),
            vec!["b".to_string(), String::new(), String::new()]
        );
        assert_eq!(fixture.pad_from_end(&[1, 2, 3]), vec![0, 0]);
        assert!(std::panic::catch_unwind(|| fixture.bad_length(&[1, 2, 3])).is_err());
        assert!(std::panic::catch_unwind(|| fixture.bad_range_order(&[1, 2, 3])).is_err());
        assert!(std::panic::catch_unwind(|| fixture.bad_range_start(&[1, 2, 3])).is_err());
        assert!(std::panic::catch_unwind(|| fixture.bad_range_past_end(&[1, 2, 3])).is_err());
    }
}
"#,
        }],
    );
}

#[test]
fn java_arrays_binary_search_lowers_as_static_platform_search_projection() {
    let translation = translate_source(
        r#"
import java.util.Arrays;

class JavaArraysBinarySearchUse {
    int found(int[] values, int key) {
        return Arrays.binarySearch(values, key);
    }

    int missing(int[] values, int key) {
        return Arrays.binarySearch(values, key);
    }

    int rangeFound(int[] values, int from, int to, int key) {
        return Arrays.binarySearch(values, from, to, key);
    }

    int rangeMissing(int[] values, int from, int to, int key) {
        return Arrays.binarySearch(values, from, to, key);
    }

    int badRangeOrder(int[] values) {
        return Arrays.binarySearch(values, 3, 1, 4);
    }

    int badRangeStart(int[] values) {
        return Arrays.binarySearch(values, -1, 2, 4);
    }

    int badRangePastEnd(int[] values) {
        return Arrays.binarySearch(values, 1, 5, 4);
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains(".binary_search(&__java_arrays_key)")
            && !translation.rust.contains("Arrays::binary_search")
            && !translation.rust.contains("JavaUtilArrays::binary_search"),
        "{}",
        translation.rust
    );

    cargo_test_generated_modules(
        "java_arrays_binary_search_use",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
#[cfg(test)]
mod generated_tests {
    use super::*;

    #[test]
    fn returns_java_index_or_negative_insertion_point() {
        let fixture = JavaArraysBinarySearchUse {};
        assert_eq!(fixture.found(&[1, 3, 5, 7], 5), 2);
        assert_eq!(fixture.missing(&[1, 3, 5, 7], 4), -3);
        assert_eq!(fixture.missing(&[1, 3, 5, 7], 0), -1);
        assert_eq!(fixture.missing(&[1, 3, 5, 7], 9), -5);
        assert_eq!(fixture.range_found(&[1, 3, 5, 7, 9], 1, 4, 5), 2);
        assert_eq!(fixture.range_missing(&[1, 3, 5, 7, 9], 1, 4, 4), -3);
        assert!(std::panic::catch_unwind(|| fixture.bad_range_order(&[1, 3, 5, 7])).is_err());
        assert!(std::panic::catch_unwind(|| fixture.bad_range_start(&[1, 3, 5, 7])).is_err());
        assert!(std::panic::catch_unwind(|| fixture.bad_range_past_end(&[1, 3, 5, 7])).is_err());
    }
}
"#,
        }],
    );
}

#[test]
fn java_arrays_compare_lowers_as_static_platform_lexicographic_projection() {
    let translation = translate_source(
        r#"
import java.util.Arrays;

class JavaArraysCompareUse {
    int ints(int[] left, int[] right) {
        return Arrays.compare(left, right);
    }

    int names(String[] left, String[] right) {
        return Arrays.compare(left, right);
    }
}
"#,
    );

    assert!(
        translation.rust.contains("std::cmp::Ordering::Less => -1")
            && translation.rust.contains("std::cmp::Ordering::Equal => 0")
            && translation
                .rust
                .contains("std::cmp::Ordering::Greater => 1")
            && !translation.rust.contains("Arrays::compare")
            && !translation.rust.contains("JavaUtilArrays::compare"),
        "{}",
        translation.rust
    );

    cargo_test_generated_modules(
        "java_arrays_compare_use",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
#[cfg(test)]
mod generated_tests {
    use super::*;

    #[test]
    fn compares_arrays_lexicographically() {
        let fixture = JavaArraysCompareUse {};
        assert_eq!(fixture.ints(&[1, 2], &[1, 3]), -1);
        assert_eq!(fixture.ints(&[1, 2], &[1, 2]), 0);
        assert_eq!(fixture.ints(&[1, 3], &[1, 2]), 1);
        assert_eq!(fixture.names(&["a".to_string()], &["b".to_string()]), -1);
    }
}
"#,
        }],
    );
}

#[test]
fn java_arrays_hash_code_lowers_as_static_platform_hash_projection() {
    let translation = translate_source(
        r#"
import java.util.Arrays;

class JavaArraysHashCodeUse {
    int ints(int[] values) {
        return Arrays.hashCode(values);
    }

    int bools(boolean[] values) {
        return Arrays.hashCode(values);
    }

    int doubles(double[] values) {
        return Arrays.hashCode(values);
    }

    int names(String[] values) {
        return Arrays.hashCode(values);
    }
}
"#,
    );

    assert!(
        translation.rust.contains("java_arrays_hash_code(values)")
            && !translation.rust.contains("Arrays::hash_code")
            && !translation.rust.contains("JavaUtilArrays::hash_code"),
        "{}",
        translation.rust
    );

    cargo_test_generated_modules(
        "java_arrays_hash_code_use",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
#[cfg(test)]
mod generated_tests {
    use super::*;

    #[test]
    fn hashes_arrays_with_java_seed_and_element_hashes() {
        let fixture = JavaArraysHashCodeUse {};
        assert_eq!(fixture.ints(&[1, 2, 3]), 30817);
        assert_eq!(fixture.ints(&[]), 1);
        assert_eq!(fixture.bools(&[true, false]), 40359);
        assert_eq!(fixture.doubles(&[f64::NAN]), 2146959391);
        assert_eq!(
            fixture.names(&["a".to_string(), "b".to_string()]),
            4066
        );
    }
}
"#,
        }],
    );
}

#[test]
fn java_arrays_to_string_lowers_as_static_platform_format_projection() {
    let translation = translate_source(
        r#"
import java.util.Arrays;

class JavaArraysToStringUse {
    String ints(int[] values) {
        return Arrays.toString(values);
    }

    String names(String[] values) {
        return Arrays.toString(values);
    }

    String empty(byte[] values) {
        return Arrays.toString(values);
    }
}
"#,
    );

    assert!(
        translation.rust.contains("java_arrays_to_string(values)")
            && !translation.rust.contains("Arrays::to_string")
            && !translation.rust.contains("JavaUtilArrays::to_string"),
        "{}",
        translation.rust
    );

    cargo_test_generated_modules(
        "java_arrays_to_string_use",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
#[cfg(test)]
mod generated_tests {
    use super::*;

    #[test]
    fn formats_arrays_with_java_delimiters() {
        let fixture = JavaArraysToStringUse {};
        assert_eq!(fixture.ints(&[1, 2, 3]), "[1, 2, 3]");
        assert_eq!(
            fixture.names(&["alpha".to_string(), "beta".to_string()]),
            "[alpha, beta]"
        );
        assert_eq!(fixture.empty(&[]), "[]");
    }
}
"#,
        }],
    );
}

#[test]
fn java_arrays_fill_lowers_whole_and_range_mutations() {
    let translation = translate_source(
        r#"
import java.util.Arrays;

class JavaArraysFillUse {
    static byte[] all(byte[] values) {
        Arrays.fill(values, (byte) 7);
        return values;
    }

    static int[] range(int[] values) {
        Arrays.fill(values, 1, 3, 9);
        return values;
    }

    static String[] names(String[] values) {
        Arrays.fill(values, 0, 2, "x");
        return values;
    }

    static int[] badRangeOrder(int[] values) {
        Arrays.fill(values, 3, 1, 9);
        return values;
    }

    static int[] noopAtEnd(int[] values) {
        Arrays.fill(values, 3, 3, 9);
        return values;
    }

    static int[] badRangeStart(int[] values) {
        Arrays.fill(values, -1, 2, 9);
        return values;
    }

    static int[] badRangeEnd(int[] values) {
        Arrays.fill(values, 1, 5, 9);
        return values;
    }
}
"#,
    );

    assert!(
        translation.rust.contains(".fill(7 as i8)")
            && translation
                .rust
                .contains("__java_arrays_target[__java_arrays_from..__java_arrays_to].fill(9)")
            && translation.rust.contains(
                "__java_arrays_target[__java_arrays_from..__java_arrays_to].fill(\"x\".to_string())"
            )
            && !translation.rust.contains("Arrays::fill")
            && !translation.rust.contains("JavaUtilArrays::fill"),
        "{}",
        translation.rust
    );

    cargo_test_generated_modules(
        "java_arrays_fill_use",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
#[cfg(test)]
mod generated_tests {
    use super::*;

    #[test]
    fn fills_whole_arrays_and_subranges() {
        let bytes = JavaArraysFillUse::all(vec![1, 2, 3]);
        assert_eq!(bytes, vec![7, 7, 7]);

        let ints = JavaArraysFillUse::range(vec![1, 2, 3, 4]);
        assert_eq!(ints, vec![1, 9, 9, 4]);

        let names = JavaArraysFillUse::names(vec!["a".to_string(), "b".to_string(), "c".to_string()]);
        assert_eq!(names, vec!["x".to_string(), "x".to_string(), "c".to_string()]);

        assert_eq!(JavaArraysFillUse::noop_at_end(vec![1, 2, 3]), vec![1, 2, 3]);
        assert!(std::panic::catch_unwind(|| JavaArraysFillUse::bad_range_order(vec![1, 2, 3])).is_err());
        assert!(std::panic::catch_unwind(|| JavaArraysFillUse::bad_range_start(vec![1, 2, 3])).is_err());
        assert!(std::panic::catch_unwind(|| JavaArraysFillUse::bad_range_end(vec![1, 2, 3])).is_err());
    }
}
"#,
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
fn negated_java_equals_uses_inequality_projection() {
    let translation = translate_source(
        r#"
import java.util.List;

class NegatedJavaEqualsProjection {
    enum Version {
        V1, V2
    }

    boolean tagDifferent(List<String> expected, int index, String value) {
        return !expected.get(index).equals(value);
    }

    boolean versionDifferent(Version hv) {
        return !hv.equals(Version.V1);
    }
}
"#,
    );

    assert!(
        translation.rust.contains("!=")
            && !translation.rust.contains("!expected")
            && !translation.rust.contains("!hv ==")
            && !translation.rust.contains("!hv !="),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "negated_java_equals_projection",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn stream_filter_negated_java_equals_parenthesizes_binary_projection() {
    let translation = translate_source(
        r#"
import java.util.Set;
import java.util.stream.Collectors;

class StreamFilterNegatedJavaEqualsProjection {
    enum Version {
        V1, V2
    }

    String versions(Set<Version> versions) {
        return versions.stream()
                .filter(hv -> !hv.equals(Version.V1))
                .map(Version::name)
                .collect(Collectors.joining(" "));
    }
}
"#,
    );

    assert!(
        translation.rust.contains(".filter(|hv| !(")
            && !translation.rust.contains(".filter(|hv| !hv =="),
        "{}",
        translation.rust
    );
}

#[test]
fn stream_joining_nullable_string_items_use_java_display_projection() {
    let translation = translate_source(
        r#"
import java.util.Set;
import java.util.stream.Collectors;

class StreamJoiningNullableStringItems {
    static class Version {
        String name() {
            return null;
        }
    }

    String names(Set<Version> versions) {
        return versions.stream()
                .map(version -> version.name())
                .collect(Collectors.joining(" "));
    }
}
"#,
    );

    assert!(
        translation.rust.contains(
            ".map(|item| item.as_ref().map(|value| value.to_string()).unwrap_or_else(|| \"null\".to_string())).collect::<Vec<_>>().join(\" \")"
        ) && !translation.rust.contains(".map(|item| item.to_string()).collect::<Vec<_>>().join(\" \")"),
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
            && translation.rust.contains("return Some(left) == right;")
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
            .contains("Some(sample_name.as_ref().unwrap().trim()) == sample_name")
            && !translation
                .rust
                .contains("Some(sample_name.as_ref().unwrap().trim()) == sample_name.as_deref()")
            && !translation
                .rust
                .contains(".as_ref().map(|value| value.as_str())")
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
        assert!(fixture.same(&left, &same));
        assert!(!fixture.same(&Value::new(7), &Value::new(8)));
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
fn object_input_stream_read_methods_have_typed_facade_returns() {
    let translation = translate_source(
        r#"
import java.io.ObjectInputStream;

class ObjectInputStreamReadShapes {
    int readCount(ObjectInputStream in) {
        return in.readInt();
    }

}
"#,
    );

    assert!(
        translation
            .rust
            .contains("pub fn read_int(&self) -> Result<i32, JavaException>")
            && translation.rust.contains("return Ok(r#in.read_int()?);")
            && !translation.rust.contains("downcast::<Object>")
            && !translation.rust.contains("pub fn read_int(&self) -> ()")
            && !translation
                .rust
                .contains("pub fn read_object(&self) -> Result<Box<dyn std::any::Any>"),
        "{}",
        translation.rust
    );
}

#[test]
fn optional_output_stream_write_byte_array_uses_slice_overload() {
    let translation = translate_source(
        r#"
import java.io.OutputStream;

class OptionalOutputStreamWriteBytes {
    void writeDecoded(byte[] decoded) throws java.io.IOException {
        OutputStream out = null;
        out.write(decoded);
    }
}
"#,
    );

    assert!(
        (translation
            .rust
            .contains("out.as_ref().expect(\"Java null receiver\").write_1(decoded)?")
            || translation
                .rust
                .contains("out.as_ref().unwrap().write_1(decoded.as_ref().expect(\"Java null receiver\").as_slice())?"))
            && !translation.rust.contains(".write(decoded.as_ref()"),
        "{}",
        translation.rust
    );
}

#[test]
fn object_stream_object_methods_have_typed_handoff_facades() {
    let translation = translate_source(
        r#"
import java.io.ObjectInputStream;
import java.io.ObjectOutputStream;

class ObjectStreamObjectShapes {
    Object readValue(ObjectInputStream in) throws Exception {
        return in.readObject();
    }

    void writeValue(ObjectOutputStream out, Object value) throws Exception {
        out.writeInt(1);
        out.writeObject(value);
    }

    Object[] readValues(ObjectInputStream in, int len) throws Exception {
        Object[] values = new Object[len];
        for (int i = 0; i < len; i++) {
            values[i] = in.readObject();
        }
        return values;
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("pub fn read_object(&self) -> Result<Box<dyn std::any::Any>, JavaException>")
            && translation
                .rust
                .contains("pub fn write_int<T0>(&self, _arg0: T0) -> Result<(), JavaException>")
            && translation.rust.contains(
                "pub fn write_object<T0>(&self, _arg0: Option<T0>) -> Result<(), JavaException>"
            )
            && translation.rust.contains("return Ok(r#in.read_object()?);")
            && translation.rust.contains("out.write_int(1)?;")
            && translation.rust.contains("out.write_object(value)?;")
            && translation
                .rust
                .contains("values[i as usize] = r#in.read_object()?;")
            && !translation.rust.contains("pub fn read_object(&self) -> ()")
            && !translation
                .rust
                .contains("pub fn write_object<T0>(&self, _arg0: T0) -> ()")
            && !translation.rust.contains("downcast::<Object>()"),
        "{}",
        translation.rust
    );
}

#[test]
fn object_input_stream_read_object_recovers_string_target_slot() {
    let translation = translate_source(
        r#"
import java.io.ObjectInputStream;

class ObjectInputStreamStringRecovery {
    String readKey(ObjectInputStream in) throws Exception {
        String key = in.readObject();
        return key;
    }
}
"#,
    );

    assert!(
        translation.rust.contains("downcast::<String>()")
            && translation
                .rust
                .contains("Java deserialization type mismatch")
            && !translation.rust.contains("expect(\"invalid Java cast\")")
            && translation.rust.contains("let key: String =")
            && !translation
                .rust
                .contains("let key: String = r#in.read_object()?;"),
        "{}",
        translation.rust
    );
}

#[test]
fn object_input_stream_read_object_cast_recovers_string_target_slot() {
    let translation = translate_source(
        r#"
import java.io.ObjectInputStream;

class ObjectInputStreamStringCastRecovery {
    String readKey(ObjectInputStream in) throws Exception {
        String key = (String) in.readObject();
        return key;
    }
}
"#,
    );

    assert!(
        translation.rust.contains("downcast::<String>()")
            && translation
                .rust
                .contains("Java deserialization type mismatch")
            && !translation.rust.contains("expect(\"invalid Java cast\")")
            && translation.rust.contains("let key: String =")
            && !translation
                .rust
                .contains("let key: String = r#in.read_object()?;"),
        "{}",
        translation.rust
    );
}

#[test]
fn object_input_stream_read_object_cast_recovers_throwable_field_slot() {
    let translation = translate_source(
        r#"
import java.io.ObjectInputStream;

class ObjectInputStreamThrowableCastRecovery {
    private Throwable throwable;

    void read(ObjectInputStream in) throws Exception {
        throwable = (Throwable) in.readObject();
    }
}
"#,
    );

    assert!(
        translation.rust.contains("downcast::<Throwable>()")
            && translation
                .rust
                .contains("Java deserialization type mismatch")
            && !translation.rust.contains("expect(\"invalid Java cast\")")
            && translation.rust.contains("self.throwable =")
            && !translation
                .rust
                .contains("self.throwable = r#in.read_object()?;"),
        "{}",
        translation.rust
    );
}

#[test]
fn object_input_stream_read_object_cast_to_source_interface_is_explicit_handoff() {
    let translation = translate_source(
        r#"
import java.io.ObjectInputStream;

interface Localizable {
    String getSourceString();
}

class ObjectInputStreamInterfaceCastRecovery {
    Localizable readPattern(ObjectInputStream in) throws Exception {
        Localizable pattern = (Localizable) in.readObject();
        return pattern;
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("let _java_deserialized = r#in.read_object()?;")
            && translation
                .rust
                .contains("unsupported Java deserialization cast to Localizable")
            && !translation
                .rust
                .contains("Box::new(r#in.read_object()?) as Box<dyn Localizable>")
            && !translation.rust.contains("Box<Box<dyn std::any::Any>>"),
        "{}",
        translation.rust
    );
}

#[test]
fn object_output_stream_write_object_accepts_source_interface_object_handoff() {
    let translation = translate_source(
        r#"
import java.io.ObjectOutputStream;

interface Localizable {
    String getSourceString();
}

class ObjectOutputStreamInterfaceHandoff {
    void write(ObjectOutputStream out, Localizable pattern) throws Exception {
        out.writeObject(pattern);
    }
}
"#,
    );

    assert!(
        translation.rust.contains("out.write_object(pattern)")
            && !translation.rust.contains("downcast_ref::<Object>")
            && !translation.rust.contains("invalid Java cast")
            && !translation
                .rust
                .contains("unsupported Java dispatch coercion"),
        "{}",
        translation.rust
    );
}

#[test]
fn object_output_stream_write_object_clones_indexed_erased_object_handoff() {
    let translation = translate_source(
        r#"
import java.io.ObjectOutputStream;

class ObjectOutputStreamIndexedObjectHandoff {
    void write(ObjectOutputStream out, Object[] args, int j) throws Exception {
        out.writeObject(args[j]);
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("java_clone_erased_object_ref(&args[j as usize])")
            && !translation
                .rust
                .contains("out.write_object(Some(args[j as usize]))"),
        "{}",
        translation.rust
    );
}

#[test]
fn constructor_stores_object_parameter_as_owned_field() {
    let translation = translate_source(
        r#"
class ConstructorStoresObjectParameterAsOwnedField {
    private Number argument;

    ConstructorStoresObjectParameterAsOwnedField(Number wrong) {
        this.argument = wrong;
    }

    Number getArgument() {
        return argument;
    }
}
"#,
    );

    assert!(
        translation.rust.contains("argument: Number")
            && translation.rust.contains("argument: wrong")
            && !translation.rust.contains("argument: Option<&Number>")
            && !translation.rust.contains("argument: Some(&wrong)"),
        "{}",
        translation.rust
    );
}

#[test]
fn optional_owned_field_getter_returns_borrowed_option_view() {
    let translation = translate_source(
        r#"
class OptionalOwnedFieldGetterReturnsBorrowedOptionView {
    private Number argument = null;

    OptionalOwnedFieldGetterReturnsBorrowedOptionView(Number wrong) {
        this.argument = wrong;
    }

    Number getArgument() {
        return argument;
    }
}
"#,
    );

    assert!(
        translation.rust.contains("argument: Option<Number>")
            && translation
                .rust
                .contains("pub fn get_argument(&self) -> Option<&Number>")
            && translation.rust.contains("return self.argument.as_ref();")
            && !translation.rust.contains("Some(&self.argument.clone()"),
        "{}",
        translation.rust
    );
}

#[test]
fn source_borrowed_getter_initializers_recover_owned_locals() {
    let translation = translate_source(
        r#"
class SourceBorrowedGetterInitializers {
    void use(Archive report) {
        Writer xhtml = report.xhtmlStream();
        Buffer data = report.dataDocument();
        Zip zip = report.zipFile();
        xhtml.write("x");
        data.append("y");
        zip.close();
    }
}

class Archive {
    private Writer xhtml = null;
    private Buffer data = null;
    private Zip zip = new Zip();

    Writer xhtmlStream() {
        return xhtml;
    }

    Buffer dataDocument() {
        return data;
    }

    Zip zipFile() {
        return zip;
    }
}

class Writer {
    void write(String value) {}
}

class Buffer {
    void append(String value) {}
}

class Zip {
    void close() {}
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("let xhtml: Option<&Writer> = report.xhtml_stream()")
            && translation
                .rust
                .contains("let data: Option<&Buffer> = report.data_document()")
            && translation
                .rust
                .contains("let zip: &Zip = report.zip_file()")
            && !translation.rust.contains("Some(report.xhtml_stream())")
            && !translation.rust.contains("Some(report.data_document())")
            && !translation
                .rust
                .contains("let zip: Zip = report.zip_file();"),
        "{}",
        translation.rust
    );
}

#[test]
fn erased_object_array_clone_and_format_use_explicit_helpers() {
    let translation = translate_source(
        r#"
import java.text.MessageFormat;
import java.util.ArrayList;
import java.util.List;

class ErasedObjectArrayCloneAndFormat {
    private List<Object[]> rows = new ArrayList<Object[]>();

    void add(Object[] values) {
        rows.add(values);
    }

    Object[] first() {
        Object[] values = rows.get(0);
        return values;
    }

    String format(MessageFormat fmt) {
        Object[] values = rows.get(0);
        return fmt.format(values);
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("java_clone_erased_object_vec_ref(")
            && translation
                .rust
                .contains("pub fn format<T0>(&self, _arg0: T0) -> String")
            && translation.rust.contains("return fmt.format(values);")
            && !translation.rust.contains("values.to_string()")
            && !translation
                .rust
                .contains(".clone()/* TODO(translation): validate added clone */"),
        "{}",
        translation.rust
    );
}

#[test]
fn serializable_instanceof_erased_object_uses_runtime_marker_helper() {
    let translation = translate_source(&idiom_gap("SerializableInstanceofObject.java"));

    assert!(
        translation.rust.contains("java_is_serializable(&value")
            && translation.rust.contains("java_is_serializable(&values[")
            && !translation
                .rust
                .contains("unsupported Java instanceof shape"),
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
                .contains("if !(skip_blank_lines && self.blank(line.as_ref()))")
            && translation.rust.contains("break;")
            && translation.rust.contains("return line;")
            && !translation.rust.contains("self.blank(line.clone()")
            && !translation
                .rust
                .contains("let line: String;\n        return line;"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "do_while_assigned_return",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
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
            .contains("JavaException::new(\"RuntimeException\", e.to_string())")
            || translation.rust.contains(
                "JavaException::new(\"RuntimeException\", java_erased_object_to_string(&e).to_string())"
            ),
        "{}",
        translation.rust
    );
}

#[test]
fn exception_constructor_erased_object_varargs_use_java_display_helper() {
    let translation = translate_source(
        r#"
class ErasedObjectVarargsExceptionMessage {
    static void check(Object value, Object pattern, Object... args) throws RuntimeException {
        if (value == null) {
            throw new RuntimeException(pattern, args);
        }
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("java_erased_object_to_string(&pattern)")
            && translation
                .rust
                .contains("java_erased_object_to_string(&args)")
            && !translation
                .rust
                .contains("format!(\"{}: {}\", pattern, args)"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "exception_constructor_erased_object_varargs",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
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
                .contains("Self::write(StringBuilder::new_1(16).into(), value)?")
            && !translation
                .rust
                .contains("Self::write(&mut StringBuilder::new_1(16).into(), value)?"),
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
                .contains("return SAMRecord::NULL_SEQUENCE.to_vec();")
            && translation
                .rust
                .contains("return SAMRecord::NULL_QUALS.to_vec();"),
        "{}",
        translation.rust
    );
}

#[test]
fn htsjdk_static_integer_constants_emit_typed_facade_surfaces() {
    let translation = translate_source(
        r#"
import htsjdk.samtools.util.IOUtil;
import htsjdk.variant.bcf2.BCF2Codec;

class HtsjdkStaticIntegerConstants {
    int combined() {
        return BCF2Codec.SIZEOF_BCF_HEADER + IOUtil.GZIP_HEADER_READ_LENGTH;
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
            .contains("pub const SIZEOF_BCF_HEADER: i32 = 5;")
            && translation
                .rust
                .contains("pub const GZIP_HEADER_READ_LENGTH: i32 = 8000;")
            && translation
                .rust
                .contains("return BCF2Codec::SIZEOF_BCF_HEADER + IOUtil::GZIP_HEADER_READ_LENGTH;")
            && !translation.rust.contains("downcast::<SIZEOF_BCF_HEADER>")
            && !translation
                .rust
                .contains("downcast::<GZIP_HEADER_READ_LENGTH>"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "htsjdk_static_integer_constants",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn sam_tag_enum_variant_constants_emit_typed_facade_surfaces() {
    let translation = translate_source(
        r#"
import htsjdk.samtools.SAMRecord;
import htsjdk.samtools.SAMTag;

class SamTagEnumVariantConstants {
    String comment(SAMRecord record) {
        return record.getStringAttribute(SAMTag.CO);
    }
}
"#,
    );

    assert!(
        translation.rust.contains("impl HtsjdkSamtoolsSAMTag")
            && translation
                .rust
                .contains("pub const CO: HtsjdkSamtoolsSAMTag = HtsjdkSamtoolsSAMTag;")
            && translation
                .rust
                .contains("record.get_string_attribute(SAMTag::CO)")
            && !translation.rust.contains("associated item `CO`"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "sam_tag_enum_variant_constants",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
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
fn commons_math_localized_formats_constants_are_pack_enum_fields() {
    let translation = translate_source(
        r#"
import org.apache.commons.math3.exception.util.LocalizedFormats;

class CommonsMathLocalizedFormatUse {
    String message() {
        return LocalizedFormats.OVERFLOW.toString();
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("LocalizedFormats::OVERFLOW.to_string()"),
        "{}",
        translation.rust
    );
    assert!(
        translation
            .rust
            .contains("pub const OVERFLOW: LocalizedFormats = LocalizedFormats;"),
        "{}",
        translation.rust
    );
    cargo_check_generated_modules(
        "commons_math_localized_formats_pack_fields",
        vec![GeneratedCrateModule {
            name: "commons_math_localized_format_use",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn commons_math_localized_formats_constants_borrow_as_localizable_pack_trait() {
    let translation = translate_source(
        r#"
import org.apache.commons.math3.exception.util.Localizable;
import org.apache.commons.math3.exception.util.LocalizedFormats;

class CommonsMathLocalizedFormatPatternUse {
    long checked(long a, long b) throws Exception {
        return add(a, b, LocalizedFormats.OVERFLOW_IN_ADDITION);
    }

    long add(long a, long b, Localizable pattern) throws Exception {
        return a + b;
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("&LocalizedFormats::OVERFLOW_IN_ADDITION as &dyn Localizable"),
        "{}",
        translation.rust
    );
    assert!(
        translation.rust.contains("pub trait Localizable")
            && translation
                .rust
                .contains("impl Localizable for LocalizedFormats"),
        "{}",
        translation.rust
    );
    assert!(
        !translation.rust.contains("impl Localizable {\n"),
        "{}",
        translation.rust
    );
    cargo_check_generated_modules(
        "commons_math_localized_formats_pack_trait_slot",
        vec![GeneratedCrateModule {
            name: "commons_math_localized_format_pattern_use",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn commons_math_localizable_pack_trait_can_be_formatted() {
    let translation = translate_source(
        r#"
import org.apache.commons.math3.exception.util.Localizable;
import org.apache.commons.math3.exception.util.LocalizedFormats;

class CommonsMathLocalizablePatternDisplay {
    String message() {
        return format(LocalizedFormats.OVERFLOW);
    }

    String format(Localizable pattern) {
        return "pattern=" + pattern;
    }
}
"#,
    );

    assert!(
        translation.rust.contains("pub trait Localizable")
            && translation.rust.contains("std::fmt::Display"),
        "{}",
        translation.rust
    );
    assert!(
        translation
            .rust
            .contains("&LocalizedFormats::OVERFLOW as &dyn Localizable"),
        "{}",
        translation.rust
    );
    cargo_check_generated_modules(
        "commons_math_localizable_pack_trait_display",
        vec![GeneratedCrateModule {
            name: "commons_math_localizable_pattern_display",
            rust: translation.rust,
            tests: "",
        }],
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
fn source_nullable_getter_local_initializer_stays_optional() {
    let translation = translate_source(
        r#"
class SourceNullableGetterLocalInitializer {
    String write(FastqRecord record) {
        String readName = record.getReadName();
        return readName == null ? "" : readName;
    }
}

class FastqRecord {
    private final String readName;

    FastqRecord(String readName) {
        this.readName = readName;
    }

    String getReadName() {
        return readName;
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("let read_name: Option<String> = record.get_read_name();")
            && !translation
                .rust
                .contains("let read_name: Option<String> = Some(record.get_read_name())"),
        "{}",
        translation.rust
    );
}

#[test]
fn guarded_nullable_copy_constructor_reads_source_fields_after_unwrap() {
    let translation = translate_source(
        r#"
class GuardedNullableCopyConstructor {
    private final String name;
    private final String bases;

    GuardedNullableCopyConstructor(GuardedNullableCopyConstructor other) {
        if (other == null) {
            throw new IllegalArgumentException("copy null");
        }
        this.name = other.name;
        this.bases = other.bases;
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("other.as_ref().expect(\"Java null receiver\").name")
            && translation
                .rust
                .contains("other.as_ref().expect(\"Java null receiver\").bases.clone()")
            && !translation
                .rust
                .contains("Some(other.as_ref().expect(\"Java null receiver\").name.clone()")
            && !translation
                .rust
                .contains("Some(other.as_ref().expect(\"Java null receiver\").bases.clone()")
            && !translation.rust.contains("name: other.name")
            && !translation.rust.contains("bases: other.bases"),
        "{}",
        translation.rust
    );
}

#[test]
fn guarded_nullable_copy_constructor_preserves_optional_source_fields() {
    let translation = translate_source(
        r#"
class NullableCopyConstructorPreservesOptions {
    private final String readName;
    private final String readString;

    NullableCopyConstructorPreservesOptions(String readName, String readString) {
        if (readName != null && !readName.isEmpty()) {
            this.readName = readName;
        } else {
            this.readName = null;
        }
        this.readString = readString;
    }

    NullableCopyConstructorPreservesOptions(NullableCopyConstructorPreservesOptions other) {
        if (other == null) {
            throw new IllegalArgumentException("copy null");
        }
        this.readName = other.readName;
        this.readString = other.readString;
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("read_name: other.as_ref().expect(\"Java null receiver\").read_name.as_ref().cloned()")
            && translation.rust.contains(
                "read_string: other.as_ref().expect(\"Java null receiver\").read_string.as_ref().cloned()"
            )
            && !translation.rust.contains(
                "read_name: Some(other.as_ref().expect(\"Java null receiver\").read_name.clone()"
            )
            && !translation.rust.contains(
                "read_string: Some(other.as_ref().expect(\"Java null receiver\").read_string.clone()"
            ),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "nullable_copy_constructor_preserves_options",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
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
    let rust_path = std::env::temp_dir().join("new_java2rust_simple_rethrow_catch_test.rs");
    let bin_path = std::env::temp_dir().join("new_java2rust_simple_rethrow_catch_test");
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
    let rust_path = std::env::temp_dir().join("new_java2rust_simple_cause_chaining_test.rs");
    let bin_path = std::env::temp_dir().join("new_java2rust_simple_cause_chaining_test");
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
    let rust_path =
        std::env::temp_dir().join("new_java2rust_internal_throwing_call_in_try_test.rs");
    let bin_path = std::env::temp_dir().join("new_java2rust_internal_throwing_call_in_try_test");
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
        assert_eq!(fixture.copy(&resource), Ok(()));
    }
}
"#
    );
    let rust_path = std::env::temp_dir().join("new_java2rust_simple_try_finally_cleanup_test.rs");
    let bin_path = std::env::temp_dir().join("new_java2rust_simple_try_finally_cleanup_test");
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
    let rust_path = std::env::temp_dir().join("new_java2rust_simple_try_catch_return_test.rs");
    let bin_path = std::env::temp_dir().join("new_java2rust_simple_try_catch_return_test");
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
fn source_static_final_primitive_expression_fields_emit_associated_constants() {
    let translation = translate_source(
        r#"
public class UsesExpressionConstants {
    static final int MAX = 750;
    static final int LEN = MAX * 2;
    private static final long MASK = -1L - (0x40000000L - 1L);
    private static final double HALF = 1d / 2d;

    public static double value(double x) {
        return x * HALF + LEN + MASK;
    }
}
"#,
    );

    assert!(
        translation.rust.contains("const MAX: i32 = 750;")
            && translation.rust.contains("const LEN: i32 = Self::MAX * 2;")
            && translation
                .rust
                .contains("const MASK: i64 = -1i64 - (1073741824i64 - 1i64);")
            && translation.rust.contains("const HALF: f64 = 1f64 / 2f64;")
            && translation
                .rust
                .contains("return ((x * Self::HALF) + (Self::LEN as f64)) + (Self::MASK as f64);")
            && !translation.rust.contains("HALF: f64,"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "source_static_final_primitive_expression_fields",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn source_static_final_string_concat_constants_use_fold_or_storage() {
    let target = r###"
public class StringConcatConstantUser {
    public static final String SAME_OWNER_KEY = "INFO";
    public static final String SAME_OWNER_START = "##" + SAME_OWNER_KEY;
    public static final String CROSS_OWNER_START = PrefixOwner.PREFIX + SAME_OWNER_KEY;
    public static final int CROSS_OWNER_OFFSET = CROSS_OWNER_START.length() + 1;

    public static boolean check(String line) {
        return line.startsWith(SAME_OWNER_START) || line.startsWith(CROSS_OWNER_START) || line.startsWith(QualifiedRuntime.START);
    }

    public static int offset() {
        return CROSS_OWNER_OFFSET;
    }
}

class PrefixOwner {
    public static final String PREFIX = "##";
}

class QualifiedRuntime {
    public static final String START = PrefixOwner.PREFIX + "QUAL";
}
"###;

    let translation = translate_source(target);

    let has_cross_owner_storage = translation
        .rust
        .contains("static STRING_CONCAT_CONSTANT_USER_CROSS_OWNER_START")
        || translation
            .rust
            .contains("static STRING_CONCAT_CONSTANT_USER_C_R_O_S_S_O_W_N_E_R_S_T_A_R_T");
    let has_cross_owner_getter = translation
        .rust
        .contains("pub fn __static_cross_owner_start() -> String")
        || translation
            .rust
            .contains("pub fn __static_c_r_o_s_s_o_w_n_e_r_s_t_a_r_t() -> String");
    let has_cross_owner_offset_storage = translation
        .rust
        .contains("static STRING_CONCAT_CONSTANT_USER_CROSS_OWNER_OFFSET")
        || translation
            .rust
            .contains("static STRING_CONCAT_CONSTANT_USER_C_R_O_S_S_O_W_N_E_R_O_F_F_S_E_T");
    assert!(
        translation
            .rust
            .contains("pub const SAME_OWNER_START: &'static str = \"##INFO\";")
            && has_cross_owner_storage
            && has_cross_owner_getter
            && has_cross_owner_offset_storage
            && translation
                .rust
                .contains("format!(\"{}{}\", PrefixOwner::PREFIX.to_string(), Self::SAME_OWNER_KEY.to_string())")
            && translation
                .rust
                .contains("line.starts_with(Self::SAME_OWNER_START)")
            && translation
                .rust
                .contains("line.starts_with(Self::__static_c_r_o_s_s_o_w_n_e_r_s_t_a_r_t().as_str())")
            && translation
                .rust
                .contains("line.starts_with(QualifiedRuntime::__static_s_t_a_r_t().as_str())")
            && translation
                .rust
                .contains("return Self::__static_c_r_o_s_s_o_w_n_e_r_o_f_f_s_e_t();")
            && translation.rust.contains(
                "unwrap_or_else(|| (Self::__static_c_r_o_s_s_o_w_n_e_r_s_t_a_r_t().len() as i32) + 1)"
            )
            && !translation.rust.contains("unwrap_or_else(|| 0)")
            && !translation.rust.contains("PREFIX + Self::SAME_OWNER_KEY")
            && !translation.rust.contains("&str + &str"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "source_static_final_string_concat_constants",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn source_static_final_mixed_case_suffix_field_access_uses_emitted_const_name() {
    let target = r###"
public class MixedCaseConstantUser {
    public static boolean check(String value) {
        return value.equals(MixedCaseConstants.MISSING_VALUE_v4)
            || value.equals(MixedCaseConstants.PASSES_FILTERS_v4);
    }
}

class MixedCaseConstants {
    public static final String MISSING_VALUE_v4 = ".";
    public static final String PASSES_FILTERS_v4 = "PASS";
}
"###;

    let translation = translate_source(target);

    assert!(
        translation
            .rust
            .contains("pub const m_i_s_s_i_n_g_v_a_l_u_e_v4: &'static str = \".\";")
            && translation
                .rust
                .contains("pub const p_a_s_s_e_s_f_i_l_t_e_r_s_v4: &'static str = \"PASS\";")
            && translation
                .rust
                .contains("MixedCaseConstants::m_i_s_s_i_n_g_v_a_l_u_e_v4")
            && translation
                .rust
                .contains("MixedCaseConstants::p_a_s_s_e_s_f_i_l_t_e_r_s_v4")
            && !translation
                .rust
                .contains("MixedCaseConstants::MISSING_VALUE_v4")
            && !translation
                .rust
                .contains("MixedCaseConstants::PASSES_FILTERS_v4"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "source_static_final_mixed_case_suffix_field_access",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn source_static_final_byte_constants_preserve_declared_slot_type() {
    let translation = translate_source(
        r#"
public class ByteConstantOwner {
    public static final byte STOP = 0;
    public static final byte LEFT = 1;

    public static byte[] fill(byte value) {
        byte[] values = new byte[2];
        values[0] = STOP;
        switch (value) {
            case LEFT:
                values[1] = LEFT;
                break;
            default:
                values[1] = STOP;
        }
        return values;
    }
}
"#,
    );

    assert!(
        translation.rust.contains("pub const STOP: i8 = 0;")
            && translation.rust.contains("pub const LEFT: i8 = 1;")
            && translation
                .rust
                .contains("values[0 as usize] = Self::STOP;")
            && translation
                .rust
                .contains("__java_switch_value == Self::LEFT")
            && !translation.rust.contains("pub const STOP: i32")
            && !translation.rust.contains("pub const LEFT: i32"),
        "{}",
        translation.rust
    );
}

#[test]
fn empty_constructor_uses_typed_float_defaults_for_primitive_fields() {
    let translation = translate_source(
        r#"
public class FloatDefaultFields {
    private float open;
    private float extend;
    private double score;
    private int count;

    public FloatDefaultFields() {
    }
}
"#,
    );

    assert!(
        translation.rust.contains("open: 0f32")
            && translation.rust.contains("extend: 0f32")
            && translation.rust.contains("score: 0f64")
            && translation.rust.contains("count: 0")
            && !translation.rust.contains("open: 0,")
            && !translation.rust.contains("score: 0,"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "empty_constructor_typed_float_defaults",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn source_static_final_constants_can_reference_other_source_constants_and_hex_sign_bits() {
    let translation = translate_source(
        r#"
class SourceConstantOwner {
    public static final double PI = 105414357.0 / 33554432.0 + 1.984187159361080883e-9;
}

public class SourceConstantUser {
    public static final double TWO_PI = 2 * SourceConstantOwner.PI;
    private static final int SIGN_MASK = 0x80000000;

    public static double normalize(double value) {
        return value + TWO_PI + SIGN_MASK;
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("pub const TWO_PI: f64 = 2f64 * SourceConstantOwner::PI;")
            && translation
                .rust
                .contains("const SIGN_MASK: i32 = -2147483648;")
            && translation
                .rust
                .contains("return (value + Self::TWO_PI) + (Self::SIGN_MASK as f64);")
            && !translation.rust.contains("TWO_PI: f64,")
            && !translation.rust.contains("SIGN_MASK: i32,"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "source_static_final_cross_owner_constants",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn source_static_final_primitive_array_fields_emit_associated_slice_constants() {
    let translation = translate_source(
        r#"
public class UsesArrayConstants {
    private static final long RECIP[] = new long[] { 1L, 2L };
    private static final double WEIGHTS[] = { 0, 0.5, 1d / 3d };
    private static final double COEF[][] = { { 1, 2 }, { 3, 4 } };

    public static double value(int i) {
        return RECIP[i] + WEIGHTS[i] + COEF[i][0];
    }

    private static double first(double values[]) {
        return values[0];
    }

    public static double firstWeight() {
        return first(WEIGHTS);
    }

    public static int rows() {
        return COEF.length;
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("const RECIP: &'static [i64] = &[1i64, 2i64];")
            && translation
                .rust
                .contains("const WEIGHTS: &'static [f64] = &[0f64, 0.5f64, 1f64 / 3f64];")
            && translation
                .rust
                .contains("const COEF: &'static [&'static [f64]] = &[&[1f64, 2f64], &[3f64, 4f64]];")
            && translation
                .rust
                .contains("return ((Self::RECIP[i as usize] as f64) + Self::WEIGHTS[i as usize]) + Self::COEF[i as usize][0 as usize];")
            && translation.rust.contains("Self::first(Self::WEIGHTS)")
            && translation.rust.contains("return Self::COEF.len() as i32;")
            && !translation.rust.contains("Self::WEIGHTS.as_slice()")
            && !translation.rust.contains("Self::COEF.nrows()")
            && !translation.rust.contains("RECIP: Vec<i64>")
            && !translation.rust.contains("WEIGHTS: Vec<f64>")
            && !translation.rust.contains("COEF: Vec<Vec<f64>>"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "source_static_final_primitive_array_fields",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn source_static_final_array_length_can_initialize_static_final_constant() {
    let translation = translate_source(
        r#"
public class ArrayLengthConstantOwner {
    private static final char[] CHARS = { 'A', 'C', 'G', 'T' };
    private static final int NUMBER_OF_CHARS = CHARS.length;

    public static int count() {
        return NUMBER_OF_CHARS;
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("const CHARS: &'static [char] = &['A', 'C', 'G', 'T'];")
            && translation.rust.contains("const NUMBER_OF_CHARS: i32 = 4;")
            && translation.rust.contains("return Self::NUMBER_OF_CHARS;")
            && !translation.rust.contains("CHARS::length"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "source_static_final_array_length_constant",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn source_static_final_facade_fields_emit_associated_defaults_not_storage() {
    let translation = translate_source(
        r#"
import java.util.regex.Matcher;
import java.util.regex.Pattern;

public class UsesStaticFinalPattern {
    private static final Pattern LINE_PATTERN = Pattern.compile("<line/>");

    public static boolean matchesLine(String line) {
        Matcher m = LINE_PATTERN.matcher(line.trim());
        return m.matches();
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("const LINE_PATTERN: Pattern = Pattern::compile_static(\"<line/>\");")
            && translation
                .rust
                .contains("let m: Matcher = Self::LINE_PATTERN.matcher(line.trim());")
            && !translation.rust.contains("LINE_PATTERN: Pattern,"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "source_static_final_pattern",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
#[cfg(test)]
mod generated_tests {
    use super::*;

    #[test]
    fn static_pattern_matches_runtime_input() {
        assert!(UsesStaticFinalPattern::matches_line("  <line/>  "));
        assert!(!UsesStaticFinalPattern::matches_line("<rect/>"));
    }
}
"#,
        }],
    );
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
            .contains("pub const DEFAULT_QUEUE_SIZE: i32 = { compile_error!(\"external static constant ChildWriter.DEFAULT_QUEUE_SIZE has no modeled value\"); 0 };")
            && translation
                .rust
                .contains("return ChildWriter::DEFAULT_QUEUE_SIZE;"),
        "{}",
        translation.rust
    );
}

#[test]
fn jdk_locale_us_static_constant_emits_typed_surface() {
    let translation = translate_source(
        r#"
import java.util.Locale;

class UsesLocaleUs {
    Locale locale() {
        return Locale.US;
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("impl Locale {\n    pub const US: Locale = Locale;")
            && translation.rust.contains("return Locale::US;"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "jdk_locale_us_static_constant",
        vec![GeneratedCrateModule {
            name: "uses_locale_us",
            rust: translation.rust,
            tests: "",
        }],
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

    void writeParsed(float[][] scores, int row, int col, String value) {
        scores[row][col] = Float.parseFloat(value);
    }

    float[][] createAndWriteParsed(int rows, int cols, int row, int col, String value) {
        float[][] scores = new float[rows][cols];
        scores[row][col] = Float.parseFloat(value);
        return scores;
    }

    float[][] createAndWriteParsedToken(int rows, int cols, int row, int col, java.util.StringTokenizer tokenizer) {
        float[][] scores = new float[rows][cols];
        scores[row][col] = Float.parseFloat(tokenizer.nextToken());
        return scores;
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
        translation
            .rust
            .contains("scores[[row as usize, col as usize]] = value.parse::<f32>().unwrap();")
            && !translation.rust.contains(
                "scores[[row as usize, col as usize]] = &value.as_str().parse::<f32>().unwrap();"
            ),
        "{}",
        translation.rust
    );
    assert!(
        translation
            .rust
            .contains("let mut scores: ndarray::Array2<f32> = ndarray::Array2::from_elem((rows as usize, cols as usize), 0.0);")
            && !translation.rust.contains("scores[[row as usize, col as usize]] = &value.as_str().parse::<f32>().unwrap();"),
        "{}",
        translation.rust
    );
    assert!(
        translation
            .rust
            .contains("scores[[row as usize, col as usize]] = tokenizer.next_token().parse::<f32>().unwrap();")
            && !translation.rust.contains("scores[[row as usize, col as usize]] = &tokenizer.next_token().parse::<f32>().unwrap();"),
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
fn resolved_source_constructor_name_beats_external_simple_name_arity_fact() {
    let translation = translate_source(
        r#"
class Matrix {
    private String id;
    private float[] scores;

    Matrix(String id, float[] scores) {
        this.id = id;
        this.scores = scores;
    }
}

class SourceMatrixFactory {
    static Matrix make(String id, int rows) {
        float[] scores = new float[rows];
        return new Matrix(id, scores);
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("pub fn new(id: String, scores: Vec<f32>) -> Matrix")
            && translation.rust.contains("return Matrix::new(")
            && !translation.rust.contains("Matrix::new_2("),
        "{}",
        translation.rust
    );
    cargo_check_generated_modules(
        "resolved_source_constructor_name_beats_external_simple_name_arity_fact",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn same_class_static_calls_borrow_ndarray_arguments_from_signature_slots() {
    let translation = translate_source(
        r#"
class SameClassStaticNdarrayBorrowCall {
    static Cell run(Matrix matrix) {
        float[][] scores = matrix.getScores();
        byte[][] pointers = new byte[2][2];
        return construct(scores, pointers);
    }

    static Cell construct(float[][] matrix, byte[][] pointers) {
        float value = matrix[0][0];
        pointers[0][0] = (byte) value;
        return new Cell();
    }
}

class Matrix {
    float[][] getScores() {
        return new float[2][2];
    }
}

class Cell {
}
"#,
    );

    assert!(
        translation.rust.contains(
            "pub fn construct(matrix: &ndarray::Array2<f32>, pointers: &mut ndarray::Array2<i8>)"
        ),
        "{}",
        translation.rust
    );
    assert!(
        translation
            .rust
            .contains("Self::construct(&scores, &mut pointers)"),
        "{}",
        translation.rust
    );
}

#[test]
fn static_final_two_dimensional_numeric_tables_can_feed_readonly_slice_helpers() {
    let translation = translate_source(
        r#"
class StaticFinalMatrixSliceUse {
    static final double[][] COEF = new double[][] {{1.0, 2.0}, {3.0, 4.0}};

    static double sum(double[] row) {
        if (row != null) {
            return row[0] + row[1];
        }
        return 0.0;
    }

    static int rows(double[][] values) {
        return values.length;
    }

    static double at(double[][] values, int row, int col) {
        return values[row][col];
    }

    static double ownedRow() {
        double row[] = COEF[1];
        return row[0];
    }

    static double[][] load() {
        return COEF.clone();
    }

    static double run() {
        return sum(COEF[0]) + rows(COEF) + at(COEF, 1, 1) + ownedRow();
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("const COEF: &'static [&'static [f64]]"),
        "{}",
        translation.rust
    );
    assert!(
        translation.rust.contains("pub fn sum(row: Option<&[f64]>)")
            && translation
                .rust
                .contains("pub fn rows(values: &ndarray::Array2<f64>) -> i32")
            && translation
                .rust
                .contains("pub fn at(values: &ndarray::Array2<f64>, row: i32, col: i32) -> f64"),
        "{}",
        translation.rust
    );
    assert!(
        translation
            .rust
            .contains("Self::sum(Some(Self::COEF[0 as usize]))")
            && translation
                .rust
                .contains("Self::rows(&ndarray::Array2::from_rows(Self::COEF))")
            && translation
                .rust
                .contains("Self::at(&ndarray::Array2::from_rows(Self::COEF), 1, 1)"),
        "{}",
        translation.rust
    );
    assert!(
        translation.rust.contains("return values.nrows() as i32;")
            && translation
                .rust
                .contains("return values[[row as usize, col as usize]];")
            && translation
                .rust
                .contains("let row: Vec<f64> = (Self::COEF[1 as usize]).to_vec()")
            && translation
                .rust
                .contains("pub fn load() -> ndarray::Array2<f64>")
            && translation
                .rust
                .contains("return ndarray::Array2::from_rows(Self::COEF);")
            && !translation.rust.contains("Vec<Vec<f64>>"),
        "{}",
        translation.rust
    );
}

#[test]
fn parameter_array_dimensions_after_name_lower_as_slice_slots() {
    let translation = translate_source(
        r#"
class ParameterArrayDimensionSlots {
    static void split(final double d, final double result[]) {
        result[0] = d;
        result[1] = d - result[0];
    }

    static void add(final double a[], final double b[], final double ans[]) {
        ans[0] = a[0] + b[0];
        ans[1] = a[1] + b[1];
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("pub fn split(d: f64, result: &mut [f64])"),
        "{}",
        translation.rust
    );
    assert!(
        translation
            .rust
            .contains("pub fn add(a: &[f64], b: &[f64], ans: &mut [f64])"),
        "{}",
        translation.rust
    );
    assert!(
        translation.rust.contains("result[0 as usize] = d;")
            && translation
                .rust
                .contains("ans[0 as usize] = a[0 as usize] + b[0 as usize];")
            && !translation.rust.contains("result: f64")
            && !translation.rust.contains("a: f64")
            && !translation.rust.contains("ans: f64"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "parameter_array_dimension_slots",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn null_guarded_array_parameter_writes_use_mut_optional_receiver_once() {
    let translation = translate_source(
        r#"
class NullGuardedArrayParameterWrite {
    static void fill(final double result[]) {
        if (result != null) {
            result[0] = 1.0;
            result[1] = result[0] + 1.0;
        }
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("result.as_mut().expect(\"Java null receiver\")[0 as usize] = 1f64;")
            && translation.rust.contains(
                "result.as_mut().expect(\"Java null receiver\")[1 as usize] = result.as_ref().expect(\"Java null receiver\")[0 as usize] + 1f64;"
            )
            && !translation.rust.contains(".unwrap().as_mut().expect")
            && !translation.rust.contains(".unwrap().as_ref().expect"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "null_guarded_array_parameter_write",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn mutable_optional_array_parameter_named_rust_keyword_uses_raw_binding() {
    let translation = translate_source(
        r#"
class KeywordArrayParameterWrite {
    static void splitReciprocal(final double in[]) {
        if (in != null) {
            if (in[0] == 0.0) {
                in[0] = in[1];
                in[1] = 0.0;
            }
        }
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("pub fn split_reciprocal(mut r#in: Option<&mut [f64]>)"),
        "{}",
        translation.rust
    );
    assert!(
        translation
            .rust
            .contains("r#in.as_mut().expect(\"Java null receiver\")[0 as usize]")
            && !translation.rust.contains("mut in:"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "keyword_array_parameter_write",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn same_source_static_owner_receiver_keeps_type_path_after_name_normalization() {
    let translation = translate_source(
        r#"
class SameSourceStaticOwnerUse {
    static void run() {
        SameSourceStaticHelper.touch();
    }

    static void dump(int count, double value) {
        SameSourceStaticHelper.print(count);
        SameSourceStaticHelper.print(value);
    }
}

class SameSourceStaticHelper {
    static void touch() {
    }

    static void print(int value) {
    }

    static void print(double value) {
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("SameSourceStaticHelper::touch();")
            && translation
                .rust
                .contains("SameSourceStaticHelper::print_int(count);")
            && translation
                .rust
                .contains("SameSourceStaticHelper::print_double(value);")
            && !translation.rust.contains("same_source_static_helper.touch"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "same_source_static_owner_use",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn nested_source_static_call_uses_declaring_outer_owner() {
    let translation = translate_source(
        r#"
class NestedStaticOuterOwner {
    static final int FLAG = 3;

    static int mask(int value) {
        return value | FLAG;
    }

    static class Worker {
        int run(int value) {
            return mask(value) + FLAG;
        }
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("NestedStaticOuterOwner::mask(value)")
            && translation.rust.contains("NestedStaticOuterOwner::FLAG")
            && !translation.rust.contains("Self::mask(value)")
            && !translation
                .rust
                .contains("return Self::mask(value) + Self::FLAG;"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "nested_source_static_outer_owner",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn static_initializer_block_reports_unsupported_member_diagnostic() {
    let translation = translate_source(
        r#"
class StaticInitializerUnsupportedDiagnostic {
    static int value = 0;

    static {
        value = 3;
    }

    static int read() {
        return value;
    }
}
"#,
    );

    assert!(translation.diagnostics.has_code("unsupported_member"));
    assert!(
        translation
            .diagnostics
            .iter()
            .any(|message| message.message.contains("static_initializer")),
        "{:?}",
        translation.diagnostics
    );
}

#[test]
fn switch_expression_lowers_to_match_expression() {
    let translation = translate_source(
        r#"
class SwitchExpressionDiagnostic {
    int run(int value) {
        int mapped = switch (value) {
            case 1 -> 10;
            default -> 0;
        };
        return mapped;
    }
}
"#,
    );

    assert!(
        translation.rust.contains("let mapped: i32 = match value {")
            && translation.rust.contains("1 => 10,")
            && translation.rust.contains("_ => 0,")
            && !translation.rust.contains("compile_error!")
            && !translation.rust.contains("switch (value)"),
        "{}",
        translation.rust
    );
    cargo_check_generated_modules(
        "switch_expression_lowers_to_match_expression",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn yield_switch_expression_lowers_to_match_expression() {
    let translation = translate_source(
        r#"
class YieldSwitchExpressionDiagnostic {
    int run(int value) {
        int mapped = switch (value) {
            case 1: yield 10;
            default: yield 0;
        };
        return mapped;
    }
}
"#,
    );

    assert!(
        translation.rust.contains("let mapped: i32 = match value {")
            && translation.rust.contains("1 => 10,")
            && translation.rust.contains("_ => 0,")
            && !translation.rust.contains("compile_error!")
            && !translation.rust.contains("switch (value)"),
        "{}",
        translation.rust
    );
    cargo_check_generated_modules(
        "yield_switch_expression_lowers_to_match_expression",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn nested_static_table_field_type_guides_static_overload_selection() {
    let translation = translate_source(
        r#"
class NestedStaticTableOverloadUse {
    static void run() {
        StaticTablePrinter.print(Table.VALUES);
    }

    static class Table {
        static final double[] VALUES;
        static {
            VALUES = new double[2];
        }
    }
}

class StaticTablePrinter {
    static void print(double[] values) {
    }

    static void print(int[] values) {
    }
}
"#,
    );

    assert!(
        translation.rust.contains(
            "StaticTablePrinter::print_double_array(NestedStaticTableOverloadUseTable::VALUES);"
        ) && !translation
            .rust
            .contains("StaticTablePrinter::print(Table::VALUES);"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "nested_static_table_overload_use",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn internal_mutable_array_argument_marks_local_vec_binding_mutable() {
    let translation = translate_source(
        r#"
class InternalMutableArrayArgumentLowering {
    static void fill(double out[]) {
        out[0] = 1.0;
    }

    static int run() {
        double[] values = new double[2];
        fill(values);
        return values.length;
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("let mut values: Vec<f64> = vec![0.0; 2 as usize];")
            && translation
                .rust
                .contains("Self::fill(values.as_mut_slice());")
            && translation.rust.contains("return values.len() as i32;"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "internal_mutable_array_argument_lowering",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn local_array_argument_to_nullable_out_parameter_stays_owned_vec() {
    let translation = translate_source(
        r#"
class LocalArrayNullableOutParameter {
    static void fill(double out[]) {
        if (out != null) {
            out[0] = 1.0;
        }
    }

    static double run() {
        double[] values = new double[2];
        fill(values);
        return values[0];
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("let mut values: Vec<f64> = vec![0.0; 2 as usize];")
            && translation
                .rust
                .contains("Self::fill(Some(values.as_mut_slice()));")
            && translation.rust.contains("return values[0 as usize];")
            && !translation.rust.contains("Option<&mut Vec<f64>>"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "local_array_nullable_out_parameter",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn nullable_mutable_array_helper_return_count_keeps_caller_storage_owned() {
    let translation = translate_source(
        r#"
class NullableMutableArrayHelperReturnCount {
    static int fill(byte source[], byte destination[], int offset) {
        if (destination != null) {
            destination[offset] = source[0];
            destination[offset + 1] = source[1];
        }
        return 2;
    }

    static byte[] run(byte source[]) {
        byte[] outBuff = new byte[4];
        int pos = 0;
        pos += fill(source, outBuff, pos);
        byte[] out = new byte[pos];
        System.arraycopy(outBuff, 0, out, 0, pos);
        return out;
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("pub fn fill(source: &[i8], mut destination: Option<&mut [i8]>, offset: i32) -> i32")
            && translation
                .rust
                .contains("let mut out_buff: Option<Vec<i8>> = Some(vec![0; 4 as usize]);")
            && translation
                .rust
                .contains("Self::fill(source, out_buff.as_deref_mut(), pos)")
            && translation.rust.contains(
                "System::arraycopy(out_buff.as_ref().expect(\"Java null receiver\").as_slice(), 0 as usize, out.as_mut_slice(), 0 as usize, pos as usize);"
            )
            && !translation.rust.contains("destination: Option<Vec<i8>>")
            && !translation.rust.contains("Self::fill(source, out_buff,"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "nullable_mutable_array_helper_return_count",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn nullable_borrowed_array_param_passes_to_slice_helper_without_extra_as_slice() {
    let translation = translate_source(
        r#"
class NullableBorrowedArrayParamSliceHelper {
    static int read(byte source[], int offset) {
        if (source == null) {
            return 0;
        }
        return consume(source, offset);
    }

    static int consume(final byte source[], int offset) {
        return source[offset];
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("pub fn read(source: Option<&[i8]>, offset: i32) -> i32")
            && translation.rust.contains("Self::consume(source, offset)")
            && !translation
                .rust
                .contains("source.expect(\"Java null receiver\").as_slice()"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "nullable_borrowed_array_param_slice_helper",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn mutable_array_parameter_direct_return_can_remain_borrowed() {
    let translation = translate_source(
        r#"
class MutableArrayParameterReturn {
    static byte[] fill(byte out[]) {
        out[0] = 1;
        return out;
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("pub fn fill<'a>(out: &'a mut [i8]) -> &'a mut [i8]")
            && translation.rust.contains("return out;")
            && !translation.rust.contains("return out.clone()"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "mutable_array_parameter_return",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn nullable_read_only_array_parameters_lower_as_optional_borrowed_slices() {
    let translation = translate_source(
        r#"
class NullableReadOnlyArrayParameter {
    static void copy(double input[], double output[]) {
        if (input != null) {
            output[0] = input[0];
        }
    }

    static void combine(final double left[], final double right[], final double out[]) {
        out[0] = left[0] + right[0];
        out[1] = left[1] + right[1];
    }

    static double run() {
        double[] values = new double[2];
        double[] out = new double[2];
        copy(values, out);
        copy(values, out);
        final double xs[] = new double[2];
        final double facts[] = new double[2];
        final double as[] = new double[2];
        combine(xs, values, as);
        combine(xs, facts, as);
        return out[0];
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("pub fn copy(input: Option<&[f64]>, output: &mut [f64])")
            && translation
                .rust
                .contains("pub fn combine(left: &[f64], right: Option<&[f64]>, out: &mut [f64])")
            && translation
                .rust
                .contains("Self::copy(Some(values.as_slice()), out.as_mut_slice());")
            && translation.rust.contains(
                "Self::combine(xs.as_slice(), Some(values.as_slice()), r#as.as_mut_slice());"
            )
            && translation.rust.contains(
                "Self::combine(xs.as_slice(), Some(facts.as_slice()), r#as.as_mut_slice());"
            )
            && !translation.rust.contains("input: Option<Vec<f64>>")
            && !translation.rust.contains("left: Option<Vec<f64>>")
            && !translation.rust.contains("Self::copy(values,"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "nullable_read_only_array_parameter",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn caller_mutable_array_binding_does_not_force_readonly_callee_params_mutable() {
    let translation = translate_source(
        r#"
class CallerMutableArrayBindingReadonlyCallee {
    static void fill(double out[]) {
        out[0] = 1.0;
    }

    static void sum(double left[], double right[], double out[]) {
        out[0] = left[0] + right[0];
    }

    static double run() {
        double[] values = new double[2];
        double[] out = new double[2];
        fill(values);
        sum(values, values, out);
        return out[0];
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("pub fn sum(left: &[f64], right: &[f64], out: &mut [f64])")
            && translation
                .rust
                .contains("Self::sum(values.as_slice(), values.as_slice(), out.as_mut_slice());")
            && !translation.rust.contains("pub fn sum(left: &mut")
            && !translation
                .rust
                .contains("Self::sum(&mut values, &mut values"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "caller_mutable_array_binding_readonly_callee",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn same_call_readonly_array_argument_snapshots_before_mutable_output_alias() {
    let translation = translate_source(
        r#"
class SameCallReadonlyArrayMutableOutputAlias {
    static void add(double left[], double right[], double out[]) {
        out[0] = left[0] + right[0];
    }

    static double run() {
        double[] values = new double[2];
        double[] temp = new double[2];
        add(values, temp, values);
        return values[0];
    }
}
"#,
    );

    assert!(
        translation.rust.contains(
            "Self::add(values.clone()/* TODO(translation): validate same-call alias snapshot */.as_slice(), temp.as_slice(), values.as_mut_slice());"
        )
            && translation
                .rust
                .contains("pub fn add(left: &[f64], right: &[f64], out: &mut [f64])")
            && !translation
                .rust
                .contains("Self::add(values.as_slice(), temp.as_slice(), values.as_mut_slice());"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "same_call_readonly_array_mutable_output_alias",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn nullable_out_parameter_reborrows_for_recursive_call_before_reuse() {
    let translation = translate_source(
        r#"
class NullableOutParameterRecursiveCall {
    static double fill(double x, double out[]) {
        if (x < 0.0) {
            double result = fill(-x, out);
            if (out != null) {
                out[0] = result;
            }
            return result;
        }
        if (out != null) {
            out[0] = x;
        }
        return x;
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("Self::fill(-x, out.as_deref_mut())")
            && translation
                .rust
                .contains("out.as_mut().expect(\"Java null receiver\")[0 as usize] = result;"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "nullable_out_parameter_recursive_call",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn optional_mutable_array_reborrows_as_readonly_slice_without_moving() {
    let translation = translate_source(
        r#"
class OptionalMutableArrayReadonlyReborrow {
    static void copy(double input[], double output[]) {
        output[0] = input[0];
    }

    static void run(double values[]) {
        double[] out = new double[2];
        copy(values, out);
        if (values != null) {
            values[0] = out[0];
        }
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("pub fn copy(input: Option<&[f64]>, output: &mut [f64])")
            && translation
                .rust
                .contains("pub fn run(mut values: Option<&mut [f64]>)")
            && translation
                .rust
                .contains("Self::copy(values.as_deref(), out.as_mut_slice());")
            && translation.rust.contains(
                "values.as_mut().expect(\"Java null receiver\")[0 as usize] = out[0 as usize];"
            )
            && !translation
                .rust
                .contains("values.map(|value| value.as_slice())"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "optional_mutable_array_readonly_reborrow",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn borrowed_primitive_enhanced_for_element_copies_into_value_argument_slot() {
    let translation = translate_source(
        r#"
class BorrowedPrimitiveEnhancedForElement {
    static String format(double d) {
        return Double.toString(d);
    }

    static void print(double[] values) {
        for (double d : values) {
            format(d);
        }
    }
}
"#,
    );

    assert!(
        translation.rust.contains("for d in values.iter().copied()")
            && translation.rust.contains("Self::format(d);")
            && !translation.rust.contains("Self::format(*d);")
            && !translation.rust.contains("for d in values.iter() {"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "borrowed_primitive_enhanced_for_element",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn optional_array_receivers_unwrap_for_read_indexing_and_length() {
    let translation = translate_source(
        r#"
class OptionalArrayReceiverReads {
    String[] labels;
    double[][] matrix;

    double localFirst() {
        double[] xs = new double[2];
        return xs[0] + xs.length;
    }

    int labelCount() {
        return labels.length;
    }

    String labelAt(int index) {
        return labels[index];
    }

    double matrixAt(int row, int col) {
        return matrix[row][col];
    }

    int matrixRows() {
        return matrix.length;
    }
}
"#,
    );

    assert!(
        translation.rust.contains("xs[0 as usize]"),
        "{}",
        translation.rust
    );
    assert!(
        translation.rust.contains("xs.len() as i32"),
        "{}",
        translation.rust
    );
    assert!(
        translation
            .rust
            .contains("self.labels.as_ref().expect(\"Java null receiver\").len() as i32"),
        "{}",
        translation.rust
    );
    assert!(
        translation
            .rust
            .contains("self.labels.as_ref().expect(\"Java null receiver\")[index as usize]"),
        "{}",
        translation.rust
    );
    assert!(
        translation.rust.contains(
            "self.matrix.as_ref().expect(\"Java null receiver\")[[row as usize, col as usize]]"
        ),
        "{}",
        translation.rust
    );
    assert!(
        translation
            .rust
            .contains("self.matrix.as_ref().expect(\"Java null receiver\").nrows() as i32"),
        "{}",
        translation.rust
    );
    assert!(
        !translation.rust.contains("self.labels[index as usize]"),
        "{}",
        translation.rust
    );
    assert!(
        !translation
            .rust
            .contains("self.matrix[[row as usize, col as usize]]"),
        "{}",
        translation.rust
    );
}

#[test]
fn optional_array_field_length_unwraps_on_non_self_receiver() {
    let translation = translate_source(
        r#"
class OptionalArrayFieldLengthOtherReceiver {
    double[] values;

    int otherLength(OptionalArrayFieldLengthOtherReceiver other) {
        return other.values.length;
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("return other.values.as_ref().expect(\"Java null receiver\").len() as i32;")
            && !translation.rust.contains("other.values.len()"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "optional_array_field_length_other_receiver",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn static_helper_mutating_parameter_fields_uses_mutable_borrow_slot() {
    let translation = translate_source(
        r#"
class StaticHelperMutatesParameterFields {
    int count;
    double[] values;

    static void copy(StaticHelperMutatesParameterFields source,
                     StaticHelperMutatesParameterFields dest) {
        dest.count = source.count;
        dest.values = new double[source.values.length];
    }
}
"#,
    );

    assert!(
        translation.rust.contains(
            "pub fn copy(source: &StaticHelperMutatesParameterFields, dest: &mut StaticHelperMutatesParameterFields)"
        ) && translation.rust.contains("dest.count = source.count;")
            && translation.rust.contains(
                "source.values.as_ref().expect(\"Java null receiver\").len() as i32"
            ),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "static_helper_mutates_parameter_fields",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn null_checked_static_helper_mutating_parameter_fields_keeps_mutable_borrow_slot() {
    let translation = translate_source(
        r#"
class NullCheckedStaticHelperMutatesParameterFields {
    int count;
    double[] values;

    static Object checkNotNull(Object value) {
        if (value == null) {
            throw new RuntimeException();
        }
        return value;
    }

    static void copy(NullCheckedStaticHelperMutatesParameterFields source,
                     NullCheckedStaticHelperMutatesParameterFields dest) {
        checkNotNull(source);
        checkNotNull(dest);
        dest.count = source.count;
        dest.values = new double[source.values.length];
    }
}
"#,
    );

    assert!(
        translation.rust.contains(
            "pub fn copy(source: &NullCheckedStaticHelperMutatesParameterFields, dest: &mut NullCheckedStaticHelperMutatesParameterFields)"
        ) && !translation.rust.contains("Self::check_not_null(source")
            && !translation.rust.contains("Self::check_not_null(dest")
            && translation.rust.contains("dest.count = source.count;"),
        "{}",
        translation.rust
    );
}

#[test]
fn cross_source_null_checked_static_helper_keeps_concrete_mutable_borrow_slot() {
    let translation = translate_source(
        r#"
class CrossSourceNullCheckHelper {
    static Object checkNotNull(Object value) {
        if (value == null) {
            throw new RuntimeException();
        }
        return value;
    }
}

class CrossSourceNullCheckedCopy {
    int count;
    double[] values;

    public static void copy(CrossSourceNullCheckedCopy source, CrossSourceNullCheckedCopy dest) {
        CrossSourceNullCheckHelper.checkNotNull(source);
        CrossSourceNullCheckHelper.checkNotNull(dest);
        dest.count = source.count;
        dest.values = new double[source.values.length];
    }
}
"#,
    );

    assert!(
        translation.rust.contains(
            "pub fn copy(source: CrossSourceNullCheckedCopy, dest: &mut CrossSourceNullCheckedCopy)"
        ) && !translation
            .rust
            .contains("CrossSourceNullCheckHelper::check_not_null(source")
            && !translation
                .rust
                .contains("CrossSourceNullCheckHelper::check_not_null(dest")
            && translation.rust.contains("dest.count = source.count;"),
        "{}",
        translation.rust
    );
}

#[test]
fn local_passed_to_resolved_static_mutable_source_param_is_declared_mutable() {
    let translation = translate_source(
        r#"
class LocalPassedToStaticMutableSourceParam {
    int count;
    double[] values;

    public static void copy(LocalPassedToStaticMutableSourceParam source,
                            LocalPassedToStaticMutableSourceParam dest) {
        dest.count = source.count;
        dest.values = new double[source.values.length];
    }

    public LocalPassedToStaticMutableSourceParam copy() {
        LocalPassedToStaticMutableSourceParam result = new LocalPassedToStaticMutableSourceParam();
        LocalPassedToStaticMutableSourceParam.copy(this, result);
        return result;
    }
}
"#,
    );

    assert!(
        translation.rust.contains(
            "pub fn copy_local_passed_to_static_mutable_source_param_local_passed_to_static_mutable_source_param(source: LocalPassedToStaticMutableSourceParam, dest: &mut LocalPassedToStaticMutableSourceParam)"
        ) && translation
            .rust
            .contains("let mut result: LocalPassedToStaticMutableSourceParam")
            && translation.rust.contains("&mut result")
            && !translation
                .rust
                .contains("let result: LocalPassedToStaticMutableSourceParam"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "local_passed_to_static_mutable_source_param",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn two_dimensional_numeric_array_literal_with_comment_lowers_to_ndarray_arr2() {
    let translation = translate_source(
        r#"
class NdarrayMatrixLiteralWithComment {
    double[][] literal() {
        return new double[][] {
            // W C M H
            {1.0, 2.0},
            {3.0, 4.0}
        };
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("return ndarray::arr2(&[[1f64, 2f64], [3f64, 4f64]]);"),
        "{}",
        translation.rust
    );
    assert!(
        !translation.rust.contains("vec![//"),
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

    void fill(int[][] values) {
        values[0] = new int[]{2, 3, 6, 4};
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
    assert!(
        translation.rust.contains(
            "values.row_mut(0 as usize).assign(&ndarray::ArrayView1::from(&vec![2, 3, 6, 4]));"
        ) && !translation
            .rust
            .contains("ArrayView1::from(vec![2, 3, 6, 4])"),
        "{}",
        translation.rust
    );
}

#[test]
fn ragged_object_two_dimensional_arrays_stay_nested_vecs() {
    let translation = translate_source(
        r#"
import java.io.File;
import java.util.Hashtable;
import java.util.Vector;

class RaggedObjectGroups {
    static File[][] groups(File[] files) {
        Hashtable<String, Vector<File>> fileBases = new Hashtable<String, Vector<File>>();
        String baseName = files[0].getName();
        fileBases.put(baseName, new Vector<File>());
        fileBases.get(baseName).add(files[0]);
        String[] baseNames = fileBases.keySet().toArray(new String[0]);
        File[][] fileGroups = new File[baseNames.length][];
        for (int i = 0; i < baseNames.length; i++) {
            fileGroups[i] = fileBases.get(baseNames[i]).toArray(new File[0]);
        }
        return fileGroups;
    }
}
"#,
    );

    assert!(
        translation.rust.contains("-> Vec<Vec<File>>")
            && translation
                .rust
                .contains("let mut file_groups: Vec<Vec<File>>")
            && translation
                .rust
                .contains("vec![Vec::new(); base_names.len() as i32 as usize]")
            && translation
                .rust
                .contains("file_bases.get_mut(&base_name).expect(\"Java null receiver\").push(")
            && !translation.rust.contains("ndarray::Array2<File>")
            && !translation.rust.contains("row_mut"),
        "{}",
        translation.rust
    );
    cargo_check_generated_modules(
        "ragged_object_groups",
        vec![GeneratedCrateModule {
            name: "ragged_object_groups",
            rust: translation.rust.clone(),
            tests: "",
        }],
    );
}

#[test]
fn explicit_pushback_input_stream_cast_receiver_keeps_facade_target() {
    let translation = translate_source(
        r#"
import java.io.IOException;
import java.io.InputStream;
import java.io.PushbackInputStream;
import java.util.zip.GZIPInputStream;

class CastedPushbackInputStreamReceiver extends GZIPInputStream {
    CastedPushbackInputStreamReceiver(InputStream input) throws IOException {
        super(input);
    }

    void push(byte[] buffer) throws IOException {
        ((PushbackInputStream)this.in).unread(buffer, 0, 1);
    }
}
"#,
    );

    assert!(
        translation.rust.contains(
            "{ let _java_facade_cast_source = &(self.r#in); PushbackInputStream::default() }"
        ) && translation.rust.contains(".unread(")
            && !translation.rust.contains("self.r#in.unread("),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "explicit_pushback_input_stream_cast_receiver",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn nested_static_table_classes_get_simple_aliases_and_default_array_consts() {
    let translation = translate_source(
        r#"
class NestedStaticTableOwner {
    private static final int LEN = 2;

    static double first(int index) {
        return ExpTable.TABLE_A[index] + lowerMant.TABLE[index][0];
    }

    private static class ExpTable {
        private static final double[] TABLE_A;
        static {
            TABLE_A = new double[NestedStaticTableOwner.LEN];
        }
    }

    private static class lowerMant {
        private static final double[][] TABLE;
        static {
            TABLE = new double[NestedStaticTableOwner.LEN][];
        }
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("pub type ExpTable = NestedStaticTableOwnerExpTable;"),
        "{}",
        translation.rust
    );
    assert!(
        translation
            .rust
            .contains("pub type LowerMant = NestedStaticTableOwnerLowerMant;"),
        "{}",
        translation.rust
    );
    assert!(
        translation
            .rust
            .contains("const TABLE_A: &'static [f64] = &[];"),
        "{}",
        translation.rust
    );
    assert!(
        translation
            .rust
            .contains("const TABLE: &'static [&'static [f64]] = &[];"),
        "{}",
        translation.rust
    );
    assert!(
        translation
            .rust
            .contains("ExpTable::TABLE_A[index as usize]")
            && translation
                .rust
                .contains("LowerMant::TABLE[index as usize][0 as usize]"),
        "{}",
        translation.rust
    );
    cargo_check_generated_modules(
        "nested_static_table_aliases",
        vec![GeneratedCrateModule {
            name: "nested_static_table_owner",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn trait_impl_forwarding_recovers_owned_array_from_slice_trait_slot() {
    let translation = translate_source(
        r#"
interface SliceArraySink {
    void accept(double[] values);
}

class OwningArraySink implements SliceArraySink {
    private double[] stored;

    public void accept(double[] values) {
        stored = values;
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("fn accept(&mut self, values: &[f64])"),
        "{}",
        translation.rust
    );
    assert!(
        translation.rust.contains(
            "fn accept(&mut self, values: &[f64]) { OwningArraySink::accept(self, values.to_vec()); }"
        ),
        "{}",
        translation.rust
    );
    assert!(
        !translation
            .rust
            .contains("OwningArraySink::accept(self, values);"),
        "{}",
        translation.rust
    );
    cargo_check_generated_modules(
        "trait_impl_array_forwarding",
        vec![GeneratedCrateModule {
            name: "trait_impl_array_forwarding",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn synchronized_statement_preserves_block_body_after_guard_returns() {
    let translation = translate_source(
        r#"
class SynchronizedBodyAfterGuard {
    private int value;

    boolean matches(Object object) {
        if (object == this) {
            return true;
        }
        if (object == null) {
            return false;
        }
        synchronized (this) {
            return value == 3;
        }
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("if object.is_none() {\n            return false;\n        }"),
        "{}",
        translation.rust
    );
    assert!(
        translation.rust.contains("return self.value == 3;"),
        "{}",
        translation.rust
    );
    assert!(
        !translation.rust.contains("expected `bool`, found `()`"),
        "{}",
        translation.rust
    );
    cargo_check_generated_modules(
        "synchronized_body_after_guard",
        vec![GeneratedCrateModule {
            name: "synchronized_body_after_guard",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn source_nested_enum_type_access_uses_flattened_enum_helpers() {
    let translation = translate_source(
        r#"
public class NestedEnumApiUser {
    public enum LocalMode {
        PRIMARY,
        SECONDARY
    }

    public static int count() {
        return NestedEnumApiUser.LocalMode.values().length;
    }

    public static boolean matches(String name) {
        return NestedEnumApiUser.LocalMode.valueOf(name) == NestedEnumApiUser.LocalMode.SECONDARY;
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("NestedEnumApiUserLocalMode::values().len()")
            && translation
                .rust
                .contains("NestedEnumApiUserLocalMode::value_of(name.as_str())")
            && translation
                .rust
                .contains("NestedEnumApiUserLocalMode::SECONDARY")
            && !translation.rust.contains("NestedEnumApiUser::LocalMode"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "source_nested_enum_type_access",
        vec![GeneratedCrateModule {
            name: "source_nested_enum_type_access",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn source_enum_values_spread_into_arrays_as_list_and_compare_borrowed_loop_items() {
    let translation = translate_source(
        r#"
import java.util.Arrays;
import java.util.LinkedHashSet;
import java.util.Set;

public class EnumValuesCollectionUser {
    public enum Mode {
        ALPHA,
        BETA
    }

    public static Set<Mode> allModes() {
        return new LinkedHashSet<Mode>(Arrays.asList(Mode.values()));
    }

    public static boolean valid(String[] names) {
        int index = 0;
        for (Mode mode : Mode.values()) {
            if (mode != EnumValuesCollectionUser.Mode.valueOf(names[index])) {
                return false;
            }
            index++;
        }
        return true;
    }
}
"#,
    );

    assert!(
        translation.rust.contains(
            "EnumValuesCollectionUserMode::values().into_iter().collect::<std::collections::HashSet<_>>()"
        )
            && translation
                .rust
                .contains("mode != EnumValuesCollectionUserMode::value_of(")
            && !translation
                .rust
                .contains("vec![EnumValuesCollectionUserMode::values()]"),
        "{}",
        translation.rust
    );

    cargo_check_generated_modules(
        "source_enum_values_collection_user",
        vec![GeneratedCrateModule {
            name: "source_enum_values_collection_user",
            rust: translation.rust,
            tests: "",
        }],
    );
}
