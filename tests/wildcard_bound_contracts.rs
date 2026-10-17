use new_java2rust::translate_source;

#[path = "common/mod.rs"]
mod common;

#[test]
fn source_class_bound_methods_bind_their_reified_wildcard_inputs() {
    let translation = translate_source(
        r#"
interface Container<T> { int size(); }
class Base {
    int inspect(Container<?> input) { return input.size(); }
}
class Use<H extends Base> {
    int invoke(H base, Container<?> input) { return base.inspect(input); }
}
"#,
    );
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_check_generated_modules(
        "wildcard_bound_contracts",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn source_class_bound_methods_preserve_bounded_wildcard_capabilities() {
    let translation = translate_source(
        r#"
interface Producer<T> { T read(); }
class Item { int id() { return 7; } }
class Base {
    int inspect(Producer<? extends Item> input) { return input.read().id(); }
}
class Use<H extends Base> {
    int invoke(H base, Producer<? extends Item> input) { return base.inspect(input); }
}
"#,
    );
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_check_generated_modules(
        "bounded_wildcard_contracts",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}
