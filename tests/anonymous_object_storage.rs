use new_java2rust::translate_source;

#[path = "common/mod.rs"]
mod common;

#[test]
fn anonymous_fields_and_local_replacements_share_the_planned_object_storage() {
    let translation = translate_source(
        r#"
class Node { int value; Node(int value) { this.value = value; } }
class Base {
    int read() { return -1; }
    int replace(int value) { return -1; }
    int viaLocal(int value) { return -1; }
}
class Use {
    static Base retained(Node initial) {
        return new Base() {
            Node node = initial;
            int read() { return this.node.value; }
            int replace(int value) { this.node = new Node(value); return this.node.value; }
            int viaLocal(int value) {
                Node next = new Node(value);
                this.node = next;
                return this.node.value;
            }
        };
    }
    static Base fresh() {
        return new Base() {
            Node node = new Node(3);
            int read() { return this.node.value; }
        };
    }
    static int run() {
        Node original = new Node(1);
        Base value = retained(original);
        original.value = 7;
        int seen = value.read();
        int replaced = value.replace(11);
        int local = value.viaLocal(13);
        return seen + replaced + local + original.value + fresh().read();
    }
}
"#,
    );
    common::cargo_test_generated_modules(
        "anonymous_object_storage",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "#[test] fn assignments_preserve_the_reference_domain() { assert_eq!(Use::run(), 41); }",
        }],
    );
}
