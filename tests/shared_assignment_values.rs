#[path = "common/mod.rs"]
mod common;

#[test]
fn shared_assignment_expressions_use_write_places_and_return_the_rhs() {
    let translation =
        new_java2rust::translate_source(include_str!("fixtures/shared_assignment_values.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    assert!(
        translation
            .rust
            .contains("std::sync::Arc<std::sync::Mutex<SharedAssignmentBox>>")
    );
    common::cargo_test_generated_modules(
        "shared_assignment_values",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn java_oracle_completes_without_holding_assignment_guards() {
    let (send, receive) = std::sync::mpsc::channel();
    std::thread::spawn(move || send.send(SharedAssignmentValues::run().value()).unwrap());
    assert_eq!(receive.recv_timeout(std::time::Duration::from_secs(5)).expect("assignment retained a shared field guard"), 0);
}
"#,
        }],
    );
}
