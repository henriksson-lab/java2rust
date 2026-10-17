use new_java2rust::facts::{
    ExternalFieldFact, ExternalFieldKind, FactProvenance, FactSet, FieldKey,
};
use new_java2rust::java_ir::JavaType;
use new_java2rust::translate_source_with_facts;

#[path = "common/mod.rs"]
mod common;
use common::{GeneratedCrateModule, cargo_test_generated_modules};

#[test]
fn external_enum_default_to_string_uses_constant_name() {
    let mut facts = FactSet::default();
    for variant in ["READY", "DONE"] {
        facts.external_field_facts.push(ExternalFieldFact {
            field: FieldKey::named("Status", variant),
            field_type: Some(JavaType::Named {
                name: "Status".to_string(),
                args: Vec::new(),
            }),
            field_kind: ExternalFieldKind::EnumVariant,
            provenance: FactProvenance::UserOverride,
        });
    }
    let translation = translate_source_with_facts(
        r#"
class Use {
    static String ready() { return Status.READY.toString(); }
    static String done() { return Status.DONE.toString(); }
}
"#,
        &facts,
    );
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    cargo_test_generated_modules(
        "external_enum_default_display",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
#[test]
fn enum_names_are_not_unmodelled_stubs() {
    assert_eq!(Use::ready(), "READY");
    assert_eq!(Use::done(), "DONE");
    assert_eq!(Status::READY.to_string(), "READY");
    assert_eq!(Status::DONE.ordinal(), 1);
}
"#,
        }],
    );
}
