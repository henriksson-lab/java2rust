#[path = "common/mod.rs"]
mod common;

#[test]
fn string_search_expression_offsets_keep_signed_java_int_semantics() {
    common::verify_java_fixture_zero_contract(
        "signed_search_offsets",
        "searchoffsets.StringSearchOffsetProbe",
        "tests/corpus/numbers/StringSearchOffsetProbe.java",
        include_str!("corpus/numbers/StringSearchOffsetProbe.java"),
        r#"
trait OracleExitCode { fn value(self) -> i32; }
impl OracleExitCode for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> OracleExitCode for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn signed_offsets() { assert_eq!(StringSearchOffsetProbe::run().value(), 0); }
"#,
    );
}

#[test]
fn nullable_local_field_failures_propagate_from_methods_and_constructors() {
    verify(
        "nullable_local_field_effects",
        r#"
class Cell { int value = 4; }
class Owner {
    Owner(boolean present) {
        Cell local = present ? new Cell() : null;
        local.value = 8;
    }
}
class Probe {
    static int read(boolean present) {
        Cell local = present ? new Cell() : null;
        return local.value;
    }
    static int write(boolean present) {
        Cell local = present ? new Cell() : null;
        local.value = 7;
        return local.value;
    }
    static int run() {
        if (read(true) != 4 || write(true) != 7) return 1;
        try { read(false); return 2; } catch (NullPointerException expected) { }
        try { write(false); return 3; } catch (NullPointerException expected) { }
        try { new Owner(false); return 4; } catch (NullPointerException expected) { }
        new Owner(true);
        return 0;
    }
}
"#,
    );
}

#[test]
fn value_backed_hierarchy_field_writes_use_the_existing_setter() {
    verify(
        "value_hierarchy_field_setter",
        r#"
class Root { int value; }
class Leaf extends Root { }
class Probe {
    static int write(boolean present) {
        Root receiver = present ? new Leaf() : null;
        receiver.value = 7;
        return receiver.value;
    }
    static int run() {
        if (write(true) != 7) return 1;
        try { write(false); return 2; } catch (NullPointerException expected) { }
        Root receiver = new Leaf();
        receiver.value = 11;
        return receiver.value == 11 ? 0 : 3;
    }
}
"#,
    );
}

#[test]
fn inherited_number_narrow_conversions_delegate_to_source_int_value() {
    common::verify_java_fixture_zero_contract(
        "inherited_number_conversions",
        "InheritedNumberConversionsProbe",
        "tests/corpus/numbers/InheritedNumberConversionsProbe.java",
        include_str!("corpus/numbers/InheritedNumberConversionsProbe.java"),
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn inherited_conversions() { assert_eq!(InheritedNumberConversionsProbe::run().value(), 0); }
"#,
    );
}

#[test]
fn inherited_number_defaults_survive_cross_file_resolution_and_assembly() {
    common::cargo_run_translated_batch_sources(
        "inherited_number_batch",
        &[
            new_java2rust::BatchSource::new(
                "Numeric.java",
                r#"
class Numeric extends java.lang.Number {
    int calls;
    public int intValue() { calls++; return 65535; }
    public long longValue() { return 65535; }
    public float floatValue() { return 65535; }
    public double doubleValue() { return 65535; }
}
"#,
            ),
            new_java2rust::BatchSource::new(
                "Probe.java",
                r#"
class Probe {
    static int run() {
        Numeric value = new Numeric();
        if (value.byteValue() != -1 || value.calls != 1) return 1;
        if (value.shortValue() != -1 || value.calls != 2) return 2;
        return 0;
    }
}
"#,
            ),
        ],
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
fn main() { assert_eq!(translated::Probe::run().value(), 0); }
"#,
    );
}

