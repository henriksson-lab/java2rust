#[path = "common/mod.rs"]
mod common;

#[test]
fn package_runtime_export_is_identical_to_the_translation_factory() {
    let result = std::process::Command::new(env!("CARGO_BIN_EXE_new-java2rust"))
        .args(["emit-runtime-support", "floating-point-display"])
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(
        String::from_utf8(result.stdout).unwrap(),
        new_java2rust::RuntimeSupport::FloatingPointDisplay.emit_rust()
    );
}

fn verify(name: &str, source: &str, tests: &'static str) {
    let translation = new_java2rust::translate_source(source);
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
            tests,
        }],
        "test",
        "2021",
    );
}

#[test]
fn floating_comparisons_reuse_ieee_ordering_for_static_and_boxed_calls() {
    verify(
        "rootfinder_float_order",
        r#"
class Probe {
    static int doubles(double a, double b) { return Double.compare(a, b); }
    static int floats(float a, float b) { return Float.compare(a, b); }
    static int boxed(double a, double b) { Double x = a; return x.compareTo(b); }
    static int promoted() { return Double.compare(-0.0f, 0); }
}
"#,
        r#"
#[test] fn ordering() {
    let doubles = [f64::NEG_INFINITY, -1.0, -f64::from_bits(1), -0.0, 0.0,
                   f64::from_bits(1), 1.0, f64::INFINITY, f64::NAN];
    let floats = [f32::NEG_INFINITY, -1.0, -f32::from_bits(1), -0.0, 0.0,
                  f32::from_bits(1), 1.0, f32::INFINITY, f32::NAN];
    for i in 0..doubles.len() { for j in 0..doubles.len() {
        let expected = if i < j {-1} else if i > j {1} else {0};
        assert_eq!(Probe::doubles(doubles[i], doubles[j]), expected, "double {i} {j}");
        assert_eq!(Probe::floats(floats[i], floats[j]), expected, "float {i} {j}");
        assert_eq!(Probe::boxed(doubles[i], doubles[j]), expected, "boxed {i} {j}");
    }}
    let negative_nan = f64::from_bits(0xfff8_0000_0000_1234);
    assert_eq!(Probe::doubles(negative_nan, f64::NAN), 0);
    assert_eq!(Probe::doubles(negative_nan, f64::INFINITY), 1);
    assert_eq!(Probe::promoted(), -1);
}
"#,
    );
}

#[test]
fn source_float_owner_precedes_native_comparison() {
    verify(
        "rootfinder_source_float",
        r#"
class Double { static int compare(double a, double b) { return 77; } }
class Probe { static int run() { return Double.compare(1, 2); } }
"#,
        "#[test] fn source_owner() { assert_eq!(Probe::run(), 77); }",
    );
}

#[test]
fn scientific_display_selects_closest_two_digit_mantissas_for_both_widths() {
    verify(
        "rootfinder_float_display",
        r#"
class Probe {
    static String doubles(double x) { return Double.toString(x); }
    static String floats(float x) { return Float.toString(x); }
}
"#,
        r#"
#[test] fn scientific_and_boundaries() {
    assert_eq!(Probe::doubles(f64::from_bits(1)).scalar_string().expect("scalar numeric output"), "4.9E-324");
    assert_eq!(Probe::doubles(-f64::from_bits(1)).scalar_string().expect("scalar numeric output"), "-4.9E-324");
    assert_eq!(Probe::doubles(f64::from_bits(2)).scalar_string().expect("scalar numeric output"), "9.9E-324");
    assert_eq!(Probe::floats(f32::from_bits(1)).scalar_string().expect("scalar numeric output"), "1.4E-45");
    assert_eq!(Probe::floats(f32::from_bits(2)).scalar_string().expect("scalar numeric output"), "2.8E-45");
    assert_eq!(Probe::doubles(1e7).scalar_string().expect("scalar numeric output"), "1.0E7");
    assert_eq!(Probe::doubles(1e-3).scalar_string().expect("scalar numeric output"), "0.001");
    assert_eq!(Probe::doubles(-0.0).scalar_string().expect("scalar numeric output"), "-0.0");
    assert_eq!(Probe::floats(f32::NAN).scalar_string().expect("scalar numeric output"), "NaN");
    assert_eq!(Probe::doubles(f64::INFINITY).scalar_string().expect("scalar numeric output"), "Infinity");
}
"#,
    );
}

