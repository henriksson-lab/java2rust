#[path = "common/mod.rs"]
mod common;

#[test]
fn checked_identity_preserves_loans_native_aliases_and_evaluation_order() {
    let translation =
        new_java2rust::translate_source(include_str!("fixtures/null_checked_borrows.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "null_checked_borrows",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn checked_reference_contract() { assert_eq!(NullCheckedBorrows::run().value(), 0); }
"#,
        }],
    );
}

#[test]
fn source_checked_identity_lookalike_keeps_its_own_copy_contract() {
    let translation = new_java2rust::translate_source(
        r#"
class Objects {
    static int[] requireNonNull(int[] values, String message) {
        return new int[] { values[0] + 1 };
    }
}
class SourceNullCheckControl {
    static int run() {
        int[] original = new int[] { 3 };
        int[] copy = Objects.requireNonNull(original, "source");
        copy[0] = 7;
        return original[0] * 10 + copy[0];
    }
}
"#,
    );
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "source_null_check_contract",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn not_a_native_identity() { assert_eq!(SourceNullCheckControl::run().value(), 37); }
"#,
        }],
    );
}
