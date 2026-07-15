use std::fs;
use std::path::Path;

use new_java2rust::facts::{
    ControlFlowFactKind, DispatchRequirementKind, ExceptionKind, ExternalApiEffectFact,
    ExternalApiRole, FactProvenance, NullabilityState, ReceiverMutabilityFact, RecursionKind,
    TrivialAccessorKind, UnresolvedTargetFact, UnresolvedTargetKind,
};
use new_java2rust::java_ir::{JavaType, PrimitiveType};
use new_java2rust::object_model::plan_object_model;
use new_java2rust::rewrite::{
    AssignmentConditionNormalizedShape, ControlFlowRewritePlan, NullTestKind,
    TailRecursiveNormalizedShape,
};
use new_java2rust::slot_shape::OwnershipShape;
use new_java2rust::stdlib::{
    canonical_external_receiver_type_key, external_api_method_policy, external_field_provenance,
};
use new_java2rust::types::optional_java_type;
use new_java2rust::usage::{FieldUse, UsageMethodKey};
use new_java2rust::{
    emit, exception_plan, external_facts, lower, parser, representation, resolve,
    semantic_analysis, signature, translate_source, translate_source_with_facts,
};

fn idiom_gap(path: &str) -> String {
    fs::read_to_string(Path::new("examples/idiom-gaps").join(path)).unwrap()
}

fn java_fixture(path: &str) -> String {
    fs::read_to_string(Path::new("examples/java").join(path)).unwrap()
}

fn analyze(source: &str) -> semantic_analysis::SemanticFacts {
    let parsed = parser::parse_java_source(source);
    let resolved = resolve::resolve_program(&parsed.program, parsed.diagnostics);
    semantic_analysis::analyze_program(
        &resolved.program,
        &resolved.facts,
        &new_java2rust::facts::FactSet::default(),
    )
}

#[test]
fn records_dispatch_requirements_for_instanceof_and_casts() {
    let source = r#"
interface Animal {}
class Dog implements Animal {}
class Use {
    boolean isDog(Animal animal) {
        return animal instanceof Dog;
    }
    Dog asDog(Animal animal) {
        return (Dog) animal;
    }
}
"#;
    let parsed = parser::parse_java_source(source);
    let resolved = resolve::resolve_program(&parsed.program, parsed.diagnostics);
    let semantic = semantic_analysis::analyze_program(
        &resolved.program,
        &resolved.facts,
        &new_java2rust::facts::FactSet::default(),
    );

    assert_eq!(
        semantic.facts.dispatch_requirements_for("Dog"),
        vec![
            DispatchRequirementKind::NeedsInstanceof,
            DispatchRequirementKind::NeedsDowncast,
        ]
    );
    assert_eq!(
        semantic.facts.dispatch_requirement_facts_for("Dog").len(),
        2
    );

    let object_plan = plan_object_model(&resolved.program, &semantic.facts);
    let dog = object_plan.strategy_for("Dog").unwrap();
    assert_eq!(
        dog.requirements,
        vec![
            DispatchRequirementKind::NeedsInstanceof,
            DispatchRequirementKind::NeedsDowncast,
        ]
    );
}

#[test]
fn dispatch_requirement_producer_ignores_primitive_cast_targets() {
    let source = r#"
class Use {
    int asInt(long value) {
        return (int) value;
    }
}
"#;
    let semantic = analyze(source);

    assert!(semantic.facts.dispatch_requirement_facts.is_empty());
}

fn analyze_with_config(
    source: &str,
    config: semantic_analysis::SemanticAnalysisConfig,
) -> semantic_analysis::SemanticFacts {
    let parsed = parser::parse_java_source(source);
    let resolved = resolve::resolve_program(&parsed.program, parsed.diagnostics);
    semantic_analysis::analyze_program_with_config(
        &resolved.program,
        &resolved.facts,
        &new_java2rust::facts::FactSet::default(),
        config,
    )
}

#[test]
fn semantic_analysis_shell_preserves_current_translation_output() {
    let source = idiom_gap("NonNullFieldFromConstructors.java");
    let parsed = parser::parse_java_source(&source);
    let resolved = resolve::resolve_program(&parsed.program, parsed.diagnostics);
    let semantic = semantic_analysis::analyze_program(
        &resolved.program,
        &resolved.facts,
        &new_java2rust::facts::FactSet::default(),
    );
    let representations = representation::plan_representations(&resolved.program, &semantic.facts);
    let signatures = signature::solve_signatures_with_facts(
        &resolved.program,
        &representations,
        &semantic.usage,
        &semantic.facts,
    );
    let lowered = lower::lower_program_with_plans(
        &resolved.program,
        resolved.diagnostics,
        &semantic.facts,
        &semantic.usage,
        &representations,
        &signatures,
    );

    assert_eq!(
        translate_source(&source).rust,
        emit::emit_file(&lowered.file)
    );
}

#[test]
fn records_non_null_field_from_all_constructors() {
    let semantic = analyze(&idiom_gap("NonNullFieldFromConstructors.java"));

    assert_eq!(
        semantic
            .facts
            .field_nullability_for("NonNullFieldFromConstructors", "name"),
        Some(NullabilityState::DefinitelyNonNull)
    );
}

#[test]
fn field_nullability_external_override_beats_constructor_proof() {
    let source = idiom_gap("NonNullFieldFromConstructors.java");
    let parsed_external = external_facts::parse_external_facts(
        "field-nullability NonNullFieldFromConstructors.name maybe-null\n",
    );
    assert!(
        parsed_external.diagnostics.is_empty(),
        "{:?}",
        parsed_external.diagnostics
    );

    let parsed = parser::parse_java_source(&source);
    let resolved = resolve::resolve_program(&parsed.program, parsed.diagnostics);
    let semantic = semantic_analysis::analyze_program(
        &resolved.program,
        &resolved.facts,
        &parsed_external.facts,
    );

    assert_eq!(
        semantic
            .facts
            .field_nullability_for("NonNullFieldFromConstructors", "name"),
        Some(NullabilityState::MaybeNull)
    );

    let translation = translate_source_with_facts(&source, &parsed_external.facts);
    assert!(
        translation.rust.contains("name: Option<String>"),
        "{}",
        translation.rust
    );
}

#[test]
fn records_nullable_field_fallback() {
    let semantic = analyze(&java_fixture("05-types/Nullability.java"));

    assert_eq!(
        semantic
            .facts
            .field_nullability_for("Nullability", "cached"),
        Some(NullabilityState::MaybeNull)
    );
}

#[test]
fn records_non_null_field_assignment_through_private_helper() {
    let semantic = analyze(&idiom_gap("NonNullFieldViaHelper.java"));

    assert_eq!(
        semantic
            .facts
            .field_nullability_for("NonNullFieldViaHelper", "name"),
        Some(NullabilityState::DefinitelyNonNull)
    );
}

#[test]
fn records_non_null_field_assignment_through_constructor_delegation() {
    let semantic = analyze(&java_fixture("03-methods/ConstructorOverloads.java"));

    assert_eq!(
        semantic
            .facts
            .field_nullability_for("ConstructorOverloads", "label"),
        Some(NullabilityState::DefinitelyNonNull)
    );
    assert!(semantic.iterations.field_nullability >= 2);
}

#[test]
fn nullable_field_storage_uses_option_shape_and_assignment_wrapping() {
    let translation = translate_source(&java_fixture("05-types/Nullability.java"));

    assert!(
        translation.rust.contains("cached: Option<String>"),
        "{}",
        translation.rust
    );
    assert!(
        translation.rust.contains("self.cached = Some(value);"),
        "{}",
        translation.rust
    );
}

#[test]
fn records_trivial_getter_and_setter_facts() {
    let semantic = analyze(&idiom_gap("JavaBeanGetterSetterNames.java"));

    let getter = semantic
        .facts
        .trivial_accessor_for("JavaBeanGetterSetterNames", "getValue")
        .unwrap();
    assert!(matches!(
        &getter.kind,
        TrivialAccessorKind::Getter { field }
            if field.type_name == "JavaBeanGetterSetterNames" && field.field_name == "value"
    ));

    let setter = semantic
        .facts
        .trivial_accessor_for("JavaBeanGetterSetterNames", "setValue")
        .unwrap();
    assert!(matches!(
        &setter.kind,
        TrivialAccessorKind::Setter {
            field,
            param_name,
            fluent: false,
        } if field.type_name == "JavaBeanGetterSetterNames"
            && field.field_name == "value"
            && param_name == "value"
    ));
}

#[test]
fn lowers_trivial_accessor_calls_as_field_access_without_deleting_api_wrappers() {
    let translation = translate_source(&idiom_gap("JavaBeanGetterSetterNames.java"));

    assert!(
        translation.rust.contains("pub fn get_value"),
        "{}",
        translation.rust
    );
    assert!(
        translation.rust.contains("pub fn set_value"),
        "{}",
        translation.rust
    );
    assert!(
        translation.rust.contains("pub fn copy_from(&mut self"),
        "{}",
        translation.rust
    );
    assert!(
        translation.rust.contains("self.value = other.value"),
        "{}",
        translation.rust
    );
    assert!(
        translation.rust.contains("return self.value;"),
        "{}",
        translation.rust
    );
    assert!(
        !translation.rust.contains("self.get_value()")
            && !translation.rust.contains("self.set_value("),
        "{}",
        translation.rust
    );
}

#[test]
fn usage_treats_trivial_accessor_calls_as_backing_field_uses() {
    let semantic = analyze(&idiom_gap("JavaBeanGetterSetterNames.java"));
    let copy_from = semantic
        .usage
        .methods
        .get(&UsageMethodKey {
            type_name: "JavaBeanGetterSetterNames".to_string(),
            method_name: "copyFrom".to_string(),
        })
        .unwrap();

    assert!(copy_from.mutates_self);
    assert!(
        copy_from.field_uses.iter().any(|field_use| matches!(
            field_use,
            FieldUse::Returned { field_name } if field_name == "value"
        )),
        "{:?}",
        copy_from.field_uses
    );
}

#[test]
fn records_external_getter_and_setter_effect_guesses() {
    let semantic = analyze(&idiom_gap("ExternalGetterSetterMutability.java"));

    let getter = semantic.facts.external_api_effect_for("getValue").unwrap();
    assert_eq!(getter.receiver_mutability, ReceiverMutabilityFact::ReadOnly);

    let setter = semantic.facts.external_api_effect_for("setValue").unwrap();
    assert_eq!(
        setter.receiver_mutability,
        ReceiverMutabilityFact::MutatesReceiver
    );
}

#[test]
fn external_collection_api_effects_are_receiver_specific_facts() {
    let semantic = analyze(
        r#"
import java.util.List;

class CollectionApiUse {
    int update(List<String> values, List<String> other) {
        int n = values.size();
        boolean empty = values.isEmpty();
        boolean includes = values.containsAll(other);
        String first = values.get(0);
        values.add("x");
        values.addAll(other);
        return n;
    }

    int intervalCount(IntervalList intervals) {
        return intervals.size();
    }
}
"#,
    );

    let size = semantic
        .facts
        .external_api_effect_for_receiver(Some("List"), "size")
        .unwrap();
    assert_eq!(size.role, ExternalApiRole::Getter);
    assert_eq!(size.receiver_mutability, ReceiverMutabilityFact::ReadOnly);
    assert_eq!(size.return_nullability, NullabilityState::DefinitelyNonNull);

    let get = semantic
        .facts
        .external_api_effect_for_receiver(Some("List"), "get")
        .unwrap();
    assert_eq!(get.role, ExternalApiRole::IndexedGetter);
    assert_eq!(get.receiver_mutability, ReceiverMutabilityFact::ReadOnly);
    assert_eq!(get.return_nullability, NullabilityState::Unknown);

    let add = semantic
        .facts
        .external_api_effect_for_receiver(Some("List"), "add")
        .unwrap();
    assert_eq!(add.role, ExternalApiRole::Mutator);
    assert_eq!(
        add.receiver_mutability,
        ReceiverMutabilityFact::MutatesReceiver
    );

    let add_all = semantic
        .facts
        .external_api_effect_for_receiver(Some("List"), "addAll")
        .unwrap();
    assert_eq!(add_all.role, ExternalApiRole::Mutator);

    let contains_all = semantic
        .facts
        .external_api_effect_for_receiver(Some("List"), "containsAll")
        .unwrap();
    assert_eq!(contains_all.role, ExternalApiRole::Getter);
    assert_eq!(
        contains_all.receiver_mutability,
        ReceiverMutabilityFact::ReadOnly
    );
    assert_eq!(
        contains_all.return_nullability,
        NullabilityState::DefinitelyNonNull
    );

    let interval_size = semantic
        .facts
        .external_api_effect_for_receiver(Some("IntervalList"), "size")
        .unwrap();
    assert_eq!(interval_size.role, ExternalApiRole::Getter);
    assert_eq!(
        interval_size.receiver_mutability,
        ReceiverMutabilityFact::ReadOnly
    );
    assert_eq!(
        interval_size.return_nullability,
        NullabilityState::DefinitelyNonNull
    );
}

#[test]
fn modeled_collection_api_effects_are_builtin_external_models() {
    let semantic = analyze(
        r#"
import java.util.List;

class CollectionApiProvenanceUse {
    void update(List<String> values, List<String> other) {
        values.size();
        values.get(0);
        values.add("x");
        values.addAll(other);
        values.containsAll(other);
    }
}
"#,
    );

    for method in ["size", "get", "add", "addAll", "containsAll"] {
        let effect = semantic
            .facts
            .external_api_effect_for_receiver(Some("List"), method)
            .unwrap_or_else(|| panic!("expected List.{method} external API fact"));
        assert_eq!(
            effect.provenance,
            FactProvenance::BuiltinExternalModel,
            "List.{method} is a modeled collection API fact, not a conservative guess"
        );
    }
}

#[test]
fn external_map_api_effects_distinguish_read_and_write_methods() {
    let semantic = analyze(
        r#"
import java.util.Map;

class MapApiUse {
    int update(Map<String, Integer> counts) {
        Integer old = counts.get("x");
        Integer fallback = counts.getOrDefault("missing", 0);
        counts.put("x", 1);
        return counts.isEmpty() ? 0 : counts.size();
    }
}
"#,
    );

    let get = semantic
        .facts
        .external_api_effect_for_receiver(Some("Map"), "get")
        .unwrap();
    assert_eq!(get.role, ExternalApiRole::Getter);
    assert_eq!(get.receiver_mutability, ReceiverMutabilityFact::ReadOnly);
    assert_eq!(get.return_nullability, NullabilityState::Unknown);

    let get_or_default = semantic
        .facts
        .external_api_effect_for_receiver(Some("Map"), "getOrDefault")
        .unwrap();
    assert_eq!(get_or_default.role, ExternalApiRole::Getter);
    assert_eq!(
        get_or_default.receiver_mutability,
        ReceiverMutabilityFact::ReadOnly
    );
    assert_eq!(get_or_default.return_nullability, NullabilityState::Unknown);

    let put = semantic
        .facts
        .external_api_effect_for_receiver(Some("Map"), "put")
        .unwrap();
    assert_eq!(put.role, ExternalApiRole::Mutator);
    assert_eq!(
        put.receiver_mutability,
        ReceiverMutabilityFact::MutatesReceiver
    );

    let size = semantic
        .facts
        .external_api_effect_for_receiver(Some("Map"), "size")
        .unwrap();
    assert_eq!(size.receiver_mutability, ReceiverMutabilityFact::ReadOnly);
    assert_eq!(size.return_nullability, NullabilityState::DefinitelyNonNull);
}

#[test]
fn modeled_map_api_effects_are_builtin_external_models() {
    let semantic = analyze(
        r#"
import java.util.Map;

class MapApiProvenanceUse {
    void update(Map<String, Integer> counts) {
        counts.get("x");
        counts.getOrDefault("missing", 0);
        counts.put("x", 1);
        counts.size();
    }
}
"#,
    );

    for method in ["get", "getOrDefault", "put", "size"] {
        let effect = semantic
            .facts
            .external_api_effect_for_receiver(Some("Map"), method)
            .unwrap_or_else(|| panic!("expected Map.{method} external API fact"));
        assert_eq!(
            effect.provenance,
            FactProvenance::BuiltinExternalModel,
            "Map.{method} is a modeled map API fact, not a conservative guess"
        );
    }
}

#[test]
fn unknown_external_api_effects_remain_conservative_fallbacks() {
    let semantic = analyze(
        r#"
class UnknownExternalApiProvenanceUse {
    void update(ExternalThing thing) {
        thing.transform();
    }
}
"#,
    );

    let effect = semantic
        .facts
        .external_api_effect_for_receiver(Some("ExternalThing"), "transform")
        .unwrap();
    assert_eq!(effect.provenance, FactProvenance::ConservativeFallback);
}

#[test]
fn external_api_effects_use_resolver_unresolved_receiver_facts() {
    let parsed = parser::parse_java_source(
        r#"
class EntryApiFactSeed {
    void run() {}
}
"#,
    );
    let resolved = resolve::resolve_program(&parsed.program, parsed.diagnostics);
    let mut resolved_facts = resolved.facts.clone();
    resolved_facts
        .unresolved_target_facts
        .push(UnresolvedTargetFact {
            enclosing_type: "EntryApiFactSeed".to_string(),
            enclosing_member: Some("run".to_string()),
            kind: UnresolvedTargetKind::MethodCall,
            target_name: "getKey".to_string(),
            receiver_type: Some(JavaType::Named {
                name: "Entry".to_string(),
                args: vec![JavaType::String, JavaType::String],
            }),
            arg_types: Vec::new(),
            is_anonymous_class: false,
            provenance: FactProvenance::ParsedSource,
        });
    resolved_facts
        .unresolved_target_facts
        .push(UnresolvedTargetFact {
            enclosing_type: "EntryApiFactSeed".to_string(),
            enclosing_member: Some("run".to_string()),
            kind: UnresolvedTargetKind::MethodCall,
            target_name: "getValue".to_string(),
            receiver_type: Some(JavaType::Named {
                name: "Entry".to_string(),
                args: vec![JavaType::String, JavaType::String],
            }),
            arg_types: Vec::new(),
            is_anonymous_class: false,
            provenance: FactProvenance::ParsedSource,
        });

    let semantic = semantic_analysis::analyze_program(
        &resolved.program,
        &resolved_facts,
        &new_java2rust::facts::FactSet::default(),
    );

    let key = semantic
        .facts
        .external_api_effect_for_receiver(Some("Entry"), "getKey")
        .unwrap();
    assert_eq!(key.role, ExternalApiRole::Getter);
    assert_eq!(key.receiver_mutability, ReceiverMutabilityFact::ReadOnly);

    let value = semantic
        .facts
        .external_api_effect_for_receiver(Some("Entry"), "getValue")
        .unwrap();
    assert_eq!(value.role, ExternalApiRole::Getter);
    assert_eq!(value.return_nullability, NullabilityState::Unknown);
}

#[test]
fn external_api_effects_explain_receiver_agnostic_methods_without_receiver_type() {
    let parsed = parser::parse_java_source(
        r#"
class UnknownReceiverApiFactSeed {
    void run() {}
}
"#,
    );
    let resolved = resolve::resolve_program(&parsed.program, parsed.diagnostics);
    let mut resolved_facts = resolved.facts.clone();
    resolved_facts
        .unresolved_target_facts
        .push(UnresolvedTargetFact {
            enclosing_type: "UnknownReceiverApiFactSeed".to_string(),
            enclosing_member: Some("run".to_string()),
            kind: UnresolvedTargetKind::MethodCall,
            target_name: "toString".to_string(),
            receiver_type: None,
            arg_types: Vec::new(),
            is_anonymous_class: false,
            provenance: FactProvenance::ParsedSource,
        });
    resolved_facts
        .unresolved_target_facts
        .push(UnresolvedTargetFact {
            enclosing_type: "UnknownReceiverApiFactSeed".to_string(),
            enclosing_member: Some("run".to_string()),
            kind: UnresolvedTargetKind::MethodCall,
            target_name: "unknownTransform".to_string(),
            receiver_type: None,
            arg_types: Vec::new(),
            is_anonymous_class: false,
            provenance: FactProvenance::ParsedSource,
        });

    let semantic = semantic_analysis::analyze_program(
        &resolved.program,
        &resolved_facts,
        &new_java2rust::facts::FactSet::default(),
    );

    let to_string = semantic
        .facts
        .external_api_effect_for_receiver(None, "toString")
        .unwrap();
    assert_eq!(to_string.role, ExternalApiRole::Getter);
    assert_eq!(
        to_string.receiver_mutability,
        ReceiverMutabilityFact::ReadOnly
    );
    assert_eq!(
        to_string.return_nullability,
        NullabilityState::DefinitelyNonNull
    );
    assert!(
        semantic
            .facts
            .external_api_effect_for_receiver(None, "unknownTransform")
            .is_none()
    );
}

#[test]
fn external_constructor_facts_explain_missing_library_constructors_only() {
    let parsed = parser::parse_java_source(
        r#"
class ExternalConstructorUse {
    void run() {
        ExternalBox external = new ExternalBox("x");
        LocalBox local = new LocalBox("x");
    }
}

class LocalBox {
    LocalBox(int value) {}
}
"#,
    );
    let resolved = resolve::resolve_program(&parsed.program, parsed.diagnostics);
    let semantic = semantic_analysis::analyze_program(
        &resolved.program,
        &resolved.facts,
        &new_java2rust::facts::FactSet::default(),
    );

    let external_box = semantic
        .facts
        .external_constructor_for("ExternalBox", &[JavaType::String])
        .unwrap();
    assert_eq!(
        external_box.provenance,
        FactProvenance::ConservativeFallback
    );
    assert!(
        semantic
            .facts
            .external_constructor_for("LocalBox", &[JavaType::String])
            .is_none()
    );
}

#[test]
fn anonymous_class_constructors_do_not_become_external_constructor_facts() {
    let parsed = parser::parse_java_source(
        r#"
class AnonymousConstructorUse {
    Object run() {
        return new ExternalIterator() {
        };
    }
}
"#,
    );
    let resolved = resolve::resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        resolved
            .facts
            .unresolved_target_facts
            .iter()
            .any(|fact| fact.kind == UnresolvedTargetKind::ConstructorCall
                && fact.target_name == "ExternalIterator"
                && fact.is_anonymous_class),
        "resolver should preserve anonymous-class constructor provenance"
    );

    let semantic = semantic_analysis::analyze_program(
        &resolved.program,
        &resolved.facts,
        &new_java2rust::facts::FactSet::default(),
    );

    assert!(
        semantic
            .facts
            .external_constructor_for("ExternalIterator", &[])
            .is_none(),
        "anonymous class construction should be handled by representation/object-model planning, not external constructor provenance"
    );
}