#[test]
fn public_callback_contract_propagates_failures_through_private_forwarders() {
    verify(
        "rootfinder_public_callback",
        r#"
import java.util.function.DoubleUnaryOperator;
class Probe {
    public static double run(DoubleUnaryOperator f, double x) { return twice(f, x); }
    static double plain() { return run(x -> x * 2.0, 3.0); }
    private static double twice(DoubleUnaryOperator f, double x) {
        return f.applyAsDouble(x) + f.applyAsDouble(x + 1);
    }
}
"#,
        r#"
#[test] fn values_and_errors() {
    assert_eq!(Probe::plain().unwrap(), 14.0);
    let mut calls = 0;
    let mut f = |x| { calls += 1; Ok(x * 2.0) };
    assert_eq!(Probe::run(&mut f, 3.0).unwrap(), 14.0);
    drop(f);
    assert_eq!(calls, 2);
    let mut calls = 0;
    let mut f = |x| { calls += 1;
        if calls == 2 { Err(JavaException::new("IllegalStateException", "callback")) }
        else { Ok(x) }
    };
    let error = Probe::run(&mut f, 3.0).unwrap_err();
    assert!(error.is_instance(&["IllegalStateException"]));
    assert_eq!(error.message().scalar_string().expect("scalar callback message"), "callback");
    drop(f);
    assert_eq!(calls, 2);
}
"#,
    );
}

#[test]
fn format_expands_reference_arrays_without_consuming_or_displaying_unused_elements() {
    verify(
        "rootfinder_format_array",
        r#"
class Marker {
    public String toString() { Probe.displays++; return "marker"; }
}
class Probe {
    static int displays;
    static int stage;
    static String pattern() { stage = stage * 10 + 1; return "%s/%s"; }
    static Object[] values() { stage = stage * 10 + 2; return new Object[] {"word", 7, new Marker()}; }
    static String format(String pattern, Object[] values) { return String.format(pattern, values); }
    static int run() {
        if (!String.format(pattern(), values()).equals("word/7")) return 1;
        if (stage != 12 || displays != 0) return 2;
        Object[] objects = new Object[] {"word", null, 7};
        if (!format("%s:%s:%s", objects).equals("word:null:7")) return 3;
        if (!format("%s", objects).equals("word")) return 4;
        String[] strings = new String[] {"first", "second"};
        if (!String.format("%s,%s", strings).equals("first,second")) return 5;
        if (!String.format("%s", strings).equals("first")) return 9;
        if (!String.format("plain", new Object[] {new Marker()}).equals("plain")) return 6;
        if (displays != 0) return 7;
        if (!format("%s,%s", null).equals("null,null")) return 8;
        return 0;
    }
}
"#,
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn reference_varargs() { assert_eq!(Probe::run().value(), 0); }
#[test] fn argument_requests_are_lazy_and_propagate_errors() {
    let mut seen = Vec::new();
    let error = java_string_format_with_arguments("%%:%s/%s/%s", |index| {
        seen.push(index);
        if index == 1 { Err(JavaException::new("IllegalStateException", "display")) }
        else { Ok("ok".to_string()) }
    }).unwrap_err();
    assert_eq!(seen, vec![0, 1]);
    assert_eq!(error.message().scalar_string().expect("scalar callback message"), "display");
}
#[test] fn optional_display_accepts_owned_and_borrowed_carriers() {
    assert_eq!(java_generic_optional_to_string_result(Some(7)).unwrap().scalar_string().unwrap(), "7");
    let value = Some("value".to_string());
    assert_eq!(java_generic_optional_to_string_result(&value).unwrap().scalar_string().unwrap(), "value");
    assert_eq!(value.as_deref(), Some("value"));
    assert_eq!(java_generic_optional_to_string_result(None::<String>).unwrap().scalar_string().unwrap(), "null");
}
"#,
    );
}

#[test]
fn format_does_not_expand_primitive_arrays_or_reference_arrays_cast_to_object() {
    // Both argument shapes now use the common lazy formatter. Check actual
    // Java array-object semantics, not the old emission shortcut's spelling.
    common::verify_java_fixture_zero_contract(
        "format_single_array_argument",
        "FormatArraySingleArgumentProbe",
        "tests/corpus/numbers/FormatArraySingleArgumentProbe.java",
        include_str!("corpus/numbers/FormatArraySingleArgumentProbe.java"),
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn arrays_stay_single_arguments() { assert_eq!(FormatArraySingleArgumentProbe::run().value(), 0); }
"#,
    );
}

