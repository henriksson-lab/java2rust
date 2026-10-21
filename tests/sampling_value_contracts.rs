#[path = "common/mod.rs"]
mod common;

fn verify(name: &str, source: &str, owner: &str) {
    let translation = new_java2rust::translate_source(&format!(
        "{source}\nclass ValueProbe {{ static int run() {{ return {owner}.run(); }} }}"
    ));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_verify_generated_modules_with_edition(
        name,
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn behavior() { assert_eq!(ValueProbe::run().value(), 0); }
"#,
        }],
        "test",
        "2021",
    );
}

#[test]
fn retained_array_field_can_lend_its_existing_handle_to_a_constructor() {
    verify(
        "sampling_retained_array_argument",
        r#"
class Cache {
    private final double[] LOG_CACHE;
    Cache(int size, double[] previous) {
        LOG_CACHE = new double[size];
        if (previous != null) LOG_CACHE[0] = previous[0];
    }
    Cache resized() { return new Cache(4, LOG_CACHE); }
    void set(double value) { LOG_CACHE[0] = value; }
    double get() { return LOG_CACHE[0]; }
    static int run() {
        Cache first = new Cache(2, null);
        first.set(3.5);
        Cache second = first.resized();
        if (second.get() != 3.5) return 1;
        first.set(7.0);
        if (second.get() != 3.5) return 2;
        return 0;
    }
}
"#,
        "Cache",
    );
}

#[test]
fn static_retained_array_initialization_and_writes_share_one_allocation() {
    verify(
        "sampling_static_retained_array",
        r#"
class Table {
    static final double[] VALUES = new double[4];
    static long divisor(int n) {
        if (n < 0) throw new IllegalArgumentException("negative");
        return n + 1;
    }
    static {
        VALUES[0] = 1.25;
        VALUES[1] = VALUES[0] + 2.0;
        double total = 0;
        for (int i = 0; i < 2; i++) total += Math.pow(2, i + 1) / divisor(i);
        VALUES[2] = total;
    }
    static int run() {
        if (VALUES.length != 4 || VALUES[1] != 3.25) return 1;
        double[] alias = VALUES;
        alias[0] = 4.0;
        if (VALUES[0] != 4.0) return 2;
        VALUES[1] = VALUES[0] + VALUES[1];
        if (alias[1] != 7.25) return 3;
        if (VALUES[2] != 4.0) return 4;
        return 0;
    }
}
"#,
        "Table",
    );
}

#[test]
fn explicit_numeric_casts_follow_promoted_calls_even_on_shared_receivers() {
    verify(
        "sampling_numeric_casts",
        r#"
class NumericCalls {
    int calls;
    long take() { calls++; return 4294967297L; }
    int clipped() { return (int) Math.min(take(), Integer.MAX_VALUE); }
    static int accept(int value) { return value; }
    int forward(double value) { return accept((int) Math.min(value, 100.0)); }
    static int run() {
        NumericCalls first = new NumericCalls();
        NumericCalls alias = first;
        if (first.clipped() != Integer.MAX_VALUE || alias.calls != 1) return 1;
        if (alias.forward(3.75) != 3) return 2;
        if (alias.forward(Double.NaN) != 0) return 3;
        if (alias.forward(Double.POSITIVE_INFINITY) != 100) return 4;
        if (alias.forward(Double.NEGATIVE_INFINITY) != Integer.MIN_VALUE) return 5;
        return 0;
    }
}
"#,
        "NumericCalls",
    );
}

#[test]
fn conditional_null_field_assignments_and_initializers_remain_nullable() {
    verify(
        "sampling_conditional_null_fields",
        r#"
class Token { int value = 7; }
class Choices {
    final Token assigned;
    final Token initialized = false ? new Token() : null;
    final Token present = true ? new Token() : new Token();
    Choices(boolean select, boolean inner) {
        assigned = select ? (inner ? new Token() : null) : (Token) null;
    }
    static int run() {
        Choices empty = new Choices(false, true);
        Choices nestedEmpty = new Choices(true, false);
        Choices full = new Choices(true, true);
        if (empty.assigned != null || nestedEmpty.assigned != null) return 1;
        if (full.assigned == null || full.assigned.value != 7) return 2;
        if (full.initialized != null || full.present.value != 7) return 3;
        return 0;
    }
}
"#,
        "Choices",
    );
}