#[test]
fn tuple_like_external_constructor_facts_are_builtin_models() {
    let parsed = parser::parse_java_source(
        r#"
class TupleLikeConstructorUse {
    void run(String name, Integer count) {
        Pair pair = new Pair(name, count);
        ImmutablePair immutablePair = new ImmutablePair(name, count);
        MutablePair mutablePair = new MutablePair(name, count);
        Tuple2 tuple = new Tuple2(name, count);
        Tuple3 tuple3 = new Tuple3(name, count, name);
        ImmutableTriple triple = new ImmutableTriple(name, count, name);
    }
}
"#,
    );
    let resolved = resolve::resolve_program(&parsed.program, parsed.diagnostics);
    let semantic = semantic_analysis::analyze_program(
        &resolved.program,
        &resolved.facts,
        &new_java2rust::facts::FactSet::default(),
    );

    for type_name in ["Pair", "ImmutablePair", "MutablePair", "Tuple2"] {
        let fact = semantic
            .facts
            .external_constructor_facts
            .iter()
            .find(|fact| fact.type_name == type_name && fact.arg_types.len() == 2)
            .unwrap_or_else(|| panic!("expected external constructor fact for {type_name}/2"));
        assert_eq!(
            fact.provenance,
            FactProvenance::BuiltinExternalModel,
            "{type_name}/2 constructor should be an audited tuple-like external model"
        );
    }

    for type_name in ["Tuple3", "ImmutableTriple"] {
        let fact = semantic
            .facts
            .external_constructor_facts
            .iter()
            .find(|fact| fact.type_name == type_name && fact.arg_types.len() == 3)
            .unwrap_or_else(|| panic!("expected external constructor fact for {type_name}/3"));
        assert_eq!(
            fact.provenance,
            FactProvenance::BuiltinExternalModel,
            "{type_name}/3 constructor should be an audited tuple-like external model"
        );
    }
}

#[test]
fn jdk_external_constructor_facts_are_builtin_models() {
    let parsed = parser::parse_java_source(
        r#"
class JdkConstructorUse {
    void run(Collection<String> names, String message, Throwable cause, InputStream input, OutputStream output, Reader reader, String path) {
        ArrayList<String> copied = new ArrayList<>(names);
        HashMap<String, Integer> counts = new HashMap<>();
        Properties properties = new Properties();
        URI uri = new URI(path);
        ProcessBuilder processBuilder = new ProcessBuilder(names);
        IllegalArgumentException bad = new IllegalArgumentException(message, cause);
        NoSuchElementException missing = new NoSuchElementException(message);
        NullPointerException nullPointer = new NullPointerException();
        ArrayIndexOutOfBoundsException badIndex = new ArrayIndexOutOfBoundsException(message);
        IndexOutOfBoundsException emptyIndex = new IndexOutOfBoundsException();
        IndexOutOfBoundsException badIndexBase = new IndexOutOfBoundsException(message);
        File file = new File(message);
        DecimalFormat formatter = new DecimalFormat(message);
        SimpleDateFormat dateFormat = new SimpleDateFormat(message);
        Date timestamp = new Date();
        Random seeded = new Random(1);
        ConsoleHandler console = new ConsoleHandler();
        ForkJoinPool pool = new ForkJoinPool(2);
        ConcurrentModificationException concurrent = new ConcurrentModificationException(message);
        Manifest manifest = new Manifest(input);
        GZIPInputStream gzip = new GZIPInputStream(input);
        LineNumberReader lineReader = new LineNumberReader(reader);
        BitSet bits = new BitSet(10);
        AbstractList<String> listView = new AbstractList<String>() {};
        AbstractSet<String> setView = new AbstractSet<String>() {};
        ArrayBlockingQueue<String> queue = new ArrayBlockingQueue<>(10);
        ConcurrentHashMap<String, Integer> concurrentCounts = new ConcurrentHashMap<>();
        AtomicBoolean flag = new AtomicBoolean();
        SimpleEntry<String, Integer> entry = new SimpleEntry<>(message, 1);
        ZipEntry zipEntry = new ZipEntry(message);
        ZipFile zipFile = new ZipFile(file);
        ZipInputStream zipInput = new ZipInputStream(input);
        ZipOutputStream zipOutput = new ZipOutputStream(output);
        StringJoiner joiner = new StringJoiner(message);
        StringTokenizer tokenizer = new StringTokenizer(message, message, true);
    }
}
"#,
    );
    let resolved = resolve::resolve_program(&parsed.program, parsed.diagnostics);
    let semantic = semantic_analysis::analyze_program(
        &resolved.program,
        &resolved.facts,
        &new_java2rust::facts::FactSet::default(),
    );

    for (type_name, args) in [
        (
            "ArrayList",
            vec![JavaType::Named {
                name: "Collection".to_string(),
                args: vec![JavaType::String],
            }],
        ),
        ("HashMap", Vec::new()),
        (
            "IllegalArgumentException",
            vec![
                JavaType::String,
                JavaType::Named {
                    name: "Throwable".to_string(),
                    args: Vec::new(),
                },
            ],
        ),
        ("Properties", Vec::new()),
        ("URI", vec![JavaType::String]),
        (
            "ProcessBuilder",
            vec![JavaType::Named {
                name: "Collection".to_string(),
                args: vec![JavaType::String],
            }],
        ),
        ("NoSuchElementException", vec![JavaType::String]),
        ("NullPointerException", Vec::new()),
        ("ArrayIndexOutOfBoundsException", vec![JavaType::String]),
        ("IndexOutOfBoundsException", Vec::new()),
        ("IndexOutOfBoundsException", vec![JavaType::String]),
        ("File", vec![JavaType::String]),
        ("DecimalFormat", vec![JavaType::String]),
        ("SimpleDateFormat", vec![JavaType::String]),
        ("Date", Vec::new()),
        ("Random", vec![JavaType::Primitive(PrimitiveType::Int)]),
        ("ConsoleHandler", Vec::new()),
        (
            "ForkJoinPool",
            vec![JavaType::Primitive(PrimitiveType::Int)],
        ),
        ("ConcurrentModificationException", vec![JavaType::String]),
        (
            "Manifest",
            vec![JavaType::Named {
                name: "InputStream".to_string(),
                args: Vec::new(),
            }],
        ),
        (
            "GZIPInputStream",
            vec![JavaType::Named {
                name: "InputStream".to_string(),
                args: Vec::new(),
            }],
        ),
        (
            "LineNumberReader",
            vec![JavaType::Named {
                name: "Reader".to_string(),
                args: Vec::new(),
            }],
        ),
        ("BitSet", vec![JavaType::Primitive(PrimitiveType::Int)]),
        (
            "ArrayBlockingQueue",
            vec![JavaType::Primitive(PrimitiveType::Int)],
        ),
        ("ConcurrentHashMap", Vec::new()),
        ("AtomicBoolean", Vec::new()),
        (
            "ZipFile",
            vec![JavaType::Named {
                name: "File".to_string(),
                args: Vec::new(),
            }],
        ),
        (
            "ZipInputStream",
            vec![JavaType::Named {
                name: "InputStream".to_string(),
                args: Vec::new(),
            }],
        ),
        (
            "ZipOutputStream",
            vec![JavaType::Named {
                name: "OutputStream".to_string(),
                args: Vec::new(),
            }],
        ),
    ] {
        let fact = semantic
            .facts
            .external_constructor_for(type_name, &args)
            .unwrap_or_else(|| panic!("expected external constructor fact for {type_name}"));
        assert_eq!(
            fact.provenance,
            FactProvenance::BuiltinExternalModel,
            "{type_name} constructor should be a builtin external model"
        );
    }

    for (type_name, arg_len) in [
        ("SimpleEntry", 2),
        ("ZipEntry", 1),
        ("StringJoiner", 1),
        ("StringTokenizer", 3),
    ] {
        let fact = semantic
            .facts
            .external_constructor_facts
            .iter()
            .find(|fact| fact.type_name == type_name && fact.arg_types.len() == arg_len)
            .unwrap_or_else(|| {
                panic!("expected external constructor fact for {type_name}/{arg_len}")
            });
        assert_eq!(
            fact.provenance,
            FactProvenance::BuiltinExternalModel,
            "{type_name}/{arg_len} constructor should be an audited JDK utility model"
        );
    }

    for type_name in ["AbstractList", "AbstractSet"] {
        assert!(
            resolved
                .facts
                .unresolved_target_facts
                .iter()
                .any(|fact| fact.kind == UnresolvedTargetKind::ConstructorCall
                    && fact.target_name == type_name
                    && fact.is_anonymous_class),
            "{type_name} anonymous construction should keep anonymous-class provenance"
        );
        assert!(
            semantic
                .facts
                .external_constructor_for(type_name, &[])
                .is_none(),
            "{type_name} anonymous construction should not become an external constructor fact"
        );
    }
}

#[test]
fn jdk_numeric_and_atomic_constructor_facts_are_builtin_models() {
    let parsed = parser::parse_java_source(
        r#"
class JdkNumericAndAtomicConstructorUse {
    void run(byte[] bytes) {
        BigInteger fromString = new BigInteger("14695981039346656037");
        BigInteger fromBytes = new BigInteger(1, bytes);
        AtomicInteger count = new AtomicInteger(1);
        AtomicLong emptyTotal = new AtomicLong();
        AtomicLong total = new AtomicLong(0);
    }
}
"#,
    );
    let resolved = resolve::resolve_program(&parsed.program, parsed.diagnostics);
    let semantic = semantic_analysis::analyze_program(
        &resolved.program,
        &resolved.facts,
        &new_java2rust::facts::FactSet::default(),
    );

    for (type_name, arg_len) in [
        ("BigInteger", 1),
        ("BigInteger", 2),
        ("AtomicInteger", 1),
        ("AtomicLong", 0),
        ("AtomicLong", 1),
    ] {
        let fact = semantic
            .facts
            .external_constructor_facts
            .iter()
            .find(|fact| fact.type_name == type_name && fact.arg_types.len() == arg_len)
            .unwrap_or_else(|| {
                panic!("expected external constructor fact for {type_name}/{arg_len}")
            });
        assert_eq!(
            fact.provenance,
            FactProvenance::BuiltinExternalModel,
            "{type_name}/{arg_len} constructor should be an audited JDK utility model"
        );
    }
}

#[test]
fn htsjdk_external_constructor_facts_are_builtin_models() {
    let parsed = parser::parse_java_source(
        r#"
class HtsjdkConstructorUse {
    void run(Set<VCFHeaderLine> lines, Set<String> samples, List<CigarElement> elements, Genotype genotype,
             VariantContext variant, SAMSequenceDictionary dictionary, SeekableStream stream,
             List<Allele> alleles, SAMFileHeader header, InputStream input, File outputFile,
             AsciiLineReader asciiLineReader, Iterator<VariantContext> iterator,
             VariantContextComparator comparator, List<CloseableIterator<VariantContext>> iterators,
             Collection<SAMFileHeader> headers, Supplier<List<String>> supplier, byte[] bases,
             Map<String, List<String>> attributes, ReferenceSource referenceSource,
             Object defaultFactory, Object header, long filePointer, Iterator<SAMRecord> records,
             Object filter, Path referencePath, Path seekablePath, Locatable locatable,
             Iterator<Interval> intervalIterator, List<SamReader> readers, LineReader lineReader,
             Object cramHeader, Object compressionHeader, Object sliceOffsets, OutputStream output,
             List<SAMSequenceRecord> sequenceRecords) {
        VCFHeaderLine headerLine = new VCFHeaderLine("source", "tool");
        VCFInfoHeaderLine info = new VCFInfoHeaderLine("DP", 1, VCFHeaderLineType.Integer, "Depth");
        VCFFormatHeaderLine format = new VCFFormatHeaderLine("GQ", 1, VCFHeaderLineType.Integer, "Quality");
        VCFFilterHeaderLine filterLine = new VCFFilterHeaderLine("PASS", "Filter description");
        VCFHeader emptyHeader = new VCFHeader();
        VCFHeader headerWithLines = new VCFHeader(lines);
        VCFHeader headerWithSamples = new VCFHeader(lines, samples);
        VCFCodec codec = new VCFCodec();
        VCFRecordCodec recordCodec = new VCFRecordCodec(header, true);
        CigarElement element = new CigarElement(10, CigarOperator.MATCH_OR_MISMATCH);
        Cigar emptyCigar = new Cigar();
        Cigar cigar = new Cigar(elements);
        GenotypeBuilder emptyBuilder = new GenotypeBuilder();
        GenotypeBuilder copiedBuilder = new GenotypeBuilder(genotype);
        VariantContextComparator comparator = new VariantContextComparator(dictionary);
        VariantContextBuilder emptyVariantBuilder = new VariantContextBuilder();
        VariantContextBuilder copiedVariantBuilder = new VariantContextBuilder(variant);
        VariantContextBuilder fullVariantBuilder = new VariantContextBuilder("source", "chr1", 1, 2, alleles);
        SeekableBufferedStream buffered = new SeekableBufferedStream(stream);
        BAMRecordCodec bamCodec = new BAMRecordCodec(header);
        BlockCompressedInputStream compressed = new BlockCompressedInputStream(input);
        BlockCompressedInputStream tolerantCompressed = new BlockCompressedInputStream(input, false);
        BAMIndexer indexer = new BAMIndexer(outputFile, header);
        BCF2Codec bcf = new BCF2Codec();
        FeatureCodecHeader featureHeader = new FeatureCodecHeader(header, filePointer);
        BufferedLineReader bufferedReader = new BufferedLineReader(input);
        AsciiLineReaderIterator iterator = new AsciiLineReaderIterator(asciiLineReader);
        LineIteratorImpl lineIterator = new LineIteratorImpl(lineReader);
        SynchronousLineReader synchronousLineReader = new SynchronousLineReader(input);
        FilteringSamIterator filtering = new FilteringSamIterator(records, filter);
        FastaSequenceFile fasta = new FastaSequenceFile(referencePath, false);
        BlockCompressedOutputStream compressedOutput = new BlockCompressedOutputStream(outputFile, 5);
        BlockCompressedOutputStream compressedOutputWithPath = new BlockCompressedOutputStream(input, outputFile, 5);
        BinaryCodec binaryCodec = new BinaryCodec();
        BinaryCodec outputBinaryCodec = new BinaryCodec(compressedOutput);
        SAMTextHeaderCodec textCodec = new SAMTextHeaderCodec();
        MetricsFile metrics = new MetricsFile();
        DefaultSAMRecordFactory recordFactory = new DefaultSAMRecordFactory();
        ComponentMethods componentMethods = new ComponentMethods();
        CramContainerIterator cramIterator = new CramContainerIterator(input);
        Container container = new Container(cramHeader, compressionHeader, sliceOffsets);
        SAMRecordCoordinateComparator coordinateComparator = new SAMRecordCoordinateComparator();
        SAMRecordQueryNameComparator queryComparator = new SAMRecordQueryNameComparator();
        SAMSequenceDictionary emptyDictionary = new SAMSequenceDictionary();
        SAMSequenceDictionary copiedDictionary = new SAMSequenceDictionary(sequenceRecords);
        SAMSequenceRecord sequenceRecord = new SAMSequenceRecord("chr1", 1000);
        SAMProgramRecord programRecord = new SAMProgramRecord("program");
        SAMFormatException formatException = new SAMFormatException("bad sam");
        SBIIndexWriter sbiWriter = new SBIIndexWriter(output, 10);
        CRAMReferenceRegion referenceRegion = new CRAMReferenceRegion(referenceSource, dictionary);
        DefaultingMap defaultingMap = new DefaultingMap(defaultFactory, true);
        SAMFileHeader emptySamHeader = new SAMFileHeader();
        SAMFileHeader dictionarySamHeader = new SAMFileHeader(dictionary);
        IntervalList intervalList = new IntervalList(emptySamHeader);
        SAMReadGroupRecord readGroup = new SAMReadGroupRecord("rg");
        SAMRecord samRecord = new SAMRecord(header);
        SAMFileWriterFactory writerFactory = new SAMFileWriterFactory();
        IndexedFastaSequenceFile indexedFasta = new IndexedFastaSequenceFile(referencePath);
        IntervalTree<String> intervalTree = new IntervalTree<>();
        IntervalMergerIterator mergedIntervals = new IntervalMergerIterator(intervalIterator, true, true, false);
        Histogram<String> emptyHistogram = new Histogram<>();
        Histogram<String> labeledHistogram = new Histogram<>("bin", "value");
        PeekableIterator<VariantContext> peekable = new PeekableIterator<>(iterator);
        MergingIterator<VariantContext> merging = new MergingIterator<>(comparator, iterators);
        SamFileHeaderMerger headerMerger = new SamFileHeaderMerger(SAMFileHeader.SortOrder.coordinate, headers, true);
        MergingSamRecordIterator mergedRecords = new MergingSamRecordIterator(headerMerger, readers, true);
        Lazy<List<String>> lazy = new Lazy<>(supplier);
        ReferenceSequence reference = new ReferenceSequence("chr1", 0, bases);
        CodecLineParsingException parsing = new CodecLineParsingException("bad line");
        RuntimeIOException runtimeIo = new RuntimeIOException("bad io");
        FastaReferenceWriterBuilder writerBuilder = new FastaReferenceWriterBuilder();
        Gff3BaseData gff3 = new Gff3BaseData("chr1", "source", "gene", 1, 10, ".", "+", ".", attributes);
        SimpleFeature simpleFeature = new SimpleFeature("chr1", 1, 10);
        SeekablePathStream pathStream = new SeekablePathStream(seekablePath);
        Interval copiedInterval = new Interval(locatable);
        Interval numericInterval = new Interval(1, 2);
        Interval namedInterval = new Interval("chr1", 1, 2);
        Interval fullInterval = new Interval("chr1", 1, 2, false, "name");
    }
}
"#,
    );
    let resolved = resolve::resolve_program(&parsed.program, parsed.diagnostics);
    let semantic = semantic_analysis::analyze_program(
        &resolved.program,
        &resolved.facts,
        &new_java2rust::facts::FactSet::default(),
    );

    for (type_name, arg_len) in [
        ("CigarElement", 2),
        ("Cigar", 0),
        ("Cigar", 1),
        ("VariantContextComparator", 1),
        ("SeekableBufferedStream", 1),
        ("BAMRecordCodec", 1),
        ("BlockCompressedInputStream", 1),
        ("BlockCompressedInputStream", 2),
        ("BAMIndexer", 2),
        ("BCF2Codec", 0),
        ("FeatureCodecHeader", 2),
        ("BufferedLineReader", 1),
        ("AsciiLineReaderIterator", 1),
        ("LineIteratorImpl", 1),
        ("SynchronousLineReader", 1),
        ("FilteringSamIterator", 2),
        ("FastaSequenceFile", 2),
        ("BlockCompressedOutputStream", 2),
        ("BlockCompressedOutputStream", 3),
        ("BinaryCodec", 0),
        ("BinaryCodec", 1),
        ("SAMTextHeaderCodec", 0),
        ("MetricsFile", 0),
        ("DefaultSAMRecordFactory", 0),
        ("ComponentMethods", 0),
        ("CramContainerIterator", 1),
        ("Container", 3),
        ("SAMRecordCoordinateComparator", 0),
        ("SAMRecordQueryNameComparator", 0),
        ("SAMProgramRecord", 1),
        ("SAMFormatException", 1),
        ("SBIIndexWriter", 2),
        ("CRAMReferenceRegion", 2),
        ("DefaultingMap", 2),
        ("SAMRecord", 1),
        ("SAMFileWriterFactory", 0),
        ("IndexedFastaSequenceFile", 1),
        ("SamFileHeaderMerger", 3),
        ("MergingSamRecordIterator", 3),
        ("Lazy", 1),
        ("ReferenceSequence", 3),
        ("CodecLineParsingException", 1),
        ("FastaReferenceWriterBuilder", 0),
        ("Gff3BaseData", 9),
        ("SeekablePathStream", 1),
    ] {
        let fact = semantic
            .facts
            .external_constructor_facts
            .iter()
            .find(|fact| fact.type_name == type_name && fact.arg_types.len() == arg_len)
            .unwrap_or_else(|| {
                panic!("expected external constructor fact for {type_name}/{arg_len}")
            });
        assert_eq!(
            fact.provenance,
            FactProvenance::BuiltinExternalModel,
            "{type_name}/{arg_len} constructor should be an audited HTSJDK external model"
        );
    }

    for (type_name, arg_len) in [
        ("VCFInfoHeaderLine", 4),
        ("VCFFormatHeaderLine", 4),
        ("VCFFilterHeaderLine", 2),
        ("VCFHeaderLine", 2),
        ("VCFHeader", 0),
        ("VCFHeader", 1),
        ("VCFHeader", 2),
        ("VCFCodec", 0),
        ("VCFRecordCodec", 2),
        ("GenotypeBuilder", 0),
        ("GenotypeBuilder", 1),
        ("VariantContextBuilder", 0),
        ("VariantContextBuilder", 1),
        ("VariantContextBuilder", 5),
        ("SAMSequenceDictionary", 0),
        ("SAMSequenceDictionary", 1),
        ("SAMSequenceRecord", 2),
        ("SAMFileHeader", 0),
        ("SAMFileHeader", 1),
        ("SAMReadGroupRecord", 1),
        ("SimpleFeature", 3),
        ("IntervalTree", 0),
        ("IntervalMergerIterator", 4),
        ("Histogram", 0),
        ("Histogram", 2),
        ("PeekableIterator", 1),
        ("MergingIterator", 2),
        ("RuntimeIOException", 1),
        ("Interval", 1),
        ("Interval", 2),
        ("Interval", 3),
        ("Interval", 5),
        ("IntervalList", 1),
    ] {
        let fact = semantic
            .facts
            .external_constructor_facts
            .iter()
            .find(|fact| fact.type_name == type_name && fact.arg_types.len() == arg_len)
            .unwrap_or_else(|| {
                panic!("expected external constructor fact for {type_name}/{arg_len}")
            });
        assert_eq!(
            fact.provenance,
            FactProvenance::ExternalLibraryPack,
            "{type_name}/{arg_len} constructor should be owned by an HTSJDK external pack"
        );
    }
}

