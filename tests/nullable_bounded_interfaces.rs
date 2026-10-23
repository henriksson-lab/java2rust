#[path = "common/mod.rs"]
mod common;

#[test]
fn nullable_bounded_borrows_preserve_null_and_generic_arguments_without_clone() {
    let translation =
        new_java2rust::translate_source(include_str!("fixtures/nullable_bounded_interfaces.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "nullable_bounded_interfaces",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
struct NoCloneNetwork;
impl Network<String, i32> for NoCloneNetwork {
    fn size(&self) -> i32 { 3 }
    fn as_any(&self) -> &dyn std::any::Any { self }
    fn clone_box(&self) -> Box<dyn Network<String, i32>> { panic!("forwarding must borrow, not clone") }
    fn clone_any(&self) -> Box<dyn std::any::Any> { panic!("forwarding must borrow, not erase-clone") }
}
trait Value { fn value(self) -> bool; }
impl Value for bool { fn value(self) -> bool { self } }
impl<E: std::fmt::Debug> Value for Result<bool, E> { fn value(self) -> bool { self.unwrap() } }
#[test] fn forwarding_keeps_optional_payload_and_reuses_the_borrow() {
    let value = NoCloneNetwork;
    assert!(Forwarders::forward::<String, i32, NoCloneNetwork>(Some(&value), &value).value());
    assert!(!Forwarders::forward::<String, i32, NoCloneNetwork>(None, &value).value());
    assert!(Forwarders::forward::<String, i32, NoCloneNetwork>(Some(&value), &value).value());
    let holder = NetworkHolder::<String, i32, NoCloneNetwork>::new(NoCloneNetwork);
    assert!(holder.forward(&value).value());
    assert!(holder.forward(&value).value());
}
"#,
        }],
    );
}
