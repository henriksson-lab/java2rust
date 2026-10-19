#[path = "common/mod.rs"]
mod common;

#[test]
fn checked_virtual_string_callbacks_preserve_results() {
    let translation =
        new_java2rust::translate_source(include_str!("fixtures/checked_virtual_strings.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "checked_virtual_strings",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
trait Text { fn text(self) -> String; }
impl Text for String { fn text(self) -> String { self } }
impl Text for Option<String> { fn text(self) -> String { self.unwrap() } }
impl<T: Text, E: std::fmt::Debug> Text for Result<T,E> { fn text(self) -> String { self.unwrap().text() } }
#[test] fn direct_nullable_and_throwing_virtual_results() {
    assert_eq!(CheckedVirtualStrings::direct(0).text(), "ok,ok");
    assert_eq!(CheckedVirtualStrings::direct(1).text(), "caught");
    assert_eq!(CheckedVirtualStrings::direct(2).text(), "ok,null");
    assert_eq!(CheckedVirtualStrings::direct(3).text(), "null receiver");
}
#[test] fn another_checked_call_does_not_nest_the_result() {
    assert_eq!(CheckedVirtualStrings::after_call(0).text(), "ok,ok");
    assert_eq!(CheckedVirtualStrings::after_call(1).text(), "caught");
    assert_eq!(CheckedVirtualStrings::after_call(2).text(), "ok,null");
    assert_eq!(CheckedVirtualStrings::after_call(3).text(), "bad argument");
}
#[test] fn method_references_share_the_same_virtual_result_boundary() {
    assert_eq!(CheckedVirtualStrings::reference(0).text(), "ok,ok");
    assert_eq!(CheckedVirtualStrings::reference(1).text(), "caught");
    assert_eq!(CheckedVirtualStrings::reference(2).text(), "ok,null");
    assert_eq!(CheckedVirtualStrings::reference(3).text(), "null receiver");
}
"#,
        }],
    );
}
