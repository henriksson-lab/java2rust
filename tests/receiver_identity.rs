use new_java2rust::translate_source;

#[path = "common/mod.rs"]
mod common;
use common::{GeneratedCrateModule, cargo_check_generated_modules, cargo_test_generated_modules};

#[test]
fn java_clone_method_does_not_shadow_native_reference_copy() {
    let translation = translate_source(
        r#"
interface CloneReference { CloneReference clone(); }
class NamedClone implements CloneReference {
    public CloneReference clone() { return this; }
    static int[] copy(int[] values) { return values; }
    static int probe() {
        int[] values = new int[] {1, 2};
        int[] alias = copy(values);
        alias[0] = 7;
        return values[0];
    }
}
"#,
    );
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    assert!(translation.rust.contains("as std::clone::Clone>::clone"));
    cargo_check_generated_modules(
        "java_clone_native_reference_copy",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn shared_receiver_independent_body_does_not_acquire_payload_guard() {
    let translation = translate_source(
        r#"
class PureReceiver {
    static final PureReceiver INST = new PureReceiver();
    static PureReceiver replaceable = new PureReceiver();
    int state;
    PureReceiver same() { return this; }
    void set(int value) { state = value; }
    public double evaluate(double x) { return x + 1.0; }
    public synchronized double synchronizedEvaluate(double x) { return x + 1.0; }
    public int read() { return state; }
    static double probe() { return INST.evaluate(2.0); }
}
class InitializedFinalReceiver {
    static final PureReceiver VALUE;
    static { VALUE = new PureReceiver(); }
}
"#,
    );
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    let companion = translation
        .rust
        .split("pub fn evaluate_with_outer(")
        .nth(1)
        .expect("shared instance method must retain its handle entry point")
        .split("\n    }")
        .next()
        .unwrap();
    assert!(
        !companion.contains(".lock()") && !companion.contains(".borrow"),
        "receiver-independent implementation acquired a guard: {companion}"
    );
    let synchronized_companion = translation
        .rust
        .split("pub fn synchronized_evaluate_with_outer(")
        .nth(1)
        .unwrap()
        .split("\n    }")
        .next()
        .unwrap();
    assert!(
        synchronized_companion.contains("__java_monitor_guard"),
        "synchronized method lost its Java monitor: {synchronized_companion}"
    );
    let read_companion = translation
        .rust
        .split("pub fn read_with_outer(")
        .nth(1)
        .unwrap()
        .split("\n    }")
        .next()
        .unwrap();
    assert!(
        read_companion.contains(".lock()"),
        "field reader lost its payload guard: {read_companion}"
    );
    let static_getter = translation
        .rust
        .split("pub fn __static_INST(")
        .nth(1)
        .unwrap()
        .split("\n    }")
        .next()
        .unwrap();
    assert!(static_getter.contains(".get_or_init("), "{static_getter}");
    assert!(!static_getter.contains(".lock()"), "{static_getter}");
    let mutable_static_getter = translation
        .rust
        .split("pub fn __static_replaceable(")
        .nth(1)
        .unwrap()
        .split("\n    }")
        .next()
        .unwrap();
    assert!(
        mutable_static_getter.contains(".lock()"),
        "{mutable_static_getter}"
    );
    let initialized_static_getter = translation
        .rust
        .split("pub fn __static_VALUE(")
        .nth(1)
        .unwrap()
        .split("\n    }")
        .next()
        .unwrap();
    assert!(
        initialized_static_getter.contains(".lock()"),
        "{initialized_static_getter}"
    );
    assert!(translation.rust.contains("fn __set_static_VALUE("));
    cargo_test_generated_modules(
        "receiver_independent_shared_body",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
#[test]
fn independent_body_can_run_while_payload_is_locked() {
    let instance = std::sync::Arc::new(std::sync::Mutex::new(PureReceiver::new()));
    let _guard = instance.lock().unwrap();
    assert_eq!(PureReceiver::evaluate_with_outer(instance.clone(), 2.0), 3.0);
    drop(_guard);
    PureReceiver::set_with_outer(instance.clone(), 7);
    assert_eq!(PureReceiver::read_with_outer(instance.clone()), 7);
    assert_eq!(PureReceiver::synchronized_evaluate_with_outer(instance, 3.0), 4.0);
}

#[test]
fn final_reference_is_shared_across_threads_but_its_payload_remains_mutable() {
    let first = PureReceiver::__static_INST();
    let second = std::thread::spawn(|| PureReceiver::__static_INST()).join().unwrap();
    assert!(std::sync::Arc::ptr_eq(&first, &second));
    PureReceiver::set_with_outer(first, 42);
    assert_eq!(PureReceiver::read_with_outer(second), 42);
}
"#,
        }],
    );
}

#[test]
fn private_receiver_is_retained_for_arithmetic_and_iteration_field_reads() {
    let translation = translate_source(
        r#"
class ReceiverRead {
    int offset = 3;
    int[] values = new int[] {1, 2};
    private int score() {
        int result = offset + 1;
        for (int value : values) { result += value; }
        return result;
    }
    int get() { return score(); }
}
"#,
    );
    assert!(
        translation.rust.contains("fn score(&self)"),
        "{}",
        method_headers(&translation.rust)
    );
    cargo_check_generated_modules(
        "private_receiver_field_reads",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn private_receiver_independence_respects_parameter_shadowing() {
    let translation = translate_source(
        r#"
class ShadowedReceiver {
    int offset;
    private int score(int offset) { return offset + 1; }
    int get() { return score(3); }
}
"#,
    );
    assert!(
        translation.rust.contains("fn score(offset: i32)"),
        "{}",
        method_headers(&translation.rust)
    );
    cargo_check_generated_modules(
        "private_receiver_shadowing",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

fn method_headers(rust: &str) -> String {
    rust.lines()
        .filter(|line| line.contains("fn score") || line.contains("fn get("))
        .collect::<Vec<_>>()
        .join("\n")
}