#[test]
fn format_element_display_can_reenter_and_mutate_the_same_argument_array() {
    verify(
        "rootfinder_format_array_reentry",
        r#"
class MutatingValue {
    public String toString() { Probe.data[1] = "changed"; return "first"; }
}
class Probe {
    static Object[] data;
    static int run() {
        data = new Object[] {new MutatingValue(), "original"};
        return String.format("%s/%s", data).equals("first/changed") ? 0 : 1;
    }
}
"#,
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn reentrant_display() { assert_eq!(Probe::run().value(), 0); }
"#,
    );
}

#[test]
fn format_argument_aliases_release_array_guards_before_fallible_display() {
    verify(
        "rootfinder_format_instance_array_reentry",
        r#"
class MutatingValue {
    Object[] data;
    MutatingValue(Object[] data) { this.data = data; }
    public String toString() { data[1] = "changed"; return "first"; }
}
class FailingValue {
    public String toString() { throw new IllegalStateException("display failure"); }
}
class Probe {
    static int run() {
        Object[] data = new Object[] {"placeholder", "original"};
        data[0] = new MutatingValue(data);
        if (!String.format("%s/%s", data).equals("first/changed")) return 1;
        try {
            String.format("%s", new Object[] {new FailingValue()});
            return 2;
        } catch (IllegalStateException expected) {
            if (!expected.getMessage().equals("display failure")) return 3;
        }
        return 0;
    }
}
"#,
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn reentrant_fallible_display() { assert_eq!(Probe::run().value(), 0); }
"#,
    );
}

#[test]
fn source_exception_construction_runs_super_expression_and_trailing_effects() {
    verify(
        "rootfinder_exception_constructor",
        r#"
class FormattedProblem extends IllegalArgumentException {
    FormattedProblem(String pattern, Object... values) {
        super(Probe.message(pattern, values));
        Probe.after();
    }
}
class Probe {
    static int stage;
    static String message(String pattern, Object[] values) {
        stage = stage * 10 + 1;
        return String.format(pattern, values);
    }
    static void after() { stage = stage * 10 + 2; }
    static int run() {
        try { throw new FormattedProblem("value=%s", 7); }
        catch (FormattedProblem error) {
            if (!error.getMessage().equals("value=7")) return 1;
        }
        return stage == 12 ? 0 : 2;
    }
}
"#,
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn constructor_state_and_order() { assert_eq!(Probe::run().value(), 0); }
"#,
    );
}

#[test]
fn nullable_callback_checks_happen_at_invocation_after_earlier_validation() {
    verify(
        "rootfinder_nullable_callback",
        r#"
import java.util.function.DoubleUnaryOperator;
class Probe {
    public static double apply(DoubleUnaryOperator f, double x) {
        if (x < 0) throw new IllegalArgumentException("range");
        return f.applyAsDouble(x);
    }
    static int run() {
        try { apply(null, 1); return 1; }
        catch (NullPointerException expected) {}
        try { apply(null, -1); return 2; }
        catch (IllegalArgumentException expected) {
            if (!expected.getMessage().equals("range")) return 3;
        }
        return 0;
    }
}
"#,
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn null_and_validation_order() { assert_eq!(Probe::run().value(), 0); }
"#,
    );
}

#[test]
fn source_exception_initialization_precedes_fields_and_existing_value_throw() {
    verify(
        "rootfinder_exception_initialization_order",
        r#"
class Problem extends IllegalArgumentException {
    int value = Probe.field();
    Problem() { super(Probe.message()); Probe.after(); }
}
class Probe {
    static int stage;
    static String message() { stage = stage * 10 + 1; return "constructed"; }
    static int field() { stage = stage * 10 + 2; return 42; }
    static void after() { stage = stage * 10 + 3; }
    static int run() {
        Problem problem = new Problem();
        if (stage != 123 || problem.value != 42) return 1;
        try { throw problem; }
        catch (IllegalArgumentException error) {
            if (!error.getMessage().equals("constructed")) return 2;
            if (!(error instanceof Problem)) return 3;
        }
        return stage == 123 ? 0 : 4;
    }
}
"#,
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn actual_instance_state() { assert_eq!(Probe::run().value(), 0); }
"#,
    );
}

#[test]
fn source_exception_super_failure_prevents_fields_and_trailing_body() {
    verify(
        "rootfinder_exception_super_failure",
        r#"
class Problem extends IllegalArgumentException {
    int value = Probe.field();
    Problem() { super(Probe.message()); Probe.after(); }
}
class Probe {
    static int stage;
    static String message() { stage = 1; throw new IllegalStateException("super failed"); }
    static int field() { stage = 2; return 42; }
    static void after() { stage = 3; }
    static int run() {
        try { throw new Problem(); }
        catch (IllegalStateException error) {
            if (!error.getMessage().equals("super failed")) return 1;
        }
        return stage == 1 ? 0 : 2;
    }
}
"#,
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn construction_failure() { assert_eq!(Probe::run().value(), 0); }
"#,
    );
}