#[test]
fn assertion_error_primitive_details_use_java_string_conversion() {
    verify(
        "assertion_error_primitive_details",
        r#"
class Probe {
    static int run() {
        try { throw new AssertionError(17); }
        catch (AssertionError error) { if (!"17".equals(error.getMessage())) return 1; }
        try { throw new AssertionError(-9223372036854775808L); }
        catch (AssertionError error) { if (!"-9223372036854775808".equals(error.getMessage())) return 2; }
        try { throw new AssertionError(true); }
        catch (AssertionError error) { if (!"true".equals(error.getMessage())) return 3; }
        try { throw new AssertionError('A'); }
        catch (AssertionError error) { if (!"A".equals(error.getMessage())) return 4; }
        try { throw new AssertionError(-0.0); }
        catch (AssertionError error) { if (!"-0.0".equals(error.getMessage())) return 5; }
        try { throw new AssertionError(1.25f); }
        catch (AssertionError error) { if (!"1.25".equals(error.getMessage())) return 6; }
        return 0;
    }
}
"#,
    );
}

#[test]
fn nullable_shared_copy_field_reads_release_guards_between_arguments() {
    verify(
        "nullable_shared_copy_field_guards",
        r#"
class Pair {
    static final Pair INSTANCE = new Pair();
    double first = 3;
    double second = 2;
    static double combine(double left, double right) { return left + right; }
    static double read(Pair value) { return combine(value.first, value.second); }
    static double negate(Pair value) { return combine(-value.first, -value.second); }
}
class Probe {
    static int run() {
        Pair value = Pair.INSTANCE;
        value.first = 7;
        if (Pair.read(value) != 9 || Pair.negate(value) != -9) return 1;
        value.second = -1;
        if (Pair.read(value) != 6 || Pair.negate(value) != -6) return 2;
        try { Pair.read(null); return 3; } catch (NullPointerException expected) { }
        try { Pair.negate(null); return 4; } catch (NullPointerException expected) { }
        return 0;
    }
}
"#,
    );
}

#[test]
fn nullable_source_interface_upcast_preserves_the_live_concrete_reference() {
    common::verify_java_fixture_zero_contract(
        "nullable_source_interface_alias",
        "Probe",
        "tests/corpus/numbers/NullableInterfaceAliasProbe.java",
        include_str!("corpus/numbers/NullableInterfaceAliasProbe.java"),
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn aliases() { assert_eq!(Probe::run().value(), 0); }
"#,
    );
}

#[test]
fn fresh_arrays_construct_nullable_retained_argument_payloads() {
    verify(
        "fresh_nullable_retained_array_argument",
        r#"
class Probe {
    static int[] retained;
    static int calls;
    static int dimension() { calls++; return 2; }
    static void keep(int[] values) { retained = values; }
    static int run() {
        keep(null);
        if (retained != null) return 1;
        keep(new int[0]);
        if (retained.length != 0) return 2;
        keep(new int[dimension()]);
        if (calls != 1 || retained.length != 2) return 3;
        retained[0] = 7;
        int[] alias = retained;
        alias[1] = 9;
        if (retained[0] != 7 || retained[1] != 9) return 4;
        keep(new int[] {3, 4});
        if (alias[0] != 7 || retained[0] != 3) return 5;
        retained[1] = 8;
        if (retained[1] != 8 || alias[1] != 9) return 6;
        keep(null);
        return retained == null ? 0 : 7;
    }
}
"#,
    );
}

#[test]
fn fallible_logical_results_keep_java_error_type_and_short_circuit_order() {
    verify(
        "fallible_logical_result_abi",
        r#"
class Probe {
    static int calls;
    static boolean check(int value) {
        calls = calls * 10 + value;
        if (value == 9) throw new IllegalStateException("nine");
        return value % 2 != 0;
    }
    static int run() {
        calls = 0;
        if (!(check(1) || check(9) || check(9)) || calls != 1) return 1;
        calls = 0;
        if (check(2) && check(9) && check(9) || calls != 2) return 2;
        calls = 0;
        if (!(check(2) || check(3)) || calls != 23) return 3;
        calls = 0;
        try { boolean ignored = check(2) || check(9) || check(3); return 4; }
        catch (IllegalStateException error) { if (calls != 29) return 5; }
        calls = 0;
        if (!(check(1) && check(3) && check(5)) || calls != 135) return 6;
        calls = 0;
        if (!check(1) || calls != 1) return 7;
        return 0;
    }
}
"#,
    );
}