#[test]
fn apache_commons_collections_constructor_facts_are_builtin_models() {
    let parsed = parser::parse_java_source(
        r#"
class ApacheCommonsCollectionsConstructorUse {
    void run(double defaultValue, Iterator values, Object predicate) {
        DefaultedMap contaminationBySample = new DefaultedMap(defaultValue);
        FilterIterator filteredValues = new FilterIterator(values, predicate);
        SingletonIterator singleton = new SingletonIterator("one");
        MultiMap multiMap = new MultiMap();
    }
}
"#,
    );
    let resolved = resolve::resolve_program(&parsed.program, parsed.diagnostics);
    let semantic = semantic_analysis::analyze_program(
        &resolved.program,
        &resolved.facts,
        &new_java2rust::facts::FactSet::default(),
    );

    for (type_name, arg_len) in [
        ("DefaultedMap", 1),
        ("FilterIterator", 2),
        ("SingletonIterator", 1),
        ("MultiMap", 0),
    ] {
        let fact = semantic
            .facts
            .external_constructor_facts
            .iter()
            .find(|fact| fact.type_name == type_name && fact.arg_types.len() == arg_len)
            .unwrap_or_else(|| {
                panic!("expected external constructor fact for {type_name}/{arg_len}")
            });
        assert_eq!(
            fact.provenance,
            FactProvenance::BuiltinExternalModel,
            "{type_name}/{arg_len} constructor should be an audited Apache Commons Collections external model"
        );
    }
}

#[test]
fn apache_commons_compress_constructor_facts_are_builtin_models() {
    let parsed = parser::parse_java_source(
        r#"
class ApacheCommonsCompressConstructorUse {
    void run(InputStream input, OutputStream output) {
        GzipCompressorInputStream gzipInput = new GzipCompressorInputStream(input);
        GzipCompressorOutputStream gzipOutput = new GzipCompressorOutputStream(output);
        NullInputStream nullInput = new NullInputStream(0);
        NullOutputStream nullOutput = new NullOutputStream(0);
        NullOutputStream emptyNullOutput = new NullOutputStream();
        TarArchiveInputStream tarInput = new TarArchiveInputStream(input);
        TarArchiveOutputStream tarOutput = new TarArchiveOutputStream(output);
        TarArchiveEntry tarEntry = new TarArchiveEntry("name", true);
    }
}
"#,
    );
    let resolved = resolve::resolve_program(&parsed.program, parsed.diagnostics);
    let semantic = semantic_analysis::analyze_program(
        &resolved.program,
        &resolved.facts,
        &new_java2rust::facts::FactSet::default(),
    );

    for (type_name, arg_len) in [
        ("GzipCompressorInputStream", 1),
        ("GzipCompressorOutputStream", 1),
        ("NullInputStream", 1),
        ("NullOutputStream", 1),
        ("NullOutputStream", 0),
        ("TarArchiveInputStream", 1),
        ("TarArchiveOutputStream", 1),
        ("TarArchiveEntry", 2),
    ] {
        let fact = semantic
            .facts
            .external_constructor_facts
            .iter()
            .find(|fact| fact.type_name == type_name && fact.arg_types.len() == arg_len)
            .unwrap_or_else(|| {
                panic!("expected external constructor fact for {type_name}/{arg_len}")
            });
        assert_eq!(
            fact.provenance,
            FactProvenance::BuiltinExternalModel,
            "{type_name}/{arg_len} constructor should be an audited Apache Commons Compress external model"
        );
    }
}

#[test]
fn apache_commons_lang_builder_constructor_facts_are_builtin_models() {
    let parsed = parser::parse_java_source(
        r#"
class ApacheCommonsLangBuilderConstructorUse {
    int run() {
        MutableInt empty = new MutableInt();
        MutableInt count = new MutableInt(0);
        MutableLong total = new MutableLong(0);
        NotImplementedException unsupported = new NotImplementedException("not implemented");
        return new HashCodeBuilder().append(count.intValue()).toHashCode();
    }
}
"#,
    );
    let resolved = resolve::resolve_program(&parsed.program, parsed.diagnostics);
    let semantic = semantic_analysis::analyze_program(
        &resolved.program,
        &resolved.facts,
        &new_java2rust::facts::FactSet::default(),
    );

    for (type_name, arg_len) in [
        ("HashCodeBuilder", 0),
        ("MutableInt", 0),
        ("MutableInt", 1),
        ("MutableLong", 1),
        ("NotImplementedException", 1),
    ] {
        let fact = semantic
            .facts
            .external_constructor_facts
            .iter()
            .find(|fact| fact.type_name == type_name && fact.arg_types.len() == arg_len)
            .unwrap_or_else(|| {
                panic!("expected external constructor fact for {type_name}/{arg_len}")
            });
        assert_eq!(
            fact.provenance,
            FactProvenance::BuiltinExternalModel,
            "{type_name}/{arg_len} constructor should be an audited Apache Commons Lang external model"
        );
    }
}

#[test]
fn apache_commons_external_constructor_facts_are_builtin_models() {
    let parsed = parser::parse_java_source(
        r#"
class ApacheCommonsConstructorUse {
    void run(double[][] matrixData, double[] vectorData, RandomGenerator generator) {
        Array2DRowRealMatrix matrixFromData = new Array2DRowRealMatrix(matrixData);
        Array2DRowRealMatrix matrixFromShape = new Array2DRowRealMatrix(2, 3);
        Array2DRowRealMatrix copiedMatrix = new Array2DRowRealMatrix(matrixData, false);
        ArrayRealVector vectorFromData = new ArrayRealVector(vectorData);
        ArrayRealVector vectorFromSize = new ArrayRealVector(4);
        DefaultRealMatrixChangingVisitor visitor = new DefaultRealMatrixChangingVisitor();
        MutableDouble total = new MutableDouble(0.0);
        Mean mean = new Mean();
        BinomialDistribution defaultRandom = new BinomialDistribution(10, 0.1);
        BinomialDistribution suppliedRandom = new BinomialDistribution(generator, 10, 0.1);
        BetaDistribution beta = new BetaDistribution(generator, 1.0, 2.0);
        BrentOptimizer optimizer = new BrentOptimizer(1.0e-6, 1.0e-9);
        AbstractIntegerDistribution abstractDistribution = new AbstractIntegerDistribution(generator);
        DescriptiveStatistics stats = new DescriptiveStatistics();
        DimensionMismatchException mismatch = new DimensionMismatchException(1, 2);
        EnumeratedDistribution enumerated = new EnumeratedDistribution(generator, vectorData);
        ExponentialDistribution exponential = new ExponentialDistribution(generator, 1.0);
        HypergeometricDistribution hypergeometric = new HypergeometricDistribution(generator, 10, 5, 2);
        JDKRandomGenerator defaultGenerator = new JDKRandomGenerator();
        MaxEval maxEval = new MaxEval(1000);
        Median median = new Median();
        NormalDistribution normal = new NormalDistribution(0.0, 1.0);
        NormalDistribution normalWithGenerator = new NormalDistribution(generator, 0.0, 1.0);
        ChiSquaredDistribution chiSquared = new ChiSquaredDistribution(3.0);
        ChiSquaredDistribution chiSquaredWithAccuracy = new ChiSquaredDistribution(3.0, 1.0e-9);
        PoissonDistribution poisson = new PoissonDistribution(1.0);
        SingularValueDecomposition decomposition = new SingularValueDecomposition(matrixFromData);
        StandardDeviation standardDeviation = new StandardDeviation();
        TDistribution tDistribution = new TDistribution(generator, 10.0);
        Percentile defaultPercentile = new Percentile();
        Percentile configuredPercentile = new Percentile(95.0);
    }
}
"#,
    );
    let resolved = resolve::resolve_program(&parsed.program, parsed.diagnostics);
    let semantic = semantic_analysis::analyze_program(
        &resolved.program,
        &resolved.facts,
        &new_java2rust::facts::FactSet::default(),
    );

    for (type_name, arg_len) in [
        ("Array2DRowRealMatrix", 1),
        ("Array2DRowRealMatrix", 2),
        ("ArrayRealVector", 1),
        ("DefaultRealMatrixChangingVisitor", 0),
        ("MutableDouble", 1),
        ("Mean", 0),
        ("BinomialDistribution", 2),
        ("BinomialDistribution", 3),
        ("BetaDistribution", 3),
        ("BrentOptimizer", 2),
        ("AbstractIntegerDistribution", 1),
        ("DescriptiveStatistics", 0),
        ("DimensionMismatchException", 2),
        ("EnumeratedDistribution", 2),
        ("ExponentialDistribution", 2),
        ("HypergeometricDistribution", 4),
        ("JDKRandomGenerator", 0),
        ("MaxEval", 1),
        ("Median", 0),
        ("NormalDistribution", 2),
        ("NormalDistribution", 3),
        ("ChiSquaredDistribution", 1),
        ("ChiSquaredDistribution", 2),
        ("PoissonDistribution", 1),
        ("SingularValueDecomposition", 1),
        ("StandardDeviation", 0),
        ("TDistribution", 2),
        ("Percentile", 0),
        ("Percentile", 1),
    ] {
        let fact = semantic
            .facts
            .external_constructor_facts
            .iter()
            .find(|fact| fact.type_name == type_name && fact.arg_types.len() == arg_len)
            .unwrap_or_else(|| {
                panic!("expected external constructor fact for {type_name}/{arg_len}")
            });
        assert_eq!(
            fact.provenance,
            FactProvenance::BuiltinExternalModel,
            "{type_name}/{arg_len} constructor should be an audited Apache Commons external model"
        );
    }
}

#[test]
fn hipparchus_constructor_facts_are_builtin_models() {
    let parsed = parser::parse_java_source(
        r#"
class HipparchusConstructorUse {
    void run() {
        EmpiricalDistribution empirical = new EmpiricalDistribution();
    }
}
"#,
    );
    let resolved = resolve::resolve_program(&parsed.program, parsed.diagnostics);
    let semantic = semantic_analysis::analyze_program(
        &resolved.program,
        &resolved.facts,
        &new_java2rust::facts::FactSet::default(),
    );

    let fact = semantic
        .facts
        .external_constructor_facts
        .iter()
        .find(|fact| fact.type_name == "EmpiricalDistribution" && fact.arg_types.is_empty())
        .unwrap_or_else(|| {
            panic!("expected external constructor fact for EmpiricalDistribution/0")
        });
    assert_eq!(
        fact.provenance,
        FactProvenance::BuiltinExternalModel,
        "EmpiricalDistribution/0 constructor should be an audited Hipparchus external model"
    );
}

#[test]
fn fastutil_constructor_facts_are_builtin_models() {
    let parsed = parser::parse_java_source(
        r#"
class FastutilConstructorUse {
    void run(int expectedSize, int[] keys, int[] values) {
        Int2ObjectArrayMap empty = new Int2ObjectArrayMap();
        Int2ObjectArrayMap sized = new Int2ObjectArrayMap(expectedSize);
        Int2ObjectOpenHashMap openEmpty = new Int2ObjectOpenHashMap();
        Int2ObjectOpenHashMap openSized = new Int2ObjectOpenHashMap(expectedSize);
        IntArrayList intList = new IntArrayList(expectedSize);
        Int2IntArrayMap intMap = new Int2IntArrayMap(keys, values);
        Object2IntOpenHashMap objectMap = new Object2IntOpenHashMap(expectedSize);
    }
}
"#,
    );
    let resolved = resolve::resolve_program(&parsed.program, parsed.diagnostics);
    let semantic = semantic_analysis::analyze_program(
        &resolved.program,
        &resolved.facts,
        &new_java2rust::facts::FactSet::default(),
    );

    for (type_name, arg_len) in [
        ("Int2ObjectArrayMap", 0),
        ("Int2ObjectArrayMap", 1),
        ("Int2ObjectOpenHashMap", 0),
        ("Int2ObjectOpenHashMap", 1),
        ("IntArrayList", 1),
        ("Int2IntArrayMap", 2),
        ("Object2IntOpenHashMap", 1),
    ] {
        let fact = semantic
            .facts
            .external_constructor_facts
            .iter()
            .find(|fact| fact.type_name == type_name && fact.arg_types.len() == arg_len)
            .unwrap_or_else(|| {
                panic!("expected external constructor fact for {type_name}/{arg_len}")
            });
        assert_eq!(
            fact.provenance,
            FactProvenance::BuiltinExternalModel,
            "{type_name}/{arg_len} constructor should be an audited fastutil external model"
        );
    }
}

#[test]
fn json_constructor_facts_are_builtin_models() {
    let parsed = parser::parse_java_source(
        r#"
class JsonConstructorUse {
    void run(InputStream input) {
        JSONObject empty = new JSONObject();
        JSONArray array = new JSONArray();
        JSONTokener tokener = new JSONTokener(input);
        JSONObject fromTokener = new JSONObject(tokener);
        JSONArray arrayFromTokener = new JSONArray(tokener);
    }
}
"#,
    );
    let resolved = resolve::resolve_program(&parsed.program, parsed.diagnostics);
    let semantic = semantic_analysis::analyze_program(
        &resolved.program,
        &resolved.facts,
        &new_java2rust::facts::FactSet::default(),
    );

    for (type_name, arg_len) in [
        ("JSONObject", 0),
        ("JSONArray", 0),
        ("JSONTokener", 1),
        ("JSONObject", 1),
        ("JSONArray", 1),
    ] {
        let fact = semantic
            .facts
            .external_constructor_facts
            .iter()
            .find(|fact| fact.type_name == type_name && fact.arg_types.len() == arg_len)
            .unwrap_or_else(|| {
                panic!("expected external constructor fact for {type_name}/{arg_len}")
            });
        assert_eq!(
            fact.provenance,
            FactProvenance::BuiltinExternalModel,
            "{type_name}/{arg_len} constructor should be an audited JSON external model"
        );
    }
}

#[test]
fn intel_gkl_constructor_facts_are_builtin_models() {
    let parsed = parser::parse_java_source(
        r#"
class IntelGklConstructorUse {
    void run() {
        IntelDeflaterFactory deflater = new IntelDeflaterFactory();
        IntelInflaterFactory inflater = new IntelInflaterFactory();
        IntelPDHMM pdhmm = new IntelPDHMM();
        IntelPairHmm pairHmm = new IntelPairHmm();
        IntelPairHmmOMP ompPairHmm = new IntelPairHmmOMP();
    }
}
"#,
    );
    let resolved = resolve::resolve_program(&parsed.program, parsed.diagnostics);
    let semantic = semantic_analysis::analyze_program(
        &resolved.program,
        &resolved.facts,
        &new_java2rust::facts::FactSet::default(),
    );

    for type_name in [
        "IntelDeflaterFactory",
        "IntelInflaterFactory",
        "IntelPDHMM",
        "IntelPairHmm",
        "IntelPairHmmOMP",
    ] {
        let fact = semantic
            .facts
            .external_constructor_facts
            .iter()
            .find(|fact| fact.type_name == type_name && fact.arg_types.is_empty())
            .unwrap_or_else(|| panic!("expected external constructor fact for {type_name}/0"));
        assert_eq!(
            fact.provenance,
            FactProvenance::BuiltinExternalModel,
            "{type_name}/0 constructor should be an audited Intel GKL external model"
        );
    }
}

#[test]
fn jama_matrix_constructor_facts_are_builtin_models() {
    let parsed = parser::parse_java_source(
        r#"
class JamaMatrixConstructorUse {
    void run(double[][] data, int rows, int columns) {
        Matrix fromData = new Matrix(data);
        Matrix fromShape = new Matrix(rows, columns);
    }
}
"#,
    );
    let resolved = resolve::resolve_program(&parsed.program, parsed.diagnostics);
    let semantic = semantic_analysis::analyze_program(
        &resolved.program,
        &resolved.facts,
        &new_java2rust::facts::FactSet::default(),
    );

    for arg_len in [1, 2] {
        let fact = semantic
            .facts
            .external_constructor_facts
            .iter()
            .find(|fact| fact.type_name == "Matrix" && fact.arg_types.len() == arg_len)
            .unwrap_or_else(|| panic!("expected external constructor fact for Matrix/{arg_len}"));
        assert_eq!(
            fact.provenance,
            FactProvenance::BuiltinExternalModel,
            "Matrix/{arg_len} constructor should be an audited Jama external model"
        );
    }
}

#[test]
fn picard_constructor_facts_are_builtin_models() {
    let parsed = parser::parse_java_source(
        r#"
class PicardConstructorUse {
    void run(String readNameRegex, int opticalDistance, Object logger, VariantContext.Type type,
             GenotypeConcordanceCounts counts) {
        OpticalDuplicateFinder defaultFinder = new OpticalDuplicateFinder();
        OpticalDuplicateFinder configuredFinder = new OpticalDuplicateFinder(readNameRegex, opticalDistance, logger);
        GenotypeConcordanceCounts emptyCounts = new GenotypeConcordanceCounts();
        GenotypeConcordanceSummaryMetrics metrics = new GenotypeConcordanceSummaryMetrics(type, counts, "truth", "eval", true);
        DiagnosticsAndQCProgramGroup diagnostics = new DiagnosticsAndQCProgramGroup();
        IntervalsManipulationProgramGroup intervals = new IntervalsManipulationProgramGroup();
        OtherProgramGroup other = new OtherProgramGroup();
        ReadDataManipulationProgramGroup readGroup = new ReadDataManipulationProgramGroup();
        ReferenceProgramGroup referenceGroup = new ReferenceProgramGroup();
    }
}
"#,
    );
    let resolved = resolve::resolve_program(&parsed.program, parsed.diagnostics);
    let semantic = semantic_analysis::analyze_program(
        &resolved.program,
        &resolved.facts,
        &new_java2rust::facts::FactSet::default(),
    );

    for (type_name, arg_len) in [
        ("OpticalDuplicateFinder", 0),
        ("OpticalDuplicateFinder", 3),
        ("GenotypeConcordanceCounts", 0),
        ("GenotypeConcordanceSummaryMetrics", 5),
        ("DiagnosticsAndQCProgramGroup", 0),
        ("IntervalsManipulationProgramGroup", 0),
        ("OtherProgramGroup", 0),
        ("ReadDataManipulationProgramGroup", 0),
        ("ReferenceProgramGroup", 0),
    ] {
        let fact = semantic
            .facts
            .external_constructor_facts
            .iter()
            .find(|fact| fact.type_name == type_name && fact.arg_types.len() == arg_len)
            .unwrap_or_else(|| {
                panic!("expected external constructor fact for {type_name}/{arg_len}")
            });
        assert_eq!(
            fact.provenance,
            FactProvenance::BuiltinExternalModel,
            "{type_name}/{arg_len} constructor should be an audited Picard external model"
        );
    }
}

#[test]
fn disq_constructor_facts_are_builtin_models() {
    let parsed = parser::parse_java_source(
        r#"
class DisqConstructorUse {
    void run(SAMFileHeader header, JavaRDD reads, Object traversalIntervals,
             boolean traverseUnmapped, VCFHeader vcfHeader, JavaRDD variants) {
        HtsjdkReadsRdd rdd = new HtsjdkReadsRdd(header, reads);
        HtsjdkReadsTraversalParameters params = new HtsjdkReadsTraversalParameters(traversalIntervals, traverseUnmapped);
        HtsjdkVariantsRdd variantsRdd = new HtsjdkVariantsRdd(vcfHeader, variants);
    }
}
"#,
    );
    let resolved = resolve::resolve_program(&parsed.program, parsed.diagnostics);
    let semantic = semantic_analysis::analyze_program(
        &resolved.program,
        &resolved.facts,
        &new_java2rust::facts::FactSet::default(),
    );

    for type_name in [
        "HtsjdkReadsRdd",
        "HtsjdkReadsTraversalParameters",
        "HtsjdkVariantsRdd",
    ] {
        let fact = semantic
            .facts
            .external_constructor_facts
            .iter()
            .find(|fact| fact.type_name == type_name && fact.arg_types.len() == 2)
            .unwrap_or_else(|| panic!("expected external constructor fact for {type_name}/2"));
        assert_eq!(
            fact.provenance,
            FactProvenance::BuiltinExternalModel,
            "{type_name}/2 constructor should be an audited Disq external model"
        );
    }
}

#[test]
fn broad_http_constructor_facts_are_builtin_models() {
    let parsed = parser::parse_java_source(
        r#"
class BroadHttpConstructorUse {
    void run(Object retrySettings, Object timeoutSettings, Object proxySettings) {
        HttpFileSystemProviderSettings settings =
            new HttpFileSystemProviderSettings(retrySettings, timeoutSettings, proxySettings);
    }
}
"#,
    );
    let resolved = resolve::resolve_program(&parsed.program, parsed.diagnostics);
    let semantic = semantic_analysis::analyze_program(
        &resolved.program,
        &resolved.facts,
        &new_java2rust::facts::FactSet::default(),
    );

    let fact = semantic
        .facts
        .external_constructor_facts
        .iter()
        .find(|fact| {
            fact.type_name == "HttpFileSystemProviderSettings" && fact.arg_types.len() == 3
        })
        .unwrap_or_else(|| {
            panic!("expected external constructor fact for HttpFileSystemProviderSettings/3")
        });
    assert_eq!(
        fact.provenance,
        FactProvenance::BuiltinExternalModel,
        "HttpFileSystemProviderSettings/3 constructor should be an audited Broad HTTP external model"
    );
}

#[test]
fn barclay_command_line_exception_constructor_facts_are_builtin_models() {
    let parsed = parser::parse_java_source(
        r#"
class BarclayCommandLineExceptionConstructorUse {
    void run(String name, String value, String message) {
        CommandLineException root = new CommandLineException(message);
        BadArgumentValue shortBad = new BadArgumentValue(message);
        BadArgumentValue bad = new BadArgumentValue(name, value, message);
        MissingArgument missing = new MissingArgument(name, message);
    }
}
"#,
    );
    let resolved = resolve::resolve_program(&parsed.program, parsed.diagnostics);
    let semantic = semantic_analysis::analyze_program(
        &resolved.program,
        &resolved.facts,
        &new_java2rust::facts::FactSet::default(),
    );

    for (type_name, arg_len) in [
        ("CommandLineException", 1),
        ("BadArgumentValue", 1),
        ("BadArgumentValue", 3),
        ("MissingArgument", 2),
    ] {
        let fact = semantic
            .facts
            .external_constructor_facts
            .iter()
            .find(|fact| fact.type_name == type_name && fact.arg_types.len() == arg_len)
            .unwrap_or_else(|| {
                panic!("expected external constructor fact for {type_name}/{arg_len}")
            });
        assert_eq!(
            fact.provenance,
            FactProvenance::BuiltinExternalModel,
            "{type_name}/{arg_len} constructor should be an audited Barclay external model"
        );
    }
}

#[test]
fn barclay_reflection_helper_constructor_facts_are_builtin_models() {
    let parsed = parser::parse_java_source(
        r#"
class BarclayReflectionHelperConstructorUse {
    void run(Object caller, List<PluginDescriptor> plugins, Set<String> hiddenArgs) {
        ClassFinder finder = new ClassFinder();
        CommandLineArgumentParser parser = new CommandLineArgumentParser(caller, plugins, hiddenArgs);
    }
}
"#,
    );
    let resolved = resolve::resolve_program(&parsed.program, parsed.diagnostics);
    let semantic = semantic_analysis::analyze_program(
        &resolved.program,
        &resolved.facts,
        &new_java2rust::facts::FactSet::default(),
    );

    for (type_name, arg_len) in [("ClassFinder", 0), ("CommandLineArgumentParser", 3)] {
        let fact = semantic
            .facts
            .external_constructor_facts
            .iter()
            .find(|fact| fact.type_name == type_name && fact.arg_types.len() == arg_len)
            .unwrap_or_else(|| {
                panic!("expected external constructor fact for {type_name}/{arg_len}")
            });
        assert_eq!(
            fact.provenance,
            FactProvenance::BuiltinExternalModel,
            "{type_name}/{arg_len} constructor should be an audited Barclay external model"
        );
    }
}

