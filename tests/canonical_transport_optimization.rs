use new_java2rust::translate_source;
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tree_sitter::{Node, Parser};

#[path = "common/mod.rs"]
mod common;

const NODE: &str = r#"
class Node {
    int value;
    Node(int value) { this.value = value; }
    void set(int value) { this.value = value; }
}
"#;

fn children(node: Node<'_>) -> Vec<Node<'_>> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor).collect()
}

/// Inspect a named source method, excluding unrelated generated runtime code.
fn method<'a>(rust: &'a str, owner: &str, name: &str) -> &'a str {
    fn find(
        node: Node<'_>,
        source: &str,
        owner: &str,
        name: &str,
    ) -> Option<std::ops::Range<usize>> {
        if node.kind() == "impl_item"
            && node.child_by_field_name("trait").is_none()
            && node
                .child_by_field_name("type")
                .is_some_and(|ty| &source[ty.byte_range()] == owner)
        {
            let body = node.child_by_field_name("body")?;
            for member in children(body) {
                if member.kind() == "function_item"
                    && member
                        .child_by_field_name("name")
                        .is_some_and(|id| &source[id.byte_range()] == name)
                {
                    return Some(member.byte_range());
                }
            }
        }
        children(node)
            .into_iter()
            .find_map(|child| find(child, source, owner, name))
    }
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_rust::LANGUAGE.into())
        .unwrap();
    let tree = parser.parse(rust, None).unwrap();
    let range = find(tree.root_node(), rust, owner, name)
        .unwrap_or_else(|| panic!("missing {owner}::{name} in generated Rust"));
    &rust[range]
}

fn call_arguments(rust_method: &str, target: &str) -> Vec<String> {
    fn collect(node: Node<'_>, source: &str, target: &str, out: &mut Vec<String>) {
        if node.kind() == "call_expression"
            && node
                .child_by_field_name("function")
                .is_some_and(|fun| &source[fun.byte_range()] == target)
        {
            let arguments = node.child_by_field_name("arguments").unwrap();
            out.push(
                source[arguments.byte_range()]
                    .chars()
                    .filter(|ch| !ch.is_whitespace())
                    .collect(),
            );
        }
        for child in children(node) {
            collect(child, source, target, out);
        }
    }
    let source = format!("impl Use {{ {rust_method} }}");
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_rust::LANGUAGE.into())
        .unwrap();
    let tree = parser.parse(&source, None).unwrap();
    let mut arguments = Vec::new();
    collect(tree.root_node(), &source, target, &mut arguments);
    arguments
}

fn assert_owned_node_parameter(rust: &str, name: &str) {
    let method = method(rust, "Use", name);
    let signature = method.split_once('{').unwrap().0.replace(' ', "");
    assert!(
        signature.contains("node:std::rc::Rc<std::cell::RefCell<Node>>")
            || signature.contains("node:Option<std::rc::Rc<std::cell::RefCell<Node>>>"),
        "{method}"
    );
    assert!(
        !signature.contains("node:&") && !signature.contains("node:Option<&"),
        "{method}"
    );
}

