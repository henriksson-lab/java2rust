use new_java2rust::translate_source;

#[path = "common/mod.rs"]
mod common;
use common::{GeneratedCrateModule, cargo_test_generated_modules};

fn run_generated(source: &str, tests: &'static str) {
    let translation = translate_source(source);
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    cargo_test_generated_modules(
        "formatted_output",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests,
        }],
    );
}

#[test]
fn system_printf_is_a_stream_operation_not_an_out_type() {
    let translation = translate_source(
        r#"
class Use {
    static void run() {
        System.out.printf("value=%s%n", "text");
        System.err.printf("%d %s%n", 3, "three");
        System.out.printf("literal%n");
    }
}
"#,
    );
    assert!(
        !translation.rust.contains("Out::printf"),
        "System.out is a stream value"
    );
    assert!(
        !translation.rust.contains("Err::printf"),
        "System.err is a stream value"
    );
    cargo_test_generated_modules(
        "system_formatted_output",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "#[test] fn system_calls_compile_and_run() { Use::run(); }",
        }],
    );
}

#[test]
fn printf_accepts_variable_arguments_and_returns_its_stream() {
    run_generated(
        r#"
import java.io.ByteArrayOutputStream;
import java.io.PrintStream;
class Use {
    static String run() {
        ByteArrayOutputStream bytes = new ByteArrayOutputStream();
        PrintStream stream = new PrintStream(bytes);
        stream.printf("start:%s", "x").println("!");
        stream.printf("%d %s%n", 3, "three");
        stream.printf("literal%n");
        stream.printf("%s %s %s %s %s %s%n", "a", "b", "c", "d", "e", "f");
        String template = "%s=%d%n";
        stream.format(template, "dynamic", 5);
        return bytes.toString();
    }
}
"#,
        r#"#[test] fn output_and_stream_identity_match_java() {
    assert_eq!(Use::run(), "start:x!\n3 three\nliteral\na b c d e f\ndynamic=5\n");
}"#,
    );
}

#[test]
fn java_format_literal_braces_do_not_consume_conversions() {
    run_generated(
        r#"
class Use {
    static String run() { return String.format("R {%s}: {%d}, literal {}", "node", 3); }
    static String nested() { return String.format("{{%s}}", "x"); }
    static String escaped() { return String.format("100%% %s%n", "ok"); }
}
"#,
        r#"#[test] fn literal_text_and_conversions_are_independent() {
    assert_eq!(Use::run(), "R {node}: {3}, literal {}");
    assert_eq!(Use::nested(), "{{x}}");
    assert_eq!(Use::escaped(), "100% ok\n");
}"#,
    );
}

#[test]
fn source_print_stream_definition_keeps_its_printf_body() {
    run_generated(
        r#"
class PrintStream {
    int value;
    PrintStream printf(String format, int number) { value = number; return this; }
}
class Use {
    static PrintStream identity(PrintStream value) { return value; }
    static int run() {
        PrintStream stream = new PrintStream();
        PrintStream same = identity(stream.printf("ignored", 3));
        same.printf("again", 4);
        return stream.value;
    }
}
"#,
        "#[test] fn source_method_wins_over_platform_model() { assert_eq!(Use::run(), 4); }",
    );
}