#[test]
fn nested_numeric_expressions_keep_lowering_stack_bounded() {
    let mut expression = "value".to_string();
    for _ in 0..16 {
        expression = format!("({expression} * 2)");
    }
    let source = format!(
        r#"
class Probe {{
    static int chain(int value) {{ return {expression}; }}
    static int run() {{
        if (chain(1) != 65536) return 1;
        if (chain(32768) != Integer.MIN_VALUE) return 2;
        return 0;
    }}
}}
"#
    );
    verify("nested_numeric_lowering_stack", &source);
}

#[test]
fn throwing_conditional_predicate_does_not_make_plain_branches_a_result() {
    verify(
        "conditional_predicate_result_boundary",
        r#"
class Probe {
    static int calls;
    static boolean check(boolean fail) {
        calls++;
        if (fail) throw new IllegalArgumentException("check");
        return true;
    }
    static int choose(boolean fail) {
        int count = 0;
        count += check(fail) ? 2 : 1;
        return count;
    }
    static int run() {
        if (choose(false) != 2) return 1;
        try { choose(true); return 2; }
        catch (IllegalArgumentException error) { if (calls != 2) return 3; }
        return 0;
    }
}
"#,
    );
}

fn verify(name: &str, source: &str) {
    let translation = new_java2rust::translate_source(source);
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    verify_rust(name, translation.rust);
}

fn verify_rust(name: &str, rust: String) {
    common::cargo_verify_generated_modules_with_edition(
        name,
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust,
            tests: r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn behavior() { assert_eq!(Probe::run().value(), 0); }
"#,
        }],
        "test",
        "2021",
    );
}

#[test]
fn source_enum_constants_share_typed_class_constant_assembly() {
    verify(
        "numbers_enum_constants",
        r#"
enum Limits {
    FIRST, SECOND;
    private static final double SMALL = 0x1.0p-511;
    static final int WRAPPED = Integer.MIN_VALUE - 1;
    static final double WIDENED = 40;
    static final double NORMAL = Double.MIN_NORMAL;
    static int calls;
    double scale(double value) { calls++; return value * SMALL; }
}
enum OnlyConstants {
    VALUE;
    static final int CODE = 54;
}
class Probe {
    static int run() {
        if (Double.doubleToRawLongBits(Limits.FIRST.scale(1.0)) != 0x2000000000000000L) return 1;
        if (Double.doubleToRawLongBits(Limits.SECOND.scale(2.0)) != 0x2010000000000000L) return 2;
        if (Limits.calls != 2 || Limits.WRAPPED != Integer.MAX_VALUE) return 3;
        if (Limits.WIDENED != 40.0 || Limits.NORMAL != Double.MIN_NORMAL) return 4;
        if (OnlyConstants.CODE != 54) return 5;
        if (Limits.FIRST == Limits.SECOND || Limits.FIRST.ordinal() != 0 || Limits.SECOND.ordinal() != 1) return 6;
        if (!Limits.FIRST.name().equals("FIRST") || Limits.values().length != 2) return 7;
        return 0;
    }
}
"#,
    );
}