#[test]
fn platform_and_source_exception_messages_distinguish_null_empty_and_causes() {
    verify(
        "rootfinder_exception_nullable_message",
        r#"
class EmptyProblem extends IllegalArgumentException {
    EmptyProblem() { super(); }
}
class MessageProblem extends IllegalArgumentException {
    MessageProblem(String message) { super(message); }
}
class CauseProblem extends RuntimeException {
    CauseProblem(String message, Throwable cause) { super(message, cause); }
}
class Probe {
    static int run() {
        try { throw new EmptyProblem(); }
        catch (EmptyProblem e) { if (e.getMessage() != null) return 1; }
        try { throw new MessageProblem(null); }
        catch (MessageProblem e) { if (e.getMessage() != null) return 2; }
        try { throw new MessageProblem(""); }
        catch (MessageProblem e) { if (!e.getMessage().equals("")) return 3; }
        try { throw new IllegalArgumentException((String) null); }
        catch (IllegalArgumentException e) { if (e.getMessage() != null) return 4; }
        try { throw new CauseProblem("outer", new IllegalStateException("inner")); }
        catch (CauseProblem e) {
            if (!e.getMessage().equals("outer")) return 5;
            if (!e.getCause().getMessage().equals("inner")) return 6;
        }
        return 0;
    }
}
"#,
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn messages_and_causes() { assert_eq!(Probe::run().value(), 0); }
"#,
    );
}

#[test]
fn source_exception_constructor_delegation_preserves_base_state_and_order() {
    verify(
        "rootfinder_exception_delegation",
        r#"
class BaseProblem extends IllegalArgumentException {
    BaseProblem(String message) { super(message); Probe.mark(1); }
}
class ChildProblem extends BaseProblem {
    ChildProblem() { this("delegated"); Probe.mark(3); }
    ChildProblem(String message) { super(message); Probe.mark(2); }
}
class Probe {
    static int stage;
    static void mark(int step) { stage = stage * 10 + step; }
    static int run() {
        try { throw new ChildProblem(); }
        catch (BaseProblem e) {
            if (!e.getMessage().equals("delegated")) return 1;
            if (!(e instanceof ChildProblem)) return 2;
        }
        return stage == 123 ? 0 : 3;
    }
}
"#,
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn constructor_delegation() { assert_eq!(Probe::run().value(), 0); }
"#,
    );
}

#[test]
fn source_exception_upcast_keeps_dynamic_type_and_source_lookalikes_stay_source() {
    verify(
        "rootfinder_exception_upcast",
        r#"
class BaseProblem extends java.lang.RuntimeException {
    BaseProblem(String message) { super(message); }
}
class ChildProblem extends BaseProblem {
    ChildProblem(String message) { super(message); }
}
class IllegalArgumentException {
    int value;
    IllegalArgumentException(int value) { this.value = value; }
}
class Probe {
    static int run() {
        if (new IllegalArgumentException(42).value != 42) return 1;
        BaseProblem value = new ChildProblem("child");
        try { throw value; }
        catch (ChildProblem error) {
            if (!error.getMessage().equals("child")) return 2;
        }
        return 0;
    }
}
"#,
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn dynamic_type_and_source_identity() { assert_eq!(Probe::run().value(), 0); }
"#,
    );
}

#[test]
fn fallible_ternaries_pin_the_exception_domain_for_nullable_and_primitive_values() {
    verify(
        "rootfinder_checked_ternary_domain",
        r#"
class Probe {
    static int calls;
    static long checked(long value) {
        calls++;
        if (value < 0) throw new IllegalArgumentException("negative");
        return value;
    }
    static String text() {
        calls++;
        if (calls < 0) throw new IllegalStateException("counter");
        return "text";
    }
    static int run(boolean empty) {
        calls = 0;
        long value = empty ? 0L : checked(7);
        String message = empty ? null : text();
        if (empty) {
            if (value != 0 || message != null || calls != 0) return 1;
        } else {
            if (value != 7 || !message.equals("text") || calls != 2) return 2;
        }
        return 0;
    }
}
"#,
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn checked_conditional_values() {
    assert_eq!(Probe::run(true).value(), 0);
    assert_eq!(Probe::run(false).value(), 0);
}
"#,
    );
}
