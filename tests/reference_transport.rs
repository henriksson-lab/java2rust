use new_java2rust::translate_source;
#[path = "common/mod.rs"]
mod common;

#[test]
fn canonical_reference_transfers_move_final_uses_and_preserve_repeated_aliases() {
    let source = include_str!("fixtures/ReferenceTransportProbe.java");
    common::verify_java_sources_zero_contract(
        "ReferenceTransportProbe",
        &[("tests/fixtures/ReferenceTransportProbe.java", source)],
    );
    let translation = translate_source(source);
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    let transfer = translation
        .rust
        .split("pub fn transfer()")
        .nth(1)
        .expect("transfer method")
        .split("pub fn store()")
        .next()
        .unwrap();
    let assignment = transfer
        .lines()
        .find(|line| line.contains("let result:"))
        .expect("final local transfer");
    assert!(
        assignment.trim_end().ends_with(" = value;"),
        "final transfer copied an already dead handle: {assignment}"
    );
    let store = translation
        .rust
        .split("pub fn store()")
        .nth(1)
        .expect("store method")
        .split("pub fn same(")
        .next()
        .unwrap();
    assert!(
        store.contains("Self::__set_static_kept(Some(value));"),
        "final store must move its handle after evaluation: {store}"
    );
    common::cargo_test_generated_modules(
        "reference_transport",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
trait Exit { fn value(self) -> i32; }
impl Exit for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Exit for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn aliases_and_repetition() { assert_eq!(ReferenceTransportProbe::run().value(), 0); }
#[test] fn single_constructor_store_does_not_require_payload_clone() {
    struct NotClone;
    let owner = TransportSingle::new(NotClone);
    assert!(owner.value.is_some());
}
"#,
        }],
    );
}