/// Check the original Java behavior independently, without translating oracle
/// scaffolding or editing any generated Rust to repair its implementation.
fn verify_java_probe(source: &str, expected: i32) {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("canonical-transport-java-{unique}"));
    std::fs::create_dir_all(&root).unwrap();
    let source_path = root.join("TransportOracle.java");
    std::fs::write(&source_path, format!(
        "{source}\nclass TransportOracle {{ public static void main(String[] args) {{ if (Use.probe() != {expected}) throw new AssertionError(); }} }}\n"
    )).unwrap();
    let compiled = common::run_with_watchdog(
        Command::new("javac").arg("-d").arg(&root).arg(&source_path),
        Duration::from_secs(30),
    );
    assert!(
        compiled.status.is_some_and(|status| status.success()),
        "{}",
        compiled.stderr
    );
    let run = common::run_with_watchdog(
        Command::new("java")
            .arg("-Xmx128m")
            .arg("-cp")
            .arg(&root)
            .arg("TransportOracle"),
        Duration::from_secs(30),
    );
    assert!(
        run.status.is_some_and(|status| status.success()),
        "{}",
        run.stderr
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn private_scalar_observer_borrows_the_canonical_handle_and_preserves_alias_writes() {
    let source = format!(
        r#"{NODE}
class Use {{
    private static int read(Node node) {{ return node.value; }}
    static int probe() {{
        Node first = new Node(1);
        Node alias = first;
        alias.set(2);
        int a = read(first);
        return a + read(first);
    }}
}}
"#
    );
    verify_java_probe(&source, 4);
    let translated = translate_source(&source);
    assert!(
        !translated.diagnostics.has_errors(),
        "{:?}",
        translated.diagnostics
    );
    let read = method(&translated.rust, "Use", "read");
    let signature = read.split_once('{').unwrap().0.replace(' ', "");
    assert!(
        signature.contains("node:&std::rc::Rc<std::cell::RefCell<Node>>"),
        "{read}"
    );
    let probe = method(&translated.rust, "Use", "probe");
    assert_eq!(
        call_arguments(probe, "Use::read"),
        vec!["(&first)", "(&first)"],
        "{probe}"
    );
    // This final-use call needs no caller-side reference copy. Borrowing is
    // still cheaper because it avoids the observer's owned field-owner copy.
    let last_use_source = format!(
        r#"{NODE}
class Use {{
    private static int read(Node node) {{ return node.value; }}
    static int probe() {{
        Node first = new Node(1);
        Node alias = first;
        alias.set(2);
        return read(first);
    }}
}}
"#
    );
    verify_java_probe(&last_use_source, 2);
    let last_use = translate_source(&last_use_source);
    assert!(
        !last_use.diagnostics.has_errors(),
        "{:?}",
        last_use.diagnostics
    );
    let last_read = method(&last_use.rust, "Use", "read");
    assert!(
        last_read
            .split_once('{')
            .unwrap()
            .0
            .replace(' ', "")
            .contains("node:&std::rc::Rc<std::cell::RefCell<Node>>"),
        "{last_read}"
    );
    let last_probe = method(&last_use.rust, "Use", "probe");
    assert_eq!(
        call_arguments(last_probe, "Use::read"),
        vec!["(&first)"],
        "{last_probe}"
    );
    // A function-pointer assignment checks the chosen ABI in compiled output.
    common::cargo_test_generated_modules(
        "canonical_transport_observer",
        vec![
            common::GeneratedCrateModule {
                name: "fixture",
                rust: translated.rust,
                tests: r#"
#[test]
fn carrier_borrow_observes_both_alias_writes() {
    let _: fn(&std::rc::Rc<std::cell::RefCell<Node>>) -> i32 = Use::read;
    assert_eq!(Use::probe(), 4);
}
"#,
            },
            common::GeneratedCrateModule {
                name: "last_use",
                rust: last_use.rust,
                tests: r#"
#[test]
fn final_use_borrow_avoids_callee_reference_copy_and_preserves_value() {
    let _: fn(&std::rc::Rc<std::cell::RefCell<Node>>) -> i32 = Use::read;
    assert_eq!(Use::probe(), 2);
}
"#,
            },
        ],
    );
}

#[test]
fn retaining_returned_and_public_boundaries_keep_owned_canonical_handles() {
    let source = format!(
        r#"{NODE}
class Holder {{
    Node node;
    Holder(Node node) {{ this.node = node; }}
}}
class Use {{
    private static Node keep(Node node) {{ return node; }}
    private static Holder retain(Node node) {{ return new Holder(node); }}
    public static int read(Node node) {{ return node.value; }}
    static int probe() {{
        Node first = new Node(1);
        Node alias = first;
        alias.set(2);
        Node returned = keep(first);
        Holder retained = retain(first);
        int before = read(first);
        returned.set(3);
        return before + read(retained.node);
    }}
}}
"#
    );
    verify_java_probe(&source, 5);
    let translated = translate_source(&source);
    assert!(
        !translated.diagnostics.has_errors(),
        "{:?}",
        translated.diagnostics
    );
    for name in ["keep", "retain", "read"] {
        assert_owned_node_parameter(&translated.rust, name);
    }
    common::cargo_test_generated_modules(
        "canonical_transport_owned_boundaries",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translated.rust,
            tests: r#"
trait ProbeValue { fn value(self) -> i32; }
impl ProbeValue for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> ProbeValue for Result<i32, E> {
    fn value(self) -> i32 { self.unwrap() }
}
#[test]
fn retained_and_returned_handles_keep_the_same_object() {
    assert_eq!(Use::probe().value(), 5);
}
"#,
        }],
    );
}

#[test]
fn a_method_reference_pins_the_private_observer_contract() {
    let source = format!(
        r#"{NODE}
interface Reader {{ int read(Node node); }}
class Use {{
    private static int read(Node node) {{ return node.value; }}
    static int probe() {{
        Node first = new Node(1);
        Node alias = first;
        alias.set(2);
        Reader reader = Use::read;
        return read(first) + reader.read(first);
    }}
}}
"#
    );
    let translated = translate_source(&source);
    assert_owned_node_parameter(&translated.rust, "read");
}

#[test]
fn a_nonlocal_actual_without_complete_transport_evidence_pins_the_contract() {
    let source = format!(
        r#"{NODE}
class Holder {{
    Node node;
    Holder(Node node) {{ this.node = node; }}
}}
class Use {{
    private static int read(Node node) {{ return node.value; }}
    static int probe() {{
        Node first = new Node(1);
        Node alias = first;
        alias.set(2);
        Holder holder = new Holder(first);
        return read(first) + read(holder.node);
    }}
}}
"#
    );
    let translated = translate_source(&source);
    assert!(
        !translated.diagnostics.has_errors(),
        "{:?}",
        translated.diagnostics
    );
    assert_owned_node_parameter(&translated.rust, "read");
}

#[test]
fn nested_lambda_and_anonymous_body_calls_pin_the_observer_contract() {
    // Both callbacks capture a same-named outer handle. A recursive expression
    // walk must not classify their calls using the enclosing method's shape
    // and statement liveness. The bounded pilot must leave those calls unobserved
    // while retaining them in its complete expected-call inventory.
    let callbacks = [
        r#"
class Use {
    private static int read(Node node) { return node.value; }
    static int probe() {
        Node first = new Node(1);
        Node alias = first;
        alias.set(2);
        java.util.function.IntSupplier callback = () -> Use.read(first);
        return Use.read(first) + callback.getAsInt();
    }
}
"#,
        r#"
interface Reader { int get(); }
class Use {
    private static int read(Node node) { return node.value; }
    static int probe() {
        Node first = new Node(1);
        Node alias = first;
        alias.set(2);
        Reader callback = new Reader() {
            public int get() { return Use.read(first); }
        };
        return Use.read(first) + callback.get();
    }
}
"#,
    ];
    for callback in callbacks {
        let translated = translate_source(&format!("{NODE}\n{callback}"));
        assert_owned_node_parameter(&translated.rust, "read");
    }
}
