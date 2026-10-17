#[path = "common/mod.rs"]
mod common;

#[test]
fn mutable_source_interface_can_be_reborrowed_read_only_and_then_mutated_again() {
    let translation = new_java2rust::translate_source(
        r#"
interface Port<Value> {
    void put(Value value);
    Value get();
}
class TextPort implements Port<String> {
    String value = "old";
    public void put(String value) { this.value = value; }
    public String get() { return value; }
}
class PortUse {
    static Port<String> make() { return new TextPort(); }
    static <Value> Value read(Port<Value> port) { return port.get(); }
    static <Value> Value update(Port<Value> port, Value value) {
        port.put(value);
        Value observed = read(port);
        port.put(value);
        return observed;
    }
    static String run() {
        TextPort port = new TextPort();
        update(port, "hello");
        return port.value;
    }
    static int boxedRun() {
        Port<String> port = make();
        update(port, "boxed");
        return port.get().length();
    }
}
"#,
    );
    common::cargo_test_generated_modules(
        "trait_reference_reborrows",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
#[test]
fn reborrows_preserve_the_original_storage_and_allow_reuse() {
    assert_eq!(PortUse::run(), "hello");
    assert_eq!(PortUse::boxed_run(), 5);
}
"#,
        }],
    );
}
