#[path = "common/mod.rs"]
mod common;
#[test]
fn fresh_list_copies_lift_nullable_interface_storage_and_preserve_element_identity() {
    common::verify_java_fixture_zero_contract(
        "interface_list_copy",
        "InterfaceListCopyProbe",
        "tests/fixtures/InterfaceListCopyProbe.java",
        include_str!("fixtures/InterfaceListCopyProbe.java"),
        r#"
trait Exit { fn value(self) -> i32; }
impl Exit for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Exit for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn copy_and_nullable_elements() { assert_eq!(InterfaceListCopyProbe::run().value(), 0); }
"#,
    );
}
