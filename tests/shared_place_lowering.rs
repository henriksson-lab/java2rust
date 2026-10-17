use new_java2rust::translate_source;

#[path = "common/mod.rs"]
mod common;
use common::{GeneratedCrateModule, cargo_check_generated_modules};

#[test]
fn shared_field_prefix_and_postfix_updates_use_a_single_mutable_place() {
    let translation = translate_source(
        r#"
import java.util.ArrayList;
import java.util.List;
class UpdateOwner<T> {
    class Node { int visited; byte small; }
    List<Node> nodes = new ArrayList<>();
    int post(Node node) { return node.visited++; }
    int pre(Node node) { return ++node.visited; }
    byte narrow(Node node) { return ++node.small; }
}
"#,
    );
    assert!(
        translation.rust.contains("__java_update_place"),
        "{}",
        relevant_output(&translation.rust)
    );
    assert!(
        !translation
            .rust
            .contains("__java_shared_field_guard.visited } +="),
        "{}",
        relevant_output(&translation.rust)
    );
    cargo_check_generated_modules(
        "shared_field_updates",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn consumer_expression_assignment_uses_shared_field_write_place() {
    let translation = translate_source(
        r#"
import java.util.ArrayList;
import java.util.List;
class LoopOwner<T> {
    class Node { int visited; }
    List<Node> nodes = new ArrayList<>();
    void clear() { nodes.forEach(node -> node.visited = 0); }
}
"#,
    );
    assert!(
        translation.rust.contains("borrow_mut().visited = 0"),
        "{}",
        relevant_output(&translation.rust)
    );
    assert!(
        !translation
            .rust
            .contains("__java_shared_field_guard.visited } ="),
        "{}",
        relevant_output(&translation.rust)
    );
    cargo_check_generated_modules(
        "consumer_shared_field_write",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

fn relevant_output(rust: &str) -> String {
    rust.lines()
        .filter(|line| {
            line.contains("node") || line.contains("visited") || line.contains("update_place")
        })
        .collect::<Vec<_>>()
        .join("\n")
}
