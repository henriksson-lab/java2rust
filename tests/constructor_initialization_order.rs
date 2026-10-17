use new_java2rust::translate_source;

#[path = "common/mod.rs"]
mod common;
use common::{GeneratedCrateModule, cargo_test_generated_modules};

#[test]
fn field_initializers_resolve_calls_and_diamond_in_class_scope() {
    use new_java2rust::{
        java_ir::{Expr, JavaType, Member, PrimitiveType},
        parser::parse_java_source,
        resolve::resolve_program,
    };
    let parsed = parse_java_source(
        r#"
class Pick {
    static int choose(int value) { return value; }
    static int choose(String value) { return -1; }
}
class Carrier<Element> { Carrier() {} }
class InitProbe<Stored> {
    Carrier<Stored> carrier = new Carrier<>();
    int chosen = Pick.choose(3);
    InitProbe(String Pick) {}
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let owner = resolved
        .program
        .types
        .iter()
        .find(|ty| ty.name == "InitProbe")
        .unwrap();
    for member in &owner.members {
        let Member::Field(field) = member else {
            continue;
        };
        match field.initializer.as_ref().unwrap() {
            Expr::New {
                resolved: Some(constructor),
                ..
            } => {
                assert_eq!(constructor.constructed_type, field.ty);
            }
            Expr::Call {
                resolved: Some(call),
                ..
            } => {
                assert_eq!(call.declaring_type, "Pick");
                assert_eq!(
                    call.arg_target_types,
                    vec![JavaType::Primitive(PrimitiveType::Int)]
                );
            }
            other => panic!("field initializer was not resolved: {other:?}"),
        }
    }
}

#[test]
fn source_constructor_phases_preserve_multilevel_order_and_default_state() {
    let translation = translate_source(
        r#"
class PhaseRoot {
    int base;
    PhaseRoot(int seed) { base = seed; }
}
class PhaseMiddle extends PhaseRoot {
    boolean enabled;
    int width;
    PhaseMiddle(int seed) {
        super(seed);
        if (seed > 0) enabled = true; else enabled = false;
        int selected = enabled ? 4 : 3;
        width = selected;
    }
}
class PhaseLeaf extends PhaseMiddle {
    int snapshot = width;
    PhaseLeaf(int seed) { super(seed); }
}
class ImplicitPhaseRoot {
    int root = 2;
    ImplicitPhaseRoot() { root += 1; }
}
class ImplicitPhaseMiddle extends ImplicitPhaseRoot {
    int middle = root * 10;
    ImplicitPhaseMiddle() { middle += 1; }
}
class ImplicitPhaseLeaf extends ImplicitPhaseMiddle {
    int leaf = middle * 10;
    ImplicitPhaseLeaf() {}
}
class DefaultStateRoot {
    int observed;
    DefaultStateRoot() { observed = read(); }
    int read() { return -1; }
}
class DefaultStateChild extends DefaultStateRoot {
    int value = 7;
    int copy = value;
    DefaultStateChild(int value) { super(); }
    int read() { return value; }
}
class PhaseTrace {
    static int state;
    static int mark(int digit) { state = state * 10 + digit; return digit; }
    static void reset() { state = 0; }
    static int read() { return state; }
}
class EffectsPhaseRoot {
    int token = PhaseTrace.mark(2);
    EffectsPhaseRoot(int argument) { token += argument; PhaseTrace.mark(3); }
}
class EffectsPhaseChild extends EffectsPhaseRoot {
    int own = PhaseTrace.mark(4);
    EffectsPhaseChild() { super(PhaseTrace.mark(1)); PhaseTrace.mark(5); }
}
class UnusedArgumentRoot { UnusedArgumentRoot(int ignored) {} }
class UnusedArgumentChild extends UnusedArgumentRoot {
    int own = PhaseTrace.mark(7);
    UnusedArgumentChild() { super(PhaseTrace.mark(6)); }
}
class ArgumentPhaseRoot {
    int base;
    ArgumentPhaseRoot(int n) { base = n; }
}
class ArgumentPhaseMiddle extends ArgumentPhaseRoot {
    int middle;
    ArgumentPhaseMiddle(int seed) { super(seed + 1); middle = seed; }
}
class ArgumentPhaseLeaf extends ArgumentPhaseMiddle {
    ArgumentPhaseLeaf(int input) { super(input + 1); }
}
"#,
    );
    assert!(
        !translation.rust.contains("compile_error!"),
        "{}",
        translation.rust
    );
    cargo_test_generated_modules(
        "source_constructor_phases",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
#[test]
fn superclass_control_flow_precedes_child_initializers() {
    let positive = PhaseLeaf::new(1);
    assert_eq!(positive.base, 1);
    assert_eq!(positive.width, 4);
    assert_eq!(positive.snapshot, 4);
    let negative = PhaseLeaf::new(-1);
    assert_eq!(negative.width, 3);
    assert_eq!(negative.snapshot, 3);
}
#[test]
fn implicit_super_calls_keep_every_initializer_and_body_in_order() {
    let value = ImplicitPhaseLeaf::new();
    assert_eq!(value.root, 3);
    assert_eq!(value.middle, 31);
    assert_eq!(value.leaf, 310);
}
#[test]
fn superclass_virtual_call_observes_child_default_before_initializer() {
    let value = DefaultStateChild::new(11);
    assert_eq!(value.observed, 0);
    assert_eq!(value.value, 7);
    assert_eq!(value.copy, 7);
}
#[test]
fn evaluates_super_arguments_before_initializers_even_if_unused() {
    PhaseTrace::reset();
    let value = EffectsPhaseChild::new();
    assert_eq!(value.token, 3);
    assert_eq!(value.own, 4);
    assert_eq!(PhaseTrace::read(), 12345);
    PhaseTrace::reset();
    let value = UnusedArgumentChild::new();
    assert_eq!(value.own, 7);
    assert_eq!(PhaseTrace::read(), 67);
}
#[test]
fn nested_argument_temporaries_do_not_shadow_outer_values() {
    let value = ArgumentPhaseLeaf::new(10);
    assert_eq!(value.base, 12);
    assert_eq!(value.middle, 11);
}
"#,
        }],
    );
}
