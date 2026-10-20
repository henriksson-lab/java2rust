#[path = "common/mod.rs"]
mod common;

#[test]
fn abstract_declarations_keep_concrete_virtual_bodies_static_helpers_and_super_calls() {
    let source = include_str!("fixtures/abstract_declaration_contracts.java");
    let resolved = new_java2rust::resolve::resolve_program(
        &new_java2rust::parser::parse_java_source(source).program,
        new_java2rust::diagnostics::DiagnosticBag::default(),
    );
    let translation = new_java2rust::translate_source(source);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    assert!(
        !translation
            .rust
            .contains("impl RestoredDrawContract for DrawDeclaration")
    );
    assert!(
        !translation
            .rust
            .contains("pub fn next_with_outer(__java_self: std::rc::Rc<LongDrawDeclaration>")
    );
    assert!(translation.rust.contains("fn check_bound("));
    assert!(translation.rust.contains("fn fill_into("));
    assert!(translation.rust.contains("let __java_array_length"));
    common::cargo_verify_generated_modules_with_edition(
        "abstract_declaration_contracts",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn concrete_dispatch_and_replay() { assert_eq!(AbstractDeclarationContracts::run().value(), 0); }
"#,
        }],
        "test",
        "2021",
    );
}
