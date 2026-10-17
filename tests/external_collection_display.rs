#[path = "common/mod.rs"]
mod common;

#[test]
fn external_collection_display_metadata_uses_native_instantiated_carriers() {
    use new_java2rust::facts::{
        ExternalApiEffectFact, ExternalApiRole, FactProvenance, FactSet, NullabilityState,
        ReceiverMutabilityFact,
    };
    let mut facts = FactSet::default();
    for owner in [
        "List",
        "Set",
        "Map",
        "SortedSet",
        "SortedMap",
        "java.util.List",
        "java.util.Set",
        "java.util.Map",
    ] {
        facts.external_api_effect_facts.push(ExternalApiEffectFact {
            method_name: "toString".to_string(),
            receiver_type: Some(owner.to_string()),
            arg_count: Some(0),
            parameter_patterns: None,
            role: ExternalApiRole::Getter,
            receiver_mutability: ReceiverMutabilityFact::ReadOnly,
            return_type: Some(new_java2rust::java_ir::JavaType::String),
            return_nullability: NullabilityState::DefinitelyNonNull,
            thrown: vec![],
            provenance: FactProvenance::ExternalLibraryPack,
        });
    }
    let translation = new_java2rust::translate_source_with_facts(
        r#"
import java.util.List;
import java.util.Set;
import java.util.Map;
import java.util.SortedSet;
import java.util.SortedMap;
class Use {
    List<String> list;
    Set<String> set;
    Map<String, String> map;
    SortedSet<String> sortedSet;
    SortedMap<String, String> sortedMap;
    static String describe(Object value) { return value.toString(); }
}
"#,
        &facts,
    );
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "external_collection_display",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
#[test]
fn collection_interfaces_display_through_their_actual_native_carriers() {
    assert_eq!(Use::describe(Box::new(vec!["only".to_string()])), "[only]");
    let set = std::collections::HashSet::from(["only".to_string()]);
    assert_eq!(Use::describe(Box::new(set)), "[only]");
    let map = std::collections::HashMap::from([("key".to_string(), "value".to_string())]);
    assert_eq!(Use::describe(Box::new(map)), "{key=value}");
    let sorted_set = std::collections::BTreeSet::from(["only".to_string()]);
    assert_eq!(Use::describe(Box::new(sorted_set)), "[only]");
    let sorted_map = std::collections::BTreeMap::from([("key".to_string(), "value".to_string())]);
    assert_eq!(Use::describe(Box::new(sorted_map)), "{key=value}");
}
"#,
        }],
    );
}