#[test]
fn hdf5_external_constructor_facts_are_crate_mapping_pack_models() {
    let parsed = parser::parse_java_source(
        r#"
class Hdf5ConstructorUse {
    void run(File file) {
        HDF5Library library = new HDF5Library();
        HDF5File defaultMode = new HDF5File(file);
        HDF5File readOnly = new HDF5File(file, HDF5File.OpenMode.READ_ONLY);
        HDF5File create = new HDF5File(file, HDF5File.OpenMode.CREATE);
    }
}
"#,
    );
    let resolved = resolve::resolve_program(&parsed.program, parsed.diagnostics);
    let semantic = semantic_analysis::analyze_program(
        &resolved.program,
        &resolved.facts,
        &new_java2rust::facts::FactSet::default(),
    );

    for (type_name, arg_len) in [("HDF5Library", 0), ("HDF5File", 1), ("HDF5File", 2)] {
        let fact = semantic
            .facts
            .external_constructor_facts
            .iter()
            .find(|fact| fact.type_name == type_name && fact.arg_types.len() == arg_len)
            .unwrap_or_else(|| {
                panic!("expected external constructor fact for {type_name}/{arg_len}")
            });
        assert_eq!(
            fact.provenance,
            FactProvenance::ExternalLibraryPack,
            "{type_name}/{arg_len} constructor should be owned by the HDF5 crate-mapping pack"
        );
    }
}

#[test]
fn opencsv_external_constructor_facts_are_builtin_models() {
    let parsed = parser::parse_java_source(
        r#"
class OpenCsvConstructorUse {
    void run(Reader reader, Writer writer, char separator, char quote, char escape) {
        CSVReader defaultReader = new CSVReader(reader);
        CSVReader configuredReader = new CSVReader(reader, separator, quote, escape);
        CSVWriter configuredWriter = new CSVWriter(writer, separator);
        CSVWriter fullWriter = new CSVWriter(writer, separator, quote, escape);
    }
}
"#,
    );
    let resolved = resolve::resolve_program(&parsed.program, parsed.diagnostics);
    let semantic = semantic_analysis::analyze_program(
        &resolved.program,
        &resolved.facts,
        &new_java2rust::facts::FactSet::default(),
    );

    for (type_name, arg_len) in [
        ("CSVReader", 1),
        ("CSVReader", 4),
        ("CSVWriter", 2),
        ("CSVWriter", 4),
    ] {
        let fact = semantic
            .facts
            .external_constructor_facts
            .iter()
            .find(|fact| fact.type_name == type_name && fact.arg_types.len() == arg_len)
            .unwrap_or_else(|| {
                panic!("expected external constructor fact for {type_name}/{arg_len}")
            });
        assert_eq!(
            fact.provenance,
            FactProvenance::BuiltinExternalModel,
            "{type_name}/{arg_len} constructor should be an audited OpenCSV external model"
        );
    }
}

#[test]
fn hadoop_configuration_constructor_facts_are_builtin_models() {
    let parsed = parser::parse_java_source(
        r#"
class HadoopConfigurationConstructorUse {
    void run() {
        Configuration configuration = new Configuration();
    }
}
"#,
    );
    let resolved = resolve::resolve_program(&parsed.program, parsed.diagnostics);
    let semantic = semantic_analysis::analyze_program(
        &resolved.program,
        &resolved.facts,
        &new_java2rust::facts::FactSet::default(),
    );

    let fact = semantic
        .facts
        .external_constructor_facts
        .iter()
        .find(|fact| fact.type_name == "Configuration" && fact.arg_types.is_empty())
        .unwrap_or_else(|| panic!("expected external constructor fact for Configuration/0"));
    assert_eq!(
        fact.provenance,
        FactProvenance::BuiltinExternalModel,
        "Configuration/0 constructor should be an audited Hadoop external model"
    );
}

#[test]
fn apache_commons_configuration_constructor_facts_are_builtin_models() {
    let parsed = parser::parse_java_source(
        r#"
class ApacheCommonsConfigurationConstructorUse {
    void run() {
        Configurations configurations = new Configurations();
    }
}
"#,
    );
    let resolved = resolve::resolve_program(&parsed.program, parsed.diagnostics);
    let semantic = semantic_analysis::analyze_program(
        &resolved.program,
        &resolved.facts,
        &new_java2rust::facts::FactSet::default(),
    );

    let fact = semantic
        .facts
        .external_constructor_facts
        .iter()
        .find(|fact| fact.type_name == "Configurations" && fact.arg_types.is_empty())
        .unwrap_or_else(|| panic!("expected external constructor fact for Configurations/0"));
    assert_eq!(
        fact.provenance,
        FactProvenance::BuiltinExternalModel,
        "Configurations/0 constructor should be an audited Apache Commons Configuration external model"
    );
}

#[test]
fn sqlite_constructor_facts_are_crate_mapping_pack_models() {
    let parsed = parser::parse_java_source(
        r#"
class SqliteConstructorUse {
    void run() {
        SQLiteConfig config = new SQLiteConfig();
    }
}
"#,
    );
    let resolved = resolve::resolve_program(&parsed.program, parsed.diagnostics);
    let semantic = semantic_analysis::analyze_program(
        &resolved.program,
        &resolved.facts,
        &new_java2rust::facts::FactSet::default(),
    );

    let fact = semantic
        .facts
        .external_constructor_facts
        .iter()
        .find(|fact| fact.type_name == "SQLiteConfig" && fact.arg_types.is_empty())
        .unwrap_or_else(|| panic!("expected external constructor fact for SQLiteConfig/0"));
    assert_eq!(
        fact.provenance,
        FactProvenance::ExternalLibraryPack,
        "SQLiteConfig/0 constructor should be owned by the SQLite crate-mapping pack"
    );
}

#[test]
fn spark_constructor_facts_are_builtin_models() {
    let parsed = parser::parse_java_source(
        r#"
class SparkConstructorUse {
    void run(int partitions, SparkConf conf, Object rdd, Object tag) {
        SparkConf defaultConf = new SparkConf();
        HashPartitioner partitioner = new HashPartitioner(partitions);
        JavaSparkContext context = new JavaSparkContext(conf);
        JavaRDD reads = new JavaRDD(rdd, tag);
        PartitionGroup group = new PartitionGroup("worker");
        RowMatrix matrix = new RowMatrix(rdd);
    }
}
"#,
    );
    let resolved = resolve::resolve_program(&parsed.program, parsed.diagnostics);
    let semantic = semantic_analysis::analyze_program(
        &resolved.program,
        &resolved.facts,
        &new_java2rust::facts::FactSet::default(),
    );

    for (type_name, arg_len) in [
        ("SparkConf", 0),
        ("HashPartitioner", 1),
        ("JavaSparkContext", 1),
        ("JavaRDD", 2),
        ("PartitionGroup", 1),
        ("RowMatrix", 1),
    ] {
        let fact = semantic
            .facts
            .external_constructor_facts
            .iter()
            .find(|fact| fact.type_name == type_name && fact.arg_types.len() == arg_len)
            .unwrap_or_else(|| {
                panic!("expected external constructor fact for {type_name}/{arg_len}")
            });
        assert_eq!(
            fact.provenance,
            FactProvenance::BuiltinExternalModel,
            "{type_name}/{arg_len} constructor should be an audited Spark external model"
        );
    }
}

#[test]
fn guava_constructor_facts_are_builtin_models() {
    let parsed = parser::parse_java_source(
        r#"
class GuavaConstructorUse {
    void run() {
        ThreadFactoryBuilder builder = new ThreadFactoryBuilder();
    }
}
"#,
    );
    let resolved = resolve::resolve_program(&parsed.program, parsed.diagnostics);
    let semantic = semantic_analysis::analyze_program(
        &resolved.program,
        &resolved.facts,
        &new_java2rust::facts::FactSet::default(),
    );

    let fact = semantic
        .facts
        .external_constructor_facts
        .iter()
        .find(|fact| fact.type_name == "ThreadFactoryBuilder" && fact.arg_types.is_empty())
        .unwrap_or_else(|| panic!("expected external constructor fact for ThreadFactoryBuilder/0"));
    assert_eq!(
        fact.provenance,
        FactProvenance::BuiltinExternalModel,
        "ThreadFactoryBuilder/0 constructor should be an audited Guava external model"
    );
}

#[test]
fn reflections_constructor_facts_are_builtin_models() {
    let parsed = parser::parse_java_source(
        r#"
class ReflectionsConstructorUse {
    void run(String packageName) {
        Reflections reflections = new Reflections(packageName);
    }
}
"#,
    );
    let resolved = resolve::resolve_program(&parsed.program, parsed.diagnostics);
    let semantic = semantic_analysis::analyze_program(
        &resolved.program,
        &resolved.facts,
        &new_java2rust::facts::FactSet::default(),
    );

    let fact = semantic
        .facts
        .external_constructor_facts
        .iter()
        .find(|fact| fact.type_name == "Reflections" && fact.arg_types.len() == 1)
        .unwrap_or_else(|| panic!("expected external constructor fact for Reflections/1"));
    assert_eq!(
        fact.provenance,
        FactProvenance::BuiltinExternalModel,
        "Reflections/1 constructor should be an audited Reflections external model"
    );
}

#[test]
fn apache_http_constructor_facts_are_builtin_models() {
    let parsed = parser::parse_java_source(
        r#"
class ApacheHttpConstructorUse {
    void run(URI uri) {
        HttpGet request = new HttpGet(uri);
        PoolingHttpClientConnectionManager manager = new PoolingHttpClientConnectionManager();
    }
}
"#,
    );
    let resolved = resolve::resolve_program(&parsed.program, parsed.diagnostics);
    let semantic = semantic_analysis::analyze_program(
        &resolved.program,
        &resolved.facts,
        &new_java2rust::facts::FactSet::default(),
    );

    for (type_name, arg_len) in [("HttpGet", 1), ("PoolingHttpClientConnectionManager", 0)] {
        let fact = semantic
            .facts
            .external_constructor_facts
            .iter()
            .find(|fact| fact.type_name == type_name && fact.arg_types.len() == arg_len)
            .unwrap_or_else(|| {
                panic!("expected external constructor fact for {type_name}/{arg_len}")
            });
        assert_eq!(
            fact.provenance,
            FactProvenance::BuiltinExternalModel,
            "{type_name}/{arg_len} constructor should be an audited Apache HTTP external model"
        );
    }
}

#[test]
fn kryo_serializer_constructor_facts_are_builtin_models() {
    let parsed = parser::parse_java_source(
        r#"
class KryoSerializerConstructorUse {
    void run(InputStream inputStream, OutputStream outputStream, Class<?> recordClass) {
        Kryo kryo = new Kryo();
        Input input = new Input(inputStream);
        Output output = new Output(outputStream);
        FieldSerializer serializer = new FieldSerializer(kryo, recordClass);
        ImmutableMapSerializer immutableMapSerializer = new ImmutableMapSerializer();
    }
}
"#,
    );
    let resolved = resolve::resolve_program(&parsed.program, parsed.diagnostics);
    let semantic = semantic_analysis::analyze_program(
        &resolved.program,
        &resolved.facts,
        &new_java2rust::facts::FactSet::default(),
    );

    for (type_name, arg_len) in [
        ("Kryo", 0),
        ("Input", 1),
        ("Output", 1),
        ("FieldSerializer", 2),
        ("ImmutableMapSerializer", 0),
    ] {
        let fact = semantic
            .facts
            .external_constructor_facts
            .iter()
            .find(|fact| fact.type_name == type_name && fact.arg_types.len() == arg_len)
            .unwrap_or_else(|| {
                panic!("expected external constructor fact for {type_name}/{arg_len}")
            });
        assert_eq!(
            fact.provenance,
            FactProvenance::BuiltinExternalModel,
            "{type_name}/{arg_len} constructor should be an audited Kryo external model"
        );
    }
}

#[test]
fn genomicsdb_reader_constructor_facts_are_builtin_models() {
    let parsed = parser::parse_java_source(
        r#"
class GenomicsDbReaderConstructorUse {
    void run(ExportConfiguration.Builder builder, FeatureCodec codec, Optional<String> reference,
             ImportConfiguration importConfig, Object config, Object sampleMap, boolean validate,
             boolean consolidate, long segmentSize, Object stream, int batchSize, Object output) {
        GenomicsDBFeatureReader reader = new GenomicsDBFeatureReader(builder, codec, reference);
        GenomicsDBImporter importer = new GenomicsDBImporter(importConfig);
        ImportConfig model = new ImportConfig(config, sampleMap, validate, consolidate, segmentSize, stream, batchSize, output);
    }
}
"#,
    );
    let resolved = resolve::resolve_program(&parsed.program, parsed.diagnostics);
    let semantic = semantic_analysis::analyze_program(
        &resolved.program,
        &resolved.facts,
        &new_java2rust::facts::FactSet::default(),
    );

    for (type_name, arg_len) in [
        ("GenomicsDBFeatureReader", 3),
        ("GenomicsDBImporter", 1),
        ("ImportConfig", 8),
    ] {
        let fact = semantic
            .facts
            .external_constructor_facts
            .iter()
            .find(|fact| fact.type_name == type_name && fact.arg_types.len() == arg_len)
            .unwrap_or_else(|| {
                panic!("expected external constructor fact for {type_name}/{arg_len}")
            });
        assert_eq!(
            fact.provenance,
            FactProvenance::BuiltinExternalModel,
            "{type_name}/{arg_len} constructor should be an audited GenomicsDB external model"
        );
    }
}

#[test]
fn avro_constructor_facts_are_builtin_models() {
    let parsed = parser::parse_java_source(
        r#"
class AvroConstructorUse {
    void run(Schema schema) {
        GenericDatumReader reader = new GenericDatumReader(schema);
    }
}
"#,
    );
    let resolved = resolve::resolve_program(&parsed.program, parsed.diagnostics);
    let semantic = semantic_analysis::analyze_program(
        &resolved.program,
        &resolved.facts,
        &new_java2rust::facts::FactSet::default(),
    );

    let fact = semantic
        .facts
        .external_constructor_facts
        .iter()
        .find(|fact| fact.type_name == "GenericDatumReader" && fact.arg_types.len() == 1)
        .unwrap_or_else(|| panic!("expected external constructor fact for GenericDatumReader/1"));
    assert_eq!(
        fact.provenance,
        FactProvenance::BuiltinExternalModel,
        "GenericDatumReader/1 constructor should be an audited Avro external model"
    );
}

#[test]
fn bwa_native_wrapper_constructor_facts_are_builtin_models() {
    let parsed = parser::parse_java_source(
        r#"
class BwaNativeWrapperConstructorUse {
    void run(String indexImage) {
        BwaMemIndex index = new BwaMemIndex(indexImage);
        BwaMemAligner aligner = new BwaMemAligner(index);
    }
}
"#,
    );
    let resolved = resolve::resolve_program(&parsed.program, parsed.diagnostics);
    let semantic = semantic_analysis::analyze_program(
        &resolved.program,
        &resolved.facts,
        &new_java2rust::facts::FactSet::default(),
    );

    for (type_name, arg_len) in [("BwaMemIndex", 1), ("BwaMemAligner", 1)] {
        let fact = semantic
            .facts
            .external_constructor_facts
            .iter()
            .find(|fact| fact.type_name == type_name && fact.arg_types.len() == arg_len)
            .unwrap_or_else(|| {
                panic!("expected external constructor fact for {type_name}/{arg_len}")
            });
        assert_eq!(
            fact.provenance,
            FactProvenance::BuiltinExternalModel,
            "{type_name}/{arg_len} constructor should be an audited BWA native-wrapper external model"
        );
    }
}

#[test]
fn jgrapht_algorithm_constructor_facts_are_builtin_models() {
    let parsed = parser::parse_java_source(
        r#"
class JGraphTAlgorithmConstructorUse {
    void run(Graph graph, Class edgeClass, Object p1, Object p2, Object p3, Object p4, Object p5) {
        IntegerComponentNameProvider componentNames = new IntegerComponentNameProvider();
        SimpleGraph simpleGraph = new SimpleGraph(edgeClass);
        ConnectivityInspector inspector = new ConnectivityInspector(graph);
        CycleDetector detector = new CycleDetector(graph);
        DefaultDirectedWeightedGraph weightedGraph = new DefaultDirectedWeightedGraph(edgeClass);
        DOTExporter exporter = new DOTExporter(p1, p2, p3, p4, p5);
    }
}
"#,
    );
    let resolved = resolve::resolve_program(&parsed.program, parsed.diagnostics);
    let semantic = semantic_analysis::analyze_program(
        &resolved.program,
        &resolved.facts,
        &new_java2rust::facts::FactSet::default(),
    );

    for type_name in [
        "IntegerComponentNameProvider",
        "SimpleGraph",
        "ConnectivityInspector",
        "CycleDetector",
        "DefaultDirectedWeightedGraph",
    ] {
        let fact = semantic
            .facts
            .external_constructor_facts
            .iter()
            .find(|fact| {
                fact.type_name == type_name
                    && fact.arg_types.len()
                        == if type_name == "IntegerComponentNameProvider" {
                            0
                        } else {
                            1
                        }
            })
            .unwrap_or_else(|| panic!("expected external constructor fact for {type_name}"));
        assert_eq!(
            fact.provenance,
            FactProvenance::BuiltinExternalModel,
            "{type_name} constructor should be an audited JGraphT external model"
        );
    }

    let dot_exporter = semantic
        .facts
        .external_constructor_facts
        .iter()
        .find(|fact| fact.type_name == "DOTExporter" && fact.arg_types.len() == 5)
        .unwrap_or_else(|| panic!("expected external constructor fact for DOTExporter/5"));
    assert_eq!(
        dot_exporter.provenance,
        FactProvenance::BuiltinExternalModel,
        "DOTExporter/5 constructor should be an audited JGraphT external model"
    );
}

#[test]
fn external_field_facts_explain_missing_library_fields_only() {
    let parsed = parser::parse_java_source(
        r#"
class ExternalFieldUse {
    int run(ExternalBox external) {
        int direct = external.count;
        int constant = Constants.MAX_COUNT;
        return direct + constant;
    }
}

class LocalBox {
    int count;

    int run(LocalBox local) {
        return local.missingCount;
    }
}
"#,
    );
    let resolved = resolve::resolve_program(&parsed.program, parsed.diagnostics);
    let semantic = semantic_analysis::analyze_program(
        &resolved.program,
        &resolved.facts,
        &new_java2rust::facts::FactSet::default(),
    );

    let external_count = semantic
        .facts
        .external_field_for("ExternalBox", "count")
        .unwrap();
    assert_eq!(
        external_count.provenance,
        FactProvenance::ConservativeFallback
    );
    let external_constant = semantic
        .facts
        .external_field_for("Constants", "MAX_COUNT")
        .unwrap();
    assert_eq!(
        external_constant.provenance,
        FactProvenance::ConservativeFallback
    );
    assert!(
        semantic
            .facts
            .external_field_for("LocalBox", "missingCount")
            .is_none()
    );
}

