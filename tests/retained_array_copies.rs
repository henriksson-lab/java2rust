#[path = "common/mod.rs"]
mod common;

#[test]
fn primitive_array_copies_are_fresh_and_constructor_calls_keep_the_emitted_abi() {
    let translation =
        new_java2rust::translate_source(include_str!("fixtures/retained_array_copies.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    assert!(
        !translation
            .rust
            .contains("native allocation conversion requires"),
        "{}",
        translation.rust
    );
    common::cargo_verify_generated_modules_with_edition(
        "retained_array_copies",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn java_copy_and_constructor_oracles() {
    assert_eq!(RetainedArrayCopies::run().value(), 0);
    assert_eq!(ConstructorArrayRebinding::run().value(), 0);
    assert_eq!(OuterArrayArgument::run().value(), 0);
}
"#,
        }],
        "test",
        "2021",
    );
}

#[test]
fn source_arrays_copy_method_keeps_its_alias_semantics() {
    let translation = new_java2rust::translate_source(
        r#"
class Arrays {
    static byte[] copyOf(byte[] source, int size) { return source; }
}
class SourceArraysCopy {
    byte[] values = new byte[] {1};
    byte[] expose() { return values; }
    int check() {
        byte[] result = Arrays.copyOf(expose(), 1);
        result[0] = 77;
        return values[0];
    }
    static int run() { return new SourceArraysCopy().check(); }
}
"#,
    );
    assert!(
        !translation.rust.contains("__java_array_copy_source"),
        "{}",
        translation.rust
    );
    common::cargo_verify_generated_modules_with_edition(
        "source_arrays_copy",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn source_alias_is_not_a_jdk_copy() { assert_eq!(SourceArraysCopy::run().value(), 77); }
"#,
        }],
        "test",
        "2021",
    );
}