#[test]
fn floating_wrapper_constants_keep_java_types_and_constant_overflow() {
    verify(
        "numbers_floating_constants",
        r#"
class Probe {
    static final int WRAPPED = Integer.MIN_VALUE - 1;
    static final long LONG_WRAPPED = Long.MAX_VALUE + 1L;
    static final int NEGATED = -Integer.MIN_VALUE;
    static final int SHIFTED = 1 << 33;
    static final double WIDENED = 40;
    static final double WIDENED_SUM = Float.MAX_EXPONENT + 1;
    static double stored = 40;
    static final byte NARROWED = 127;
    static final int LOW = java.lang.Double.MIN_EXPONENT - 1;
    static final double NORMAL = Double.MIN_NORMAL;
    static int run() {
        if (WRAPPED != Integer.MAX_VALUE) return 1;
        if (LONG_WRAPPED != Long.MIN_VALUE) return 2;
        if (LOW != -1023 || Double.MAX_EXPONENT + 1 != 1024) return 3;
        if (Float.MIN_EXPONENT != -126 || Float.MAX_EXPONENT != 127) return 4;
        if (Double.doubleToRawLongBits(NORMAL) != 0x0010000000000000L) return 5;
        if (Float.floatToRawIntBits(Float.MIN_NORMAL) != 0x00800000) return 6;
        if (Float.MIN_NORMAL <= Float.MIN_VALUE || NORMAL <= Double.MIN_VALUE) return 7;
        if (NEGATED != Integer.MIN_VALUE || SHIFTED != 2) return 8;
        if (WIDENED != 40.0 || WIDENED_SUM != 128.0 || stored != 40.0 || NARROWED != 127) return 9;
        return 0;
    }
}
"#,
    );
}

#[test]
fn native_static_method_references_reuse_direct_call_plans() {
    verify(
        "numbers_native_static_references",
        r#"
import java.util.function.DoubleUnaryOperator;
import java.util.function.DoubleBinaryOperator;
class Probe {
    static double apply(DoubleUnaryOperator op, double x) { return op.applyAsDouble(x); }
    static double apply2(DoubleBinaryOperator op, double x, double y) { return op.applyAsDouble(x, y); }
    static int run() {
        if (apply(Math::floor, -1.25) != -2.0) return 1;
        if (apply(java.lang.Math::ceil, -1.25) != -1.0) return 2;
        if (Double.doubleToRawLongBits(apply(Math::ceil, -0.25)) != Long.MIN_VALUE) return 3;
        if (apply(Math::sqrt, 9.0) != 3.0) return 4;
        if (apply2(Math::max, 3.0, 7.0) != 7.0) return 5;
        if (apply2(Math::pow, 2.0, 5.0) != 32.0) return 6;
        return 0;
    }
}
"#,
    );
}

#[test]
fn anonymous_adapters_call_the_lexical_static_owner_not_rust_self() {
    let source = r#"
class Probe {
    interface Equivalence {
        int compare(double a, double b);
        default boolean equivalent(double a, double b) { return compare(a, b) == 0; }
    }
    static int compareTo(double a, double b, double epsilon) {
        return Math.abs(a - b) <= epsilon ? 0 : Double.compare(a, b);
    }
    static int compareTo(int a, int b) { return a - b; }
    static Equivalence create(double eps) {
        return new Equivalence() {
            private final double epsilon = eps;
            public int compare(double a, double b) { return compareTo(a, b, epsilon); }
        };
    }
    static int run() {
        Equivalence coarse = create(0.5);
        Equivalence fine = create(0.01);
        if (!coarse.equivalent(2.0, 2.25)) return 1;
        if (fine.compare(2.0, 2.25) != -1) return 2;
        if (coarse.compare(4.0, 2.0) != 1) return 3;
        if (compareTo(4, 2) != 2) return 4;
        return 0;
    }
}
"#;
    verify("numbers_anonymous_lexical_static", source);
    let batch = new_java2rust::translate_batch_sources_with_context_sources_and_summary_sources(
        &[new_java2rust::BatchSource::new("Probe.java", source)],
        &[],
        &[],
        true,
        &Default::default(),
    );
    assert_eq!(batch.len(), 1);
    assert!(
        !batch[0].diagnostics().has_errors(),
        "{:?}",
        batch[0].diagnostics()
    );
    verify_rust(
        "numbers_batch_anonymous_lexical_static",
        batch[0].rust().to_string(),
    );
}