#[test]
fn known_external_field_constants_do_not_use_conservative_field_fallbacks() {
    let parsed = parser::parse_java_source(
        r#"
class KnownExternalFieldConstantUse {
    void run() {
        int max = Integer.MAX_VALUE;
        int size = Long.SIZE;
        int bytes = Byte.BYTES;
        boolean yes = Boolean.TRUE;
        String index = FileExtensions.BAI_INDEX;
        String bam = FileExtensions.BAM;
        byte[] emptyBytes = ArrayUtils.EMPTY_BYTE_ARRAY;
        int[] emptyInts = ArrayUtils.EMPTY_INT_ARRAY;
        ByteOrder order = ByteOrder.BIG_ENDIAN;
        Cigar cigar = SAMRecord.NO_ALIGNMENT_CIGAR;
        int noAlignmentStart = SAMRecord.NO_ALIGNMENT_START;
        int noAlignmentReferenceIndex = SAMRecord.NO_ALIGNMENT_REFERENCE_INDEX;
        String noAlignmentReferenceName = SAMRecord.NO_ALIGNMENT_REFERENCE_NAME;
        Allele allele = Allele.NO_CALL;
        Allele deletion = Allele.SV_SIMPLE_DEL;
        Allele duplication = Allele.SV_SIMPLE_DUP;
        OpenMode create = OpenMode.CREATE;
        LogLevel debug = LogLevel.DEBUG;
        SortOrder coordinate = SortOrder.coordinate;
        SortOrder unsorted = SortOrder.unsorted;
        List emptyList = Collections.EMPTY_LIST;
        Set emptySet = Collections.EMPTY_SET;
        Map emptyMap = Collections.EMPTY_MAP;
        CigarOperator matchOrMismatch = CigarOperator.MATCH_OR_MISMATCH;
        VCFHeaderLineType headerString = VCFHeaderLineType.String;
        VCFHeaderLineType headerInteger = VCFHeaderLineType.Integer;
        VCFHeaderLineCount headerCount = VCFHeaderLineCount.A;
        VCFHeaderLineCount headerUnbounded = VCFHeaderLineCount.UNBOUNDED;
        GenotypeType genotype = GenotypeType.HET;
        Strand strand = Strand.POSITIVE;
        CallState callState = GenotypeConcordanceStates.CallState.HOM_REF;
        TruthState truthState = GenotypeConcordanceStates.TruthState.NO_CALL;
        Type variantType = VariantContext.Type.INDEL;
        SAMFlag samFlag = SAMFlag.SECONDARY_ALIGNMENT;
        VCFHeaderVersion headerVersion = VCFHeaderVersion.VCF4_2;
        DuplicateTaggingPolicy duplicateTaggingPolicy = MarkDuplicates.DuplicateTaggingPolicy.DontTag;
        int blockHeaderLength = BlockCompressedStreamConstants.BLOCK_HEADER_LENGTH;
        String pass = VCFConstants.PASSES_FILTERS_v4;
        String genotypeAlleleDepths = VCFConstants.GENOTYPE_ALLELE_DEPTHS;
        String svType = VCFConstants.SVTYPE;
        String unfiltered = VCFConstants.UNFILTERED;
        char separator = VCFConstants.INFO_FIELD_ARRAY_SEPARATOR_CHAR;
        int maxGenotypeQual = VCFConstants.MAX_GENOTYPE_QUAL;
        int maxDiploidAltAlleles = GenotypeLikelihoods.MAX_DIPLOID_ALT_ALLELES_THAT_CAN_BE_GENOTYPED;
        double noLog10PError = CommonInfo.NO_LOG10_PERROR;
        byte[] emptyGzipBlock = BlockCompressedStreamConstants.EMPTY_GZIP_BLOCK;
        int blockLengthOffset = BlockCompressedStreamConstants.BLOCK_LENGTH_OFFSET;
        int maxPhredScore = SAMUtils.MAX_PHRED_SCORE;
        int unknownSequenceLength = SAMSequenceRecord.UNKNOWN_SEQUENCE_LENGTH;
        int noMappingQuality = SAMRecord.NO_MAPPING_QUALITY;
        byte[] nullQuals = SAMRecord.NULL_QUALS;
        String nullQualsString = SAMRecord.NULL_QUALS_STRING;
        String positional = TemplateProperties.ARGUMENTS_POSITIONAL;
        String defaultValue = TemplateProperties.ARGUMENT_DEFAULT_VALUE;
        String argumentType = TemplateProperties.ARGUMENT_TYPE;
        ReadsFormatWriteOption bam = ReadsFormatWriteOption.BAM;
        String genotypeConcordance = GenotypeConcordanceSummaryMetrics.GENOTYPE_CONCORDANCE;
        String hetPpv = GenotypeConcordanceSummaryMetrics.HET_PPV;
        String varSpecificity = GenotypeConcordanceSummaryMetrics.VAR_SPECIFICITY;
        HEADER_FIELDS headerField = VCFHeader.HEADER_FIELDS.INFO;
        String referenceKey = VCFHeader.REFERENCE_KEY;
        TimeUnit seconds = TimeUnit.SECONDS;
        OutputType outputType = VariantContextWriterBuilder.OutputType;
        String reservedTag = ReservedTagConstants.XT;
        byte[] validBases = SequenceUtil.VALID_BASES_UPPER;
        int fixedBlockSize = BAMFileConstants.FIXED_BLOCK_SIZE;
        String configFile = GATKConfig.CONFIG_FILE_VARIABLE_FILE_NAME;
        String cloudScheme = CloudStorageFileSystem.URI_SCHEME;
        int uninitializedReferenceId = ReferenceContext.UNINITIALIZED_REFERENCE_ID;
        Object binaryTagValue = SAMBinaryTagAndValue.value;
        byte[] readBases = ReadDataHolder.readBases;
        byte[] readQuals = ReadDataHolder.readQuals;
        byte[] insertionGop = ReadDataHolder.insertionGOP;
        byte[] deletionGop = ReadDataHolder.deletionGOP;
        byte[] overallGcp = ReadDataHolder.overallGCP;
        byte[] haplotypeBases = HaplotypeDataHolder.haplotypeBases;
        AVXLevel avxLevel = PDHMMNativeArguments.avxLevel;
        OpenMPSetting openMPSetting = PDHMMNativeArguments.openMPSetting;
        PairOrientation orientation = SamPairUtil.PairOrientation.FR;
    }
}
"#,
    );
    let resolved = resolve::resolve_program(&parsed.program, parsed.diagnostics);
    let semantic = semantic_analysis::analyze_program(
        &resolved.program,
        &resolved.facts,
        &new_java2rust::facts::FactSet::default(),
    );

    for (type_name, field_name) in [
        ("Integer", "MAX_VALUE"),
        ("Long", "SIZE"),
        ("Byte", "BYTES"),
        ("Boolean", "TRUE"),
        ("FileExtensions", "BAI_INDEX"),
        ("FileExtensions", "BAM"),
        ("ArrayUtils", "EMPTY_BYTE_ARRAY"),
        ("ArrayUtils", "EMPTY_INT_ARRAY"),
        ("ByteOrder", "BIG_ENDIAN"),
        ("SAMRecord", "NO_ALIGNMENT_CIGAR"),
        ("SAMRecord", "NO_ALIGNMENT_START"),
        ("SAMRecord", "NO_ALIGNMENT_REFERENCE_INDEX"),
        ("SAMRecord", "NO_ALIGNMENT_REFERENCE_NAME"),
        ("SAMRecord", "UNKNOWN_MAPPING_QUALITY"),
        ("Allele", "NO_CALL"),
        ("Allele", "SV_SIMPLE_DEL"),
        ("Allele", "SV_SIMPLE_DUP"),
        ("OpenMode", "CREATE"),
        ("LogLevel", "DEBUG"),
        ("SortOrder", "coordinate"),
        ("SortOrder", "unsorted"),
        ("Collections", "EMPTY_LIST"),
        ("Collections", "EMPTY_SET"),
        ("Collections", "EMPTY_MAP"),
        ("CigarOperator", "MATCH_OR_MISMATCH"),
        ("VCFHeaderLineType", "String"),
        ("VCFHeaderLineType", "Integer"),
        ("VCFHeaderLineCount", "A"),
        ("VCFHeaderLineCount", "UNBOUNDED"),
        ("GenotypeType", "HET"),
        ("Strand", "POSITIVE"),
        ("GenotypeConcordanceStates", "CallState"),
        ("CallState", "HOM_REF"),
        ("GenotypeConcordanceStates", "TruthState"),
        ("TruthState", "NO_CALL"),
        ("VariantContext", "Type"),
        ("Type", "INDEL"),
        ("SAMFlag", "SECONDARY_ALIGNMENT"),
        ("VCFHeaderVersion", "VCF4_2"),
        ("MarkDuplicates", "DuplicateTaggingPolicy"),
        ("DuplicateTaggingPolicy", "DontTag"),
        ("BlockCompressedStreamConstants", "BLOCK_HEADER_LENGTH"),
        ("VCFConstants", "PASSES_FILTERS_v4"),
        ("VCFConstants", "GENOTYPE_ALLELE_DEPTHS"),
        ("VCFConstants", "SVTYPE"),
        ("VCFConstants", "UNFILTERED"),
        ("VCFConstants", "MISSING_VALUE_v4"),
        ("VCFConstants", "INFO_FIELD_ARRAY_SEPARATOR_CHAR"),
        ("VCFConstants", "MAX_GENOTYPE_QUAL"),
        (
            "GenotypeLikelihoods",
            "MAX_DIPLOID_ALT_ALLELES_THAT_CAN_BE_GENOTYPED",
        ),
        ("CommonInfo", "NO_LOG10_PERROR"),
        ("BlockCompressedStreamConstants", "EMPTY_GZIP_BLOCK"),
        ("BlockCompressedStreamConstants", "BLOCK_LENGTH_OFFSET"),
        ("SAMUtils", "MAX_PHRED_SCORE"),
        ("SAMSequenceRecord", "UNKNOWN_SEQUENCE_LENGTH"),
        ("SAMRecord", "NO_MAPPING_QUALITY"),
        ("SAMRecord", "NULL_QUALS"),
        ("SAMRecord", "NULL_QUALS_STRING"),
        ("TemplateProperties", "ARGUMENTS_POSITIONAL"),
        ("TemplateProperties", "ARGUMENT_DEFAULT_VALUE"),
        ("TemplateProperties", "ARGUMENT_TYPE"),
        ("ReadsFormatWriteOption", "BAM"),
        ("GenotypeConcordanceSummaryMetrics", "GENOTYPE_CONCORDANCE"),
        ("GenotypeConcordanceSummaryMetrics", "HET_PPV"),
        ("GenotypeConcordanceSummaryMetrics", "VAR_SPECIFICITY"),
        ("VCFHeader", "HEADER_FIELDS"),
        ("HEADER_FIELDS", "INFO"),
        ("VCFHeader", "REFERENCE_KEY"),
        ("TimeUnit", "SECONDS"),
        ("VariantContextWriterBuilder", "OutputType"),
        ("ReservedTagConstants", "XT"),
        ("SequenceUtil", "VALID_BASES_UPPER"),
        ("BAMFileConstants", "FIXED_BLOCK_SIZE"),
        ("GATKConfig", "CONFIG_FILE_VARIABLE_FILE_NAME"),
        ("CloudStorageFileSystem", "URI_SCHEME"),
        ("ReferenceContext", "UNINITIALIZED_REFERENCE_ID"),
        ("SamPairUtil", "PairOrientation"),
        ("PairOrientation", "FR"),
        ("Options", "INDEX_ON_THE_FLY"),
        ("Options", "DO_NOT_WRITE_GENOTYPES"),
        ("SWOverhangStrategy", "INDEL"),
        ("SWOverhangStrategy", "SOFTCLIP"),
        ("JexlVCMatchExp", "name"),
        ("SAMTagAndValue", "tag"),
        ("SAMTagAndValue", "value"),
        ("SAMBinaryTagAndValue", "value"),
        ("BatchCompletionCallbackFunctionArgument", "batchCount"),
        ("BatchCompletionCallbackFunctionArgument", "totalBatchCount"),
        ("ReadDataHolder", "readBases"),
        ("ReadDataHolder", "readQuals"),
        ("ReadDataHolder", "insertionGOP"),
        ("ReadDataHolder", "deletionGOP"),
        ("ReadDataHolder", "overallGCP"),
        ("HaplotypeDataHolder", "haplotypeBases"),
        ("PDHMMNativeArguments", "avxLevel"),
        ("PDHMMNativeArguments", "openMPSetting"),
        ("Coordinates", "GenomicsDBColumn"),
        ("Coordinates", "ContigPosition"),
    ] {
        if let Some(fact) = semantic.facts.external_field_for(type_name, field_name) {
            assert_eq!(
                fact.provenance,
                FactProvenance::BuiltinExternalModel,
                "known external field {type_name}.{field_name} should not use conservative fallback"
            );
        }
    }
}

#[test]
fn external_field_provenance_uses_builtin_model_for_known_field_types() {
    for (type_name, field_name) in [
        ("Integer", "MAX_VALUE"),
        ("Long", "SIZE"),
        ("Byte", "BYTES"),
        ("Boolean", "FALSE"),
        ("FileExtensions", "BAM"),
        ("ArrayUtils", "EMPTY_BYTE_ARRAY"),
        ("ArrayUtils", "EMPTY_INT_ARRAY"),
        ("ByteOrder", "LITTLE_ENDIAN"),
        ("SAMRecord", "NO_ALIGNMENT_CIGAR"),
        ("SAMRecord", "NO_ALIGNMENT_START"),
        ("SAMRecord", "NO_ALIGNMENT_REFERENCE_INDEX"),
        ("SAMRecord", "NO_ALIGNMENT_REFERENCE_NAME"),
        ("Allele", "NO_CALL"),
        ("Allele", "SV_SIMPLE_DEL"),
        ("Allele", "SV_SIMPLE_DUP"),
        ("OpenMode", "CREATE"),
        ("LogLevel", "DEBUG"),
        ("SortOrder", "coordinate"),
        ("SortOrder", "unsorted"),
        ("Collections", "EMPTY_LIST"),
        ("Collections", "EMPTY_SET"),
        ("Collections", "EMPTY_MAP"),
        ("CigarOperator", "MATCH_OR_MISMATCH"),
        ("VCFHeaderLineType", "String"),
        ("VCFHeaderLineType", "Integer"),
        ("VCFHeaderLineCount", "A"),
        ("VCFHeaderLineCount", "UNBOUNDED"),
        ("GenotypeType", "HET"),
        ("Strand", "POSITIVE"),
        ("GenotypeConcordanceStates", "CallState"),
        ("CallState", "HOM_REF"),
        ("GenotypeConcordanceStates", "TruthState"),
        ("TruthState", "NO_CALL"),
        ("VariantContext", "Type"),
        ("Type", "INDEL"),
        ("SAMFlag", "SECONDARY_ALIGNMENT"),
        ("VCFHeaderVersion", "VCF4_2"),
        ("MarkDuplicates", "DuplicateTaggingPolicy"),
        ("DuplicateTaggingPolicy", "DontTag"),
        ("BlockCompressedStreamConstants", "BLOCK_HEADER_LENGTH"),
        ("VCFConstants", "PASSES_FILTERS_v4"),
        ("VCFConstants", "GENOTYPE_ALLELE_DEPTHS"),
        ("VCFConstants", "SVTYPE"),
        ("VCFConstants", "UNFILTERED"),
        ("VCFConstants", "INFO_FIELD_ARRAY_SEPARATOR_CHAR"),
        ("VCFConstants", "MAX_GENOTYPE_QUAL"),
        (
            "GenotypeLikelihoods",
            "MAX_DIPLOID_ALT_ALLELES_THAT_CAN_BE_GENOTYPED",
        ),
        ("CommonInfo", "NO_LOG10_PERROR"),
        ("BlockCompressedStreamConstants", "EMPTY_GZIP_BLOCK"),
        ("BlockCompressedStreamConstants", "BLOCK_LENGTH_OFFSET"),
        ("SAMUtils", "MAX_PHRED_SCORE"),
        ("SAMSequenceRecord", "UNKNOWN_SEQUENCE_LENGTH"),
        ("SAMRecord", "NO_MAPPING_QUALITY"),
        ("SAMRecord", "NULL_QUALS"),
        ("SAMRecord", "NULL_QUALS_STRING"),
        ("TemplateProperties", "ARGUMENTS_POSITIONAL"),
        ("TemplateProperties", "ARGUMENT_DEFAULT_VALUE"),
        ("TemplateProperties", "ARGUMENT_TYPE"),
        ("ReadsFormatWriteOption", "BAM"),
        ("GenotypeConcordanceSummaryMetrics", "GENOTYPE_CONCORDANCE"),
        ("GenotypeConcordanceSummaryMetrics", "HET_PPV"),
        ("GenotypeConcordanceSummaryMetrics", "VAR_SPECIFICITY"),
        ("VCFHeader", "HEADER_FIELDS"),
        ("HEADER_FIELDS", "INFO"),
        ("VCFHeader", "REFERENCE_KEY"),
        ("TimeUnit", "SECONDS"),
        ("VariantContextWriterBuilder", "OutputType"),
        ("ReservedTagConstants", "XT"),
        ("SequenceUtil", "VALID_BASES_UPPER"),
        ("BAMFileConstants", "FIXED_BLOCK_SIZE"),
        ("GATKConfig", "CONFIG_FILE_VARIABLE_FILE_NAME"),
        ("CloudStorageFileSystem", "URI_SCHEME"),
        ("ReferenceContext", "UNINITIALIZED_REFERENCE_ID"),
        ("HttpFileSystemProvider", "SCHEME"),
        ("HttpsFileSystemProvider", "SCHEME"),
        ("Slice", "EMBEDDED_REFERENCE_ABSENT_CONTENT_ID"),
        ("TabixFormat", "GENERIC_FLAGS"),
        ("TabixFormat", "ZERO_BASED"),
        ("Allele", "NON_REF_STRING"),
        ("Allele", "UNSPECIFIED_ALTERNATE_ALLELE_STRING"),
        ("File", "separator"),
        ("File", "separatorChar"),
        ("VCFConstants", "NULL_ALLELE"),
        ("VariantContext", "NO_LOG10_PERROR"),
        (
            "ReadLikelihoodCalculationEngine",
            "DEFAULT_EXPECTED_ERROR_RATE_PER_BASE",
        ),
        ("SpecialArgumentsCollection", "SHOW_HIDDEN"),
        (
            "BlockCompressedStreamConstants",
            "DEFAULT_COMPRESSION_LEVEL",
        ),
        ("SBIIndexWriter", "DEFAULT_GRANULARITY"),
        ("TarArchiveOutputStream", "BIGNUMBER_STAR"),
        ("Spliterator", "IMMUTABLE"),
        ("Spliterator", "NONNULL"),
        ("StandardOpenOption", "CREATE_NEW"),
        ("AccessMode", "READ"),
        ("AccessMode", "WRITE"),
        ("PosixFilePermission", "GROUP_EXECUTE"),
        ("HttpMethod", "GET"),
        ("MapperFeature", "ACCEPT_CASE_INSENSITIVE_PROPERTIES"),
        ("DataFormat", "AVRO"),
        ("DataSeriesType", "BYTE_ARRAY"),
        ("CloudStorageConfiguration", "DEFAULT"),
        ("NullOutputStream", "INSTANCE"),
        ("NullOutputStream", "NULL_OUTPUT_STREAM"),
        ("TarArchiveOutputStream", "LONGFILE_GNU"),
        ("Math", "PI"),
        ("SystemUtils", "OS_ARCH"),
        ("SystemUtils", "OS_NAME"),
        ("SystemUtils", "IS_OS_MAC"),
        ("SystemUtils", "IS_OS_LINUX"),
        ("Defaults", "COMPRESSION_LEVEL"),
        ("Defaults", "USE_ASYNC_IO_READ_FOR_SAMTOOLS"),
        ("Defaults", "USE_ASYNC_IO_WRITE_FOR_SAMTOOLS"),
        ("Defaults", "USE_ASYNC_IO_WRITE_FOR_TRIBBLE"),
        ("RetryHandler", "DEFAULT_RETRYABLE_HTTP_CODES"),
        ("RetryHandler", "DEFAULT_RETRYABLE_EXCEPTIONS"),
        ("RetryHandler", "DEFALT_RETRYABLE_MESSAGES"),
        ("ValidationStringency", "LENIENT"),
        ("IndexFactory", "IndexType"),
        ("IndexFactory", "IndexBalanceApproach"),
        ("SamReaderFactory", "Option"),
        ("VCFCompoundHeaderLine", "SupportedHeaderLineType"),
        ("BwaMemAligner", "MEM_F_ALL"),
        ("FileCardinalityWriteOption", "MULTIPLE"),
        ("FileCardinalityWriteOption", "SINGLE"),
        ("GenotypesContext", "NO_GENOTYPES"),
        ("FeatureCodecHeader", "NO_HEADER_END"),
        ("MessageDigestAlgorithms", "SHA_256"),
        ("SAMReadGroupRecord", "READ_GROUP_ID_TAG"),
        ("SAMReadGroupRecord", "READ_GROUP_SAMPLE_TAG"),
        ("NullWriter", "INSTANCE"),
        ("DeserializationFeature", "UNWRAP_ROOT_VALUE"),
        ("JexlMissingValueTreatment", "TREAT_AS_MATCH"),
        ("JexlMissingValueTreatment", "TREAT_AS_MISMATCH"),
        ("WDLWorkUnitHandler", "POSITIONAL_ARGS"),
        ("Genotype", "UNPHASED_ALLELE_SEPARATOR"),
        ("GenomicsDBImportConfiguration", "ImportConfiguration"),
        ("GenomicsDBImportConfiguration", "Partition"),
        ("GenomicsDBVidMapProto", "VidMappingPB"),
        ("GenomicsDBVidMapProto", "FieldLengthDescriptorComponentPB"),
        ("ReadSession", "TableReadOptions"),
        ("Option", "CACHE_FILE_BASED_INDEXES"),
        ("Option", "INCLUDE_SOURCE_IN_RECORDS"),
        ("SupportedHeaderLineType", "FORMAT"),
        ("SupportedHeaderLineType", "INFO"),
        ("GenotypeConcordance", "CONTINGENCY_STATE_TAG"),
        ("VariantContextWriterBuilder", "DEFAULT_OPTIONS"),
        ("HttpFileSystemProviderSettings", "DEFAULT_SETTINGS"),
        ("Level", "DEBUG"),
        ("Level", "INFO"),
        ("Level", "ERROR"),
        ("Locale", "ENGLISH"),
        ("FastqConstants", "FIRST_OF_PAIR"),
        ("FastqConstants", "SECOND_OF_PAIR"),
        ("ElementKind", "FIELD"),
        ("GenomicsDBExportConfiguration", "ExportConfiguration"),
        ("ReadLikelihoodCalculationEngine", "MAX_REPEAT_LENGTH"),
        ("ReservedTagConstants", "PROGRAM_GROUP_ID"),
        ("ReservedTagConstants", "READ_GROUP_ID"),
        ("SAMRecord", "NULL_SEQUENCE"),
        ("IndexType", "TABIX"),
        ("IndexBalanceApproach", "FOR_SEEK_TIME"),
        ("SamPairUtil", "PairOrientation"),
        ("PairOrientation", "FR"),
        ("FieldValue", "Attribute"),
        ("Attribute", "PRIMITIVE"),
        ("QueryJobConfiguration", "Priority"),
        ("Priority", "BATCH"),
        ("BlockCompressedInputStream", "FileTermination"),
        ("FileTermination", "DEFECTIVE"),
        ("SAMFileHeader", "GroupOrder"),
        ("GroupOrder", "query"),
        ("StructuralVariantType", "DEL"),
        ("GoalType", "MAXIMIZE"),
        ("Percentile", "EstimationType"),
        ("EstimationType", "R_1"),
        ("ReadEnds", "FR"),
        ("StandardCharsets", "US_ASCII"),
        ("Pattern", "CASE_INSENSITIVE"),
        ("RoundingMode", "HALF_UP"),
        ("DuplicationMetrics", "LIBRARY"),
        ("DuplicationMetrics", "READ_PAIRS_EXAMINED"),
        ("DuplicationMetrics", "PERCENT_DUPLICATION"),
        ("SAMBinaryTagAndValue", "value"),
        ("SAMBinaryTagAndValue", "tag"),
        ("ReadDataHolder", "readBases"),
        ("ReadDataHolder", "readQuals"),
        ("ReadDataHolder", "insertionGOP"),
        ("ReadDataHolder", "deletionGOP"),
        ("ReadDataHolder", "overallGCP"),
        ("HaplotypeDataHolder", "haplotypeBases"),
        ("HaplotypeDataHolder", "haplotypePDBases"),
        ("PDHMMNativeArguments", "avxLevel"),
        ("PDHMMNativeArguments", "openMPSetting"),
        ("PDHMMNativeArguments", "maxNumberOfThreads"),
        ("PairHMMNativeArguments", "maxNumberOfThreads"),
        ("PairHMMNativeArguments", "useDoublePrecision"),
        ("GenotypeLikelihoodsAllelePair", "alleleIndex1"),
        ("GenotypeLikelihoodsAllelePair", "alleleIndex2"),
        ("SWNativeAlignerResult", "alignment_offset"),
        ("SWNativeAlignerResult", "cigar"),
        ("Storage", "SignUrlOption"),
    ] {
        assert_eq!(
            external_field_provenance(type_name, field_name),
            FactProvenance::BuiltinExternalModel
        );
    }
    assert_eq!(
        external_field_provenance("ExternalBox", "count"),
        FactProvenance::ConservativeFallback
    );
    assert_eq!(
        external_field_provenance("ExternalEnum", "unsorted"),
        FactProvenance::ConservativeFallback
    );
}

