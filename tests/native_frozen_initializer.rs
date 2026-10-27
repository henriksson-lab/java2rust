#[path = "common/mod.rs"]
mod common;

#[test]
fn factory_array_initializer_uses_frozen_recipe_and_preserves_alias_mutation() {
    let source = include_str!("fixtures/NativeInitializerProbe.java");
    let translated = new_java2rust::translate_source(source);
    assert!(
        !translated.diagnostics.has_errors(),
        "{:?}",
        translated.diagnostics
    );
    assert!(
        translated.rust.contains("__java_frozen_initializer_value"),
        "factory-field path did not consume frozen conversion: {}",
        translated.rust
    );
    common::verify_java_fixture_zero_contract(
        "native_frozen_initializer",
        "NativeInitializerProbe",
        "tests/fixtures/NativeInitializerProbe.java",
        source,
        r#"
trait Exit { fn value(self) -> i32; }
impl Exit for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Exit for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn retained_initializer_array_identity() { assert_eq!(NativeInitializerProbe::run().value(), 0); }
"#,
    );
}
