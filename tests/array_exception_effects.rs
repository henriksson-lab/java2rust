use new_java2rust::translate_source;

#[path = "common/mod.rs"]
mod common;

#[test]
fn simple_and_compound_array_writes_keep_java_rhs_exception_order() {
    let translation = translate_source(
        r#"
class Use {
    static int counter = 0;
    static int next() { counter++; return 9; }
    static int run() {
        int[] values = {1};
        int stage = 0;
        try { values[3] = next(); }
        catch (IndexOutOfBoundsException error) { stage = counter; }
        try { values[3] += next(); }
        catch (IndexOutOfBoundsException error) { return stage * 10 + counter; }
        return -1;
    }
}
"#,
    );
    common::cargo_test_generated_modules(
        "array_bounds_assignment_order",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "#[test] fn simple_rhs_runs_but_compound_rhs_is_not_reached() { assert_eq!(Use::run(), 11); }",
        }],
    );
}

#[test]
fn nonnull_array_bounds_failures_propagate_through_arithmetic_calls_and_writes() {
    let translation = translate_source(
        r#"
class Use {
    static int read(int[] values, int index) { return values[index] + 2; }
    static void write(int[] values, int index) { values[index] = 9; }
    static int run() {
        int[] values = {5};
        int stage = read(values, 0);
        try { read(values, -1); }
        catch (ArrayIndexOutOfBoundsException error) { stage += 10; }
        try { read(values, 1); }
        catch (IndexOutOfBoundsException error) { stage += 20; }
        try { write(values, 2); }
        catch (ArrayIndexOutOfBoundsException error) { stage += 30; }
        return stage + values[0];
    }
}
"#,
    );
    common::cargo_test_generated_modules(
        "nonnull_array_bounds_effects",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "#[test] fn read_and_write_errors_are_java_exceptions() { assert_eq!(Use::run().unwrap(), 72); }",
        }],
    );
}

#[test]
fn locally_caught_array_bounds_do_not_escape_the_method() {
    let translation = translate_source(
        r#"
class Use {
    static int run() {
        int[] values = {1};
        try { return values[-1]; }
        catch (IndexOutOfBoundsException error) { return 9; }
    }
}
"#,
    );
    common::cargo_test_generated_modules(
        "locally_handled_array_bounds",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "#[test] fn matching_handler_keeps_a_plain_abi() { assert_eq!(Use::run(), 9); }",
        }],
    );
}
