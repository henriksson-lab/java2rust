#[path = "common/mod.rs"]
mod common;

#[test]
fn string_lengths_use_utf16_units_across_calls_callbacks_and_live_sequences() {
    common::verify_java_fixture_zero_contract(
        "string_length_utf16_contracts",
        "StringLengthProbe",
        "tests/corpus/numbers/StringLengthProbe.java",
        include_str!("corpus/numbers/StringLengthProbe.java"),
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn lengths() { assert_eq!(StringLengthProbe::run().value(), 0); }
"#,
    );
}

#[test]
fn string_search_uses_utf16_offsets_for_scalar_and_identity_inputs() {
    common::verify_java_fixture_zero_contract(
        "string_search_utf16_contracts",
        "StringSearchProbe",
        "tests/corpus/numbers/StringSearchProbe.java",
        include_str!("corpus/numbers/StringSearchProbe.java"),
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn search() { assert_eq!(StringSearchProbe::run().value(), 0); }
"#,
    );
    common::verify_java_fixture_zero_contract(
        "string_search_scalar_utf16_contracts",
        "StringSearchScalarProbe",
        "tests/corpus/numbers/StringSearchScalarProbe.java",
        include_str!("corpus/numbers/StringSearchScalarProbe.java"),
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn search() { assert_eq!(StringSearchScalarProbe::run().value(), 0); }
"#,
    );
}

#[test]
fn source_only_identity_views_emit_their_complete_runtime_dependencies() {
    let translated = new_java2rust::translate_source(
        r#"
class SourceOnlyStringViews {
    static boolean same(String left, String right) { return left == right; }
    static int size(String value) { return value.length(); }
    static boolean run() { String value = new String("x"); return same(value,value) && size(value) == 1; }
}
"#,
    );
    assert!(
        !translated.diagnostics.has_errors(),
        "{:?}",
        translated.diagnostics
    );
    common::cargo_test_generated_modules(
        "source_only_string_views",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translated.rust,
            tests: r#"
trait Value { fn value(self) -> bool; }
impl Value for bool { fn value(self) -> bool { self } }
impl<E:std::fmt::Debug> Value for Result<bool,E> { fn value(self) -> bool { self.unwrap() } }
#[test] fn references() { assert!(SourceOnlyStringViews::run().value()); }
#[test] fn erased_logical_text_preserves_units_and_nullable_results() {
    let high = JavaString::from_units(vec![0xd800]);
    let low = JavaString::from_units(vec![0xdc00]);
    let direct = java_erased_object_to_string(&high);
    assert!(direct.java_identity_eq(&high));
    assert_eq!(direct.units(), &[0xd800]);
    let values = vec![Some(high.clone()), None, Some(low.clone())];
    assert_eq!(java_erased_object_to_string(&values).units(),
        &[91, 0xd800, 44, 32, 110, 117, 108, 108, 44, 32, 0xdc00, 93]);
    assert_eq!(java_erased_object_to_string(&Some(high.clone())).units(),
        &[79, 112, 116, 105, 111, 110, 97, 108, 91, 0xd800, 93]);
    assert_eq!(java_erased_object_to_string(&JavaCodeUnit::from_u16(0xdc00)).units(), &[0xdc00]);
    let null: Option<JavaString> = None;
    assert_eq!(java_erased_display_unwrap(null).units(), &[110, 117, 108, 108]);
    let nullable: Result<Option<JavaString>, JavaException> = Ok(Some(high.clone()));
    assert!(java_erased_display_unwrap(nullable).java_identity_eq(&high));
    let failed: Result<Option<JavaString>, JavaException> =
        Err(JavaException::new("IllegalArgumentException", "failure".to_string()));
    assert!(failed.into_java_erased_display_result().is_err());
    assert!(!java_erased_any_equals(&high, &"?".to_string()));
    assert!(java_erased_any_equals(&JavaString::fresh("é"), &"é".to_string()));
    assert_eq!(java_arrays_deep_to_string(vec![high.clone(), low.clone()]).units(),
        &[91, 0xd800, 44, 32, 0xdc00, 93]);
    let events = std::cell::RefCell::new(Vec::new());
    let result = java_map_iter_display_result([(1, 2), (3, 4)],
        |key| { events.borrow_mut().push(key); Ok(high.clone()) },
        |value| {
            events.borrow_mut().push(value);
            if value == 4 { Err(JavaException::new("IllegalStateException", "stop".to_string())) }
            else { Ok(low.clone()) }
        });
    assert!(result.is_err());
    assert_eq!(*events.borrow(), [1, 2, 3, 4]);
}
"#,
        }],
    );
}

#[test]
fn string_reference_views_share_owned_nullable_and_array_contracts() {
    let source = include_str!("corpus/numbers/StringReferenceViewProbe.java");
    common::verify_java_fixture_zero_contract(
        "string_reference_view_contracts",
        "StringReferenceViewProbe",
        "tests/corpus/numbers/StringReferenceViewProbe.java",
        source,
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn references() { assert_eq!(StringReferenceViewProbe::run().value(), 0); }
"#,
    );
    let translated = new_java2rust::translate_source(source);
    let inspect = translated
        .rust
        .lines()
        .find(|line| line.contains("pub fn inspect("))
        .unwrap();
    assert!(
        inspect.contains("JavaString"),
        "borrowed formal must retain the carrier: {inspect}"
    );
    assert!(
        inspect.contains("&JavaString"),
        "read-only parameter should use a carrier borrow: {inspect}"
    );
    assert!(!inspect.contains("&str"), "{inspect}");
}
