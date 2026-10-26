#[path = "common/mod.rs"]
mod common;

#[test]
fn specialized_payloads_retain_logical_generics_without_spurious_clone_bounds() {
    common::verify_java_fixture_zero_contract(
        "generic_hierarchy_marker",
        "GenericHierarchyMarkerProbe",
        "tests/fixtures/GenericHierarchyMarkerProbe.java",
        include_str!("fixtures/GenericHierarchyMarkerProbe.java"),
        r#"
trait Exit { fn value(self) -> i32; }
impl Exit for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Exit for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn aliases_and_dispatch() { assert_eq!(GenericHierarchyMarkerProbe::run().value(), 0); }
#[test]
fn clone_copies_payload_handles_without_cloning_the_logical_type_parameter() {
    struct NotClone;
    let _copy: fn(&MarkerRootValue<NotClone>) -> MarkerRootValue<NotClone> = std::clone::Clone::clone;
}
"#,
    );
}