#[test]
fn source_math_method_references_keep_source_precedence() {
    verify(
        "numbers_source_math_references",
        r#"
import java.util.function.DoubleUnaryOperator;
class Math {
    static double floor(double value) { return value + 40.0; }
}
class Probe {
    static double apply(DoubleUnaryOperator op, double x) { return op.applyAsDouble(x); }
    static int run() {
        if (apply(Math::floor, 2.5) != 42.5) return 1;
        if (apply(java.lang.Math::floor, 2.5) != 2.0) return 2;
        return 0;
    }
}
"#,
    );
}

#[test]
fn shared_receiver_companions_preserve_retained_output_array_aliases() {
    verify(
        "numbers_companion_output_array",
        r#"
class Output {
    static final Output ONE = new Output(1.0);
    final double value;
    Output(double value) { this.value = value; }
    Output split(int[] exp) { exp[0] = 7; return this; }
    Output chain(int[] exp) {
        Output result = split(exp);
        result = result.split(exp);
        return result;
    }
}
class Probe {
    static int[] saved;
    static int run() {
        int[] exp = {0};
        saved = exp;
        Output result = Output.ONE.chain(exp);
        if (exp[0] != 7 || saved[0] != 7) return 1;
        exp[0] = 9;
        if (saved[0] != 9) return 2;
        int[] fresh = {0};
        Output other = new Output(2.0).split(fresh);
        if (fresh[0] != 7 || other.value != 2.0 || result.value != 1.0) return 3;
        return 0;
    }
}
"#,
    );
}

#[test]
fn uncaught_numeric_comparisons_keep_boolean_result_after_promotion() {
    verify(
        "numbers_static_comparisons",
        r#"
class Probe {
    static int divisor = 3;
    static int checks;
    static {
        int n = 7;
        if (n % divisor == 1) checks++;
        if (n % divisor != 0) checks++;
        if (n / divisor < 3L) checks++;
        if (n / divisor <= 2L) checks++;
        if (n / divisor > 1L) checks++;
        if (n / divisor >= 2L) checks++;
    }
    static int run() { return checks == 6 ? 0 : 1; }
}
"#,
    );
}

#[test]
fn nullable_entry_slots_reuse_tuple_construction_and_checked_projection() {
    verify(
        "numbers_nullable_entry",
        r#"
import java.util.Map;
import java.util.AbstractMap.SimpleImmutableEntry;
class Probe {
    static final Map.Entry<String, String> FIRST;
    static Map.Entry<String, String> second = new SimpleImmutableEntry<>("x", "y");
    static Map.Entry<String, String> empty;
    static { FIRST = new SimpleImmutableEntry<>("a", "b"); }
    static int run() {
        if (!FIRST.getKey().equals("a") || !FIRST.getValue().equals("b")) return 1;
        if (!second.getValue().equals("y")) return 2;
        try { empty.getValue(); return 3; }
        catch (NullPointerException expected) {}
        return 0;
    }
}
"#,
    );
}

#[test]
fn stored_entry_array_components_preserve_java_reference_aliases() {
    verify(
        "numbers_entry_array_alias",
        r#"
import java.util.Map;
import java.util.AbstractMap.SimpleImmutableEntry;
class Probe {
    static final int[] original = new int[] { 3, 4 };
    static final Map.Entry<String, int[]> ENTRY;
    static { ENTRY = new SimpleImmutableEntry<>("key", original); }
    static int run() {
        original[0] = 9;
        int[] alias = ENTRY.getValue();
        if (alias[0] != 9) return 1;
        alias[1] = 12;
        if (original[1] != 12) return 2;
        if (ENTRY.getValue()[1] != 12) return 3;
        return 0;
    }
}
"#,
    );
}

