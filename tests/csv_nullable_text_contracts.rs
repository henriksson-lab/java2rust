#[path = "common/mod.rs"]
mod common;

#[test]
fn nullable_text_results_and_refined_casts_use_destination_contracts() {
    common::verify_java_fixture_zero_contract(
        "csv_nullable_text",
        "NullableTextContracts",
        "tests/corpus/csv/NullableTextContracts.java",
        include_str!("corpus/csv/NullableTextContracts.java"),
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn copy_contract() { assert_eq!(NullableTextContracts::run().value(), 0); }
struct FormatText<'a, T: ?Sized>(&'a T);
impl<T: JavaTextView + ?Sized> std::fmt::Display for FormatText<'_, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        java_native_format_text(f, self.0)
    }
}
#[test] fn formatting_borrows_native_logical_and_utf16_text_without_changing_java_units() {
    let payload = JavaUtf16::from_units(vec![0x61, 0xd83d, 0xde80, 0xd800]);
    let logical = JavaString::from_payload(payload.clone());
    let original = logical.units().as_ptr();
    assert_eq!(format!("{}", FormatText(&String::from("native"))), "native");
    assert_eq!(format!("{}", FormatText(&payload)), "a🚀�");
    assert_eq!(format!("{}", FormatText(&logical)), "a🚀�");
    assert_eq!(logical.units(), payload.units());
    assert_eq!(logical.units().as_ptr(), original);
    assert_eq!(logical.units().last(), Some(&0xd800));
}
"#,
    );
}
