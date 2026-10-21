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
fn plain_local_and_retained_arrays_copy_in_both_directions() {
    let translation = new_java2rust::translate_source(
        r#"
import java.util.Arrays;
class MixedCopy {
    int[] values = new int[] {1, 2, 3};
    int[] expose() { return values; }
    static final int WIDTH = 3;
    int check() {
        int[] alias = expose();
        int[] first = Arrays.copyOf(new int[] {4, 5, 6}, WIDTH);
        int[] second = Arrays.copyOf(new int[] {7, 8, 9}, WIDTH);
        int[] tail = Arrays.copyOf(new int[] {10}, 1);
        int[] combined = new int[WIDTH * 2 + tail.length];
        // This escape makes only the output retained. Inputs remain plain.
        values = combined;
        System.arraycopy(first, (int) (0L % 2L), combined, 0, WIDTH);
        System.arraycopy(second, 0, combined, WIDTH, WIDTH);
        System.arraycopy(tail, 0, combined, WIDTH * 2, tail.length);
        if (values[0] != 4 || values[3] != 7 || values[6] != 10) return 1;
        int[] output = new int[combined.length];
        System.arraycopy(combined, 0, output, 0, output.length);
        if (output[0] != 4 || output[6] != 10 || alias[0] != 1) return 2;
        try { System.arraycopy(first, 1 / 0, combined, 0, 1); return 3; }
        catch (ArithmeticException error) {}
        return 0;
    }
    static int run() { return new MixedCopy().check(); }
}
"#,
    );
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    assert!(
        !translation
            .rust
            .contains("native allocation conversion requires")
    );
    common::cargo_verify_generated_modules_with_edition(
        "mixed_arraycopy",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn copies() { assert_eq!(MixedCopy::run().value(), 0); }
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