#[test]
fn literal_static_final_arrays_keep_writes_and_returned_aliases() {
    verify(
        "numbers_literal_static_array",
        r#"
class Probe {
    static final int[] VALUES = { 4, 5 };
    static int[] values() { return VALUES; }
    static int run() {
        VALUES[0] = 7;
        int[] alias = values();
        if (alias[0] != 7) return 1;
        alias[1] = 13;
        if (VALUES[1] != 13) return 2;
        return 0;
    }
}
"#,
    );
}

#[test]
fn instance_entry_components_retain_constructor_argument_arrays() {
    verify(
        "numbers_instance_entry_array",
        r#"
import java.util.Map;
import java.util.AbstractMap.SimpleImmutableEntry;
class Holder {
    final Map.Entry<String, int[]> entry;
    Holder(int[] value) { entry = new SimpleImmutableEntry<>("key", value); }
    int[] values() { return entry.getValue(); }
}
class Probe {
    static int run() {
        int[] original = { 3, 4 };
        Holder holder = new Holder(original);
        original[0] = 9;
        int[] alias = holder.values();
        if (alias[0] != 9) return 1;
        alias[1] = 12;
        if (original[1] != 12) return 2;
        return 0;
    }
}
"#,
    );
}

#[test]
fn initializer_locals_share_stored_components_before_and_after_the_store() {
    verify(
        "numbers_initializer_local_alias",
        r#"
import java.util.Map;
import java.util.AbstractMap.SimpleImmutableEntry;
class Probe {
    static final Map.Entry<String, int[]> ENTRY;
    static {
        int[] values = new int[] { 3, 4 };
        ENTRY = new SimpleImmutableEntry<>("key", values);
        values[0] = 9;
    }
    static int run() {
        int[] alias = ENTRY.getValue();
        return alias[0] == 9 && alias[1] == 4 ? 0 : 1;
    }
}
"#,
    );
}

#[test]
fn separate_initializer_blocks_keep_lexical_scope_and_textual_order() {
    verify(
        "numbers_initializer_scopes",
        r#"
class Probe {
    static int result;
    static { int value = 2; result = value; }
    static int middle = result + 1;
    static { int value = 3; result += value; }
    static int run() { return result == 5 && middle == 3 ? 0 : 1; }
}
"#,
    );
}

#[test]
fn retained_array_search_evaluates_arguments_before_borrowing_or_null_check() {
    verify(
        "numbers_retained_array_search",
        r#"
import java.util.Arrays;
class Probe {
    static final int[] VALUES = { 1, 2, 9 };
    static int[] empty;
    static int calls;
    static int key() { calls++; VALUES[1] = 8; return 8; }
    static int search(int key) { return Arrays.binarySearch(VALUES, key); }
    static int identity(int value) { return value; }
    static int run() {
        if (Arrays.binarySearch(VALUES, key()) != 1 || calls != 1) return 1;
        VALUES[1] = 2;
        VALUES[2] = 3;
        if (Arrays.binarySearch(VALUES, 0, 3, 2) != 1) return 2;
        if (Arrays.binarySearch(VALUES, 4) != -4) return 3;
        int found = Arrays.binarySearch(VALUES, 2);
        if (found != 1 || search(3) != 2) return 6;
        if (identity(Arrays.binarySearch(VALUES, 1)) != 0) return 7;
        try { Arrays.binarySearch(empty, key()); return 4; }
        catch (NullPointerException expected) {}
        return calls == 2 ? 0 : 5;
    }
}
"#,
    );
}