#[test]
fn external_field_facts_explain_fields_inherited_from_external_superclass() {
    let parsed = parser::parse_java_source(
        r#"
class MetricsRecord extends ExternalMetricsBase {
    int copyFrom(MetricsRecord other) {
        this.count = other.count;
        return this.count;
    }
}

class PlainRecord {
    int copyFrom(PlainRecord other) {
        this.count = other.count;
        return this.count;
    }
}
"#,
    );
    let resolved = resolve::resolve_program(&parsed.program, parsed.diagnostics);
    let semantic = semantic_analysis::analyze_program(
        &resolved.program,
        &resolved.facts,
        &new_java2rust::facts::FactSet::default(),
    );

    assert!(
        semantic
            .facts
            .external_field_for("MetricsRecord", "count")
            .is_some()
    );
    assert!(
        semantic
            .facts
            .external_field_for("PlainRecord", "count")
            .is_none()
    );
}

#[test]
fn external_core_object_api_effects_are_read_only_when_known() {
    let semantic = analyze(
        r#"
class CoreObjectApiUse {
    boolean same(ExternalRecord left, Object right) {
        return left.equals(right)
            && left.hashCode() != 0
            && left.toString().length() > 0
            && left.getClass() == right.getClass();
    }
}
"#,
    );

    let equals = semantic
        .facts
        .external_api_effect_for_receiver(Some("ExternalRecord"), "equals")
        .unwrap();
    assert_eq!(equals.role, ExternalApiRole::Getter);
    assert_eq!(equals.receiver_mutability, ReceiverMutabilityFact::ReadOnly);
    assert_eq!(
        equals.return_nullability,
        NullabilityState::DefinitelyNonNull
    );
    assert_eq!(equals.provenance, FactProvenance::BuiltinExternalModel);

    let hash_code = semantic
        .facts
        .external_api_effect_for_receiver(Some("ExternalRecord"), "hashCode")
        .unwrap();
    assert_eq!(hash_code.role, ExternalApiRole::Getter);
    assert_eq!(
        hash_code.receiver_mutability,
        ReceiverMutabilityFact::ReadOnly
    );
    assert_eq!(
        hash_code.return_nullability,
        NullabilityState::DefinitelyNonNull
    );
    assert_eq!(hash_code.provenance, FactProvenance::BuiltinExternalModel);

    let to_string = semantic
        .facts
        .external_api_effect_for_receiver(Some("ExternalRecord"), "toString")
        .unwrap();
    assert_eq!(to_string.role, ExternalApiRole::Getter);
    assert_eq!(
        to_string.receiver_mutability,
        ReceiverMutabilityFact::ReadOnly
    );
    assert_eq!(
        to_string.return_nullability,
        NullabilityState::DefinitelyNonNull
    );
    assert_eq!(to_string.provenance, FactProvenance::BuiltinExternalModel);

    let get_class = semantic
        .facts
        .external_api_effect_for_receiver(Some("ExternalRecord"), "getClass")
        .unwrap();
    assert_eq!(get_class.role, ExternalApiRole::Getter);
    assert_eq!(
        get_class.receiver_mutability,
        ReceiverMutabilityFact::ReadOnly
    );
    assert_eq!(
        get_class.return_nullability,
        NullabilityState::DefinitelyNonNull
    );
    assert_eq!(get_class.provenance, FactProvenance::BuiltinExternalModel);
}

#[test]
fn java_wide_comparison_api_effects_are_builtin_read_only_facts() {
    let semantic = analyze(
        r#"
class JavaWideComparisonApiUse {
    int compare(String left, String right) {
        return left.compareTo(right);
    }
}
"#,
    );

    let compare_to = semantic
        .facts
        .external_api_effect_for_receiver(Some("String"), "compareTo")
        .unwrap();
    assert_eq!(compare_to.role, ExternalApiRole::Getter);
    assert_eq!(
        compare_to.receiver_mutability,
        ReceiverMutabilityFact::ReadOnly
    );
    assert_eq!(
        compare_to.return_nullability,
        NullabilityState::DefinitelyNonNull
    );
    assert_eq!(compare_to.provenance, FactProvenance::BuiltinExternalModel);

    let method_ref_policy = external_api_method_policy(None, "compareTo", 0);
    assert_eq!(method_ref_policy.role, ExternalApiRole::Getter);
    assert_eq!(
        method_ref_policy.return_nullability,
        NullabilityState::DefinitelyNonNull
    );
    assert_eq!(
        method_ref_policy.provenance,
        FactProvenance::BuiltinExternalModel
    );
}

#[test]
fn inherited_object_methods_on_this_are_external_read_facts() {
    let semantic = analyze(
        r#"
class InheritedObjectMethodUse {
    boolean same(Object other) {
        return this.equals(other) && this.getClass() == other.getClass();
    }
}
"#,
    );

    let equals = semantic
        .facts
        .external_api_effect_for_receiver(Some("InheritedObjectMethodUse"), "equals")
        .unwrap();
    assert_eq!(equals.role, ExternalApiRole::Getter);
    assert_eq!(equals.receiver_mutability, ReceiverMutabilityFact::ReadOnly);
    assert_eq!(
        equals.return_nullability,
        NullabilityState::DefinitelyNonNull
    );
    assert_eq!(equals.provenance, FactProvenance::BuiltinExternalModel);

    let get_class = semantic
        .facts
        .external_api_effect_for_receiver(Some("InheritedObjectMethodUse"), "getClass")
        .unwrap();
    assert_eq!(get_class.role, ExternalApiRole::Getter);
    assert_eq!(get_class.provenance, FactProvenance::BuiltinExternalModel);
}

#[test]
fn external_file_path_accessors_are_builtin_string_getter_facts() {
    let semantic = analyze(
        r#"
class FilePathAccessorUse {
    String canonical(File file) {
        return file.getCanonicalPath();
    }
}
"#,
    );

    let canonical_path = semantic
        .facts
        .external_api_effect_for_receiver(Some("File"), "getCanonicalPath")
        .unwrap();
    assert_eq!(canonical_path.role, ExternalApiRole::Getter);
    assert_eq!(
        canonical_path.receiver_mutability,
        ReceiverMutabilityFact::ReadOnly
    );
    assert_eq!(
        canonical_path.return_nullability,
        NullabilityState::DefinitelyNonNull
    );
    assert_eq!(
        canonical_path.provenance,
        FactProvenance::BuiltinExternalModel
    );

    let unresolved_receiver_policy = external_api_method_policy(None, "getCanonicalPath", 0);
    assert_eq!(unresolved_receiver_policy.role, ExternalApiRole::Getter);
    assert_eq!(
        unresolved_receiver_policy.return_nullability,
        NullabilityState::DefinitelyNonNull
    );
    assert_eq!(
        unresolved_receiver_policy.provenance,
        FactProvenance::BuiltinExternalModel
    );
}

#[test]
fn external_static_utility_api_effects_are_facts_not_lowering_rules() {
    let semantic = analyze(
        r#"
class StaticUtilityApiUse {
    boolean ok(String value) {
        return Objects.nonNull(value) && nonNull(value) && !StringUtils.isEmpty(value);
    }

    String label(String name, int count) {
        return String.format("%s:%d", name, count);
    }
}
"#,
    );

    let objects_non_null = semantic
        .facts
        .external_api_effect_for_receiver(Some("Objects"), "nonNull")
        .unwrap();
    assert_eq!(objects_non_null.role, ExternalApiRole::BooleanGetter);
    assert_eq!(
        objects_non_null.receiver_mutability,
        ReceiverMutabilityFact::ReadOnly
    );
    assert_eq!(
        objects_non_null.return_nullability,
        NullabilityState::DefinitelyNonNull
    );

    let receiverless_non_null = semantic
        .facts
        .external_api_effect_for_receiver(None, "nonNull")
        .unwrap();
    assert_eq!(receiverless_non_null.role, ExternalApiRole::BooleanGetter);
    assert_eq!(
        receiverless_non_null.return_nullability,
        NullabilityState::DefinitelyNonNull
    );

    let string_utils_is_empty = semantic
        .facts
        .external_api_effect_for_receiver(Some("StringUtils"), "isEmpty")
        .unwrap();
    assert_eq!(string_utils_is_empty.role, ExternalApiRole::BooleanGetter);
    assert_eq!(
        string_utils_is_empty.receiver_mutability,
        ReceiverMutabilityFact::ReadOnly
    );
    assert_eq!(
        string_utils_is_empty.return_nullability,
        NullabilityState::DefinitelyNonNull
    );

    let format = semantic
        .facts
        .external_api_effect_for_receiver(Some("String"), "format")
        .unwrap();
    assert_eq!(format.role, ExternalApiRole::Getter);
    assert_eq!(format.receiver_mutability, ReceiverMutabilityFact::ReadOnly);
    assert_eq!(
        format.return_nullability,
        NullabilityState::DefinitelyNonNull
    );
}

#[test]
fn external_static_math_special_functions_are_read_only_non_null_getter_facts() {
    let semantic = analyze(
        r#"
import org.apache.commons.math3.special.Beta;

class StaticMathSpecialFunctionUse {
    double score(double alpha, double beta) {
        return Beta.logBeta(alpha, beta);
    }
}
"#,
    );

    let log_beta = semantic
        .facts
        .external_api_effect_for_receiver(Some("Beta"), "logBeta")
        .unwrap();
    assert_eq!(log_beta.role, ExternalApiRole::Getter);
    assert_eq!(
        log_beta.receiver_mutability,
        ReceiverMutabilityFact::ReadOnly
    );
    assert_eq!(
        log_beta.return_nullability,
        NullabilityState::DefinitelyNonNull
    );
}

#[test]
fn external_formatter_api_effects_are_string_getter_facts() {
    let semantic = analyze(
        r#"
class FormatterApiUse {
    DecimalFormat formatter;

    String label(double value) {
        return formatter.format(value);
    }
}
"#,
    );

    let format = semantic
        .facts
        .external_api_effect_for_receiver(Some("DecimalFormat"), "format")
        .unwrap();
    assert_eq!(format.role, ExternalApiRole::Getter);
    assert_eq!(format.receiver_mutability, ReceiverMutabilityFact::ReadOnly);
    assert_eq!(
        format.return_nullability,
        NullabilityState::DefinitelyNonNull
    );
}

#[test]
fn external_builder_make_is_product_getter_fact_with_conservative_nullability() {
    let semantic = analyze(
        r#"
class ExternalBuilderMakeUse {
    Widget build(WidgetBuilder builder) {
        return builder.withPart(new Part()).make();
    }
}
"#,
    );

    let with_part = semantic
        .facts
        .external_api_effect_for_receiver(Some("WidgetBuilder"), "withPart")
        .unwrap();
    assert_eq!(with_part.role, ExternalApiRole::FluentMutator);
    assert_eq!(
        with_part.receiver_mutability,
        ReceiverMutabilityFact::MutatesReceiver
    );
    assert_eq!(with_part.return_nullability, NullabilityState::Unknown);

    let make = semantic
        .facts
        .external_api_effect_for_receiver(Some("WidgetBuilder"), "make")
        .unwrap();
    assert_eq!(make.role, ExternalApiRole::Getter);
    assert_eq!(make.receiver_mutability, ReceiverMutabilityFact::ReadOnly);
    assert_eq!(make.return_nullability, NullabilityState::Unknown);
}

#[test]
fn external_to_path_is_read_only_path_getter_fact() {
    let semantic = analyze(
        r#"
class ExternalToPathFactUse {
    Path makePath(ExternalPathSource source) {
        return source.toPath();
    }
}
"#,
    );

    let to_path = semantic
        .facts
        .external_api_effect_for_receiver(Some("ExternalPathSource"), "toPath")
        .unwrap();
    assert_eq!(to_path.role, ExternalApiRole::Getter);
    assert_eq!(
        to_path.receiver_mutability,
        ReceiverMutabilityFact::ReadOnly
    );
    assert_eq!(to_path.return_nullability, NullabilityState::Unknown);
}

#[test]
fn external_replace_extension_is_non_null_read_only_string_fact() {
    let semantic = analyze(
        r#"
class ExternalReplaceExtensionFactUse {
    String replace(File file) {
        return IOUtils.replaceExtension(file.getPath(), FileExtensions.BAI_INDEX);
    }
}
"#,
    );

    let replace = semantic
        .facts
        .external_api_effect_for_receiver(Some("IOUtils"), "replaceExtension")
        .unwrap();
    assert_eq!(replace.role, ExternalApiRole::Getter);
    assert_eq!(
        replace.receiver_mutability,
        ReceiverMutabilityFact::ReadOnly
    );
    assert_eq!(
        replace.return_nullability,
        NullabilityState::DefinitelyNonNull
    );
}

#[test]
fn external_integer_parse_is_non_null_read_only_int_fact() {
    let semantic = analyze(
        r#"
class ExternalIntegerParserFactUse {
    int parse(String text) {
        return Integer.parseInt(text);
    }
}
"#,
    );

    let parse = semantic
        .facts
        .external_api_effect_for_receiver(Some("Integer"), "parseInt")
        .unwrap();
    assert_eq!(parse.role, ExternalApiRole::Getter);
    assert_eq!(parse.receiver_mutability, ReceiverMutabilityFact::ReadOnly);
    assert_eq!(
        parse.return_nullability,
        NullabilityState::DefinitelyNonNull
    );
}

#[test]
fn string_split_is_non_null_read_only_string_array_fact() {
    let semantic = analyze(
        r#"
class StringSplitFactUse {
    String[] fields(String line) {
        return line.split(",", -1);
    }
}
"#,
    );

    let split = semantic
        .facts
        .external_api_effect_for_receiver(Some("String"), "split")
        .unwrap();
    assert_eq!(split.role, ExternalApiRole::Getter);
    assert_eq!(split.receiver_mutability, ReceiverMutabilityFact::ReadOnly);
    assert_eq!(
        split.return_nullability,
        NullabilityState::DefinitelyNonNull
    );
}

#[test]
fn external_domain_getters_are_facts_with_conservative_return_nullability() {
    let semantic = analyze(
        r#"
class DomainGetterUse {
    int start(Locatable locatable) {
        return locatable.getStart();
    }

    String header(HeaderProvider provider) {
        return provider.getHeaderForReads();
    }

    List<Allele> alternates(VariantRecord record) {
        return record.getAlternateAlleles();
    }

    List<String> genotypeSamples(ExternalVariantHeader header) {
        return header.getGenotypeSamples();
    }

    SAMReadGroupRecord readGroup(ExternalRead read) {
        return read.getReadGroup();
    }

    TraversalParameters traversal(IntervalSource source, SAMSequenceDictionary dictionary) {
        return source.getTraversalParameters(dictionary);
    }
}
"#,
    );

    let start = semantic
        .facts
        .external_api_effect_for_receiver(Some("Locatable"), "getStart")
        .unwrap();
    assert_eq!(start.role, ExternalApiRole::Getter);
    assert_eq!(start.receiver_mutability, ReceiverMutabilityFact::ReadOnly);
    assert_eq!(
        start.return_nullability,
        NullabilityState::DefinitelyNonNull
    );
    assert_eq!(start.provenance, FactProvenance::ExternalLibraryPack);

    let header = semantic
        .facts
        .external_api_effect_for_receiver(Some("HeaderProvider"), "getHeaderForReads")
        .unwrap();
    assert_eq!(header.role, ExternalApiRole::Getter);
    assert_eq!(header.return_nullability, NullabilityState::Unknown);

    let alternates = semantic
        .facts
        .external_api_effect_for_receiver(Some("VariantRecord"), "getAlternateAlleles")
        .unwrap();
    assert_eq!(alternates.role, ExternalApiRole::Getter);
    assert_eq!(
        alternates.receiver_mutability,
        ReceiverMutabilityFact::ReadOnly
    );
    assert_eq!(alternates.return_nullability, NullabilityState::Unknown);

    let genotype_samples = semantic
        .facts
        .external_api_effect_for_receiver(Some("ExternalVariantHeader"), "getGenotypeSamples")
        .unwrap();
    assert_eq!(genotype_samples.role, ExternalApiRole::Getter);
    assert_eq!(
        genotype_samples.receiver_mutability,
        ReceiverMutabilityFact::ReadOnly
    );
    assert_eq!(
        genotype_samples.return_nullability,
        NullabilityState::Unknown
    );

    let read_group = semantic
        .facts
        .external_api_effect_for_receiver(Some("ExternalRead"), "getReadGroup")
        .unwrap();
    assert_eq!(read_group.role, ExternalApiRole::Getter);
    assert_eq!(
        read_group.receiver_mutability,
        ReceiverMutabilityFact::ReadOnly
    );
    assert_eq!(read_group.return_nullability, NullabilityState::Unknown);

    let traversal = semantic
        .facts
        .external_api_effect_for_receiver(Some("IntervalSource"), "getTraversalParameters")
        .unwrap();
    assert_eq!(traversal.role, ExternalApiRole::Getter);
    assert_eq!(
        traversal.receiver_mutability,
        ReceiverMutabilityFact::ReadOnly
    );
    assert_eq!(traversal.return_nullability, NullabilityState::Unknown);
}

#[test]
fn method_reference_external_api_effects_are_facts() {
    let semantic = analyze(
        r#"
class MethodReferenceExternalApiUse {
    void run(Stream<RecordCollection> collections) {
        collections.map(RecordCollection::getMetadata);
    }
}
"#,
    );

    let metadata = semantic
        .facts
        .external_api_effect_for_receiver(Some("RecordCollection"), "getMetadata")
        .unwrap();
    assert_eq!(metadata.role, ExternalApiRole::Getter);
    assert_eq!(
        metadata.receiver_mutability,
        ReceiverMutabilityFact::ReadOnly
    );
    assert_eq!(metadata.return_nullability, NullabilityState::Unknown);

    assert!(
        !semantic
            .facts
            .external_api_effect_facts
            .iter()
            .any(|fact| fact.receiver_type.is_none() && fact.method_name == "getMetadata"),
        "type-target method references should not add receiverless fallback facts when the target type is known"
    );
}

#[test]
fn method_reference_expression_targets_use_local_receiver_type_facts() {
    let semantic = analyze(
        r#"
class MethodReferenceExpressionTargetUse {
    void run(List<String> values, Stream<Integer> indexes) {
        indexes.map(values::get);
    }
}
"#,
    );

    let get = semantic
        .facts
        .external_api_effect_for_receiver(Some("List"), "get")
        .unwrap();
    assert_eq!(get.role, ExternalApiRole::IndexedGetter);
    assert_eq!(get.receiver_mutability, ReceiverMutabilityFact::ReadOnly);
    assert_eq!(get.provenance, FactProvenance::BuiltinExternalModel);

    assert!(
        !semantic
            .facts
            .external_api_effect_facts
            .iter()
            .any(
                |fact| fact.receiver_type.as_deref() == Some("values") && fact.method_name == "get"
            ),
        "expression-target method references should use the expression type, not the variable name as a receiver type"
    );
    assert!(
        !semantic
            .facts
            .external_api_effect_facts
            .iter()
            .any(|fact| fact.receiver_type.is_none() && fact.method_name == "get"),
        "expression-target method references should not add a receiverless fallback when the local receiver type is known"
    );
}

#[test]
fn array_constructor_method_references_do_not_become_external_api_facts() {
    let semantic = analyze(
        r#"
class ArrayConstructorMethodReferenceUse {
    String[] names(Stream<String> values) {
        return values.toArray(String[]::new);
    }
}
"#,
    );

    assert!(
        !semantic
            .facts
            .external_api_effect_facts
            .iter()
            .any(|fact| fact.method_name == "new"
                && fact
                    .receiver_type
                    .as_deref()
                    .is_some_and(|receiver| receiver.ends_with("[]"))),
        "array constructor method references are Java allocation/type operators, not external API method effects"
    );

    assert!(
        !semantic
            .facts
            .external_api_effect_facts
            .iter()
            .any(|fact| fact.receiver_type.as_deref() == Some("String[]")
                && fact.method_name == "new"),
        "the array target type should not be recorded as an external receiver"
    );
}

#[test]
fn source_receiver_method_references_do_not_become_receiverless_external_api_facts() {
    let semantic = analyze(
        r#"
class SourceReceiverMethodReferenceUse {
    interface WriterFactory {
        Object create(WriteFn fn);
    }

    interface WriteFn {
        void write(Object item);
    }

    void writeItem(Object item) {}

    Object writer(WriterFactory factory) {
        return factory.create(this::writeItem);
    }
}
"#,
    );

    assert!(
        !semantic
            .facts
            .external_api_effect_facts
            .iter()
            .any(|fact| fact.receiver_type.is_none() && fact.method_name == "writeItem"),
        "this::method references should not be recorded as receiverless external API effects"
    );
    assert!(
        !semantic
            .facts
            .external_api_effect_facts
            .iter()
            .any(
                |fact| fact.receiver_type.as_deref() == Some("SourceReceiverMethodReferenceUse")
                    && fact.method_name == "writeItem"
            ),
        "source receiver method references should not be converted into external API facts for the current type"
    );
}

#[test]
fn external_api_effect_receivers_use_type_parameter_bounds() {
    let semantic = analyze(
        r#"
class TypeParameterBoundExternalApiUse<A extends Allele> {
    boolean run(A allele, A other) {
        return allele.equals(other) && allele.isReference() && allele.getBases().length > 0;
    }
}
"#,
    );

    for method in ["equals", "isReference", "getBases"] {
        assert!(
            semantic
                .facts
                .external_api_effect_for_receiver(Some("Allele"), method)
                .is_some(),
            "expected bounded receiver fact for Allele.{method}"
        );
        assert!(
            semantic
                .facts
                .external_api_effect_for_receiver(Some("A"), method)
                .is_none(),
            "type-parameter receiver A should be normalized through its bound for {method}"
        );
    }
    assert!(
        semantic
            .facts
            .external_api_effect_for_receiver(Some("Allele"), "length")
            .is_none(),
        "array length on a bounded receiver getter result should not be modeled as Allele.length"
    );
}

#[test]
fn external_array_length_on_domain_getter_result_is_not_receiver_method_fact() {
    let semantic = analyze(
        r#"
class ExternalArrayLengthOnGetterUse {
    int bases(ExternalSequence sequence) {
        return sequence.getBases().length;
    }
}
"#,
    );

    assert!(
        semantic
            .facts
            .external_api_effect_for_receiver(Some("ExternalSequence"), "getBases")
            .is_some(),
        "the external byte-array getter itself should still be modeled"
    );
    assert!(
        semantic
            .facts
            .external_api_effect_for_receiver(Some("ExternalSequence"), "length")
            .is_none(),
        "array length is Java field-like array metadata, not an external method on the getter receiver"
    );
    assert!(
        semantic
            .facts
            .external_api_effect_for_receiver(Some("Array"), "length")
            .is_none(),
        "array length should not become an external API method fact"
    );
}

