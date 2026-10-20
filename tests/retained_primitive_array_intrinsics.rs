#[path = "common/mod.rs"]
mod common;

#[test]
fn retained_primitive_arraycopy_preserves_aliases_overlap_and_argument_order() {
    let translation = new_java2rust::translate_source(include_str!(
        "fixtures/retained_primitive_array_intrinsics.java"
    ));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    assert!(translation.rust.contains("__java_arraycopy_values"));
    assert!(
        !translation
            .rust
            .contains("native allocation conversion requires")
    );
    common::cargo_verify_generated_modules_with_edition(
        "retained_primitive_array_intrinsics",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32, E> {
    fn value(self) -> i32 { self.unwrap() }
}

#[test] fn checked_copy() {
    assert_eq!(RetainedPrimitiveArrayIntrinsics::run().value(), 0);
}
"#,
        }],
        "test",
        "2021",
    );
}

#[test]
fn source_system_arraycopy_is_not_replaced_by_the_jdk_intrinsic() {
    let translation = new_java2rust::translate_source(
        r#"
class System {
    static void arraycopy(int[] source, int start, int[] target, int end, int length) {
        target[end] = 77;
    }
}
class SourceSystemArraycopy {
    int[] values = new int[] {1, 2};
    int[] expose() { return values; }
    int run() { System.arraycopy(expose(), 0, expose(), 1, 1); return values[1]; }
    static int verify() { return new SourceSystemArraycopy().run(); }
}
"#,
    );
    assert!(
        !translation.rust.contains("__java_arraycopy_values"),
        "{}",
        translation.rust
    );
    assert!(translation.rust.contains("77"));
    common::cargo_verify_generated_modules_with_edition(
        "source_system_arraycopy",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32, E> {
    fn value(self) -> i32 { self.unwrap() }
}
#[test] fn source_method_dispatch() {
    assert_eq!(SourceSystemArraycopy::verify().value(), 77);
}
"#,
        }],
        "test",
        "2021",
    );
}
