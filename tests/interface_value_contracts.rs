use new_java2rust::{
    effective_facts::EffectiveFacts,
    object_model::{ObjectRepresentation, plan_object_model_with_effective_facts},
    parser::parse_java_source,
    resolve::resolve_program,
};

#[path = "common/mod.rs"]
mod common;
use common::{GeneratedCrateModule, cargo_test_generated_modules};

fn assert_dynamic_interface(source: &str) {
    let parsed = parse_java_source(source);
    assert!(!parsed.diagnostics.has_errors(), "{:?}", parsed.diagnostics);
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );
    let effective = EffectiveFacts::new(&resolved.facts);
    let plan = plan_object_model_with_effective_facts(&resolved.program, &effective);
    let entry = plan.strategy_for("Link").unwrap();
    assert_eq!(
        entry.strategy(),
        ObjectRepresentation::DynTrait {
            trait_name: "Link".into()
        },
        "{:?}",
        entry.diagnostics()
    );
}

#[test]
fn semantic_interface_return_types_do_not_depend_on_method_names() {
    for name in ["next", "copy", "clone", "negate", "recreate"] {
        assert_dynamic_interface(&format!("interface Link {{ Link {name}(); }}"));
    }
}

#[test]
fn semantic_nested_interface_returns_are_reference_values_not_rust_self() {
    assert_dynamic_interface(
        "import java.util.List; interface Link<T> { List<Link<T>> children(); Link<T>[] peers(); }",
    );
}

#[test]
fn semantic_nested_interface_parameters_are_reference_values_not_rust_self() {
    assert_dynamic_interface(
        "import java.util.List; interface Link<T> { void attach(List<Link<T>> peers); }",
    );
}

#[test]
fn interface_return_values_keep_dynamic_dispatch() {
    let translation = new_java2rust::translate_source(
        r#"
import java.util.List;
interface Link<T> {
    List<Link<T>> children();
    Link<T>[] peers();
    void attach(List<Link<T>> peers);
}
class Holder { List<Link<String>> items; }
interface Value { Value next(); int get(); }
class Cell implements Value {
    private int number;
    Cell(int number) { this.number = number; }
    public Value next() { return new Cell(number + 1); }
    public int get() { return number; }
}
interface Ref { Ref same(); void increment(); int get(); }
class MutableCell implements Ref {
    private int number;
    MutableCell(int number) { this.number = number; }
    public Ref same() { return this; }
    public Object erased() { return this; }
    public void increment() { number++; }
    public int get() { return number; }
}
class Use {
    static int run() {
        Value first = new Cell(4);
        Value second = first.next();
        return second.get() * 10 + first.get();
    }
    static int alias() {
        Ref first = new MutableCell(4);
        Ref second = first.same();
        second.increment();
        return first.get();
    }
}
"#,
    );
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    cargo_test_generated_modules(
        "interface_return_values",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
#[test]
fn interface_results_dispatch_to_their_actual_objects() {
    assert_eq!(Use::run(), 54);
    assert_eq!(Use::alias(), 5);
    assert!(Holder::new().items.is_none());
    let receiver = std::rc::Rc::new(std::cell::RefCell::new(MutableCell::new(9)));
    let erased = MutableCell::erased_with_outer(receiver.clone());
    let alias = erased.downcast_ref::<std::rc::Rc<std::cell::RefCell<MutableCell>>>().unwrap();
    assert!(std::rc::Rc::ptr_eq(&receiver, alias));
}
"#,
        }],
    );
}