#[test]
fn external_scalar_length_method_is_builtin_read_only_int_fact() {
    let semantic = analyze(
        r#"
class ExternalScalarLengthUse {
    int length(ExternalSequence sequence) {
        return sequence.length();
    }
}
"#,
    );

    let length = semantic
        .facts
        .external_api_effect_for_receiver(Some("ExternalSequence"), "length")
        .unwrap();
    assert_eq!(length.role, ExternalApiRole::Getter);
    assert_eq!(length.receiver_mutability, ReceiverMutabilityFact::ReadOnly);
    assert_eq!(
        length.return_nullability,
        NullabilityState::DefinitelyNonNull
    );
    assert_eq!(length.provenance, FactProvenance::BuiltinExternalModel);
}

#[test]
fn source_static_field_call_receivers_use_resolved_field_type_facts() {
    let semantic = analyze(
        r#"
class Constants {
    static final List<String> ITEMS = null;
}

class SourceStaticFieldReceiverUse {
    boolean run(String item) {
        Constants.ITEMS.forEach(value -> value.toString());
        return Constants.ITEMS.contains(item) && Constants.ITEMS.size() > 0;
    }
}
"#,
    );

    for method in ["forEach", "contains", "size"] {
        assert!(
            semantic
                .facts
                .external_api_effect_for_receiver(Some("List"), method)
                .is_some(),
            "expected resolver-backed List.{method} fact for source static field receiver"
        );
        assert!(
            semantic
                .facts
                .external_api_effect_facts
                .iter()
                .all(|fact| fact.receiver_type.as_deref() != Some("ITEMS")
                    || fact.method_name != method),
            "source static field receiver name should not become an external receiver for {method}"
        );
        assert!(
            semantic
                .facts
                .external_api_effect_facts
                .iter()
                .all(|fact| fact.receiver_type.is_some() || fact.method_name != method),
            "source static field receiver calls should not become receiverless fallback facts for {method}"
        );
    }
}

#[test]
fn receiverless_record_collection_accessors_are_current_type_external_list_getter_facts() {
    let semantic = analyze(
        r#"
class ReceiverlessRecordCollectionAccessorUse {
    Object records() {
        return getRecords();
    }
}
"#,
    );

    let records = semantic
        .facts
        .external_api_effect_for_receiver(
            Some("ReceiverlessRecordCollectionAccessorUse"),
            "getRecords",
        )
        .unwrap();
    assert_eq!(records.role, ExternalApiRole::Getter);
    assert_eq!(
        records.receiver_mutability,
        ReceiverMutabilityFact::ReadOnly
    );
    assert_eq!(records.return_nullability, NullabilityState::Unknown);
}

#[test]
fn external_numeric_domain_getters_are_read_only_facts() {
    let semantic = analyze(
        r#"
class NumericDomainGetterFactUse {
    int period(RepeatLocus locus) {
        return locus.getPeriod();
    }

    int length(SequenceRecord record) {
        return record.getSequenceLength();
    }

    int ploidy(VariantRecord record) {
        return record.getMaxPloidy(2);
    }

    long mask(EncodedRecord record) {
        return record.getMask();
    }

    String maskString(EncodedRecord record) {
        return Long.toBinaryString(record.getMask());
    }
}
"#,
    );

    for (receiver, method) in [
        ("RepeatLocus", "getPeriod"),
        ("SequenceRecord", "getSequenceLength"),
        ("VariantRecord", "getMaxPloidy"),
        ("EncodedRecord", "getMask"),
    ] {
        let effect = semantic
            .facts
            .external_api_effect_for_receiver(Some(receiver), method)
            .unwrap();
        assert_eq!(effect.role, ExternalApiRole::Getter);
        assert_eq!(effect.receiver_mutability, ReceiverMutabilityFact::ReadOnly);
        assert_eq!(effect.return_nullability, NullabilityState::Unknown);
    }

    let to_binary_string = semantic
        .facts
        .external_api_effect_for_receiver(Some("Long"), "toBinaryString")
        .unwrap();
    assert_eq!(to_binary_string.role, ExternalApiRole::Getter);
    assert_eq!(
        to_binary_string.return_nullability,
        NullabilityState::DefinitelyNonNull
    );
}

#[test]
fn external_throwable_get_message_is_maybe_null_string_getter_fact() {
    let semantic = analyze(
        r#"
class ThrowableMessageUse {
    String message(IOException error) {
        return error.getMessage();
    }
}
"#,
    );

    let message = semantic
        .facts
        .external_api_effect_for_receiver(Some("IOException"), "getMessage")
        .unwrap();
    assert_eq!(message.role, ExternalApiRole::Getter);
    assert_eq!(
        message.receiver_mutability,
        ReceiverMutabilityFact::ReadOnly
    );
    assert_eq!(message.return_nullability, NullabilityState::MaybeNull);
}

#[test]
fn external_optional_is_present_is_non_null_boolean_getter_fact() {
    let semantic = analyze(
        r#"
class OptionalPresenceUse {
    boolean present(Optional<String> value, OptionalInt count) {
        return value.isPresent() && count.isPresent();
    }
}
"#,
    );

    let value = semantic
        .facts
        .external_api_effect_for_receiver(Some("Optional"), "isPresent")
        .unwrap();
    assert_eq!(value.role, ExternalApiRole::BooleanGetter);
    assert_eq!(value.receiver_mutability, ReceiverMutabilityFact::ReadOnly);
    assert_eq!(
        value.return_nullability,
        NullabilityState::DefinitelyNonNull
    );

    let count = semantic
        .facts
        .external_api_effect_for_receiver(Some("OptionalInt"), "isPresent")
        .unwrap();
    assert_eq!(count.role, ExternalApiRole::BooleanGetter);
    assert_eq!(count.receiver_mutability, ReceiverMutabilityFact::ReadOnly);
    assert_eq!(
        count.return_nullability,
        NullabilityState::DefinitelyNonNull
    );
}

#[test]
fn external_enum_name_is_non_null_string_getter_fact() {
    let semantic = analyze(
        r#"
class ExternalEnumNameUse {
    String tagName() {
        return SAMTag.NM.name();
    }
}
"#,
    );

    let name = semantic
        .facts
        .external_api_effect_for_receiver(Some("SAMTag"), "name")
        .unwrap();
    assert_eq!(name.role, ExternalApiRole::Getter);
    assert_eq!(name.receiver_mutability, ReceiverMutabilityFact::ReadOnly);
    assert_eq!(name.return_nullability, NullabilityState::DefinitelyNonNull);
}

#[test]
fn external_primitive_array_utility_to_array_is_non_null_getter_fact() {
    let semantic = analyze(
        r#"
class PrimitiveArrayUtilityUse {
    int[] ints(Collection<Integer> values) {
        return Ints.toArray(values);
    }

    double[] doubles(Collection<Double> values) {
        return Doubles.toArray(values);
    }
}
"#,
    );

    let ints = semantic
        .facts
        .external_api_effect_for_receiver(Some("Ints"), "toArray")
        .unwrap();
    assert_eq!(ints.role, ExternalApiRole::Getter);
    assert_eq!(ints.receiver_mutability, ReceiverMutabilityFact::ReadOnly);
    assert_eq!(ints.return_nullability, NullabilityState::DefinitelyNonNull);

    let doubles = semantic
        .facts
        .external_api_effect_for_receiver(Some("Doubles"), "toArray")
        .unwrap();
    assert_eq!(doubles.role, ExternalApiRole::Getter);
    assert_eq!(
        doubles.receiver_mutability,
        ReceiverMutabilityFact::ReadOnly
    );
    assert_eq!(
        doubles.return_nullability,
        NullabilityState::DefinitelyNonNull
    );
}

#[test]
fn array_clone_and_collection_to_array_are_non_null_read_only_facts() {
    let semantic = analyze(
        r#"
class ArrayAndCollectionConversionFacts {
    byte[] copy(byte[] values) {
        return values.clone();
    }

    String[] names(List<String> values) {
        return values.toArray(new String[0]);
    }

    Object[] objects(Collection<Object> values) {
        return values.toArray();
    }
}
"#,
    );

    for (receiver, method) in [
        ("Array", "clone"),
        ("List", "toArray"),
        ("Collection", "toArray"),
    ] {
        let effect = semantic
            .facts
            .external_api_effect_for_receiver(Some(receiver), method)
            .unwrap();
        assert_eq!(effect.role, ExternalApiRole::Getter);
        assert_eq!(effect.receiver_mutability, ReceiverMutabilityFact::ReadOnly);
        assert_eq!(
            effect.return_nullability,
            NullabilityState::DefinitelyNonNull
        );
    }
}

#[test]
fn arrays_copy_of_range_is_non_null_read_only_array_fact() {
    let semantic = analyze(
        r#"
class ArraysCopyOfRangeFactUse {
    byte[] bytes(byte[] values) {
        return Arrays.copyOfRange(values, 1, 3);
    }

    String[] names(String[] values) {
        return Arrays.copyOfRange(values, 0, 2);
    }
}
"#,
    );

    let copy = semantic
        .facts
        .external_api_effect_for_receiver(Some("Arrays"), "copyOfRange")
        .unwrap();
    assert_eq!(copy.role, ExternalApiRole::Getter);
    assert_eq!(copy.receiver_mutability, ReceiverMutabilityFact::ReadOnly);
    assert_eq!(copy.return_nullability, NullabilityState::DefinitelyNonNull);
    assert_eq!(copy.provenance, FactProvenance::BuiltinExternalModel);
}

#[test]
fn arrays_collection_fact_roles_are_builtin_external_models() {
    let semantic = analyze(
        r#"
class ArraysCollectionFactUse {
    List<String> names(String first, String second) {
        return Arrays.asList(first, second);
    }

    Stream<String> stream(String[] values) {
        return Arrays.stream(values);
    }
}
"#,
    );

    for method in ["asList", "stream"] {
        let effect = semantic
            .facts
            .external_api_effect_for_receiver(Some("Arrays"), method)
            .unwrap();
        assert_eq!(effect.role, ExternalApiRole::Getter);
        assert_eq!(effect.receiver_mutability, ReceiverMutabilityFact::ReadOnly);
        assert_eq!(
            effect.return_nullability,
            NullabilityState::DefinitelyNonNull
        );
        assert_eq!(effect.provenance, FactProvenance::BuiltinExternalModel);
    }
}

#[test]
fn external_array_math_is_non_null_read_only_array_fact() {
    let semantic = analyze(
        r#"
class ExternalArrayMathFactUse {
    double[] multiply(double[] left, double[] right) {
        return MathArrays.ebeMultiply(left, right);
    }
}
"#,
    );

    let multiply = semantic
        .facts
        .external_api_effect_for_receiver(Some("MathArrays"), "ebeMultiply")
        .unwrap();
    assert_eq!(multiply.role, ExternalApiRole::Getter);
    assert_eq!(
        multiply.receiver_mutability,
        ReceiverMutabilityFact::ReadOnly
    );
    assert_eq!(
        multiply.return_nullability,
        NullabilityState::DefinitelyNonNull
    );
}

#[test]
fn external_core_enum_quality_and_string_utilities_are_read_only_facts() {
    let semantic = analyze(
        r#"
class ExternalCoreUtilityFactUse {
    int index(ExternalEnumLike state) {
        return state.ordinal();
    }

    double probability(byte quality) {
        return QualityUtils.qualToProb(quality);
    }

    String joined(Object[] columns) {
        return Utils.join("\t", columns);
    }

    List<String> split(String text) {
        return Utils.split(text, ',');
    }
}
"#,
    );

    for (receiver, method) in [
        ("ExternalEnumLike", "ordinal"),
        ("QualityUtils", "qualToProb"),
        ("Utils", "join"),
        ("Utils", "split"),
    ] {
        let effect = semantic
            .facts
            .external_api_effect_for_receiver(Some(receiver), method)
            .unwrap();
        assert_eq!(effect.role, ExternalApiRole::Getter);
        assert_eq!(effect.receiver_mutability, ReceiverMutabilityFact::ReadOnly);
        assert_eq!(
            effect.return_nullability,
            NullabilityState::DefinitelyNonNull
        );
    }
}

#[test]
fn external_static_decoder_is_non_null_read_only_fact() {
    let semantic = analyze(
        r#"
class ExternalStaticDecoderFactUse {
    Cigar decode(String cigarText) {
        return TextCigarCodec.decode(cigarText);
    }
}
"#,
    );

    let decode = semantic
        .facts
        .external_api_effect_for_receiver(Some("TextCigarCodec"), "decode")
        .unwrap();
    assert_eq!(decode.role, ExternalApiRole::Getter);
    assert_eq!(decode.receiver_mutability, ReceiverMutabilityFact::ReadOnly);
    assert_eq!(
        decode.return_nullability,
        NullabilityState::DefinitelyNonNull
    );
}

#[test]
fn external_allele_factory_is_non_null_read_only_fact() {
    let semantic = analyze(
        r#"
class ExternalAlleleFactoryUse {
    Allele one(String bases) {
        return Allele.create(bases);
    }

    Allele reference(String bases) {
        return Allele.create(bases, true);
    }

    String bases(Allele allele) {
        return allele.getBaseString();
    }
}
"#,
    );

    let create = semantic
        .facts
        .external_api_effect_for_receiver(Some("Allele"), "create")
        .unwrap();
    assert_eq!(create.role, ExternalApiRole::Getter);
    assert_eq!(create.receiver_mutability, ReceiverMutabilityFact::ReadOnly);
    assert_eq!(
        create.return_nullability,
        NullabilityState::DefinitelyNonNull
    );
    assert_eq!(create.provenance, FactProvenance::ExternalLibraryPack);

    let base_string = semantic
        .facts
        .external_api_effect_for_receiver(Some("Allele"), "getBaseString")
        .unwrap();
    assert_eq!(base_string.role, ExternalApiRole::Getter);
    assert_eq!(
        base_string.receiver_mutability,
        ReceiverMutabilityFact::ReadOnly
    );
    assert_eq!(
        base_string.return_nullability,
        NullabilityState::DefinitelyNonNull
    );
    assert_eq!(base_string.provenance, FactProvenance::ExternalLibraryPack);
}

#[test]
fn external_functional_interface_apply_is_read_only_builtin_fact() {
    let semantic = analyze(
        r#"
class ExternalFunctionalApplyUse {
    Object run(BiFunction<String, Integer, Object> combiner, Function<String, Object> mapper) {
        Object first = combiner.apply("x", 1);
        return mapper.apply("y");
    }
}
"#,
    );

    for receiver in ["BiFunction", "Function"] {
        let apply = semantic
            .facts
            .external_api_effect_for_receiver(Some(receiver), "apply")
            .unwrap_or_else(|| panic!("expected {receiver}.apply external API fact"));
        assert_eq!(apply.role, ExternalApiRole::Getter);
        assert_eq!(apply.receiver_mutability, ReceiverMutabilityFact::ReadOnly);
        assert_eq!(apply.return_nullability, NullabilityState::Unknown);
        assert_eq!(apply.provenance, FactProvenance::BuiltinExternalModel);
    }
}

#[test]
fn external_collection_union_utilities_are_non_null_read_only_facts() {
    let semantic = analyze(
        r#"
class CollectionUnionFactUse {
    List<String> list(List<String> left, List<String> right) {
        return ListUtils.union(left, right);
    }

    Set<String> set(Set<String> left, Set<String> right) {
        return Sets.union(left, right);
    }
}
"#,
    );

    for (receiver, method) in [("ListUtils", "union"), ("Sets", "union")] {
        let effect = semantic
            .facts
            .external_api_effect_for_receiver(Some(receiver), method)
            .unwrap();
        assert_eq!(effect.role, ExternalApiRole::Getter);
        assert_eq!(effect.receiver_mutability, ReceiverMutabilityFact::ReadOnly);
        assert_eq!(
            effect.return_nullability,
            NullabilityState::DefinitelyNonNull
        );
    }
}

#[test]
fn stream_collect_and_collector_fact_roles_are_read_only_non_null() {
    let semantic = analyze(
        r#"
class StreamCollectFactUse {
    List<String> list(List<String> values) {
        return values.stream().collect(Collectors.toList());
    }

    Set<String> set(Set<String> values) {
        return values.stream().collect(Collectors.toSet());
    }
}
"#,
    );

    for (receiver, method) in [
        ("List", "stream"),
        ("Set", "stream"),
        ("Stream", "collect"),
        ("Collectors", "toList"),
        ("Collectors", "toSet"),
    ] {
        let effect = semantic
            .facts
            .external_api_effect_for_receiver(Some(receiver), method)
            .unwrap();
        assert_eq!(effect.role, ExternalApiRole::Getter);
        assert_eq!(effect.receiver_mutability, ReceiverMutabilityFact::ReadOnly);
        assert_eq!(
            effect.return_nullability,
            NullabilityState::DefinitelyNonNull
        );
    }
}

#[test]
fn primitive_stream_mapper_and_sum_facts_are_read_only_non_null_builtin() {
    for (receiver, method, arg_count) in [("Stream", "mapToInt", 1), ("IntStream", "sum", 0)] {
        let effect = external_api_method_policy(Some(receiver), method, arg_count);
        assert_eq!(effect.role, ExternalApiRole::Getter);
        assert_eq!(effect.receiver_mutability, ReceiverMutabilityFact::ReadOnly);
        assert_eq!(
            effect.return_nullability,
            NullabilityState::DefinitelyNonNull
        );
        assert_eq!(effect.provenance, FactProvenance::BuiltinExternalModel);
    }
}

#[test]
fn external_get_string_lookup_is_string_getter_fact_with_unknown_nullability() {
    let semantic = analyze(
        r#"
class ExternalStringLookupUse {
    String fromConfig(ConfigObject config, ResultRow row) {
        return config.getString("name") + row.getString(1);
    }
}
"#,
    );

    let config = semantic
        .facts
        .external_api_effect_for_receiver(Some("ConfigObject"), "getString")
        .unwrap();
    assert_eq!(config.role, ExternalApiRole::Getter);
    assert_eq!(config.receiver_mutability, ReceiverMutabilityFact::ReadOnly);
    assert_eq!(config.return_nullability, NullabilityState::Unknown);

    let row = semantic
        .facts
        .external_api_effect_for_receiver(Some("ResultRow"), "getString")
        .unwrap();
    assert_eq!(row.role, ExternalApiRole::Getter);
    assert_eq!(row.receiver_mutability, ReceiverMutabilityFact::ReadOnly);
    assert_eq!(row.return_nullability, NullabilityState::Unknown);
}

#[test]
fn external_zero_arg_string_named_getter_is_string_fact_with_unknown_nullability() {
    let semantic = analyze(
        r#"
class ExternalStringNamedGetterUse {
    String raw(InputDescriptor input) {
        return input.getRawInputString();
    }

    String display(DisplayRecord record) {
        return record.getDisplayString();
    }
}
"#,
    );

    for (receiver, method) in [
        ("InputDescriptor", "getRawInputString"),
        ("DisplayRecord", "getDisplayString"),
    ] {
        let effect = semantic
            .facts
            .external_api_effect_for_receiver(Some(receiver), method)
            .unwrap();
        assert_eq!(effect.role, ExternalApiRole::Getter);
        assert_eq!(effect.receiver_mutability, ReceiverMutabilityFact::ReadOnly);
        assert_eq!(effect.return_nullability, NullabilityState::Unknown);
    }
}

#[test]
fn external_quality_conversion_is_read_only_non_null_getter_fact() {
    let semantic = analyze(
        r#"
class ExternalQualityConversionUse {
    int one(char quality) {
        return QualityCodec.fastqToPhred(quality);
    }

    byte[] many(String encoded) {
        return QualityCodec.fastqToPhred(encoded);
    }
}
"#,
    );

    let conversion = semantic
        .facts
        .external_api_effect_for_receiver(Some("quality_codec"), "fastqToPhred")
        .unwrap();
    assert_eq!(conversion.role, ExternalApiRole::Getter);
    assert_eq!(
        conversion.receiver_mutability,
        ReceiverMutabilityFact::ReadOnly
    );
    assert_eq!(
        conversion.return_nullability,
        NullabilityState::DefinitelyNonNull
    );
}

#[test]
fn boxed_number_value_extractors_are_read_only_non_null_getter_facts() {
    let semantic = analyze(
        r#"
class BoxedNumberValueExtractorUse {
    int count(Integer value) {
        return value.intValue();
    }

    long length(Long value) {
        return value.longValue();
    }

    float ratio(Float value) {
        return value.floatValue();
    }

    double score(Number value) {
        return value.doubleValue();
    }
}
"#,
    );

    for (receiver, method) in [
        ("Integer", "intValue"),
        ("Long", "longValue"),
        ("Float", "floatValue"),
        ("Number", "doubleValue"),
    ] {
        let effect = semantic
            .facts
            .external_api_effect_for_receiver(Some(receiver), method)
            .unwrap();
        assert_eq!(effect.role, ExternalApiRole::Getter);
        assert_eq!(effect.receiver_mutability, ReceiverMutabilityFact::ReadOnly);
        assert_eq!(
            effect.return_nullability,
            NullabilityState::DefinitelyNonNull
        );
    }
}

#[test]
fn external_byte_array_accessors_are_read_only_getter_facts() {
    let semantic = analyze(
        r#"
class ExternalByteArrayAccessorUse {
    byte[] reversed(ExternalSequence sequence) {
        return ReadUtils.getBasesReverseComplement(sequence.getBases());
    }

    byte[] readBases(ExternalRead read) {
        return read.getReadBases();
    }

    byte[] qualities(ExternalRead read) {
        return read.getBaseQualities();
    }
}
"#,
    );

    for (receiver, method) in [
        ("ExternalSequence", "getBases"),
        ("ExternalRead", "getReadBases"),
        ("ExternalRead", "getBaseQualities"),
    ] {
        let effect = semantic
            .facts
            .external_api_effect_for_receiver(Some(receiver), method)
            .unwrap_or_else(|| panic!("expected {receiver}.{method} external API fact"));
        assert_eq!(effect.role, ExternalApiRole::Getter);
        assert_eq!(effect.receiver_mutability, ReceiverMutabilityFact::ReadOnly);
        assert_eq!(effect.return_nullability, NullabilityState::Unknown);
        assert_eq!(effect.provenance, FactProvenance::BuiltinExternalModel);
    }

    let reverse_complement = semantic
        .facts
        .external_api_effect_for_receiver(Some("read_utils"), "getBasesReverseComplement")
        .unwrap();
    assert_eq!(reverse_complement.role, ExternalApiRole::Getter);
    assert_eq!(
        reverse_complement.receiver_mutability,
        ReceiverMutabilityFact::ReadOnly
    );
    assert_eq!(
        reverse_complement.return_nullability,
        NullabilityState::DefinitelyNonNull
    );
}

