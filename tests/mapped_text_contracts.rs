#[path = "common/mod.rs"]
mod common;

#[test]
fn pack_text_abi_defaults_are_explicit_and_invalid_encodings_are_rejected() {
    use new_java2rust::facts::ExternalStringAbi;
    for (metadata, expected) in [
        ("", ExternalStringAbi::Utf8),
        (" string-abi=utf8", ExternalStringAbi::Utf8),
        (" string-abi=utf16", ExternalStringAbi::Utf16),
    ] {
        let manifest = new_java2rust::external_packs::parse_external_pack_manifest(&format!(
            "external-pack fixture.text fixture.Text rust-crate-mapping mapped target=native{metadata}\nexternal-pack-constructor fixture.text fixture.Text 0 mapped rust=new\n"
        ));
        assert!(
            !manifest.diagnostics().has_errors(),
            "{:?}",
            manifest.diagnostics()
        );
        let facts = manifest.facts().clone();
        assert_eq!(
            facts
                .external_library_pack_for_type("fixture.Text")
                .unwrap()
                .string_abi,
            expected
        );
        assert_eq!(
            facts
                .external_library_pack_for_type("Text")
                .unwrap()
                .string_abi,
            expected
        );
    }
    let invalid = new_java2rust::external_packs::parse_external_pack_manifest(
        "external-pack fixture.text fixture.Text rust-crate-mapping mapped target=native string-abi=lossy\n",
    );
    assert!(invalid.diagnostics().has_errors());
}

