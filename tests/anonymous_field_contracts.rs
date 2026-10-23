#[path = "common/mod.rs"]
mod common;

#[test]
fn anonymous_field_scopes_preserve_java_overload_behavior() {
    common::verify_java_fixture_zero_contract(
        "anonymous_field_scopes",
        "AnonymousFieldScopeProbe",
        "tests/corpus/numbers/AnonymousFieldScopeProbe.java",
        include_str!("corpus/numbers/AnonymousFieldScopeProbe.java"),
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn scopes() { assert_eq!(AnonymousFieldScopeProbe::run().value(), 0); }
"#,
    );
}

#[test]
fn anonymous_helpers_bind_before_outer_methods_and_propagate_exceptions() {
    common::verify_java_fixture_zero_contract(
        "anonymous_helper_calls",
        "AnonymousHelperCallProbe",
        "tests/corpus/numbers/AnonymousHelperCallProbe.java",
        include_str!("corpus/numbers/AnonymousHelperCallProbe.java"),
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn helpers() { assert_eq!(AnonymousHelperCallProbe::run().value(), 0); }
"#,
    );
}