#[test]
fn external_predicate_naming_is_non_null_boolean_getter_fact() {
    let semantic = analyze(
        r#"
class ExternalPredicateUse {
    boolean ok(GraphLike graph, Vertex vertex, Marker marker) {
        return graph.containsVertex(vertex) && marker.isNonReference() && marker.hasCoverage();
    }
}
"#,
    );

    let contains_vertex = semantic
        .facts
        .external_api_effect_for_receiver(Some("GraphLike"), "containsVertex")
        .unwrap();
    assert_eq!(contains_vertex.role, ExternalApiRole::BooleanGetter);
    assert_eq!(
        contains_vertex.receiver_mutability,
        ReceiverMutabilityFact::ReadOnly
    );
    assert_eq!(
        contains_vertex.return_nullability,
        NullabilityState::DefinitelyNonNull
    );

    let is_non_reference = semantic
        .facts
        .external_api_effect_for_receiver(Some("Marker"), "isNonReference")
        .unwrap();
    assert_eq!(is_non_reference.role, ExternalApiRole::BooleanGetter);
    assert_eq!(
        is_non_reference.return_nullability,
        NullabilityState::DefinitelyNonNull
    );

    let has_coverage = semantic
        .facts
        .external_api_effect_for_receiver(Some("Marker"), "hasCoverage")
        .unwrap();
    assert_eq!(has_coverage.role, ExternalApiRole::BooleanGetter);
    assert_eq!(
        has_coverage.return_nullability,
        NullabilityState::DefinitelyNonNull
    );
}

#[test]
fn external_count_array_getter_is_read_only_getter_fact() {
    let semantic = analyze(
        r#"
class ExternalCountArrayGetterUse {
    int[] counts(DepthRecord record) {
        return record.getAD();
    }
}
"#,
    );

    let ad = semantic
        .facts
        .external_api_effect_for_receiver(Some("DepthRecord"), "getAD")
        .unwrap();
    assert_eq!(ad.role, ExternalApiRole::Getter);
    assert_eq!(ad.receiver_mutability, ReceiverMutabilityFact::ReadOnly);
    assert_eq!(ad.return_nullability, NullabilityState::Unknown);
}

#[test]
fn inherited_receiverless_domain_getters_are_current_type_external_facts() {
    let semantic = analyze(
        r#"
class HeaderWalker {
    String sortOrder() {
        return getHeader().getSortOrder();
    }

    int start() {
        return getStart();
    }
}
"#,
    );

    let header = semantic
        .facts
        .external_api_effect_for_receiver(Some("HeaderWalker"), "getHeader")
        .unwrap();
    assert_eq!(header.role, ExternalApiRole::Getter);
    assert_eq!(header.receiver_mutability, ReceiverMutabilityFact::ReadOnly);
    assert_eq!(header.return_nullability, NullabilityState::Unknown);

    let start = semantic
        .facts
        .external_api_effect_for_receiver(Some("HeaderWalker"), "getStart")
        .unwrap();
    assert_eq!(start.role, ExternalApiRole::Getter);
    assert_eq!(start.receiver_mutability, ReceiverMutabilityFact::ReadOnly);

    assert!(
        semantic
            .facts
            .external_api_effect_for_receiver(None, "getHeader")
            .is_none()
    );
}

#[test]
fn external_logger_style_side_effects_do_not_require_mutable_receiver() {
    let semantic = analyze(
        r#"
class LoggerApiUse {
    void run(Logger log, Writer out) {
        log.info("started");
        log.warn("slow");
        out.append("x");
        out.newLine();
    }
}
"#,
    );

    let info = semantic
        .facts
        .external_api_effect_for_receiver(Some("Logger"), "info")
        .unwrap();
    assert_eq!(info.role, ExternalApiRole::SideEffect);
    assert_eq!(info.receiver_mutability, ReceiverMutabilityFact::ReadOnly);

    let warn = semantic
        .facts
        .external_api_effect_for_receiver(Some("Logger"), "warn")
        .unwrap();
    assert_eq!(warn.role, ExternalApiRole::SideEffect);
    assert_eq!(warn.receiver_mutability, ReceiverMutabilityFact::ReadOnly);

    let append = semantic
        .facts
        .external_api_effect_for_receiver(Some("Writer"), "append")
        .unwrap();
    assert_eq!(append.role, ExternalApiRole::SideEffect);
    assert_eq!(append.receiver_mutability, ReceiverMutabilityFact::ReadOnly);

    let new_line = semantic
        .facts
        .external_api_effect_for_receiver(Some("Writer"), "newLine")
        .unwrap();
    assert_eq!(new_line.role, ExternalApiRole::SideEffect);
    assert_eq!(
        new_line.receiver_mutability,
        ReceiverMutabilityFact::ReadOnly
    );
}

#[test]
fn external_writer_and_event_side_effects_are_builtin_read_only_facts() {
    let semantic = analyze(
        r#"
class WriterAndEventApiUse {
    void run(AlignmentWriter writer, EventSink sink, HeaderBuilder header) {
        writer.addAlignment("read");
        writer.addAndFlush("read");
        sink.addEvent("event");
        header.addMetaDataLine("line");
    }
}
"#,
    );

    for (receiver, method) in [
        ("AlignmentWriter", "addAlignment"),
        ("AlignmentWriter", "addAndFlush"),
        ("EventSink", "addEvent"),
        ("HeaderBuilder", "addMetaDataLine"),
    ] {
        let effect = semantic
            .facts
            .external_api_effect_for_receiver(Some(receiver), method)
            .unwrap_or_else(|| panic!("expected {receiver}.{method} external API fact"));
        assert_eq!(effect.role, ExternalApiRole::SideEffect);
        assert_eq!(effect.receiver_mutability, ReceiverMutabilityFact::ReadOnly);
        assert_eq!(effect.provenance, FactProvenance::BuiltinExternalModel);
    }
}

#[test]
fn external_receiver_type_keys_ignore_java_declaration_modifiers() {
    assert_eq!(
        canonical_external_receiver_type_key("final BufferedWriter"),
        "BufferedWriter"
    );
    assert_eq!(
        canonical_external_receiver_type_key("volatile Writer"),
        "Writer"
    );
}

#[test]
fn semantic_analysis_reports_external_api_budget_diagnostic() {
    let semantic = analyze_with_config(
        &idiom_gap("ExternalGetterSetterMutability.java"),
        semantic_analysis::SemanticAnalysisConfig {
            max_external_guess_iterations: Some(1),
            enable_external_api_guesses: true,
            enable_control_flow_facts: true,
        },
    );

    assert!(
        semantic.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "semantic_external_api_effect_budget_exhausted"
        })
    );
}

#[test]
fn external_setter_effect_requires_mutable_receiver_binding_in_signature() {
    let source = idiom_gap("ExternalGetterSetterMutability.java");
    let parsed = parser::parse_java_source(&source);
    let resolved = resolve::resolve_program(&parsed.program, parsed.diagnostics);
    let semantic = semantic_analysis::analyze_program(
        &resolved.program,
        &resolved.facts,
        &new_java2rust::facts::FactSet::default(),
    );
    let representations = representation::plan_representations(&resolved.program, &semantic.facts);
    let signatures = signature::solve_signatures_with_facts(
        &resolved.program,
        &representations,
        &semantic.usage,
        &semantic.facts,
    );
    let ty = &resolved.program.types[0];
    let method = ty
        .members
        .iter()
        .find_map(|member| match member {
            new_java2rust::java_ir::Member::Method(method) if method.name == "update" => {
                Some(method)
            }
            _ => None,
        })
        .unwrap();
    let param_shape = signatures.method_param_plans(ty, method).unwrap()[0]
        .shape
        .clone();

    assert_eq!(param_shape.ownership, OwnershipShape::MutableBorrow);
}

#[test]
fn external_api_user_override_beats_later_name_guess() {
    let source = idiom_gap("ExternalGetterSetterMutability.java");
    let parsed = parser::parse_java_source(&source);
    let resolved = resolve::resolve_program(&parsed.program, parsed.diagnostics);
    let mut external_facts = new_java2rust::facts::FactSet::default();
    external_facts
        .external_api_effect_facts
        .push(ExternalApiEffectFact {
            method_name: "setValue".to_string(),
            receiver_type: None,
            role: ExternalApiRole::Getter,
            receiver_mutability: ReceiverMutabilityFact::ReadOnly,
            return_type: None,
            return_nullability: NullabilityState::Unknown,
            provenance: FactProvenance::UserOverride,
        });
    let semantic =
        semantic_analysis::analyze_program(&resolved.program, &resolved.facts, &external_facts);

    let effect = semantic.facts.external_api_effect_for("setValue").unwrap();
    assert_eq!(effect.provenance, FactProvenance::UserOverride);
    assert_eq!(effect.receiver_mutability, ReceiverMutabilityFact::ReadOnly);

    let representations = representation::plan_representations(&resolved.program, &semantic.facts);
    let signatures = signature::solve_signatures_with_facts(
        &resolved.program,
        &representations,
        &semantic.usage,
        &semantic.facts,
    );
    let ty = &resolved.program.types[0];
    let method = ty
        .members
        .iter()
        .find_map(|member| match member {
            new_java2rust::java_ir::Member::Method(method) if method.name == "update" => {
                Some(method)
            }
            _ => None,
        })
        .unwrap();
    let param_shape = signatures.method_param_plans(ty, method).unwrap()[0]
        .shape
        .clone();

    assert_ne!(param_shape.ownership, OwnershipShape::MutableBorrow);
}

#[test]
fn external_api_receiver_specific_override_beats_global_override_in_signature() {
    let source = idiom_gap("ExternalGetterSetterMutability.java");
    let parsed = parser::parse_java_source(&source);
    let resolved = resolve::resolve_program(&parsed.program, parsed.diagnostics);
    let mut external_facts = new_java2rust::facts::FactSet::default();
    external_facts
        .external_api_effect_facts
        .push(ExternalApiEffectFact {
            method_name: "setValue".to_string(),
            receiver_type: None,
            role: ExternalApiRole::Getter,
            receiver_mutability: ReceiverMutabilityFact::ReadOnly,
            return_type: None,
            return_nullability: NullabilityState::Unknown,
            provenance: FactProvenance::UserOverride,
        });
    external_facts
        .external_api_effect_facts
        .push(ExternalApiEffectFact {
            method_name: "setValue".to_string(),
            receiver_type: Some("ExternalCounter".to_string()),
            role: ExternalApiRole::Setter,
            receiver_mutability: ReceiverMutabilityFact::MutatesReceiver,
            return_type: None,
            return_nullability: NullabilityState::Unknown,
            provenance: FactProvenance::UserOverride,
        });
    let semantic =
        semantic_analysis::analyze_program(&resolved.program, &resolved.facts, &external_facts);

    let effect = semantic
        .facts
        .external_api_effect_for_receiver(Some("ExternalCounter"), "setValue")
        .unwrap();
    assert_eq!(effect.receiver_type.as_deref(), Some("ExternalCounter"));
    assert_eq!(
        effect.receiver_mutability,
        ReceiverMutabilityFact::MutatesReceiver
    );

    let representations = representation::plan_representations(&resolved.program, &semantic.facts);
    let signatures = signature::solve_signatures_with_facts(
        &resolved.program,
        &representations,
        &semantic.usage,
        &semantic.facts,
    );
    let ty = &resolved.program.types[0];
    let method = ty
        .members
        .iter()
        .find_map(|member| match member {
            new_java2rust::java_ir::Member::Method(method) if method.name == "update" => {
                Some(method)
            }
            _ => None,
        })
        .unwrap();
    let param_shape = signatures.method_param_plans(ty, method).unwrap()[0]
        .shape
        .clone();

    assert_eq!(param_shape.ownership, OwnershipShape::MutableBorrow);
}

#[test]
fn external_getter_effect_preserves_shared_receiver_binding_in_signature() {
    let source = idiom_gap("ExternalGetterReadOnly.java");
    let parsed = parser::parse_java_source(&source);
    let resolved = resolve::resolve_program(&parsed.program, parsed.diagnostics);
    let semantic = semantic_analysis::analyze_program(
        &resolved.program,
        &resolved.facts,
        &new_java2rust::facts::FactSet::default(),
    );
    let representations = representation::plan_representations(&resolved.program, &semantic.facts);
    let signatures = signature::solve_signatures_with_facts(
        &resolved.program,
        &representations,
        &semantic.usage,
        &semantic.facts,
    );
    let ty = &resolved.program.types[0];
    let method = ty
        .members
        .iter()
        .find_map(|member| match member {
            new_java2rust::java_ir::Member::Method(method) if method.name == "read" => Some(method),
            _ => None,
        })
        .unwrap();
    let param_shape = signatures.method_param_plans(ty, method).unwrap()[0]
        .shape
        .clone();

    assert_ne!(param_shape.ownership, OwnershipShape::MutableBorrow);
}

#[test]
fn records_mutual_recursion_edges_without_unbounded_growth() {
    let semantic = analyze(&idiom_gap("MutualRecursion.java"));

    let even = semantic
        .facts
        .recursion_facts_for("MutualRecursion", "even");
    assert!(
        even.iter()
            .any(|fact| { fact.calls.method_name == "odd" && fact.kind == RecursionKind::Mutual })
    );

    let odd = semantic.facts.recursion_facts_for("MutualRecursion", "odd");
    assert!(
        odd.iter()
            .any(|fact| { fact.calls.method_name == "even" && fact.kind == RecursionKind::Mutual })
    );
}

#[test]
fn records_assignment_in_condition_marker() {
    let semantic = analyze(&idiom_gap("WhileAssignmentNotNull.java"));

    assert!(semantic.facts.has_control_flow_fact(
        "WhileAssignmentNotNull",
        "drain",
        ControlFlowFactKind::AssignmentInCondition
    ));
}

#[test]
fn plans_assignment_in_condition_normalization_shape() {
    let semantic = analyze(&idiom_gap("WhileAssignmentNotNull.java"));
    let plan = semantic
        .control_flow_plans
        .iter()
        .find(|plan| {
            plan.method.type_name == "WhileAssignmentNotNull" && plan.method.method_name == "drain"
        })
        .unwrap();

    assert!(plan.rewrites.iter().any(|rewrite| matches!(
        rewrite,
        ControlFlowRewritePlan::AssignmentInCondition(assignment)
            if assignment.local_name == "row"
                && assignment.source_call == "next"
                && assignment.null_test == NullTestKind::NotNull
                && assignment.normalized_shape
                    == AssignmentConditionNormalizedShape::LoopAssignThenBreak
    )));
}

#[test]
fn records_tail_recursive_self_call_marker() {
    let semantic = analyze(&idiom_gap("TailRecursiveAccumulator.java"));

    assert!(semantic.facts.has_control_flow_fact(
        "TailRecursiveAccumulator",
        "factorialLoop",
        ControlFlowFactKind::TailRecursiveSelfCall
    ));
}

#[test]
fn plans_tail_recursive_loop_normalization_shape() {
    let semantic = analyze(&idiom_gap("TailRecursiveAccumulator.java"));
    let plan = semantic
        .control_flow_plans
        .iter()
        .find(|plan| {
            plan.method.type_name == "TailRecursiveAccumulator"
                && plan.method.method_name == "factorialLoop"
        })
        .unwrap();

    assert!(plan.rewrites.iter().any(|rewrite| matches!(
        rewrite,
        ControlFlowRewritePlan::TailRecursiveLoop(tail)
            if tail.method_name == "factorialLoop"
                && tail.arg_count == 2
                && tail.normalized_shape
                    == TailRecursiveNormalizedShape::LoopWithParameterReassignment
    )));
}

#[test]
fn translation_reports_pending_control_flow_rewrite_diagnostics() {
    let translation = translate_source(&idiom_gap("WhileAssignmentNotNull.java"));

    assert!(
        translation
            .diagnostics
            .has_code("control_flow_rewrite_pending")
    );
    assert!(translation.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("assignment-in-condition normalization")
    }));
}

#[test]
fn translation_reports_pending_tail_recursive_rewrite_diagnostics() {
    let translation = translate_source(&idiom_gap("TailRecursiveAccumulator.java"));

    assert!(
        translation
            .diagnostics
            .has_code("control_flow_rewrite_pending")
    );
    assert!(translation.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("tail-recursive loop normalization")
    }));
}

#[test]
fn records_exception_subclass_edges() {
    let semantic = analyze(&idiom_gap("ExceptionSubclassCatchOrder.java"));
    let specific_problem = JavaType::Named {
        name: "SpecificProblem".to_string(),
        args: Vec::new(),
    };
    let reachable = semantic
        .facts
        .reachable_hierarchy_edges_for_type(&specific_problem);

    assert!(
        reachable
            .iter()
            .any(|edge| { edge.sub_name == "SpecificProblem" && edge.super_name == "BaseProblem" })
    );
    assert!(
        reachable
            .iter()
            .any(|edge| { edge.sub_name == "SpecificProblem" && edge.super_name == "Exception" })
    );
}

#[test]
fn records_catch_assignability_from_reachable_exception_hierarchy() {
    let semantic = analyze(&idiom_gap("ExceptionSubclassCatchOrder.java"));
    let catches = semantic
        .facts
        .catch_facts_for("ExceptionSubclassCatchOrder", "parse");

    assert_eq!(catches.len(), 2);
    assert_eq!(
        catches[0].assignable_from,
        vec![JavaType::Named {
            name: "SpecificProblem".to_string(),
            args: Vec::new(),
        }]
    );
    assert_eq!(
        catches[1].assignable_from,
        vec![
            JavaType::Named {
                name: "BaseProblem".to_string(),
                args: Vec::new(),
            },
            JavaType::Named {
                name: "SpecificProblem".to_string(),
                args: Vec::new(),
            },
        ]
    );
}

#[test]
fn records_checked_and_unchecked_exception_kinds() {
    let semantic = analyze(&idiom_gap("ExceptionRuntimeCheckedHierarchy.java"));

    assert_eq!(
        semantic.facts.exception_kind_for("CheckedProblem"),
        Some(ExceptionKind::Checked)
    );
    assert_eq!(
        semantic.facts.exception_kind_for("UncheckedProblem"),
        Some(ExceptionKind::Unchecked)
    );
}

#[test]
fn records_declared_throws_as_method_effect_fact() {
    let semantic = analyze(&idiom_gap("ExceptionRuntimeCheckedHierarchy.java"));
    let effect = semantic
        .facts
        .method_effect_for("ExceptionRuntimeCheckedHierarchy", "classify")
        .unwrap();

    assert_eq!(
        effect.thrown,
        vec![JavaType::Named {
            name: "CheckedProblem".to_string(),
            args: Vec::new(),
        }]
    );
}

#[test]
fn records_catch_variable_rethrow_as_method_effect_fact() {
    let semantic = analyze(&idiom_gap("ExceptionRethrowSpecific.java"));
    let effect = semantic
        .facts
        .method_effect_for("ExceptionRethrowSpecific", "validate")
        .unwrap();

    assert_eq!(
        effect.thrown,
        vec![JavaType::Named {
            name: "ValidationProblem".to_string(),
            args: Vec::new(),
        }]
    );
}

#[test]
fn records_receiver_mutability_as_method_effect_fact() {
    let source = r#"
class Counter {
    int value;

    int get() {
        return this.value;
    }

    void set(int next) {
        this.value = next;
    }
}
"#;
    let semantic = analyze(source);

    assert_eq!(
        semantic
            .facts
            .method_effect_for("Counter", "get")
            .unwrap()
            .receiver_mutability,
        ReceiverMutabilityFact::ReadOnly
    );
    assert_eq!(
        semantic
            .facts
            .method_effect_for("Counter", "set")
            .unwrap()
            .receiver_mutability,
        ReceiverMutabilityFact::MutatesReceiver
    );
}

#[test]
fn records_return_nullability_as_method_effect_fact() {
    let source = r#"
class MaybeName {
    String choose(boolean flag) {
        if (flag) {
            return "name";
        }
        return null;
    }

    String create() {
        return new String();
    }
}
"#;
    let semantic = analyze(source);

    assert_eq!(
        semantic
            .facts
            .method_effect_for("MaybeName", "choose")
            .unwrap()
            .return_nullability,
        NullabilityState::MaybeNull
    );
    assert_eq!(
        semantic
            .facts
            .method_effect_for("MaybeName", "create")
            .unwrap()
            .return_nullability,
        NullabilityState::DefinitelyNonNull
    );
}

#[test]
fn return_nullability_method_effect_shapes_nullable_returns_as_option() {
    let source = r#"
class MaybeName {
    String choose(boolean flag) {
        if (flag) {
            return "name";
        }
        return null;
    }
}
"#;
    let parsed = parser::parse_java_source(source);
    let resolved = resolve::resolve_program(&parsed.program, parsed.diagnostics);
    let semantic = semantic_analysis::analyze_program(
        &resolved.program,
        &resolved.facts,
        &new_java2rust::facts::FactSet::default(),
    );
    let representations = representation::plan_representations(&resolved.program, &semantic.facts);
    let signatures = signature::solve_signatures_with_facts(
        &resolved.program,
        &representations,
        &semantic.usage,
        &semantic.facts,
    );
    let ty = &resolved.program.types[0];
    let method = ty
        .members
        .iter()
        .find_map(|member| match member {
            new_java2rust::java_ir::Member::Method(method) if method.name == "choose" => {
                Some(method)
            }
            _ => None,
        })
        .unwrap();

    assert_eq!(
        signatures.method_return_shape(ty, method).unwrap().java_ty,
        optional_java_type(JavaType::String)
    );
}

#[test]
fn exception_support_decision_consumes_semantic_exception_facts() {
    let source = idiom_gap("ExceptionRuntimeCheckedHierarchy.java");
    let parsed = parser::parse_java_source(&source);
    let resolved = resolve::resolve_program(&parsed.program, parsed.diagnostics);
    let semantic = semantic_analysis::analyze_program(
        &resolved.program,
        &resolved.facts,
        &new_java2rust::facts::FactSet::default(),
    );

    assert!(exception_plan::program_uses_java_exception_with_facts(
        &resolved.program,
        &semantic.facts
    ));
}

#[test]
fn records_multi_catch_types_in_source_order() {
    let semantic = analyze(&java_fixture("06-stdlib/MultiCatch.java"));
    let catches = semantic.facts.catch_facts_for("MultiCatch", "parseOrZero");

    assert_eq!(catches.len(), 1);
    assert_eq!(catches[0].clause_index, 0);
    assert_eq!(
        catches[0].types,
        vec![
            JavaType::Named {
                name: "NumberFormatException".to_string(),
                args: Vec::new(),
            },
            JavaType::Named {
                name: "NullPointerException".to_string(),
                args: Vec::new(),
            },
        ]
    );
    assert_eq!(
        semantic.facts.exception_kind_for("NumberFormatException"),
        Some(ExceptionKind::Unchecked)
    );
    assert_eq!(
        semantic.facts.exception_kind_for("NullPointerException"),
        Some(ExceptionKind::Unchecked)
    );
}
