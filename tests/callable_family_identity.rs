#[path = "common/mod.rs"]
mod common;

#[test]
fn inherited_abstract_calls_keep_their_java_identity() {
    let translation =
        new_java2rust::translate_source(include_str!("fixtures/callable_family_identity.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "callable_family_identity",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
trait JavaIntOutcome { fn value(self) -> i32; }
impl JavaIntOutcome for i32 { fn value(self) -> i32 { self } }
impl JavaIntOutcome for Result<i32, JavaException> {
    fn value(self) -> i32 { self.expect("unexpected uncaught Java exception") }
}
#[test]
fn inherited_calls_do_not_select_a_longer_member_name() {
    assert_eq!(CallableFamilyIdentity::probe().value(), 8);
}
"#,
        }],
    );
}
