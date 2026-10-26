#[path = "common/mod.rs"]
mod common;

#[test]
fn interface_only_generic_object_bridge_has_its_runtime_dependencies() {
    verify(
        "sampling_interface_only_bridge",
        "interface View<T> { T get(); } interface ChildView<T> extends View<T> {}",
        "",
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
fn native_log1p_and_expm1_preserve_small_values_and_ieee_boundaries() {
    verify(
        "sampling_stable_math",
        r#"
class StableMath {
    static double log(double x) { return Math.log1p(x); }
    static double exp(double x) { return Math.expm1(x); }
}
"#,
        r#"
#[test] fn stable_boundaries() {
    let tiny = 1.0 / 18014398509481984.0;
    assert_eq!(StableMath::log(tiny).to_bits(), tiny.to_bits());
    assert_eq!(StableMath::exp(tiny).to_bits(), tiny.to_bits());
    assert_eq!(StableMath::log(-0.0).to_bits(), (-0.0_f64).to_bits());
    assert_eq!(StableMath::exp(-0.0).to_bits(), (-0.0_f64).to_bits());
    assert_eq!(StableMath::log(-1.0), f64::NEG_INFINITY);
    assert!(StableMath::log(-2.0).is_nan());
    assert_eq!(StableMath::log(f64::INFINITY), f64::INFINITY);
    assert_eq!(StableMath::exp(f64::NEG_INFINITY), -1.0);
    assert_eq!(StableMath::exp(f64::INFINITY), f64::INFINITY);
    assert!(StableMath::log(f64::NAN).is_nan());
    assert!(StableMath::exp(f64::NAN).is_nan());
}
"#,
    );
}

#[test]
fn native_math_extension_keeps_parsed_source_precedence() {
    verify(
        "sampling_math_source_precedence",
        r#"
class Math {
    static double log1p(double x) { return x + 7; }
    static double expm1(double x) { return x + 11; }
}

class MathUse {
    static double log(double x) { return Math.log1p(x); }
    static double exp(double x) { return Math.expm1(x); }
}
"#,
        "#[test] fn source_methods() { assert_eq!(MathUse::log(2.0), 9.0); assert_eq!(MathUse::exp(2.0), 13.0); }",
    );
}

#[test]
fn interface_object_string_dispatch_preserves_state_null_and_exceptions() {
    verify(
        "sampling_interface_object_string",
        r#"
interface Labelled { int id(); }
interface ChildLabelled extends Labelled {}
class StatefulLabel implements ChildLabelled {
    int calls;
    int mode;
    LabelHolder holder;
    StatefulLabel(int mode) { this.mode = mode; }
    public int id() { return calls; }
    public String toString() {
        calls++;
        if (holder != null && holder.peek() != 7) throw new IllegalStateException("holder");
        if (mode == 2) throw new IllegalStateException("label");
        return mode == 1 ? null : "label:" + calls;
    }
}
class LabelHolder {
    Labelled value;
    int marker = 7;
    LabelHolder(Labelled value) { this.value = value; }
    int peek() { return marker; }
    String read() { return value.toString(); }
}
class LabelUse {
    static Labelled make() { return new StatefulLabel(0); }
    static String describe(Labelled value) { return value.toString(); }
    static int run() {
        StatefulLabel first = new StatefulLabel(0);
        ChildLabelled child = first;
        if (!describe(child).equals("label:1")) return 1;
        if (first.calls != 1) return 2;
        first.holder = new LabelHolder(first);
        if (!first.holder.read().equals("label:2") || first.calls != 2) return 5;
        if (describe(new StatefulLabel(1)) != null) return 3;
        try { describe(new StatefulLabel(2)); return 4; }
        catch (IllegalStateException expected) {}
        return 0;
    }
}
"#,
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn source_dispatch() { assert_eq!(LabelUse::run().value(), 0); }
#[test] fn rust_interface_bridge() {
    let mut value = LabelUse::make();
    assert_eq!(Labelled::to_string(&*value), "label:1".into());
    assert_eq!(Labelled::id(&*value).value(), 1);
}
#[derive(Clone)] struct ForeignLabel;
impl Labelled for ForeignLabel {
    fn id(&self) -> i32 { 0 }
    fn as_any(&self) -> &dyn std::any::Any { self }
    fn clone_any(&self) -> Box<dyn std::any::Any> { Box::new(self.clone()) }
    fn clone_box(&self) -> Box<dyn Labelled> { Box::new(self.clone()) }
    fn to_string(&self) -> JavaLogicalString { "foreign".into() }
}
#[test] fn foreign_interface_override() {
    assert_eq!(LabelUse::describe(&ForeignLabel).unwrap(), Some("foreign".into()));
}
"#,
    );
}
