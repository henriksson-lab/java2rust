use new_java2rust::translate_source;

#[path = "common/mod.rs"]
mod common;
use common::{GeneratedCrateModule, cargo_check_generated_modules};

#[test]
fn private_receiver_is_retained_for_arithmetic_and_iteration_field_reads() {
    let translation = translate_source(
        r#"
class ReceiverRead {
    int offset = 3;
    int[] values = new int[] {1, 2};
    private int score() {
        int result = offset + 1;
        for (int value : values) { result += value; }
        return result;
    }
    int get() { return score(); }
}
"#,
    );
    assert!(
        translation.rust.contains("fn score(&self)"),
        "{}",
        method_headers(&translation.rust)
    );
    cargo_check_generated_modules(
        "private_receiver_field_reads",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn private_receiver_independence_respects_parameter_shadowing() {
    let translation = translate_source(
        r#"
class ShadowedReceiver {
    int offset;
    private int score(int offset) { return offset + 1; }
    int get() { return score(3); }
}
"#,
    );
    assert!(
        translation.rust.contains("fn score(offset: i32)"),
        "{}",
        method_headers(&translation.rust)
    );
    cargo_check_generated_modules(
        "private_receiver_shadowing",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

fn method_headers(rust: &str) -> String {
    rust.lines()
        .filter(|line| line.contains("fn score") || line.contains("fn get("))
        .collect::<Vec<_>>()
        .join("\n")
}