#[test]
fn utf16_foreign_constructor_and_method_slots_use_the_declared_pack_abi() {
    let manifest = new_java2rust::external_packs::parse_external_pack_manifest(
        "external-pack fixture.units fixture.ForeignUnits rust-crate-mapping mapped target=crate::native_runtime string-abi=utf16\n\
         external-pack-constructor fixture.units fixture.ForeignUnits 2 mapped rust-type=ForeignUnits rust=new\n\
         external-pack-api fixture.units fixture.ForeignUnits echo mapped getter read-only definitely-non-null params=String return=String rust=echo\n\
         external-pack-api fixture.units fixture.ForeignUnits nullable mapped getter read-only maybe-null params=String return=String rust=nullable\n\
         external-pack-argument fixture.units fixture.ForeignUnits nullable 0 maybe-null read-only no-alias type=String\n\
         external-pack-api fixture.units fixture.ForeignUnits checked mapped getter read-only definitely-non-null params=String,int return=String rust=checked rust-result=true throws=java.lang.IllegalStateException\n\
         external-pack-api fixture.units fixture.ForeignUnits checkedNullable mapped getter read-only maybe-null params=String,int return=String rust=checked_nullable rust-result=true throws=java.lang.IllegalStateException\n\
         external-pack-argument fixture.units fixture.ForeignUnits checkedNullable 0 maybe-null read-only no-alias type=String\n",
    );
    assert!(
        !manifest.diagnostics().has_errors(),
        "{:?}",
        manifest.diagnostics()
    );
    let translation = new_java2rust::translate_source_with_facts(
        r#"
import fixture.ForeignUnits;
class Probe {
    static int calls;
    static int mark() { calls++; return calls; }
    static int run() {
        String raw = "x\ud800😀";
        ForeignUnits value = new ForeignUnits(raw, mark());
        if (!value.echo(raw).equals(raw) || calls != 1 || raw.charAt(1) != '\ud800') return 1;
        if (value.nullable(null) != null || !value.nullable(raw).equals(raw)) return 2;
        if (!value.checked(raw, 0).equals(raw)) return 3;
        if (value.checkedNullable(null, 0) != null || !value.checkedNullable(raw, 0).equals(raw)) return 4;
        try { value.checked(raw, 1); return 5; }
        catch (IllegalStateException error) { if (!error.getMessage().equals("failure\ud800")) return 6; }
        try { new ForeignUnits((String)null, mark()); return 7; }
        catch (NullPointerException expected) { if (calls != 2) return 8; }
        return 0;
    }
}
"#,
        manifest.facts(),
    );
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules("utf16_foreign_text_slots", vec![
        common::GeneratedCrateModule { name: "native_runtime", rust: r#"
use crate::fixture::{JavaException, JavaString};
#[derive(Clone)] pub struct ForeignUnits;
impl ForeignUnits { pub fn new(_: Vec<u16>, _: i32) -> Self { Self } }
pub fn echo(_: ForeignUnits, value: Vec<u16>) -> Vec<u16> { value }
pub fn nullable(_: ForeignUnits, value: Option<Vec<u16>>) -> Option<Vec<u16>> { value }
pub fn checked(_: ForeignUnits, value: Vec<u16>, mode: i32) -> Result<Vec<u16>, JavaException> {
    if mode == 0 { Ok(value) } else { Err(JavaException::new("IllegalStateException", JavaString::from_units(vec![102,97,105,108,117,114,101,0xd800]))) }
}
pub fn checked_nullable(_: ForeignUnits, value: Option<Vec<u16>>, _: i32) -> Result<Option<Vec<u16>>, JavaException> { Ok(value) }
"#.into(), tests: "" },
        common::GeneratedCrateModule { name: "fixture", rust: translation.rust, tests: r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn slots() { assert_eq!(Probe::run().value(), 0); }
"# }
    ]);
}

#[test]
fn foreign_text_results_preserve_plain_nullable_checked_and_checked_nullable_abis() {
    let manifest = new_java2rust::external_packs::parse_external_pack_manifest(
        "external-pack fixture.text fixture.ForeignText rust-crate-mapping mapped target=crate::native_runtime\n\
         external-pack-constructor fixture.text fixture.ForeignText 0 mapped rust-type=ForeignText rust=new\n\
         external-pack-api fixture.text fixture.ForeignText plainText mapped getter read-only definitely-non-null args=1 return=String rust=fetch_plain\n\
         external-pack-api fixture.text fixture.ForeignText nullableText mapped getter read-only maybe-null args=1 return=String rust=fetch_nullable\n\
         external-pack-api fixture.text fixture.ForeignText checkedText mapped getter read-only definitely-non-null args=1 return=String rust=fetch_checked rust-result=true throws=java.lang.IllegalStateException\n\
         external-pack-api fixture.text fixture.ForeignText checkedNullableText mapped getter read-only maybe-null args=1 return=String rust=fetch_checked_nullable rust-result=true throws=java.lang.IllegalStateException\n",
    );
    assert!(
        !manifest.diagnostics().has_errors(),
        "{:?}",
        manifest.diagnostics()
    );
    let translation = new_java2rust::translate_source_with_facts(
        r#"
import fixture.ForeignText;
class Probe {
    static String plain(int mode) { return new ForeignText().plainText(mode); }
    static String nullable(int mode) { return new ForeignText().nullableText(mode); }
    static String checked(int mode) { return new ForeignText().checkedText(mode); }
    static String checkedNullable(int mode) { return new ForeignText().checkedNullableText(mode); }
}
"#,
        manifest.facts(),
    );
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules("mapped_text_result_layers", vec![
        common::GeneratedCrateModule {
            name: "native_runtime",
            rust: r#"
use crate::fixture::JavaException;
#[derive(Clone, Default)] pub struct ForeignText;
impl ForeignText { pub fn new() -> Self { Self } }
pub fn fetch_plain(_: ForeignText, _: i32) -> String { "native😀".into() }
pub fn fetch_nullable(_: ForeignText, mode: i32) -> Option<String> {
    (mode != 1).then(|| "native😀".into())
}
pub fn fetch_checked(value: ForeignText, mode: i32) -> Result<String, JavaException> {
    if mode == 2 { Err(JavaException::new("IllegalStateException", "native failure")) }
    else { Ok(fetch_plain(value, mode)) }
}
pub fn fetch_checked_nullable(value: ForeignText, mode: i32) -> Result<Option<String>, JavaException> {
    if mode == 2 { Err(JavaException::new("IllegalStateException", "native failure")) }
    else { Ok(fetch_nullable(value, mode)) }
}
"#.into(),
            tests: "",
        },
        common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
trait TextResult { fn text(self) -> Result<Option<JavaString>, JavaException>; }
impl TextResult for JavaString { fn text(self) -> Result<Option<JavaString>, JavaException> { Ok(Some(self)) } }
impl TextResult for Option<JavaString> { fn text(self) -> Result<Option<JavaString>, JavaException> { Ok(self) } }
impl TextResult for Result<JavaString, JavaException> { fn text(self) -> Result<Option<JavaString>, JavaException> { self.map(Some) } }
impl TextResult for Result<Option<JavaString>, JavaException> { fn text(self) -> Result<Option<JavaString>, JavaException> { self } }
fn units(value: impl TextResult) -> Option<Vec<u16>> { value.text().unwrap().map(|text| text.units().to_vec()) }
#[test] fn result_layers() {
    let expected = Some("native😀".encode_utf16().collect::<Vec<_>>());
    assert_eq!(units(Probe::plain(0)), expected);
    assert_eq!(units(Probe::nullable(0)), expected);
    assert_eq!(units(Probe::checked(0)), expected);
    assert_eq!(units(Probe::checked_nullable(0)), expected);
    assert_eq!(units(Probe::nullable(1)), None);
    assert_eq!(units(Probe::checked_nullable(1)), None);
    for error in [Probe::checked(2).text().unwrap_err(), Probe::checked_nullable(2).text().unwrap_err()] {
        assert_eq!(error.class_name(), "IllegalStateException");
        assert_eq!(error.message().units(), "native failure".encode_utf16().collect::<Vec<_>>());
    }
}
"#,
        },
    ]);
}