#[test]
fn synchronized_retained_collections_reuse_live_iteration_and_fallible_reduction() {
    verify(
        "numbers_sync_collection_reduction",
        r#"
import java.util.Set;
import java.util.HashSet;
class Probe {
    static final Set<Integer> VALUES = new HashSet<>();
    static { VALUES.add(2); VALUES.add(3); VALUES.add(5); }
    static int touches;
    static int touch(int value) { touches++; return value - 1; }
    static int change(int value) { VALUES.add(7); return value; }
    static int identityChange() { VALUES.add(11); return 0; }
    static int run() {
        int product = VALUES.stream().reduce(1, (a, b) -> a * b);
        if (product != 30) return 1;
        int reduced = VALUES.stream().mapToInt(a -> touch(a)).reduce(1, (a, b) -> a * b);
        if (reduced != 8 || touches != 3) return 2;
        int sum = 0;
        for (int value : VALUES) sum += value;
        if (sum != 10) return 3;
        try {
            VALUES.stream().mapToInt(a -> 10 / (a - a)).reduce(0, (a, b) -> a + b);
            return 4;
        } catch (ArithmeticException expected) {}
        int overflow = VALUES.stream().mapToInt(a -> a * 2147483647).reduce(0, (a, b) -> a + b);
        if (overflow != -10) return 5;
        try {
            VALUES.stream().mapToInt(a -> change(a)).reduce(0, (a, b) -> a + b);
            return 6;
        } catch (java.util.ConcurrentModificationException expected) {}
        int late = VALUES.stream().reduce(identityChange(), (a, b) -> a + b);
        if (late != 28) return 7;
        return 0;
    }
}
"#,
    );
}

#[test]
fn class_initializer_guard_does_not_discard_a_fallible_body_result() {
    let translation = new_java2rust::translate_source(
        r#"
class Probe {
    static int divisor;
    static int result;
    static { result = 1 / divisor; }
    static int run() { return result; }
}
"#,
    );
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "numbers_initializer_failure",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
#[test] fn initialization_failure_is_not_ignored() {
    let failure = std::panic::catch_unwind(|| Probe::run()).expect_err("initializer error was discarded");
    let message = failure.downcast_ref::<String>().map(String::as_str)
        .or_else(|| failure.downcast_ref::<&str>().copied()).unwrap_or("");
    assert!(message.contains("ArithmeticException") && message.contains("/ by zero"), "{message}");
}
"#,
        }],
    );
}

#[test]
fn integral_bit_counts_use_the_java_formal_width_including_zero_and_sign_bits() {
    verify(
        "numbers_integral_bit_counts",
        r#"
class Probe {
    static int trailing(int value) { return Integer.numberOfTrailingZeros(value); }
    static int leading(long value) { return Long.numberOfLeadingZeros(value); }
    static int run() {
        if (trailing(0) != 32 || trailing(-1) != 0) return 1;
        if (trailing(-2147483648) != 31 || trailing(48) != 4) return 2;
        if (Integer.numberOfLeadingZeros(0) != 32 || Integer.numberOfLeadingZeros(-1) != 0) return 3;
        if (Integer.numberOfLeadingZeros(1) != 31) return 4;
        if (leading(0L) != 64 || leading(-1L) != 0 || leading(1L) != 63) return 5;
        if (Long.numberOfTrailingZeros(0L) != 64 || Long.numberOfTrailingZeros(-9223372036854775808L) != 63) return 6;
        if (Integer.bitCount(-1) != 32 || Long.bitCount(-1) != 64) return 7;
        if (Long.bitCount(1) != 1 || Long.numberOfLeadingZeros(1) != 63) return 8;
        return 0;
    }
}
"#,
    );
}

#[test]
fn source_integer_methods_outrank_jdk_bit_intrinsics() {
    verify(
        "numbers_source_integer",
        r#"
class Integer {
    static int numberOfTrailingZeros(int value) { return value + 77; }
    static int bitCount(int value) { return value + 88; }
}
class Probe {
    static int run() {
        return Integer.numberOfTrailingZeros(0) == 77 && Integer.bitCount(1) == 89 ? 0 : 1;
    }
}
"#,
    );
}
