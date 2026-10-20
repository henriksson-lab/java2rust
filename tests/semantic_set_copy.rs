#[path = "common/mod.rs"]
mod common;

#[test]
fn generic_set_copies_preserve_java_equality_without_rust_hash_bounds() {
    let translation =
        new_java2rust::translate_source(include_str!("fixtures/semantic_set_copy.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "semantic_set_copy",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn java_set_equality_and_copy_contract() { assert_eq!(SemanticSetCopy::run().value(), 0); }
"#,
        }],
    );
}

#[test]
fn source_hash_set_constructor_is_not_native_materialization() {
    let translation = new_java2rust::translate_source(
        r#"
import java.util.Arrays;
import java.util.List;
class HashSet<T> {
    HashSet(List<T> values) {}
    int size() { return 7; }
}
class SourceSetUse {
    static <T> int count(T value) {
        HashSet<T> result = new HashSet<T>(Arrays.asList(value));
        return result.size();
    }
    static int run() { return count("word"); }
}
"#,
    );
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "source_set_copy_precedence",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
#[test] fn source_constructor_and_method_win() { assert_eq!(SourceSetUse::run(), 7); }
"#,
        }],
    );
}
