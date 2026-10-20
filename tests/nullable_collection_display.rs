#[path = "common/mod.rs"]
mod common;

#[test]
fn nullable_native_collections_display_values_and_null_without_consuming_storage() {
    let translation =
        new_java2rust::translate_source(include_str!("fixtures/nullable_collection_display.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "nullable_collection_display",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
trait Value { type Output; fn value(self) -> Self::Output; }
impl Value for String { type Output = String; fn value(self) -> String { self } }
impl Value for i32 { type Output = i32; fn value(self) -> i32 { self } }
impl<T, E: std::fmt::Debug> Value for Result<T, E> { type Output = T; fn value(self) -> T { self.unwrap() } }
#[test]
fn display_contract() {
    assert_eq!(NullableCollectionDisplay::probe(false).value(), "list=null;set=null;map=null;<null>;A=null;B=null");
    assert_eq!(NullableCollectionDisplay::probe(true).value(), "list=[3, 1];set=[3, 1];map={k=5};<[3, 1]>;A=[3, 1];B=[3, 1]");
    assert_eq!(NullableCollectionDisplay::from_return(false).value(), "R=null");
    assert_eq!(NullableCollectionDisplay::from_return(true).value(), "R=[3, 1]");
    assert_eq!(NullableCollectionDisplay::from_field(false).value(), "F=null");
    assert_eq!(NullableCollectionDisplay::from_field(true).value(), "F=[3, 1]");
    assert_eq!(NullableCollectionDisplay::from_map(false).value(), "M=null");
    assert_eq!(NullableCollectionDisplay::from_map(true).value(), "M=[3, 1]");
    assert_eq!(NullableCollectionDisplay::direct_null().value(), 8);
}
"#,
        }],
    );
}

#[test]
fn source_collection_lookalike_display_is_not_native_collection_display() {
    let translation = new_java2rust::translate_source(
        r#"
class List {
    boolean nullText;
    List(boolean nullText) { this.nullText = nullText; }
    public String toString() { if (nullText) return null; return "source"; }
}
class SourceNullableCollectionDisplay {
    static String render(List value) { return "S=" + value; }
    static String probe(boolean present) {
        List value = null;
        if (present) value = new List(false);
        return render(value);
    }
    static String nullText() { return render(new List(true)); }
}
"#,
    );
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "source_nullable_collection_display",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
#[test]
fn source_display_contract() {
    assert_eq!(SourceNullableCollectionDisplay::probe(false), "S=null");
    assert_eq!(SourceNullableCollectionDisplay::probe(true), "S=source");
    assert_eq!(SourceNullableCollectionDisplay::null_text(), "S=null");
}
"#,
        }],
    );
}
