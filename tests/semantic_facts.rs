use std::collections::HashSet;
use std::fs;
use std::path::Path;

use new_java2rust::facts::{
    ControlFlowFactKind, DispatchRequirementKind, ExceptionKind, ExternalApiArgumentEffectFact,
    ExternalApiCoverage, ExternalApiEffectFact, ExternalApiRole, ExternalArgumentAliasPolicy,
    ExternalFieldFact, ExternalFieldKind, ExternalLibraryMode, ExternalLibraryPackFact,
    FactProvenance, FactSet, FieldKey, NullabilityState, ReceiverMutabilityFact, RecursionKind,
    TrivialAccessorKind, TypeCapabilityOverride, UnresolvedTargetFact, UnresolvedTargetKind,
};
use new_java2rust::java_ir::{JavaType, PrimitiveType};
use new_java2rust::object_model::plan_object_model;
use new_java2rust::parser::parse_java_source;
use new_java2rust::resolve::resolve_program;
use new_java2rust::rewrite::{
    AssignmentConditionNormalizedShape, ControlFlowRewritePlan, NullTestKind,
    TailRecursiveNormalizedShape,
};
use new_java2rust::slot_shape::{OwnershipShape, ViewShape};
use new_java2rust::stdlib::{
    canonical_external_receiver_type_key, external_api_method_policy, external_field_provenance,
};
use new_java2rust::types::optional_java_type;
use new_java2rust::usage::{
    BindingUse, CallArgumentBoundary, FieldUse, UsageMethodKey, analyze_usage,
};
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

#[test]
fn source_summary_declaration_facts_preserve_nullable_method_return() {
    let parsed = parse_java_source(
        r#"
class Lookup {
    Path find(String sample) {
        return sample == null ? null : new Path();
    }
}

class Path {
}
"#,
    );
    let facts = resolve::declaration_facts_from_program(&parsed.program);
    let effect = facts
        .method_effect_for("Lookup", "find")
        .expect("summary declaration facts should include method effect");
    assert_eq!(effect.return_nullability, NullabilityState::MaybeNull);
}

#[test]
fn source_summary_declaration_facts_survive_stripped_summary_semantic_analysis() {
    let parsed = parse_java_source(
        r#"
class UsesNullableSourceSummaryReturn {
    Lookup lookup;

    Path run(String sample) {
        Path path = lookup.find(sample);
        return path;
    }
}
"#,
    );
    let summary = parse_java_source(
        r#"
class Lookup {
    Path find(String sample) {
        return sample == null ? null : new Path();
    }
}

class Path {
}
"#,
    );
    let mut source_program = parsed.program.clone();
    let mut summary_declarations = summary.program.clone();
    for ty in &mut summary_declarations.types {
        for member in &mut ty.members {
            if let new_java2rust::java_ir::Member::Method(method) = member {
                method.body = None;
            }
        }
    }
    source_program.types.extend(summary_declarations.types);
    let summary_facts = resolve::declaration_facts_from_program(&summary.program);
    let resolved = resolve::resolve_program_with_source_program_and_facts(
        &parsed.program,
        parsed.diagnostics,
        &source_program,
        &summary_facts,
    );
    let semantic_program = resolve::resolve_program_with_source_program_and_facts(
        &source_program,
        new_java2rust::diagnostics::DiagnosticBag::new(),
        &source_program,
        &summary_facts,
    )
    .program;
    let semantic =
        semantic_analysis::analyze_program(&semantic_program, &resolved.facts, &summary_facts);
    let find_effects = semantic
        .facts
        .method_effect_facts
        .iter()
        .filter(|fact| fact.method.method_name == "find")
        .collect::<Vec<_>>();
    let effect = semantic
        .facts
        .method_effect_for("Lookup", "find")
        .expect("summary declaration facts should remain visible after semantic analysis");
    assert_eq!(
        effect.return_nullability,
        NullabilityState::MaybeNull,
        "{find_effects:#?}"
    );
}

#[test]
fn indexed_hierarchy_reachability_matches_legacy_worklist() {
    let mut facts = FactSet::default();
    facts
        .type_capability_overrides
        .push(TypeCapabilityOverride {
            type_name: "pkg.Concrete".to_string(),
            copy: None,
            clone: None,
            string_like: None,
            sequence_like: None,
            implements: vec![JavaType::Named {
                name: "pkg.Marker".to_string(),
                args: Vec::new(),
            }],
            extends: Some(JavaType::Named {
                name: "pkg.Middle".to_string(),
                args: Vec::new(),
            }),
            final_type: None,
            object_safe_trait: None,
            provenance: FactProvenance::ExternalLibraryPack,
        });
    facts
        .type_capability_overrides
        .push(TypeCapabilityOverride {
            type_name: "pkg.Middle".to_string(),
            copy: None,
            clone: None,
            string_like: None,
            sequence_like: None,
            implements: Vec::new(),
            extends: Some(JavaType::Named {
                name: "pkg.Base".to_string(),
                args: Vec::new(),
            }),
            final_type: None,
            object_safe_trait: None,
            provenance: FactProvenance::ExternalLibraryPack,
        });
    let concrete_ty = JavaType::Named {
        name: "pkg.Concrete".to_string(),
        args: Vec::new(),
    };
    let base_ty = JavaType::Named {
        name: "pkg.Base".to_string(),
        args: Vec::new(),
    };
    let indexed = facts.hierarchy_lookup_index();

    assert_eq!(
        indexed.reachable_hierarchy_edges_for_type_with_stats(&facts, &concrete_ty),
        facts.reachable_hierarchy_edges_for_type_with_stats(&concrete_ty)
    );
    assert_eq!(
        indexed.hierarchy_subtype_policy(&facts, &concrete_ty, &base_ty),
        facts.hierarchy_subtype_policy(&concrete_ty, &base_ty)
    );
    assert!(
        indexed
            .hierarchy_subtype_policy(&facts, &concrete_ty, &base_ty)
            .reaches
    );
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
fn known_unchecked_exception_fact_beats_parsed_unknown_exception_fact() {
    let semantic = analyze(
        r#"
import htsjdk.samtools.SAMException;

class SamOnly {
    static String encode() {
        return write();
    }

    static String write() {
        throw new SAMException("x");
    }
}
"#,
    );

    assert_eq!(
        semantic.facts.exception_kind_for("SAMException"),
        Some(ExceptionKind::Unchecked)
    );
    assert_eq!(
        semantic
            .facts
            .exception_kind_for("htsjdk.samtools.SAMException"),
        Some(ExceptionKind::Unchecked)
    );
    assert!(
        !semantic
            .facts
            .method_has_thrown_exceptions("SamOnly", "write")
            && !semantic
                .facts
                .method_has_thrown_exceptions("SamOnly", "encode"),
        "{:?}",
        semantic.facts.method_effect_facts
    );
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
fn final_initialized_reference_field_is_non_null_despite_constructor() {
    let semantic = analyze(
        r#"
final class Constants {
    public static final String NAME = "alpha";

    private Constants() {}
}
"#,
    );

    assert_eq!(
        semantic.facts.field_nullability_for("Constants", "NAME"),
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
fn field_nullability_lattice_does_not_prove_unknown_assignments_non_null() {
    let semantic = analyze(
        r#"
class UnknownAssignedFields {
    String fromCall;
    String fromParam;
    String fromHelper;

    UnknownAssignedFields(String seed, Helper helper) {
        fromCall = helper.fetch();
        fromParam = seed;
        assignHelper(seed);
    }

    private void assignHelper(String seed) {
        fromHelper = seed;
    }
}

class Helper {
    String fetch() { return null; }
}
"#,
    );

    for field in ["fromCall", "fromParam", "fromHelper"] {
        assert_ne!(
            semantic
                .facts
                .field_nullability_for("UnknownAssignedFields", field),
            Some(NullabilityState::DefinitelyNonNull),
            "{field} must not be proven non-null from unknown assignment evidence"
        );
    }
}

#[test]
fn field_nullability_lattice_preserves_nullable_constructor_conflicts() {
    let semantic = analyze(
        r#"
class ConflictingAssignments {
    String name;

    ConflictingAssignments() {
        this("safe");
    }

    ConflictingAssignments(String seed) {
        name = "safe";
        name = null;
    }
}
"#,
    );

    assert_eq!(
        semantic
            .facts
            .field_nullability_for("ConflictingAssignments", "name"),
        Some(NullabilityState::MaybeNull)
    );
}

#[test]
fn field_nullability_lattice_keeps_constructor_delegation_cycles_finite() {
    let semantic = analyze(
        r#"
class CyclicFieldDelegation {
    String name;

    CyclicFieldDelegation() {
        this(1);
    }

    CyclicFieldDelegation(int value) {
        this();
        name = "safe";
    }
}
"#,
    );

    assert!(
        semantic.iterations.field_nullability > 0,
        "{:?}",
        semantic.iterations
    );
    assert!(
        semantic
            .diagnostics
            .iter()
            .all(|diagnostic| diagnostic.code != "semantic_field_nullability_budget_exhausted"),
        "{:?}",
        semantic.diagnostics
    );
    assert_ne!(
        semantic
            .facts
            .field_nullability_for("CyclicFieldDelegation", "name"),
        Some(NullabilityState::DefinitelyNonNull)
    );
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
        translation.rust.contains("self.cached = Some(value")
            && translation
                .rust
                .contains("/* TODO(translation): validate added clone */);"),
        "{}",
        translation.rust
    );
}

#[test]
fn field_nullability_and_signature_optionality_are_co_solved_for_constructor_coverage() {
    let source = r#"
class FieldOptionalityCases {
    String proven;
    String nullable;
    String externalUnknown;
    String missingCoverage;

    FieldOptionalityCases() {
        this.proven = "safe";
        this.nullable = null;
        this.externalUnknown = ExternalHelper.fetch();
    }

    FieldOptionalityCases(int skipped) {
        this.proven = "also safe";
        this.nullable = null;
        this.externalUnknown = ExternalHelper.fetch();
        this.missingCoverage = "present here only";
    }
}
"#;
    let semantic = analyze(source);
    let translation = translate_source(source);

    assert_eq!(
        semantic
            .facts
            .field_nullability_for("FieldOptionalityCases", "proven"),
        Some(NullabilityState::DefinitelyNonNull)
    );
    assert_eq!(
        semantic
            .facts
            .field_nullability_for("FieldOptionalityCases", "nullable"),
        Some(NullabilityState::MaybeNull)
    );
    assert_eq!(
        semantic
            .facts
            .field_nullability_for("FieldOptionalityCases", "externalUnknown"),
        Some(NullabilityState::MaybeNull)
    );
    assert_eq!(
        semantic
            .facts
            .field_nullability_for("FieldOptionalityCases", "missingCoverage"),
        Some(NullabilityState::MaybeNull)
    );

    assert!(
        translation.rust.contains("proven: String"),
        "{}",
        translation.rust
    );
    assert!(
        !translation.rust.contains("proven: Option<String>"),
        "{}",
        translation.rust
    );
    for field in ["nullable", "external_unknown", "missing_coverage"] {
        assert!(
            translation
                .rust
                .contains(&format!("{field}: Option<String>")),
            "{}",
            translation.rust
        );
    }
}

#[test]
fn bare_constructor_field_assignments_prove_non_null_when_unshadowed() {
    let source = r#"
class BareFieldAssignmentsAreFields {
    String name;

    BareFieldAssignmentsAreFields() {
        name = "safe";
    }

    BareFieldAssignmentsAreFields(int ignored) {
        name = new String("also safe");
    }

    int length() {
        return name.length();
    }
}
"#;
    let semantic = analyze(source);
    let translation = translate_source(source);

    assert_eq!(
        semantic
            .facts
            .field_nullability_for("BareFieldAssignmentsAreFields", "name"),
        Some(NullabilityState::DefinitelyNonNull)
    );
    assert!(
        translation.rust.contains("name: String")
            && !translation.rust.contains("name: Option<String>")
            && translation.rust.contains("return self.name.len() as i32;"),
        "{}",
        translation.rust
    );
}

#[test]
fn bare_constructor_assignment_respects_parameter_shadowing() {
    let source = r#"
class BareFieldAssignmentShadowedByParam {
    String name;

    BareFieldAssignmentShadowedByParam(String name) {
        name = "local only";
    }
}
"#;
    let semantic = analyze(source);
    let translation = translate_source(source);

    assert_eq!(
        semantic
            .facts
            .field_nullability_for("BareFieldAssignmentShadowedByParam", "name"),
        Some(NullabilityState::MaybeNull)
    );
    assert!(
        translation.rust.contains("name: Option<String>"),
        "{}",
        translation.rust
    );
}

#[test]
fn constructor_initialized_sorted_map_field_is_not_optional_carrier() {
    let source = r#"
import java.net.URI;
import java.util.SortedMap;
import java.util.TreeMap;

class ConstructorInitializedSortedMapField {
    SortedMap<String, URI> sampleNameToPath;

    ConstructorInitializedSortedMapField() {
        sampleNameToPath = new TreeMap<>();
    }

    int size() {
        return sampleNameToPath.size();
    }
}
"#;
    let semantic = analyze(source);
    let translation = translate_source(source);

    assert_eq!(
        semantic
            .facts
            .field_nullability_for("ConstructorInitializedSortedMapField", "sampleNameToPath",),
        Some(NullabilityState::DefinitelyNonNull)
    );
    assert!(
        translation
            .rust
            .contains("sample_name_to_path: std::collections::HashMap<String, URI>")
            && !translation.rust.contains("sample_name_to_path: Option<")
            && translation
                .rust
                .contains("return self.sample_name_to_path.len() as i32;")
            && !translation.rust.contains("sample_name_to_path.size()"),
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
fn preserves_public_trivial_accessor_calls_when_api_wrappers_must_survive() {
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
        translation
            .rust
            .contains("self.set_value(other.get_value())"),
        "{}",
        translation.rust
    );
    assert!(
        translation.rust.contains("return self.get_value();"),
        "{}",
        translation.rust
    );
}

#[test]
fn lowers_package_private_trivial_accessor_calls_as_field_access() {
    let translation = translate_source(&idiom_gap("PackagePrivateGetterSetterInlining.java"));

    assert!(
        translation.rust.contains("fn get_value(&self) -> i32"),
        "{}",
        translation.rust
    );
    assert!(
        translation.rust.contains("fn set_value(&mut self"),
        "{}",
        translation.rust
    );
    assert!(
        translation.rust.contains("fn copy_from(&mut self"),
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
fn usage_treats_native_collection_field_mutators_as_self_mutation() {
    let semantic = analyze(&idiom_gap("CollectionFieldMutator.java"));
    let put_value = semantic
        .usage
        .methods
        .get(&UsageMethodKey {
            type_name: "CollectionFieldMutator".to_string(),
            method_name: "putValue".to_string(),
        })
        .unwrap();

    assert!(put_value.mutates_self, "{put_value:?}");
}

#[test]
fn usage_propagates_self_mutability_through_same_receiver_calls() {
    let parsed = parse_java_source(
        r#"
class SelfMutabilityPropagation {
    int count;

    void outer() {
        middle();
    }

    void middle() {
        this.inner();
    }

    void inner() {
        count = count + 1;
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let usage = analyze_usage(&resolved.program);

    for method_name in ["outer", "middle", "inner"] {
        let method_usage = usage
            .methods
            .get(&new_java2rust::usage::UsageMethodKey {
                type_name: "SelfMutabilityPropagation".to_string(),
                method_name: method_name.to_string(),
            })
            .unwrap();
        assert!(
            method_usage.mutates_self,
            "{method_name} should inherit mutating receiver pressure through same-receiver calls: {method_usage:?}"
        );
    }
}

#[test]
fn method_throw_effects_propagate_through_source_call_graph() {
    let semantic = analyze(&idiom_gap("ThrowingCallChainResultShell.java"));

    assert!(
        semantic
            .facts
            .method_has_thrown_exceptions("ThrowingCallChainResultShell", "leaf")
    );
    assert!(
        semantic
            .facts
            .method_has_thrown_exceptions("ThrowingCallChainResultShell", "middle")
    );
    assert!(
        semantic
            .facts
            .method_has_thrown_exceptions("ThrowingCallChainResultShell", "top")
    );
}

#[test]
fn method_throw_effects_saturate_recursive_source_call_graph() {
    let semantic = analyze(
        r#"
class RecursiveThrowPropagation {
    String a(boolean ok) {
        if (ok) {
            return b(ok);
        }
        return leaf();
    }

    String b(boolean ok) {
        return a(ok);
    }

    String leaf() throws IOException {
        throw new IOException("bad");
    }
}

class IOException extends Exception {
    IOException(String message) {
        super(message);
    }
}
"#,
    );

    assert!(
        semantic
            .facts
            .method_has_thrown_exceptions("RecursiveThrowPropagation", "a")
    );
    assert!(
        semantic
            .facts
            .method_has_thrown_exceptions("RecursiveThrowPropagation", "b")
    );
    assert!(
        semantic
            .facts
            .method_has_thrown_exceptions("RecursiveThrowPropagation", "leaf")
    );
}

#[test]
fn lambda_block_throw_marks_enclosing_method_throwing() {
    let semantic = analyze(
        r#"
import java.util.Map;

class LambdaThrowPropagation {
    void waitAll(Map<String, Future> futures) {
        futures.forEach((name, future) -> {
            try {
                future.get();
            } catch (ExecutionException e) {
                throw new UserException("bad", e);
            }
        });
    }
}
"#,
    );

    assert!(
        semantic
            .facts
            .method_has_thrown_exceptions("LambdaThrowPropagation", "waitAll")
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
fn external_api_effect_receiver_lookup_matches_simple_owner_for_fqn_facts() {
    let mut facts = FactSet::default();
    facts.external_api_effect_facts.push(ExternalApiEffectFact {
        method_name: "isGenomicsDBArray".to_string(),
        receiver_type: Some("org.genomicsdb.GenomicsDBUtils".to_string()),
        arg_count: Some(2),
        role: ExternalApiRole::Getter,
        receiver_mutability: new_java2rust::facts::ReceiverMutabilityFact::ReadOnly,
        return_type: Some(JavaType::Primitive(PrimitiveType::Boolean)),
        return_nullability: NullabilityState::DefinitelyNonNull,
        thrown: Vec::new(),
        provenance: FactProvenance::ExternalLibraryPack,
    });

    let effect = facts
        .external_api_effect_for_receiver_with_arity(
            Some("GenomicsDBUtils"),
            "is_genomics_d_b_array",
            Some(2),
        )
        .expect("FQN external API facts should be reusable through simple imported owners");

    assert_eq!(
        effect.return_type,
        Some(JavaType::Primitive(PrimitiveType::Boolean))
    );
}

#[test]
fn indexed_external_api_lookup_preserves_override_and_arity_precedence() {
    let mut facts = FactSet::default();
    facts.external_api_effect_facts.push(ExternalApiEffectFact {
        method_name: "readValue".to_string(),
        receiver_type: Some("dep.NativeHandle".to_string()),
        arg_count: Some(1),
        role: ExternalApiRole::Getter,
        receiver_mutability: ReceiverMutabilityFact::ReadOnly,
        return_type: Some(JavaType::Primitive(PrimitiveType::Int)),
        return_nullability: NullabilityState::DefinitelyNonNull,
        thrown: Vec::new(),
        provenance: FactProvenance::ExternalLibraryPack,
    });
    facts.external_api_effect_facts.push(ExternalApiEffectFact {
        method_name: "readValue".to_string(),
        receiver_type: None,
        arg_count: Some(1),
        role: ExternalApiRole::Getter,
        receiver_mutability: ReceiverMutabilityFact::ReadOnly,
        return_type: Some(JavaType::String),
        return_nullability: NullabilityState::MaybeNull,
        thrown: Vec::new(),
        provenance: FactProvenance::UserOverride,
    });

    let indexed = facts.external_lookup_index();
    let legacy = facts
        .external_api_effect_for_receiver_with_arity(Some("NativeHandle"), "read_value", Some(1))
        .unwrap();
    let indexed_effect = indexed
        .external_api_effect_for_receiver_with_arity(Some("NativeHandle"), "read_value", Some(1))
        .unwrap();

    assert_eq!(indexed_effect, legacy);
    assert_eq!(indexed_effect.provenance, FactProvenance::UserOverride);
    assert_eq!(indexed_effect.return_type, Some(JavaType::String));
}

#[test]
fn indexed_external_api_lookup_preserves_global_reverse_order_across_alias_buckets() {
    let mut facts = FactSet::default();
    facts.external_api_effect_facts.push(ExternalApiEffectFact {
        method_name: "readValue".to_string(),
        receiver_type: Some("NativeHandle".to_string()),
        arg_count: Some(0),
        role: ExternalApiRole::Getter,
        receiver_mutability: ReceiverMutabilityFact::ReadOnly,
        return_type: Some(JavaType::String),
        return_nullability: NullabilityState::DefinitelyNonNull,
        thrown: Vec::new(),
        provenance: FactProvenance::ExternalLibraryPack,
    });
    facts.external_api_effect_facts.push(ExternalApiEffectFact {
        method_name: "read_value".to_string(),
        receiver_type: Some("NativeHandle".to_string()),
        arg_count: Some(0),
        role: ExternalApiRole::Getter,
        receiver_mutability: ReceiverMutabilityFact::ReadOnly,
        return_type: Some(JavaType::Primitive(PrimitiveType::Int)),
        return_nullability: NullabilityState::DefinitelyNonNull,
        thrown: Vec::new(),
        provenance: FactProvenance::ExternalLibraryPack,
    });

    let indexed = facts.external_lookup_index();
    let legacy = facts
        .external_api_effect_for_receiver_with_arity(Some("NativeHandle"), "readValue", Some(0))
        .unwrap();
    let indexed_effect = indexed
        .external_api_effect_for_receiver_with_arity(Some("NativeHandle"), "readValue", Some(0))
        .unwrap();

    assert_eq!(indexed_effect, legacy);
    assert_eq!(
        indexed_effect.return_type,
        Some(JavaType::Primitive(PrimitiveType::Int))
    );
}

#[test]
fn indexed_external_field_lookup_preserves_direct_then_alias_precedence() {
    let mut facts = FactSet::default();
    facts.external_field_facts.push(ExternalFieldFact {
        field: FieldKey {
            type_name: "dep.Constants".to_string(),
            field_name: "MAX_COUNT".to_string(),
        },
        field_type: Some(JavaType::Primitive(PrimitiveType::Long)),
        field_kind: ExternalFieldKind::StaticFinal,
        provenance: FactProvenance::ExternalLibraryPack,
    });
    facts.external_field_facts.push(ExternalFieldFact {
        field: FieldKey {
            type_name: "Constants".to_string(),
            field_name: "MAX_COUNT".to_string(),
        },
        field_type: Some(JavaType::Primitive(PrimitiveType::Int)),
        field_kind: ExternalFieldKind::StaticFinal,
        provenance: FactProvenance::UserOverride,
    });

    let indexed = facts.external_lookup_index();
    let legacy = facts.external_field_for("Constants", "MAX_COUNT").unwrap();
    let indexed_field = indexed
        .external_field_for("Constants", "MAX_COUNT")
        .unwrap();

    assert_eq!(indexed_field, legacy);
    assert_eq!(indexed_field.provenance, FactProvenance::UserOverride);
    assert_eq!(
        indexed_field.field_type,
        Some(JavaType::Primitive(PrimitiveType::Int))
    );
}

#[test]
fn indexed_external_pack_lookup_preserves_member_alias_fallback() {
    let mut facts = FactSet::default();
    facts.external_library_packs.push(ExternalLibraryPackFact {
        pack_name: "dep".to_string(),
        package_prefix: "dep".to_string(),
        mode: ExternalLibraryMode::RustCrateMapping,
        coverage: ExternalApiCoverage::Mapped,
        rust_target: Some("dep_facade".to_string()),
        provenance: FactProvenance::ExternalLibraryPack,
    });
    facts.external_api_effect_facts.push(ExternalApiEffectFact {
        method_name: "read".to_string(),
        receiver_type: Some("dep.NativeHandle".to_string()),
        arg_count: Some(0),
        role: ExternalApiRole::Getter,
        receiver_mutability: ReceiverMutabilityFact::ReadOnly,
        return_type: Some(JavaType::String),
        return_nullability: NullabilityState::DefinitelyNonNull,
        thrown: Vec::new(),
        provenance: FactProvenance::ExternalLibraryPack,
    });

    let indexed = facts.external_lookup_index();
    let legacy = facts
        .external_library_pack_for_type("other.NativeHandle")
        .unwrap();
    let indexed_pack = indexed
        .external_library_pack_for_type("other.NativeHandle")
        .unwrap();

    assert_eq!(indexed_pack, legacy);
    assert_eq!(indexed_pack.pack_name, "dep");
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
fn tuple_like_external_constructor_facts_keep_dependency_tuples_pack_owned() {
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

    for (type_name, arity, owner) in [
        ("Pair", 2, "Apache Commons Lang"),
        ("ImmutablePair", 2, "Apache Commons Lang"),
        ("MutablePair", 2, "Apache Commons Lang"),
        ("ImmutableTriple", 3, "Apache Commons Lang"),
        ("Tuple2", 2, "Scala tuple"),
        ("Tuple3", 3, "Scala tuple"),
    ] {
        let fact = semantic
            .facts
            .external_constructor_facts
            .iter()
            .find(|fact| fact.type_name == type_name && fact.arg_types.len() == arity)
            .unwrap_or_else(|| {
                panic!("expected external constructor fact for {type_name}/{arity}")
            });
        assert_eq!(
            fact.provenance,
            FactProvenance::ExternalLibraryPack,
            "{type_name}/{arity} constructor should be owned by the {owner} pack"
        );
        let args = vec![JavaType::Unknown("arg".to_string()); arity];
        assert_eq!(
            new_java2rust::stdlib::external_constructor_provenance(type_name, &args),
            FactProvenance::ExternalLibraryPack,
            "{type_name}/{arity} precision should come from the {owner} pack, not stdlib"
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
        URL url = new URL(path);
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
        ("URL", vec![JavaType::String]),
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
        ("SeekablePathStream", 1),
        ("BinaryCodec", 0),
        ("BinaryCodec", 1),
        ("BufferedLineReader", 1),
        ("AsciiLineReaderIterator", 1),
        ("LineIteratorImpl", 1),
        ("SynchronousLineReader", 1),
        ("FeatureCodecHeader", 2),
        ("CodecLineParsingException", 1),
        ("FilteringSamIterator", 2),
        ("SAMTextHeaderCodec", 0),
        ("SAMFormatException", 1),
        ("FastaSequenceFile", 2),
        ("IndexedFastaSequenceFile", 1),
        ("ReferenceSequence", 3),
        ("FastaReferenceWriterBuilder", 0),
        ("BlockCompressedOutputStream", 2),
        ("BlockCompressedOutputStream", 3),
        ("DefaultingMap", 2),
        ("Lazy", 1),
        ("SamFileHeaderMerger", 3),
        ("MergingSamRecordIterator", 3),
        ("Gff3BaseData", 9),
        ("SAMFileWriterFactory", 0),
        ("BAMRecordCodec", 1),
        ("BAMIndexer", 2),
        ("SBIIndexWriter", 2),
        ("CramContainerIterator", 1),
        ("Container", 3),
        ("CRAMReferenceRegion", 2),
        ("BCF2Codec", 0),
        ("MetricsFile", 0),
        ("ComponentMethods", 0),
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

    let program_record = semantic
        .facts
        .external_constructor_facts
        .iter()
        .find(|fact| fact.type_name == "SAMProgramRecord" && fact.arg_types.len() == 1)
        .unwrap_or_else(|| panic!("expected external constructor fact for SAMProgramRecord/1"));
    assert_eq!(
        program_record.provenance,
        FactProvenance::ExternalLibraryPack,
        "SAMProgramRecord/1 constructor should be owned by the SAM header external pack"
    );

    let sam_record = semantic
        .facts
        .external_constructor_facts
        .iter()
        .find(|fact| fact.type_name == "SAMRecord" && fact.arg_types.len() == 1)
        .unwrap_or_else(|| panic!("expected external constructor fact for SAMRecord/1"));
    assert_eq!(
        sam_record.provenance,
        FactProvenance::ExternalLibraryPack,
        "SAMRecord/1 constructor should be owned by the SAM record external pack"
    );

    for type_name in [
        "DefaultSAMRecordFactory",
        "SAMRecordCoordinateComparator",
        "SAMRecordQueryNameComparator",
    ] {
        let fact = semantic
            .facts
            .external_constructor_facts
            .iter()
            .find(|fact| fact.type_name == type_name && fact.arg_types.is_empty())
            .unwrap_or_else(|| panic!("expected external constructor fact for {type_name}/0"));
        assert_eq!(
            fact.provenance,
            FactProvenance::ExternalLibraryPack,
            "{type_name}/0 constructor should be owned by the SAM record external pack"
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
fn apache_commons_collections_constructor_facts_are_external_pack_models() {
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
            FactProvenance::ExternalLibraryPack,
            "{type_name}/{arg_len} constructor should be an audited Apache Commons Collections external model"
        );
    }
}

#[test]
fn apache_commons_compress_constructor_facts_are_external_pack_models() {
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
            FactProvenance::ExternalLibraryPack,
            "{type_name}/{arg_len} constructor should be an audited Apache Commons Compress external model"
        );
    }
}

#[test]
fn apache_commons_lang_builder_constructor_facts_are_external_pack_models() {
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
            FactProvenance::ExternalLibraryPack,
            "{type_name}/{arg_len} constructor should be an audited Apache Commons Lang external model"
        );
    }
}

#[test]
fn apache_commons_external_constructor_facts_are_external_pack_models() {
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
            "{type_name}/{arg_len} constructor should be an audited Apache Commons external pack model"
        );
    }
    for (type_name, arg_len) in [("Percentile", 0), ("Percentile", 1)] {
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
            "{type_name}/{arg_len} constructor should be owned by the Apache Commons stats pack"
        );
    }
}

#[test]
fn hipparchus_constructor_facts_are_external_pack_models() {
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
        FactProvenance::ExternalLibraryPack,
        "EmpiricalDistribution/0 constructor should be an audited Hipparchus external model"
    );
}

#[test]
fn fastutil_constructor_facts_are_external_pack_models() {
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
            FactProvenance::ExternalLibraryPack,
            "{type_name}/{arg_len} constructor should be an audited fastutil external pack model"
        );
    }
}

#[test]
fn json_constructor_facts_are_external_pack_models() {
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
            FactProvenance::ExternalLibraryPack,
            "{type_name}/{arg_len} constructor should be an audited JSON external pack model"
        );
    }
}

#[test]
fn intel_gkl_constructor_facts_are_external_pack_models() {
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
            FactProvenance::ExternalLibraryPack,
            "{type_name}/0 constructor should be owned by the Intel GKL pack"
        );
    }
}

#[test]
fn jama_matrix_constructor_facts_are_external_pack_models() {
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
            FactProvenance::ExternalLibraryPack,
            "Matrix/{arg_len} constructor should be an audited Jama external model"
        );
    }
}

#[test]
fn picard_constructor_facts_have_family_specific_provenance() {
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

    for (type_name, arg_len, provenance) in [
        ("OpticalDuplicateFinder", 0),
        ("OpticalDuplicateFinder", 3),
        ("GenotypeConcordanceCounts", 0),
        ("GenotypeConcordanceSummaryMetrics", 5),
    ]
    .into_iter()
    .map(|(type_name, arg_len)| (type_name, arg_len, FactProvenance::ExternalLibraryPack))
    .chain(
        [
            ("DiagnosticsAndQCProgramGroup", 0),
            ("IntervalsManipulationProgramGroup", 0),
            ("OtherProgramGroup", 0),
            ("ReadDataManipulationProgramGroup", 0),
            ("ReferenceProgramGroup", 0),
        ]
        .into_iter()
        .map(|(type_name, arg_len)| (type_name, arg_len, FactProvenance::ExternalLibraryPack)),
    ) {
        let fact = semantic
            .facts
            .external_constructor_facts
            .iter()
            .find(|fact| fact.type_name == type_name && fact.arg_types.len() == arg_len)
            .unwrap_or_else(|| {
                panic!("expected external constructor fact for {type_name}/{arg_len}")
            });
        assert_eq!(
            fact.provenance, provenance,
            "{type_name}/{arg_len} constructor should have the expected Picard family provenance"
        );
    }
}

#[test]
fn disq_constructor_facts_are_external_pack_models() {
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
            FactProvenance::ExternalLibraryPack,
            "{type_name}/2 constructor should be owned by the DISQ pack"
        );
    }
}

#[test]
fn broad_http_constructor_facts_are_external_pack_models() {
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
        FactProvenance::ExternalLibraryPack,
        "HttpFileSystemProviderSettings/3 constructor should be an audited Broad HTTP external-pack model"
    );
}

#[test]
fn barclay_command_line_exception_constructor_facts_are_external_pack_models() {
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
            FactProvenance::ExternalLibraryPack,
            "{type_name}/{arg_len} constructor should be owned by the Barclay argparser pack"
        );
    }
}

#[test]
fn barclay_reflection_helper_constructor_facts_are_external_pack_models() {
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
            FactProvenance::ExternalLibraryPack,
            "{type_name}/{arg_len} constructor should be owned by a Barclay external pack"
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
fn opencsv_external_constructor_facts_are_external_pack_models() {
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
            FactProvenance::ExternalLibraryPack,
            "{type_name}/{arg_len} constructor should be an audited OpenCSV pack model"
        );
    }
}

#[test]
fn hadoop_configuration_constructor_facts_are_external_pack_models() {
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
        FactProvenance::ExternalLibraryPack,
        "Configuration/0 constructor should be an audited Hadoop conf pack model"
    );
}

#[test]
fn apache_commons_configuration_constructor_facts_are_external_pack_models() {
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
        FactProvenance::ExternalLibraryPack,
        "Configurations/0 constructor should be an audited Apache Commons Configuration pack model"
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
fn spark_constructor_facts_are_external_pack_models() {
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
            FactProvenance::ExternalLibraryPack,
            "{type_name}/{arg_len} constructor should be an audited Spark pack model"
        );
    }
}

#[test]
fn guava_concurrent_constructor_facts_are_external_pack_models() {
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
        FactProvenance::ExternalLibraryPack,
        "ThreadFactoryBuilder/0 constructor should be owned by the Guava concurrent pack"
    );
}

#[test]
fn reflections_constructor_facts_are_external_pack_models() {
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
        FactProvenance::ExternalLibraryPack,
        "Reflections/1 constructor should be an audited Reflections external pack model"
    );
}

#[test]
fn apache_http_constructor_facts_are_external_pack_models() {
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
            FactProvenance::ExternalLibraryPack,
            "{type_name}/{arg_len} constructor should be an audited Apache HTTP external model"
        );
    }
}

#[test]
fn kryo_serializer_constructor_facts_are_external_pack_models() {
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
            FactProvenance::ExternalLibraryPack,
            "{type_name}/{arg_len} constructor should be an audited Kryo pack model"
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
            FactProvenance::ExternalLibraryPack,
            "{type_name}/{arg_len} constructor should be owned by the GenomicsDB external pack"
        );
    }
}

#[test]
fn avro_constructor_facts_are_external_pack_models() {
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
        FactProvenance::ExternalLibraryPack,
        "GenericDatumReader/1 constructor should be owned by the Avro external pack"
    );
}

#[test]
fn bwa_native_wrapper_constructor_facts_are_external_pack_models() {
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
            FactProvenance::ExternalLibraryPack,
            "{type_name}/{arg_len} constructor should be an audited BWA native-wrapper pack model"
        );
    }
}

#[test]
fn jgrapht_algorithm_constructor_facts_are_external_pack_models() {
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
            FactProvenance::ExternalLibraryPack,
            "{type_name} constructor should be an audited JGraphT external pack model"
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
        FactProvenance::ExternalLibraryPack,
        "DOTExporter/5 constructor should be an audited JGraphT external pack model"
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
fn parsed_field_facts_preserve_source_summary_constant_types() {
    let parsed = parser::parse_java_source(
        r#"
class SourceSummaryConstantUse {
    String read() {
        return GATKConfig.CONFIG_FILE_VARIABLE_FILE_NAME;
    }
}
"#,
    );
    let resolved = resolve::resolve_program(&parsed.program, parsed.diagnostics);
    let external = external_facts::parse_external_facts(
        "field GATKConfig.CONFIG_FILE_VARIABLE_FILE_NAME String\n",
    );
    assert!(
        external.diagnostics.is_empty(),
        "{:?}",
        external.diagnostics
    );
    let semantic =
        semantic_analysis::analyze_program(&resolved.program, &resolved.facts, &external.facts);

    let config = semantic
        .facts
        .external_field_for("GATKConfig", "CONFIG_FILE_VARIABLE_FILE_NAME")
        .expect("source-summary field fact should be preserved");
    assert_eq!(config.provenance, FactProvenance::UserOverride);
    assert_eq!(config.field_type, Some(JavaType::String));
    assert_eq!(
        semantic
            .facts
            .external_field_facts
            .iter()
            .filter(|fact| fact.field.type_name == "GATKConfig"
                && fact.field.field_name == "CONFIG_FILE_VARIABLE_FILE_NAME")
            .count(),
        1,
        "semantic analysis should not add a conservative duplicate for parsed field facts"
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
        String cloudScheme = CloudStorageFileSystem.URI_SCHEME;
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
        ("Collections", "EMPTY_LIST"),
        ("Collections", "EMPTY_SET"),
        ("Collections", "EMPTY_MAP"),
        ("VCFHeaderLineType", "String"),
        ("VCFHeaderLineType", "Integer"),
        ("VCFHeaderLineCount", "A"),
        ("VCFHeaderLineCount", "UNBOUNDED"),
        ("GenotypeType", "HET"),
        ("Strand", "POSITIVE"),
        ("SAMFlag", "SECONDARY_ALIGNMENT"),
        ("VCFHeaderVersion", "VCF4_2"),
        ("HEADER_FIELDS", "INFO"),
        ("TimeUnit", "SECONDS"),
        ("Options", "INDEX_ON_THE_FLY"),
        ("Options", "DO_NOT_WRITE_GENOTYPES"),
        ("SWOverhangStrategy", "INDEL"),
        ("SWOverhangStrategy", "SOFTCLIP"),
    ] {
        if let Some(fact) = semantic.facts.external_field_for(type_name, field_name) {
            assert!(
                matches!(
                    fact.provenance,
                    FactProvenance::BuiltinExternalModel | FactProvenance::ExternalLibraryPack
                ),
                "known external field {type_name}.{field_name} should not use conservative fallback: {:?}",
                fact.provenance
            );
        }
    }

    let sam_header_facts = new_java2rust::external_packs::htsjdk_sam_header_pack_facts();
    let unknown_sequence_length = sam_header_facts
        .external_field_for("SAMSequenceRecord", "UNKNOWN_SEQUENCE_LENGTH")
        .expect("SAM sequence length sentinel should be explained by the SAM header pack");
    assert_eq!(
        unknown_sequence_length.provenance,
        FactProvenance::ExternalLibraryPack
    );
    assert_eq!(
        unknown_sequence_length.field_type,
        Some(JavaType::Primitive(PrimitiveType::Int))
    );

    let hdf5_facts = new_java2rust::external_packs::hdf5_pack_facts();
    let create_mode = hdf5_facts
        .external_field_for("OpenMode", "CREATE")
        .expect("HDF5 open mode should be explained by the HDF5 pack");
    assert_eq!(create_mode.provenance, FactProvenance::ExternalLibraryPack);
    assert_eq!(
        create_mode.field_type,
        Some(JavaType::Named {
            name: "OpenMode".to_string(),
            args: Vec::new(),
        })
    );

    let disq_facts = new_java2rust::external_packs::disq_write_pack_facts();
    for (type_name, field_name) in [
        ("ReadsFormatWriteOption", "BAM"),
        ("FileCardinalityWriteOption", "MULTIPLE"),
        ("FileCardinalityWriteOption", "SINGLE"),
    ] {
        let field = disq_facts
            .external_field_for(type_name, field_name)
            .unwrap_or_else(|| panic!("{type_name}.{field_name} should be explained by DISQ"));
        assert_eq!(field.provenance, FactProvenance::ExternalLibraryPack);
    }

    let intel_gkl_facts = new_java2rust::external_packs::intel_gkl_pack_facts();
    for (type_name, field_name, expected_ty) in [
        (
            "PDHMMNativeArguments",
            "avxLevel",
            JavaType::Named {
                name: "AVXLevel".to_string(),
                args: Vec::new(),
            },
        ),
        (
            "PDHMMNativeArguments",
            "openMPSetting",
            JavaType::Named {
                name: "OpenMPSetting".to_string(),
                args: Vec::new(),
            },
        ),
        (
            "PDHMMNativeArguments",
            "maxNumberOfThreads",
            JavaType::Primitive(PrimitiveType::Int),
        ),
        (
            "PairHMMNativeArguments",
            "maxNumberOfThreads",
            JavaType::Primitive(PrimitiveType::Int),
        ),
        (
            "PairHMMNativeArguments",
            "useDoublePrecision",
            JavaType::Primitive(PrimitiveType::Boolean),
        ),
        (
            "ReadDataHolder",
            "readBases",
            JavaType::Array(Box::new(JavaType::Primitive(PrimitiveType::Byte))),
        ),
        (
            "ReadDataHolder",
            "readQuals",
            JavaType::Array(Box::new(JavaType::Primitive(PrimitiveType::Byte))),
        ),
        (
            "ReadDataHolder",
            "insertionGOP",
            JavaType::Array(Box::new(JavaType::Primitive(PrimitiveType::Byte))),
        ),
        (
            "ReadDataHolder",
            "deletionGOP",
            JavaType::Array(Box::new(JavaType::Primitive(PrimitiveType::Byte))),
        ),
        (
            "ReadDataHolder",
            "overallGCP",
            JavaType::Array(Box::new(JavaType::Primitive(PrimitiveType::Byte))),
        ),
        (
            "HaplotypeDataHolder",
            "haplotypeBases",
            JavaType::Array(Box::new(JavaType::Primitive(PrimitiveType::Byte))),
        ),
        (
            "HaplotypeDataHolder",
            "haplotypePDBases",
            JavaType::Array(Box::new(JavaType::Primitive(PrimitiveType::Byte))),
        ),
        (
            "BatchCompletionCallbackFunctionArgument",
            "batchCount",
            JavaType::Primitive(PrimitiveType::Int),
        ),
        (
            "BatchCompletionCallbackFunctionArgument",
            "totalBatchCount",
            JavaType::Primitive(PrimitiveType::Int),
        ),
        (
            "SWNativeAlignerResult",
            "alignment_offset",
            JavaType::Primitive(PrimitiveType::Int),
        ),
        ("SWNativeAlignerResult", "cigar", JavaType::String),
    ] {
        let field = intel_gkl_facts
            .external_field_for(type_name, field_name)
            .unwrap_or_else(|| panic!("{type_name}.{field_name} should be explained by GKL"));
        assert_eq!(field.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(field.field_type, Some(expected_ty));
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
        ("Collections", "EMPTY_LIST"),
        ("Collections", "EMPTY_SET"),
        ("Collections", "EMPTY_MAP"),
        ("VCFHeaderLineType", "String"),
        ("VCFHeaderLineType", "Integer"),
        ("VCFHeaderLineCount", "A"),
        ("VCFHeaderLineCount", "UNBOUNDED"),
        ("GenotypeType", "HET"),
        ("Strand", "POSITIVE"),
        ("SAMFlag", "SECONDARY_ALIGNMENT"),
        ("VCFHeaderVersion", "VCF4_2"),
        ("HEADER_FIELDS", "INFO"),
        ("TimeUnit", "SECONDS"),
        ("File", "separator"),
        ("File", "separatorChar"),
        ("Spliterator", "IMMUTABLE"),
        ("Spliterator", "NONNULL"),
        ("StandardOpenOption", "CREATE_NEW"),
        ("AccessMode", "READ"),
        ("AccessMode", "WRITE"),
        ("PosixFilePermission", "GROUP_EXECUTE"),
        ("HttpMethod", "GET"),
        ("Math", "PI"),
        ("Option", "CACHE_FILE_BASED_INDEXES"),
        ("Option", "INCLUDE_SOURCE_IN_RECORDS"),
        ("Level", "DEBUG"),
        ("Level", "INFO"),
        ("Level", "ERROR"),
        ("Locale", "ENGLISH"),
        ("ElementKind", "FIELD"),
        ("Attribute", "PRIMITIVE"),
        ("Priority", "BATCH"),
        ("StructuralVariantType", "DEL"),
        ("StandardCharsets", "US_ASCII"),
        ("Pattern", "CASE_INSENSITIVE"),
        ("RoundingMode", "HALF_UP"),
    ] {
        let provenance = external_field_provenance(type_name, field_name);
        assert!(
            matches!(
                provenance,
                FactProvenance::BuiltinExternalModel | FactProvenance::ExternalLibraryPack
            ),
            "{type_name}.{field_name} should not use conservative fallback: {provenance:?}"
        );
    }
    let apache_commons_math_optim_facts =
        new_java2rust::external_packs::apache_commons_math_optim_pack_facts();
    for field_name in ["MAXIMIZE", "MINIMIZE"] {
        let field = apache_commons_math_optim_facts
            .external_field_for("GoalType", field_name)
            .unwrap_or_else(|| panic!("GoalType.{field_name} should be explained by Commons Math"));
        assert_eq!(field.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(
            field.field_type,
            Some(JavaType::Named {
                name: "GoalType".to_string(),
                args: Vec::new(),
            })
        );
        assert_eq!(
            external_field_provenance("GoalType", field_name),
            FactProvenance::ExternalLibraryPack
        );
    }
    let picard_metrics_facts = new_java2rust::external_packs::picard_metrics_pack_facts();
    for (field_name, expected_ty) in [
        ("LIBRARY", JavaType::String),
        (
            "READ_PAIRS_EXAMINED",
            JavaType::Primitive(PrimitiveType::Long),
        ),
        (
            "PERCENT_DUPLICATION",
            JavaType::Primitive(PrimitiveType::Double),
        ),
    ] {
        let field = picard_metrics_facts
            .external_field_for("DuplicationMetrics", field_name)
            .unwrap_or_else(|| {
                panic!("DuplicationMetrics.{field_name} should be explained by Picard metrics")
            });
        assert_eq!(field.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(field.field_type, Some(expected_ty));
        assert_eq!(
            external_field_provenance("DuplicationMetrics", field_name),
            FactProvenance::ExternalLibraryPack
        );
    }
    let broad_http_facts = new_java2rust::external_packs::broad_http_pack_facts();
    let default_settings = broad_http_facts
        .external_field_for("HttpFileSystemProviderSettings", "DEFAULT_SETTINGS")
        .expect("Broad HTTP default settings should be explained by the Broad HTTP pack");
    assert_eq!(
        default_settings.provenance,
        FactProvenance::ExternalLibraryPack
    );
    assert_eq!(
        default_settings.field_type,
        Some(JavaType::Named {
            name: "HttpFileSystemProviderSettings".to_string(),
            args: Vec::new(),
        })
    );
    for type_name in ["HttpFileSystemProvider", "HttpsFileSystemProvider"] {
        let field = broad_http_facts
            .external_field_for(type_name, "SCHEME")
            .unwrap_or_else(|| {
                panic!("{type_name}.SCHEME should be explained by the Broad HTTP pack")
            });
        assert_eq!(field.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(field.field_type, Some(JavaType::String));
        assert_eq!(
            external_field_provenance(type_name, "SCHEME"),
            FactProvenance::ExternalLibraryPack
        );
    }
    for field_name in [
        "DEFAULT_RETRYABLE_HTTP_CODES",
        "DEFAULT_RETRYABLE_EXCEPTIONS",
        "DEFALT_RETRYABLE_MESSAGES",
    ] {
        let field = broad_http_facts
            .external_field_for("RetryHandler", field_name)
            .unwrap_or_else(|| {
                panic!("RetryHandler.{field_name} should be explained by the Broad HTTP pack")
            });
        assert_eq!(field.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(
            field.field_type,
            Some(JavaType::Named {
                name: "Object".to_string(),
                args: Vec::new(),
            })
        );
        assert_eq!(
            external_field_provenance("RetryHandler", field_name),
            FactProvenance::ExternalLibraryPack
        );
    }
    let barclay_help_facts = new_java2rust::external_packs::barclay_help_pack_facts();
    for field_name in [
        "ARGUMENTS_POSITIONAL",
        "ARGUMENT_DEFAULT_VALUE",
        "ARGUMENT_TYPE",
    ] {
        let field = barclay_help_facts
            .external_field_for("TemplateProperties", field_name)
            .unwrap_or_else(|| {
                panic!("TemplateProperties.{field_name} should be explained by Barclay help pack")
            });
        assert_eq!(field.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(field.field_type, Some(JavaType::String));
        assert_eq!(
            external_field_provenance("TemplateProperties", field_name),
            FactProvenance::ExternalLibraryPack
        );
    }
    let barclay_argparser_facts = new_java2rust::external_packs::barclay_argparser_pack_facts();
    let show_hidden = barclay_argparser_facts
        .external_field_for("SpecialArgumentsCollection", "SHOW_HIDDEN")
        .expect(
            "SpecialArgumentsCollection.SHOW_HIDDEN should be explained by Barclay argparser pack",
        );
    assert_eq!(show_hidden.provenance, FactProvenance::ExternalLibraryPack);
    assert_eq!(
        show_hidden.field_type,
        Some(JavaType::Primitive(PrimitiveType::Boolean))
    );
    assert_eq!(
        external_field_provenance("SpecialArgumentsCollection", "SHOW_HIDDEN"),
        FactProvenance::ExternalLibraryPack
    );
    let hdf5_facts = new_java2rust::external_packs::hdf5_pack_facts();
    for (type_name, field_name) in [
        ("HDF5File", "OpenMode"),
        ("HDF5File.OpenMode", "READ_ONLY"),
        ("HDF5File.OpenMode", "CREATE"),
        ("OpenMode", "READ_ONLY"),
        ("OpenMode", "CREATE"),
    ] {
        let field = hdf5_facts
            .external_field_for(type_name, field_name)
            .unwrap_or_else(|| panic!("{type_name}.{field_name} should be explained by HDF5 pack"));
        assert_eq!(field.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(
            field.field_type,
            Some(JavaType::Named {
                name: "OpenMode".to_string(),
                args: Vec::new(),
            })
        );
        assert_eq!(
            external_field_provenance(type_name, field_name),
            FactProvenance::ExternalLibraryPack
        );
    }
    let variantcontext_facts = new_java2rust::external_packs::htsjdk_variantcontext_pack_facts();
    for (field_name, expected_type) in [
        (
            "NO_CALL",
            JavaType::Named {
                name: "Allele".to_string(),
                args: Vec::new(),
            },
        ),
        (
            "NON_REF_ALLELE",
            JavaType::Named {
                name: "Allele".to_string(),
                args: Vec::new(),
            },
        ),
        (
            "SPAN_DEL",
            JavaType::Named {
                name: "Allele".to_string(),
                args: Vec::new(),
            },
        ),
        (
            "SV_SIMPLE_DEL",
            JavaType::Named {
                name: "Allele".to_string(),
                args: Vec::new(),
            },
        ),
        (
            "SV_SIMPLE_DUP",
            JavaType::Named {
                name: "Allele".to_string(),
                args: Vec::new(),
            },
        ),
        (
            "UNSPECIFIED_ALTERNATE_ALLELE",
            JavaType::Named {
                name: "Allele".to_string(),
                args: Vec::new(),
            },
        ),
        ("NON_REF_STRING", JavaType::String),
        ("UNSPECIFIED_ALTERNATE_ALLELE_STRING", JavaType::String),
    ] {
        let field = variantcontext_facts
            .external_field_for("Allele", field_name)
            .unwrap_or_else(|| {
                panic!("Allele.{field_name} should be explained by variantcontext pack")
            });
        assert_eq!(field.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(field.field_type, Some(expected_type));
        assert_eq!(
            external_field_provenance("Allele", field_name),
            FactProvenance::ExternalLibraryPack
        );
    }
    let max_diploid_alt_alleles = variantcontext_facts
        .external_field_for(
            "GenotypeLikelihoods",
            "MAX_DIPLOID_ALT_ALLELES_THAT_CAN_BE_GENOTYPED",
        )
        .expect(
            "GenotypeLikelihoods max alt allele limit should be explained by variantcontext pack",
        );
    assert_eq!(
        max_diploid_alt_alleles.provenance,
        FactProvenance::ExternalLibraryPack
    );
    assert_eq!(
        max_diploid_alt_alleles.field_type,
        Some(JavaType::Primitive(PrimitiveType::Int))
    );
    assert_eq!(
        external_field_provenance(
            "GenotypeLikelihoods",
            "MAX_DIPLOID_ALT_ALLELES_THAT_CAN_BE_GENOTYPED"
        ),
        FactProvenance::ExternalLibraryPack
    );
    let no_genotypes = variantcontext_facts
        .external_field_for("GenotypesContext", "NO_GENOTYPES")
        .expect("GenotypesContext.NO_GENOTYPES should be explained by variantcontext pack");
    assert_eq!(no_genotypes.provenance, FactProvenance::ExternalLibraryPack);
    assert_eq!(
        no_genotypes.field_type,
        Some(JavaType::Named {
            name: "GenotypesContext".to_string(),
            args: Vec::new(),
        })
    );
    assert_eq!(
        external_field_provenance("GenotypesContext", "NO_GENOTYPES"),
        FactProvenance::ExternalLibraryPack
    );
    let variant_type = variantcontext_facts
        .external_field_for("VariantContext", "Type")
        .expect("VariantContext.Type should be explained by variantcontext pack");
    assert_eq!(variant_type.provenance, FactProvenance::ExternalLibraryPack);
    assert_eq!(
        variant_type.field_type,
        Some(JavaType::Named {
            name: "Type".to_string(),
            args: Vec::new(),
        })
    );
    assert_eq!(
        external_field_provenance("VariantContext", "Type"),
        FactProvenance::ExternalLibraryPack
    );
    for type_name in [
        "CommonInfo",
        "htsjdk.variant.variantcontext.CommonInfo",
        "VariantContext",
        "htsjdk.variant.variantcontext.VariantContext",
    ] {
        let field = variantcontext_facts
            .external_field_for(type_name, "NO_LOG10_PERROR")
            .unwrap_or_else(|| {
                panic!("{type_name}.NO_LOG10_PERROR should be explained by variantcontext pack")
            });
        assert_eq!(field.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(
            field.field_type,
            Some(JavaType::Primitive(PrimitiveType::Double))
        );
        assert_eq!(
            external_field_provenance(type_name, "NO_LOG10_PERROR"),
            FactProvenance::ExternalLibraryPack
        );
    }
    let unphased_separator = variantcontext_facts
        .external_field_for("Genotype", "UNPHASED_ALLELE_SEPARATOR")
        .expect("Genotype.UNPHASED_ALLELE_SEPARATOR should be explained by variantcontext pack");
    assert_eq!(
        unphased_separator.provenance,
        FactProvenance::ExternalLibraryPack
    );
    assert_eq!(
        unphased_separator.field_type,
        Some(JavaType::Primitive(PrimitiveType::Char))
    );
    assert_eq!(
        external_field_provenance("Genotype", "UNPHASED_ALLELE_SEPARATOR"),
        FactProvenance::ExternalLibraryPack
    );
    for field_name in ["INDEL", "NO_VARIATION"] {
        let field = variantcontext_facts
            .external_field_for("Type", field_name)
            .unwrap_or_else(|| {
                panic!("Type.{field_name} should be explained by variantcontext pack")
            });
        assert_eq!(field.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(
            field.field_type,
            Some(JavaType::Named {
                name: "Type".to_string(),
                args: Vec::new(),
            })
        );
        assert_eq!(
            external_field_provenance("Type", field_name),
            FactProvenance::ExternalLibraryPack
        );
    }
    for field_name in ["alleleIndex1", "alleleIndex2"] {
        let field = variantcontext_facts
            .external_field_for("GenotypeLikelihoodsAllelePair", field_name)
            .unwrap_or_else(|| {
                panic!(
                    "GenotypeLikelihoodsAllelePair.{field_name} should be explained by variantcontext pack"
                )
            });
        assert_eq!(field.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(
            field.field_type,
            Some(JavaType::Primitive(PrimitiveType::Int))
        );
        assert_eq!(
            external_field_provenance("GenotypeLikelihoodsAllelePair", field_name),
            FactProvenance::ExternalLibraryPack
        );
    }
    let sam_bam_facts = new_java2rust::external_packs::htsjdk_sam_bam_pack_facts();
    let default_granularity = sam_bam_facts
        .external_field_for("SBIIndexWriter", "DEFAULT_GRANULARITY")
        .expect("SBIIndexWriter.DEFAULT_GRANULARITY should be explained by SAM BAM pack");
    assert_eq!(
        default_granularity.provenance,
        FactProvenance::ExternalLibraryPack
    );
    assert_eq!(
        default_granularity.field_type,
        Some(JavaType::Primitive(PrimitiveType::Long))
    );
    assert_eq!(
        external_field_provenance("SBIIndexWriter", "DEFAULT_GRANULARITY"),
        FactProvenance::ExternalLibraryPack
    );
    let sam_record_facts = new_java2rust::external_packs::htsjdk_sam_record_pack_facts();
    let cigar_operator = sam_record_facts
        .external_field_for("CigarOperator", "MATCH_OR_MISMATCH")
        .expect("CigarOperator.MATCH_OR_MISMATCH should be explained by SAM record pack");
    assert_eq!(
        cigar_operator.provenance,
        FactProvenance::ExternalLibraryPack
    );
    assert_eq!(
        cigar_operator.field_type,
        Some(JavaType::Named {
            name: "CigarOperator".to_string(),
            args: Vec::new(),
        })
    );
    assert_eq!(
        external_field_provenance("CigarOperator", "MATCH_OR_MISMATCH"),
        FactProvenance::ExternalLibraryPack
    );
    for (field_name, expected_ty) in [
        (
            "NO_ALIGNMENT_CIGAR",
            JavaType::Named {
                name: "Cigar".to_string(),
                args: Vec::new(),
            },
        ),
        (
            "NO_ALIGNMENT_START",
            JavaType::Primitive(PrimitiveType::Int),
        ),
        (
            "NO_ALIGNMENT_REFERENCE_INDEX",
            JavaType::Primitive(PrimitiveType::Int),
        ),
        ("NO_ALIGNMENT_REFERENCE_NAME", JavaType::String),
        (
            "UNKNOWN_MAPPING_QUALITY",
            JavaType::Primitive(PrimitiveType::Int),
        ),
        (
            "NO_MAPPING_QUALITY",
            JavaType::Primitive(PrimitiveType::Int),
        ),
        (
            "NULL_QUALS",
            JavaType::Array(Box::new(JavaType::Primitive(PrimitiveType::Byte))),
        ),
        ("NULL_QUALS_STRING", JavaType::String),
        (
            "NULL_SEQUENCE",
            JavaType::Array(Box::new(JavaType::Primitive(PrimitiveType::Byte))),
        ),
    ] {
        let field = sam_record_facts
            .external_field_for("SAMRecord", field_name)
            .unwrap_or_else(|| {
                panic!("SAMRecord.{field_name} should be explained by SAM record pack")
            });
        assert_eq!(field.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(field.field_type, Some(expected_ty));
        assert_eq!(
            external_field_provenance("SAMRecord", field_name),
            FactProvenance::ExternalLibraryPack
        );
    }
    for (type_name, field_name, expected_ty) in [
        ("SAMTagAndValue", "tag", JavaType::String),
        (
            "SAMTagAndValue",
            "value",
            JavaType::Named {
                name: "Object".to_string(),
                args: Vec::new(),
            },
        ),
        ("SAMBinaryTagAndValue", "tag", JavaType::String),
        (
            "SAMBinaryTagAndValue",
            "value",
            JavaType::Named {
                name: "Object".to_string(),
                args: Vec::new(),
            },
        ),
    ] {
        let field = sam_record_facts
            .external_field_for(type_name, field_name)
            .unwrap_or_else(|| {
                panic!("{type_name}.{field_name} should be explained by SAM record pack")
            });
        assert_eq!(field.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(field.field_type, Some(expected_ty));
        assert_eq!(
            external_field_provenance(type_name, field_name),
            FactProvenance::ExternalLibraryPack
        );
    }
    for (field_name, expected_type) in [
        ("name", JavaType::String),
        (
            "exp",
            JavaType::Named {
                name: "Expression".to_string(),
                args: Vec::new(),
            },
        ),
    ] {
        let field = variantcontext_facts
            .external_field_for("JexlVCMatchExp", field_name)
            .unwrap_or_else(|| {
                panic!("JexlVCMatchExp.{field_name} should be explained by variantcontext pack")
            });
        assert_eq!(field.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(field.field_type, Some(expected_type));
        assert_eq!(
            external_field_provenance("JexlVCMatchExp", field_name),
            FactProvenance::ExternalLibraryPack
        );
    }
    for field_name in ["TREAT_AS_MATCH", "TREAT_AS_MISMATCH"] {
        let field = variantcontext_facts
            .external_field_for("JexlMissingValueTreatment", field_name)
            .unwrap_or_else(|| {
                panic!(
                    "JexlMissingValueTreatment.{field_name} should be explained by variantcontext pack"
                )
            });
        assert_eq!(field.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(
            field.field_type,
            Some(JavaType::Named {
                name: "JexlMissingValueTreatment".to_string(),
                args: Vec::new(),
            })
        );
        assert_eq!(
            external_field_provenance("JexlMissingValueTreatment", field_name),
            FactProvenance::ExternalLibraryPack
        );
    }
    let vcf_facts = new_java2rust::external_packs::htsjdk_vcf_pack_facts();
    for (field_name, expected_ty) in [
        ("PASSES_FILTERS_v4", JavaType::String),
        ("MISSING_VALUE_v4", JavaType::String),
        ("GENOTYPE_ALLELE_DEPTHS", JavaType::String),
        ("SVTYPE", JavaType::String),
        ("NULL_ALLELE", JavaType::String),
        ("UNFILTERED", JavaType::String),
        (
            "INFO_FIELD_ARRAY_SEPARATOR_CHAR",
            JavaType::Primitive(PrimitiveType::Char),
        ),
        ("MAX_GENOTYPE_QUAL", JavaType::Primitive(PrimitiveType::Int)),
    ] {
        let field = vcf_facts
            .external_field_for("VCFConstants", field_name)
            .unwrap_or_else(|| panic!("VCFConstants.{field_name} should be explained by VCF pack"));
        assert_eq!(field.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(field.field_type, Some(expected_ty));
        assert_eq!(
            external_field_provenance("VCFConstants", field_name),
            FactProvenance::ExternalLibraryPack
        );
    }
    for (field_name, expected_ty) in [
        (
            "HEADER_FIELDS",
            JavaType::Named {
                name: "HEADER_FIELDS".to_string(),
                args: Vec::new(),
            },
        ),
        ("REFERENCE_KEY", JavaType::String),
    ] {
        let field = vcf_facts
            .external_field_for("VCFHeader", field_name)
            .unwrap_or_else(|| panic!("VCFHeader.{field_name} should be explained by VCF pack"));
        assert_eq!(field.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(field.field_type, Some(expected_ty));
        assert_eq!(
            external_field_provenance("VCFHeader", field_name),
            FactProvenance::ExternalLibraryPack
        );
    }
    let supported_header_line_type = vcf_facts
        .external_field_for("VCFCompoundHeaderLine", "SupportedHeaderLineType")
        .expect("VCFCompoundHeaderLine.SupportedHeaderLineType should be explained by VCF pack");
    assert_eq!(
        supported_header_line_type.provenance,
        FactProvenance::ExternalLibraryPack
    );
    assert_eq!(
        supported_header_line_type.field_type,
        Some(JavaType::Named {
            name: "SupportedHeaderLineType".to_string(),
            args: Vec::new(),
        })
    );
    assert_eq!(
        external_field_provenance("VCFCompoundHeaderLine", "SupportedHeaderLineType"),
        FactProvenance::ExternalLibraryPack
    );
    for field_name in ["FORMAT", "INFO"] {
        let field = vcf_facts
            .external_field_for("SupportedHeaderLineType", field_name)
            .unwrap_or_else(|| {
                panic!("SupportedHeaderLineType.{field_name} should be explained by VCF pack")
            });
        assert_eq!(field.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(
            field.field_type,
            Some(JavaType::Named {
                name: "SupportedHeaderLineType".to_string(),
                args: Vec::new(),
            })
        );
        assert_eq!(
            external_field_provenance("SupportedHeaderLineType", field_name),
            FactProvenance::ExternalLibraryPack
        );
    }
    let writer_output_type = vcf_facts
        .external_field_for("VariantContextWriterBuilder", "OutputType")
        .expect("VariantContextWriterBuilder.OutputType should be explained by VCF pack");
    assert_eq!(
        writer_output_type.provenance,
        FactProvenance::ExternalLibraryPack
    );
    assert_eq!(
        writer_output_type.field_type,
        Some(JavaType::Named {
            name: "OutputType".to_string(),
            args: Vec::new(),
        })
    );
    assert_eq!(
        external_field_provenance("VariantContextWriterBuilder", "OutputType"),
        FactProvenance::ExternalLibraryPack
    );
    let default_options = vcf_facts
        .external_field_for("VariantContextWriterBuilder", "DEFAULT_OPTIONS")
        .expect("VariantContextWriterBuilder.DEFAULT_OPTIONS should be explained by VCF pack");
    assert_eq!(
        default_options.provenance,
        FactProvenance::ExternalLibraryPack
    );
    assert_eq!(
        default_options.field_type,
        Some(JavaType::Named {
            name: "EnumSet".to_string(),
            args: vec![JavaType::Named {
                name: "Options".to_string(),
                args: Vec::new(),
            }],
        })
    );
    assert_eq!(
        external_field_provenance("VariantContextWriterBuilder", "DEFAULT_OPTIONS"),
        FactProvenance::ExternalLibraryPack
    );
    let picard_vcf_facts = new_java2rust::external_packs::picard_vcf_pack_facts();
    for (field_name, expected_ty) in [
        (
            "CallState",
            JavaType::Named {
                name: "CallState".to_string(),
                args: Vec::new(),
            },
        ),
        (
            "TruthState",
            JavaType::Named {
                name: "TruthState".to_string(),
                args: Vec::new(),
            },
        ),
    ] {
        let field = picard_vcf_facts
            .external_field_for("GenotypeConcordanceStates", field_name)
            .unwrap_or_else(|| {
                panic!(
                    "GenotypeConcordanceStates.{field_name} should be explained by Picard VCF pack"
                )
            });
        assert_eq!(field.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(field.field_type, Some(expected_ty));
        assert_eq!(
            external_field_provenance("GenotypeConcordanceStates", field_name),
            FactProvenance::ExternalLibraryPack
        );
    }
    for (type_name, field_name, expected_ty) in [
        (
            "CallState",
            "HOM_REF",
            JavaType::Named {
                name: "CallState".to_string(),
                args: Vec::new(),
            },
        ),
        (
            "CallState",
            "HET_REF_VAR1",
            JavaType::Named {
                name: "CallState".to_string(),
                args: Vec::new(),
            },
        ),
        (
            "TruthState",
            "HOM_REF",
            JavaType::Named {
                name: "TruthState".to_string(),
                args: Vec::new(),
            },
        ),
        (
            "TruthState",
            "NO_CALL",
            JavaType::Named {
                name: "TruthState".to_string(),
                args: Vec::new(),
            },
        ),
    ] {
        let field = picard_vcf_facts
            .external_field_for(type_name, field_name)
            .unwrap_or_else(|| {
                panic!("{type_name}.{field_name} should be explained by Picard VCF pack")
            });
        assert_eq!(field.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(field.field_type, Some(expected_ty));
        assert_eq!(
            external_field_provenance(type_name, field_name),
            FactProvenance::ExternalLibraryPack
        );
    }
    for (type_name, field_name) in [
        ("GenotypeConcordance", "CONTINGENCY_STATE_TAG"),
        ("GenotypeConcordanceSummaryMetrics", "GENOTYPE_CONCORDANCE"),
        (
            "GenotypeConcordanceSummaryMetrics",
            "NON_REF_GENOTYPE_CONCORDANCE",
        ),
        ("GenotypeConcordanceSummaryMetrics", "HET_PPV"),
        ("GenotypeConcordanceSummaryMetrics", "HET_SENSITIVITY"),
        ("GenotypeConcordanceSummaryMetrics", "HOMVAR_PPV"),
        ("GenotypeConcordanceSummaryMetrics", "HOMVAR_SENSITIVITY"),
        ("GenotypeConcordanceSummaryMetrics", "VAR_PPV"),
        ("GenotypeConcordanceSummaryMetrics", "VAR_SENSITIVITY"),
        ("GenotypeConcordanceSummaryMetrics", "VAR_SPECIFICITY"),
    ] {
        let field = picard_vcf_facts
            .external_field_for(type_name, field_name)
            .unwrap_or_else(|| {
                panic!("{type_name}.{field_name} should be explained by Picard VCF pack")
            });
        assert_eq!(field.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(field.field_type, Some(JavaType::String));
        assert_eq!(
            external_field_provenance(type_name, field_name),
            FactProvenance::ExternalLibraryPack
        );
    }
    let picard_markduplicates_facts =
        new_java2rust::external_packs::picard_markduplicates_pack_facts();
    let duplicate_tagging_policy = picard_markduplicates_facts
        .external_field_for("MarkDuplicates", "DuplicateTaggingPolicy")
        .expect("MarkDuplicates.DuplicateTaggingPolicy should be explained by Picard pack");
    assert_eq!(
        duplicate_tagging_policy.provenance,
        FactProvenance::ExternalLibraryPack
    );
    assert_eq!(
        duplicate_tagging_policy.field_type,
        Some(JavaType::Named {
            name: "DuplicateTaggingPolicy".to_string(),
            args: Vec::new(),
        })
    );
    assert_eq!(
        external_field_provenance("MarkDuplicates", "DuplicateTaggingPolicy"),
        FactProvenance::ExternalLibraryPack
    );
    for field_name in ["DontTag", "OpticalOnly", "All"] {
        let field = picard_markduplicates_facts
            .external_field_for("DuplicateTaggingPolicy", field_name)
            .unwrap_or_else(|| {
                panic!("DuplicateTaggingPolicy.{field_name} should be explained by Picard pack")
            });
        assert_eq!(field.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(
            field.field_type,
            Some(JavaType::Named {
                name: "DuplicateTaggingPolicy".to_string(),
                args: Vec::new(),
            })
        );
        assert_eq!(
            external_field_provenance("DuplicateTaggingPolicy", field_name),
            FactProvenance::ExternalLibraryPack
        );
    }
    let sam_header_facts = new_java2rust::external_packs::htsjdk_sam_header_pack_facts();
    for (field_name, expected_ty) in [
        (
            "SortOrder",
            JavaType::Named {
                name: "SortOrder".to_string(),
                args: Vec::new(),
            },
        ),
        (
            "GroupOrder",
            JavaType::Named {
                name: "GroupOrder".to_string(),
                args: Vec::new(),
            },
        ),
    ] {
        let field = sam_header_facts
            .external_field_for("SAMFileHeader", field_name)
            .unwrap_or_else(|| {
                panic!("SAMFileHeader.{field_name} should be explained by SAM header pack")
            });
        assert_eq!(field.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(field.field_type, Some(expected_ty));
        assert_eq!(
            external_field_provenance("SAMFileHeader", field_name),
            FactProvenance::ExternalLibraryPack
        );
    }
    for field_name in ["READ_GROUP_ID_TAG", "READ_GROUP_SAMPLE_TAG"] {
        let field = sam_header_facts
            .external_field_for("SAMReadGroupRecord", field_name)
            .unwrap_or_else(|| {
                panic!("SAMReadGroupRecord.{field_name} should be explained by SAM header pack")
            });
        assert_eq!(field.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(field.field_type, Some(JavaType::String));
        assert_eq!(
            external_field_provenance("SAMReadGroupRecord", field_name),
            FactProvenance::ExternalLibraryPack
        );
    }
    for (type_name, field_name, expected_type_name) in [
        ("SortOrder", "coordinate", "SortOrder"),
        ("SortOrder", "unsorted", "SortOrder"),
        ("GroupOrder", "query", "GroupOrder"),
    ] {
        let field = sam_header_facts
            .external_field_for(type_name, field_name)
            .unwrap_or_else(|| {
                panic!("{type_name}.{field_name} should be explained by SAM header pack")
            });
        assert_eq!(field.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(
            field.field_type,
            Some(JavaType::Named {
                name: expected_type_name.to_string(),
                args: Vec::new(),
            })
        );
        assert_eq!(
            external_field_provenance(type_name, field_name),
            FactProvenance::ExternalLibraryPack
        );
    }
    let samtools_util_facts = new_java2rust::external_packs::htsjdk_samtools_util_pack_facts();
    for (type_name, field_name, expected_ty) in [
        (
            "BlockCompressedInputStream",
            "FileTermination",
            JavaType::Named {
                name: "FileTermination".to_string(),
                args: Vec::new(),
            },
        ),
        (
            "SamPairUtil",
            "PairOrientation",
            JavaType::Named {
                name: "PairOrientation".to_string(),
                args: Vec::new(),
            },
        ),
        (
            "Log",
            "LogLevel",
            JavaType::Named {
                name: "LogLevel".to_string(),
                args: Vec::new(),
            },
        ),
    ] {
        let field = samtools_util_facts
            .external_field_for(type_name, field_name)
            .unwrap_or_else(|| {
                panic!("{type_name}.{field_name} should be explained by samtools-util pack")
            });
        assert_eq!(field.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(field.field_type, Some(expected_ty));
        assert_eq!(
            external_field_provenance(type_name, field_name),
            FactProvenance::ExternalLibraryPack
        );
    }
    for (type_name, field_name, expected_ty) in [
        (
            "Defaults",
            "COMPRESSION_LEVEL",
            JavaType::Primitive(PrimitiveType::Int),
        ),
        (
            "Defaults",
            "USE_ASYNC_IO_READ_FOR_SAMTOOLS",
            JavaType::Primitive(PrimitiveType::Boolean),
        ),
        (
            "Defaults",
            "USE_ASYNC_IO_WRITE_FOR_SAMTOOLS",
            JavaType::Primitive(PrimitiveType::Boolean),
        ),
        (
            "Defaults",
            "USE_ASYNC_IO_WRITE_FOR_TRIBBLE",
            JavaType::Primitive(PrimitiveType::Boolean),
        ),
        (
            "SAMUtils",
            "MAX_PHRED_SCORE",
            JavaType::Primitive(PrimitiveType::Int),
        ),
        ("ReservedTagConstants", "XT", JavaType::String),
        ("ReservedTagConstants", "PROGRAM_GROUP_ID", JavaType::String),
        ("ReservedTagConstants", "READ_GROUP_ID", JavaType::String),
        (
            "BlockCompressedStreamConstants",
            "BLOCK_HEADER_LENGTH",
            JavaType::Primitive(PrimitiveType::Int),
        ),
        (
            "BlockCompressedStreamConstants",
            "BLOCK_LENGTH_OFFSET",
            JavaType::Primitive(PrimitiveType::Int),
        ),
        (
            "BlockCompressedStreamConstants",
            "DEFAULT_COMPRESSION_LEVEL",
            JavaType::Primitive(PrimitiveType::Int),
        ),
        (
            "BlockCompressedStreamConstants",
            "EMPTY_GZIP_BLOCK",
            JavaType::Array(Box::new(JavaType::Primitive(PrimitiveType::Byte))),
        ),
        ("FastqConstants", "FIRST_OF_PAIR", JavaType::String),
        ("FastqConstants", "SECOND_OF_PAIR", JavaType::String),
        (
            "htsjdk.samtools.fastq.FastqConstants",
            "FIRST_OF_PAIR",
            JavaType::String,
        ),
        (
            "htsjdk.samtools.fastq.FastqConstants",
            "SECOND_OF_PAIR",
            JavaType::String,
        ),
        (
            "SequenceUtil",
            "VALID_BASES_UPPER",
            JavaType::Array(Box::new(JavaType::Primitive(PrimitiveType::Byte))),
        ),
    ] {
        let field = samtools_util_facts
            .external_field_for(type_name, field_name)
            .unwrap_or_else(|| {
                panic!("{type_name}.{field_name} should be explained by samtools-util pack")
            });
        assert_eq!(field.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(field.field_type, Some(expected_ty));
        assert_eq!(
            external_field_provenance(type_name, field_name),
            FactProvenance::ExternalLibraryPack
        );
    }
    for (type_name, field_name, expected_ty) in [
        (
            "PairOrientation",
            "FR",
            JavaType::Named {
                name: "PairOrientation".to_string(),
                args: Vec::new(),
            },
        ),
        (
            "PairOrientation",
            "RF",
            JavaType::Named {
                name: "PairOrientation".to_string(),
                args: Vec::new(),
            },
        ),
        (
            "PairOrientation",
            "TANDEM",
            JavaType::Named {
                name: "PairOrientation".to_string(),
                args: Vec::new(),
            },
        ),
        (
            "FileTermination",
            "DEFECTIVE",
            JavaType::Named {
                name: "FileTermination".to_string(),
                args: Vec::new(),
            },
        ),
        (
            "FileTermination",
            "HAS_TERMINATOR_BLOCK",
            JavaType::Named {
                name: "FileTermination".to_string(),
                args: Vec::new(),
            },
        ),
        (
            "LogLevel",
            "ERROR",
            JavaType::Named {
                name: "LogLevel".to_string(),
                args: Vec::new(),
            },
        ),
        (
            "LogLevel",
            "WARNING",
            JavaType::Named {
                name: "LogLevel".to_string(),
                args: Vec::new(),
            },
        ),
        (
            "LogLevel",
            "INFO",
            JavaType::Named {
                name: "LogLevel".to_string(),
                args: Vec::new(),
            },
        ),
        (
            "LogLevel",
            "DEBUG",
            JavaType::Named {
                name: "LogLevel".to_string(),
                args: Vec::new(),
            },
        ),
    ] {
        let field = samtools_util_facts
            .external_field_for(type_name, field_name)
            .unwrap_or_else(|| {
                panic!("{type_name}.{field_name} should be explained by samtools-util pack")
            });
        assert_eq!(field.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(field.field_type, Some(expected_ty));
        assert_eq!(
            external_field_provenance(type_name, field_name),
            FactProvenance::ExternalLibraryPack
        );
    }
    let tribble_feature_facts = new_java2rust::external_packs::htsjdk_tribble_feature_pack_facts();
    let no_header_end = tribble_feature_facts
        .external_field_for("FeatureCodecHeader", "NO_HEADER_END")
        .expect("FeatureCodecHeader.NO_HEADER_END should be explained by tribble feature pack");
    assert_eq!(
        no_header_end.provenance,
        FactProvenance::ExternalLibraryPack
    );
    assert_eq!(
        no_header_end.field_type,
        Some(JavaType::Primitive(PrimitiveType::Long))
    );
    assert_eq!(
        external_field_provenance("FeatureCodecHeader", "NO_HEADER_END"),
        FactProvenance::ExternalLibraryPack
    );
    let tribble_index_facts = new_java2rust::external_packs::htsjdk_tribble_index_pack_facts();
    for (field_name, expected_ty) in [
        (
            "IndexType",
            JavaType::Named {
                name: "IndexType".to_string(),
                args: Vec::new(),
            },
        ),
        (
            "IndexBalanceApproach",
            JavaType::Named {
                name: "IndexBalanceApproach".to_string(),
                args: Vec::new(),
            },
        ),
    ] {
        let field = tribble_index_facts
            .external_field_for("IndexFactory", field_name)
            .unwrap_or_else(|| {
                panic!("IndexFactory.{field_name} should be explained by tribble index pack")
            });
        assert_eq!(field.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(field.field_type, Some(expected_ty));
        assert_eq!(
            external_field_provenance("IndexFactory", field_name),
            FactProvenance::ExternalLibraryPack
        );
    }
    for (type_name, field_name, expected_ty) in [
        (
            "IndexType",
            "TABIX",
            JavaType::Named {
                name: "IndexType".to_string(),
                args: Vec::new(),
            },
        ),
        (
            "IndexBalanceApproach",
            "FOR_SEEK_TIME",
            JavaType::Named {
                name: "IndexBalanceApproach".to_string(),
                args: Vec::new(),
            },
        ),
    ] {
        let field = tribble_index_facts
            .external_field_for(type_name, field_name)
            .unwrap_or_else(|| {
                panic!("{type_name}.{field_name} should be explained by tribble index pack")
            });
        assert_eq!(field.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(field.field_type, Some(expected_ty));
        assert_eq!(
            external_field_provenance(type_name, field_name),
            FactProvenance::ExternalLibraryPack
        );
    }
    for (type_name, field_name) in [
        ("TabixFormat", "GENERIC_FLAGS"),
        ("TabixFormat", "ZERO_BASED"),
        ("htsjdk.tribble.index.tabix.TabixFormat", "GENERIC_FLAGS"),
        ("htsjdk.tribble.index.tabix.TabixFormat", "ZERO_BASED"),
    ] {
        let field = tribble_index_facts
            .external_field_for(type_name, field_name)
            .unwrap_or_else(|| {
                panic!("{type_name}.{field_name} should be explained by tribble index pack")
            });
        assert_eq!(field.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(
            field.field_type,
            Some(JavaType::Primitive(PrimitiveType::Int))
        );
        assert_eq!(
            external_field_provenance(type_name, field_name),
            FactProvenance::ExternalLibraryPack
        );
    }
    let sam_bam_facts = new_java2rust::external_packs::htsjdk_sam_bam_pack_facts();
    let fixed_block_size = sam_bam_facts
        .external_field_for("BAMFileConstants", "FIXED_BLOCK_SIZE")
        .expect("BAMFileConstants.FIXED_BLOCK_SIZE should be explained by SAM BAM pack");
    assert_eq!(
        fixed_block_size.provenance,
        FactProvenance::ExternalLibraryPack
    );
    assert_eq!(
        fixed_block_size.field_type,
        Some(JavaType::Primitive(PrimitiveType::Int))
    );
    assert_eq!(
        external_field_provenance("BAMFileConstants", "FIXED_BLOCK_SIZE"),
        FactProvenance::ExternalLibraryPack
    );
    let sam_reader_facts = new_java2rust::external_packs::htsjdk_sam_reader_pack_facts();
    let reader_option = sam_reader_facts
        .external_field_for("SamReaderFactory", "Option")
        .expect("SamReaderFactory.Option should be explained by SAM reader pack");
    assert_eq!(
        reader_option.provenance,
        FactProvenance::ExternalLibraryPack
    );
    assert_eq!(
        reader_option.field_type,
        Some(JavaType::Named {
            name: "Option".to_string(),
            args: Vec::new(),
        })
    );
    assert_eq!(
        external_field_provenance("SamReaderFactory", "Option"),
        FactProvenance::ExternalLibraryPack
    );
    for field_name in ["CACHE_FILE_BASED_INDEXES", "INCLUDE_SOURCE_IN_RECORDS"] {
        let field = sam_reader_facts
            .external_field_for("SamReaderFactory.Option", field_name)
            .unwrap_or_else(|| {
                panic!(
                    "SamReaderFactory.Option.{field_name} should be explained by SAM reader pack"
                )
            });
        assert_eq!(field.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(
            field.field_type,
            Some(JavaType::Named {
                name: "Option".to_string(),
                args: Vec::new(),
            })
        );
        assert_eq!(
            external_field_provenance("SamReaderFactory.Option", field_name),
            FactProvenance::ExternalLibraryPack
        );
        assert_eq!(
            external_field_provenance("Option", field_name),
            FactProvenance::BuiltinExternalModel
        );
    }
    let apache_stats_facts = new_java2rust::external_packs::apache_commons_math_stats_pack_facts();
    let estimation_type = apache_stats_facts
        .external_field_for("Percentile", "EstimationType")
        .expect("Percentile.EstimationType should be explained by Apache Commons stats pack");
    assert_eq!(
        estimation_type.provenance,
        FactProvenance::ExternalLibraryPack
    );
    assert_eq!(
        estimation_type.field_type,
        Some(JavaType::Named {
            name: "EstimationType".to_string(),
            args: Vec::new(),
        })
    );
    assert_eq!(
        external_field_provenance("Percentile", "EstimationType"),
        FactProvenance::ExternalLibraryPack
    );
    let r1 = apache_stats_facts
        .external_field_for("EstimationType", "R_1")
        .expect("EstimationType.R_1 should be explained by Apache Commons stats pack");
    assert_eq!(r1.provenance, FactProvenance::ExternalLibraryPack);
    assert_eq!(
        r1.field_type,
        Some(JavaType::Named {
            name: "EstimationType".to_string(),
            args: Vec::new(),
        })
    );
    assert_eq!(
        external_field_provenance("EstimationType", "R_1"),
        FactProvenance::ExternalLibraryPack
    );
    let commons_compress_facts =
        new_java2rust::external_packs::apache_commons_compress_pack_facts();
    for (type_name, field_name, expected_ty) in [
        (
            "TarArchiveOutputStream",
            "BIGNUMBER_STAR",
            JavaType::Primitive(PrimitiveType::Int),
        ),
        (
            "TarArchiveOutputStream",
            "LONGFILE_GNU",
            JavaType::Primitive(PrimitiveType::Int),
        ),
        (
            "NullOutputStream",
            "INSTANCE",
            JavaType::Named {
                name: "OutputStream".to_string(),
                args: Vec::new(),
            },
        ),
        (
            "NullOutputStream",
            "NULL_OUTPUT_STREAM",
            JavaType::Named {
                name: "OutputStream".to_string(),
                args: Vec::new(),
            },
        ),
    ] {
        let field = commons_compress_facts
            .external_field_for(type_name, field_name)
            .unwrap_or_else(|| {
                panic!("{type_name}.{field_name} should be explained by Commons Compress pack")
            });
        assert_eq!(field.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(field.field_type, Some(expected_ty));
        assert_eq!(
            external_field_provenance(type_name, field_name),
            FactProvenance::ExternalLibraryPack
        );
    }
    let commons_lang_facts = new_java2rust::external_packs::apache_commons_lang_pack_facts();
    for (type_name, field_name, expected_ty) in [
        ("SystemUtils", "OS_ARCH", JavaType::String),
        ("SystemUtils", "OS_NAME", JavaType::String),
        (
            "SystemUtils",
            "IS_OS_MAC",
            JavaType::Primitive(PrimitiveType::Boolean),
        ),
        (
            "SystemUtils",
            "IS_OS_LINUX",
            JavaType::Primitive(PrimitiveType::Boolean),
        ),
        (
            "SystemUtils",
            "IS_OS_WINDOWS",
            JavaType::Primitive(PrimitiveType::Boolean),
        ),
    ] {
        let field = commons_lang_facts
            .external_field_for(type_name, field_name)
            .unwrap_or_else(|| {
                panic!("{type_name}.{field_name} should be explained by Commons Lang pack")
            });
        assert_eq!(field.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(field.field_type, Some(expected_ty));
        assert_eq!(
            external_field_provenance(type_name, field_name),
            FactProvenance::ExternalLibraryPack
        );
    }
    let commons_codec_facts = new_java2rust::external_packs::apache_commons_codec_pack_facts();
    let sha_256 = commons_codec_facts
        .external_field_for("MessageDigestAlgorithms", "SHA_256")
        .expect("MessageDigestAlgorithms.SHA_256 should be explained by Commons Codec pack");
    assert_eq!(sha_256.provenance, FactProvenance::ExternalLibraryPack);
    assert_eq!(sha_256.field_type, Some(JavaType::String));
    assert_eq!(
        external_field_provenance("MessageDigestAlgorithms", "SHA_256"),
        FactProvenance::ExternalLibraryPack
    );
    let bigquery_facts = new_java2rust::external_packs::google_bigquery_pack_facts();
    for (type_name, field_name, expected_ty) in [
        (
            "FieldValue",
            "Attribute",
            JavaType::Named {
                name: "Attribute".to_string(),
                args: Vec::new(),
            },
        ),
        (
            "QueryJobConfiguration",
            "Priority",
            JavaType::Named {
                name: "Priority".to_string(),
                args: Vec::new(),
            },
        ),
    ] {
        let field = bigquery_facts
            .external_field_for(type_name, field_name)
            .unwrap_or_else(|| {
                panic!("{type_name}.{field_name} should be explained by BigQuery pack")
            });
        assert_eq!(field.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(field.field_type, Some(expected_ty));
        assert_eq!(
            external_field_provenance(type_name, field_name),
            FactProvenance::ExternalLibraryPack
        );
    }
    for (type_name, field_name, expected_ty) in [
        (
            "FieldValue.Attribute",
            "PRIMITIVE",
            JavaType::Named {
                name: "Attribute".to_string(),
                args: Vec::new(),
            },
        ),
        (
            "QueryJobConfiguration.Priority",
            "BATCH",
            JavaType::Named {
                name: "Priority".to_string(),
                args: Vec::new(),
            },
        ),
        (
            "QueryJobConfiguration.Priority",
            "INTERACTIVE",
            JavaType::Named {
                name: "Priority".to_string(),
                args: Vec::new(),
            },
        ),
    ] {
        let field = bigquery_facts
            .external_field_for(type_name, field_name)
            .unwrap_or_else(|| {
                panic!("{type_name}.{field_name} should be explained by BigQuery pack")
            });
        assert_eq!(field.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(field.field_type, Some(expected_ty));
        assert_eq!(
            external_field_provenance(type_name, field_name),
            FactProvenance::ExternalLibraryPack
        );
    }
    assert_eq!(
        external_field_provenance("Attribute", "PRIMITIVE"),
        FactProvenance::BuiltinExternalModel
    );
    assert_eq!(
        external_field_provenance("Priority", "BATCH"),
        FactProvenance::BuiltinExternalModel
    );
    let bigquery_storage_facts =
        new_java2rust::external_packs::google_bigquery_storage_pack_facts();
    for field_name in ["AVRO", "ARROW"] {
        let field = bigquery_storage_facts
            .external_field_for("DataFormat", field_name)
            .unwrap_or_else(|| {
                panic!("DataFormat.{field_name} should be explained by BigQuery Storage pack")
            });
        assert_eq!(field.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(
            field.field_type,
            Some(JavaType::Named {
                name: "DataFormat".to_string(),
                args: Vec::new(),
            })
        );
        assert_eq!(
            external_field_provenance("DataFormat", field_name),
            FactProvenance::ExternalLibraryPack
        );
    }
    let jackson_facts = new_java2rust::external_packs::jackson_databind_pack_facts();
    for (type_name, field_name) in [
        ("MapperFeature", "ACCEPT_CASE_INSENSITIVE_PROPERTIES"),
        ("DeserializationFeature", "UNWRAP_ROOT_VALUE"),
    ] {
        let field = jackson_facts
            .external_field_for(type_name, field_name)
            .unwrap_or_else(|| {
                panic!("{type_name}.{field_name} should be explained by Jackson databind pack")
            });
        assert_eq!(field.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(
            field.field_type,
            Some(JavaType::Named {
                name: type_name.to_string(),
                args: Vec::new(),
            })
        );
        assert_eq!(
            external_field_provenance(type_name, field_name),
            FactProvenance::ExternalLibraryPack
        );
    }
    let storage_facts = new_java2rust::external_packs::google_storage_pack_facts();
    let sign_url_option = storage_facts
        .external_field_for("Storage", "SignUrlOption")
        .expect("Storage.SignUrlOption should be explained by Google Storage pack");
    assert_eq!(
        sign_url_option.provenance,
        FactProvenance::ExternalLibraryPack
    );
    assert_eq!(
        sign_url_option.field_type,
        Some(JavaType::Named {
            name: "SignUrlOption".to_string(),
            args: Vec::new(),
        })
    );
    assert_eq!(
        external_field_provenance("Storage", "SignUrlOption"),
        FactProvenance::ExternalLibraryPack
    );
    let cloud_nio_facts = new_java2rust::external_packs::google_cloud_nio_pack_facts();
    for (type_name, field_name, expected_ty) in [
        ("CloudStorageFileSystem", "URI_SCHEME", JavaType::String),
        (
            "CloudStorageConfiguration",
            "DEFAULT",
            JavaType::Named {
                name: "CloudStorageConfiguration".to_string(),
                args: Vec::new(),
            },
        ),
    ] {
        let field = cloud_nio_facts
            .external_field_for(type_name, field_name)
            .unwrap_or_else(|| {
                panic!("{type_name}.{field_name} should be explained by Google Cloud NIO pack")
            });
        assert_eq!(field.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(field.field_type, Some(expected_ty));
        assert_eq!(
            external_field_provenance(type_name, field_name),
            FactProvenance::ExternalLibraryPack
        );
    }
    let genomicsdb_facts = new_java2rust::external_packs::genomicsdb_pack_facts();
    for (type_name, field_name, expected_ty) in [
        (
            "Coordinates",
            "GenomicsDBColumn",
            JavaType::Named {
                name: "GenomicsDBColumn".to_string(),
                args: Vec::new(),
            },
        ),
        (
            "Coordinates",
            "ContigPosition",
            JavaType::Named {
                name: "ContigPosition".to_string(),
                args: Vec::new(),
            },
        ),
        (
            "GenomicsDBExportConfiguration",
            "ExportConfiguration",
            JavaType::Named {
                name: "ExportConfiguration".to_string(),
                args: Vec::new(),
            },
        ),
        (
            "GenomicsDBImportConfiguration",
            "ImportConfiguration",
            JavaType::Named {
                name: "ImportConfiguration".to_string(),
                args: Vec::new(),
            },
        ),
        (
            "GenomicsDBImportConfiguration",
            "Partition",
            JavaType::Named {
                name: "Partition".to_string(),
                args: Vec::new(),
            },
        ),
        (
            "GenomicsDBVidMapProto",
            "VidMappingPB",
            JavaType::Named {
                name: "VidMappingPB".to_string(),
                args: Vec::new(),
            },
        ),
        (
            "GenomicsDBVidMapProto",
            "FieldLengthDescriptorComponentPB",
            JavaType::Named {
                name: "FieldLengthDescriptorComponentPB".to_string(),
                args: Vec::new(),
            },
        ),
        (
            "ReadSession",
            "TableReadOptions",
            JavaType::Named {
                name: "TableReadOptions".to_string(),
                args: Vec::new(),
            },
        ),
    ] {
        let field = genomicsdb_facts
            .external_field_for(type_name, field_name)
            .unwrap_or_else(|| {
                panic!("{type_name}.{field_name} should be explained by GenomicsDB pack")
            });
        assert_eq!(field.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(field.field_type, Some(expected_ty));
        assert_eq!(
            external_field_provenance(type_name, field_name),
            FactProvenance::ExternalLibraryPack
        );
    }
    let bwa_facts = new_java2rust::external_packs::bwa_pack_facts();
    let bwa_flags = bwa_facts
        .external_field_for("BwaMemAligner", "MEM_F_ALL")
        .expect("BwaMemAligner.MEM_F_ALL should be explained by BWA pack");
    assert_eq!(bwa_flags.provenance, FactProvenance::ExternalLibraryPack);
    assert_eq!(
        bwa_flags.field_type,
        Some(JavaType::Primitive(PrimitiveType::Int))
    );
    assert_eq!(
        external_field_provenance("BwaMemAligner", "MEM_F_ALL"),
        FactProvenance::ExternalLibraryPack
    );
    assert_eq!(
        external_field_provenance("ExternalBox", "count"),
        FactProvenance::ConservativeFallback
    );
    assert_eq!(
        external_field_provenance("ExternalEnum", "unsorted"),
        FactProvenance::ConservativeFallback
    );
    assert_eq!(
        external_field_provenance("SAMSequenceRecord", "UNKNOWN_SEQUENCE_LENGTH"),
        FactProvenance::ExternalLibraryPack
    );
    assert_eq!(
        external_field_provenance("HttpFileSystemProviderSettings", "DEFAULT_SETTINGS"),
        FactProvenance::ExternalLibraryPack
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
fn external_api_effect_lookup_uses_shared_java_method_aliases() {
    let mut facts = FactSet::default();
    facts.external_api_effect_facts.push(ExternalApiEffectFact {
        method_name: "valueOf".to_string(),
        receiver_type: Some("ExternalEnum".to_string()),
        arg_count: Some(1),
        role: ExternalApiRole::Getter,
        receiver_mutability: ReceiverMutabilityFact::ReadOnly,
        return_type: Some(JavaType::String),
        return_nullability: NullabilityState::DefinitelyNonNull,
        thrown: Vec::new(),
        provenance: FactProvenance::UserOverride,
    });

    let effect = facts
        .external_api_effect_for_receiver_with_arity(Some("ExternalEnum"), "value_of", Some(1))
        .expect("shared Java method aliases should connect snake and camel spellings");
    assert_eq!(effect.method_name, "valueOf");
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
        create.return_type,
        Some(JavaType::Named {
            name: "Allele".to_string(),
            args: Vec::new(),
        })
    );
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
    assert_eq!(base_string.return_type, Some(JavaType::String));
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
            ..semantic_analysis::SemanticAnalysisConfig::default()
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
fn external_setter_effect_keeps_local_storage_owned_and_mutable() {
    let source = r#"
class ExternalLocalReceiverMutability {
    void run() {
        ExternalCounter counter = new ExternalCounter();
        counter.setValue(1);
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
            new_java2rust::java_ir::Member::Method(method) if method.name == "run" => Some(method),
            _ => None,
        })
        .unwrap();
    let method_signature = signatures.method_signature(ty, method).unwrap();
    let local = signatures
        .local_shape(&method_signature.locals, "counter")
        .expect("counter local should have a solved shape");

    assert_eq!(local.ownership, OwnershipShape::Owned);
    assert_eq!(
        signatures.local_binding_mutability(&method_signature.locals, "counter"),
        Some(true)
    );
}

#[test]
fn readonly_external_receiver_usage_can_borrow_public_helper_param() {
    let source = r#"
class ReadOnlyExternalReceiverParam {
    public static String read(ExternalHandle handle) {
        return handle.getName();
    }
}
"#;
    let parsed = parser::parse_java_source(source);
    let resolved = resolve::resolve_program(&parsed.program, parsed.diagnostics);
    let mut external_facts = new_java2rust::facts::FactSet::default();
    external_facts
        .external_api_effect_facts
        .push(ExternalApiEffectFact {
            method_name: "getName".to_string(),
            receiver_type: Some("ExternalHandle".to_string()),
            arg_count: Some(0),
            role: ExternalApiRole::Getter,
            receiver_mutability: ReceiverMutabilityFact::ReadOnly,
            return_type: Some(JavaType::String),
            return_nullability: NullabilityState::DefinitelyNonNull,
            thrown: Vec::new(),
            provenance: FactProvenance::UserOverride,
        });
    let semantic =
        semantic_analysis::analyze_program(&resolved.program, &resolved.facts, &external_facts);
    let method_usage = semantic
        .usage
        .methods
        .get(&UsageMethodKey {
            type_name: "ReadOnlyExternalReceiverParam".to_string(),
            method_name: "read".to_string(),
        })
        .expect("read usage should be recorded");
    let handle_uses = &method_usage
        .bindings
        .get("handle")
        .expect("handle binding should be recorded")
        .uses;
    assert_eq!(
        handle_uses,
        &vec![
            BindingUse::ReceiverForCall {
                name: "getName".to_string(),
                boundary: CallArgumentBoundary::Unknown,
            },
            BindingUse::ContentRead,
        ]
    );
    assert!(
        semantic
            .facts
            .external_api_effect_for_receiver(Some("ExternalHandle"), "getName")
            .is_some_and(|effect| {
                effect.receiver_mutability == ReceiverMutabilityFact::ReadOnly
            })
    );
    let representations = representation::plan_representations(&resolved.program, &semantic.facts);
    let raw_param_constraints = signature::method_param_shape_constraints(
        &resolved.program.types[0],
        resolved.program.types[0]
            .members
            .iter()
            .find_map(|member| match member {
                new_java2rust::java_ir::Member::Method(method) if method.name == "read" => {
                    Some(method)
                }
                _ => None,
            })
            .unwrap(),
        &resolved.program.types[0]
            .members
            .iter()
            .find_map(|member| match member {
                new_java2rust::java_ir::Member::Method(method) if method.name == "read" => {
                    method.params.first()
                }
                _ => None,
            })
            .unwrap(),
        &semantic.usage,
        &semantic.facts,
    );
    assert!(
        raw_param_constraints.contains(&signature::SignatureConstraint::RequiredOwnership(
            OwnershipShape::SharedBorrow,
        ))
    );
    assert!(
        !raw_param_constraints.contains(&signature::SignatureConstraint::RequiredOwnership(
            OwnershipShape::Owned,
        ))
    );
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

    assert_eq!(param_shape.ownership, OwnershipShape::SharedBorrow);
}

#[test]
fn readonly_same_type_public_helper_param_stays_owned_for_replacement_return() {
    let source = r#"
class ReadOnlySameTypeReplacementParam {
    public static com.acme.ExternalHandle copy(ExternalHandle handle) {
        return handle.copy();
    }
}
"#;
    let parsed = parser::parse_java_source(source);
    let resolved = resolve::resolve_program(&parsed.program, parsed.diagnostics);
    let mut external_facts = new_java2rust::facts::FactSet::default();
    external_facts
        .external_api_effect_facts
        .push(ExternalApiEffectFact {
            method_name: "copy".to_string(),
            receiver_type: Some("ExternalHandle".to_string()),
            arg_count: Some(0),
            role: ExternalApiRole::Getter,
            receiver_mutability: ReceiverMutabilityFact::ReadOnly,
            return_type: Some(JavaType::Unknown("ExternalHandle".to_string())),
            return_nullability: NullabilityState::DefinitelyNonNull,
            thrown: Vec::new(),
            provenance: FactProvenance::UserOverride,
        });
    let semantic =
        semantic_analysis::analyze_program(&resolved.program, &resolved.facts, &external_facts);
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
            new_java2rust::java_ir::Member::Method(method) if method.name == "copy" => Some(method),
            _ => None,
        })
        .unwrap();
    let param_shape = signatures.method_param_plans(ty, method).unwrap()[0]
        .shape
        .clone();

    assert_eq!(param_shape.ownership, OwnershipShape::Owned);
}

#[test]
fn readonly_public_helper_returning_opaque_external_product_keeps_param_owned() {
    let source = r#"
class ReadOnlyOpaqueProductParam {
    public static ExternalWidget make(ExternalHandle handle) {
        return handle.makeWidget();
    }
}
"#;
    let parsed = parser::parse_java_source(source);
    let resolved = resolve::resolve_program(&parsed.program, parsed.diagnostics);
    let mut external_facts = new_java2rust::facts::FactSet::default();
    external_facts
        .external_api_effect_facts
        .push(ExternalApiEffectFact {
            method_name: "makeWidget".to_string(),
            receiver_type: Some("ExternalHandle".to_string()),
            arg_count: Some(0),
            role: ExternalApiRole::Getter,
            receiver_mutability: ReceiverMutabilityFact::ReadOnly,
            return_type: Some(JavaType::Unknown("ExternalWidget".to_string())),
            return_nullability: NullabilityState::DefinitelyNonNull,
            thrown: Vec::new(),
            provenance: FactProvenance::UserOverride,
        });
    let semantic =
        semantic_analysis::analyze_program(&resolved.program, &resolved.facts, &external_facts);
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
            new_java2rust::java_ir::Member::Method(method) if method.name == "make" => Some(method),
            _ => None,
        })
        .unwrap();
    let param_shape = signatures.method_param_plans(ty, method).unwrap()[0]
        .shape
        .clone();

    assert_eq!(param_shape.ownership, OwnershipShape::Owned);
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
            arg_count: None,
            role: ExternalApiRole::Getter,
            receiver_mutability: ReceiverMutabilityFact::ReadOnly,
            return_type: None,
            return_nullability: NullabilityState::Unknown,
            thrown: Vec::new(),
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
            arg_count: None,
            role: ExternalApiRole::Getter,
            receiver_mutability: ReceiverMutabilityFact::ReadOnly,
            return_type: None,
            return_nullability: NullabilityState::Unknown,
            thrown: Vec::new(),
            provenance: FactProvenance::UserOverride,
        });
    external_facts
        .external_api_effect_facts
        .push(ExternalApiEffectFact {
            method_name: "setValue".to_string(),
            receiver_type: Some("ExternalCounter".to_string()),
            arg_count: None,
            role: ExternalApiRole::Setter,
            receiver_mutability: ReceiverMutabilityFact::MutatesReceiver,
            return_type: None,
            return_nullability: NullabilityState::Unknown,
            thrown: Vec::new(),
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
fn external_argument_effects_shape_method_params_in_signature_solver() {
    let source = r#"
class ExternalArgumentShapeUse {
    void use(String text, ExternalBuffer buffer, ExternalPayload payload) {
        ExternalApi.borrowString(text);
        ExternalApi.fill(buffer);
        ExternalApi.store(payload);
    }
}
"#;
    let parsed = parser::parse_java_source(source);
    let resolved = resolve::resolve_program(&parsed.program, parsed.diagnostics);
    let mut external_facts = new_java2rust::facts::FactSet::default();
    external_facts
        .external_api_argument_effect_facts
        .push(ExternalApiArgumentEffectFact {
            method_name: "borrowString".to_string(),
            receiver_type: None,
            arg_count: None,
            parameter_index: 0,
            parameter_type: Some(JavaType::String),
            parameter_nullability: NullabilityState::DefinitelyNonNull,
            argument_mutability: ReceiverMutabilityFact::ReadOnly,
            alias_policy: ExternalArgumentAliasPolicy::Borrowed,
            provenance: FactProvenance::UserOverride,
        });
    external_facts
        .external_api_argument_effect_facts
        .push(ExternalApiArgumentEffectFact {
            method_name: "store".to_string(),
            receiver_type: None,
            arg_count: None,
            parameter_index: 0,
            parameter_type: Some(JavaType::Named {
                name: "ExternalPayload".to_string(),
                args: Vec::new(),
            }),
            parameter_nullability: NullabilityState::Unknown,
            argument_mutability: ReceiverMutabilityFact::ReadOnly,
            alias_policy: ExternalArgumentAliasPolicy::Escapes,
            provenance: FactProvenance::UserOverride,
        });
    external_facts
        .external_api_argument_effect_facts
        .push(ExternalApiArgumentEffectFact {
            method_name: "fill".to_string(),
            receiver_type: None,
            arg_count: None,
            parameter_index: 0,
            parameter_type: Some(JavaType::Named {
                name: "ExternalBuffer".to_string(),
                args: Vec::new(),
            }),
            parameter_nullability: NullabilityState::Unknown,
            argument_mutability: ReceiverMutabilityFact::MutatesReceiver,
            alias_policy: ExternalArgumentAliasPolicy::Borrowed,
            provenance: FactProvenance::UserOverride,
        });
    let semantic =
        semantic_analysis::analyze_program(&resolved.program, &resolved.facts, &external_facts);
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
            new_java2rust::java_ir::Member::Method(method) if method.name == "use" => Some(method),
            _ => None,
        })
        .unwrap();
    let params = signatures.method_param_plans(ty, method).unwrap();

    assert_eq!(params[0].shape.ownership, OwnershipShape::SharedBorrow);
    assert_eq!(params[0].shape.view, ViewShape::Str);
    assert_eq!(params[0].shape.java_ty, JavaType::String);
    assert_eq!(params[1].shape.ownership, OwnershipShape::MutableBorrow);
    assert_eq!(
        params[1].shape.java_ty,
        JavaType::Named {
            name: "ExternalBuffer".to_string(),
            args: Vec::new(),
        }
    );
    assert_eq!(params[2].shape.ownership, OwnershipShape::Owned);
    assert_eq!(
        params[2].shape.java_ty,
        JavaType::Named {
            name: "ExternalPayload".to_string(),
            args: Vec::new(),
        }
    );
}

#[test]
fn external_constructor_arguments_default_to_non_null_unless_fact_widens_them() {
    let callback_ty = JavaType::Named {
        name: "Function".to_string(),
        args: Vec::new(),
    };
    let mut facts = FactSet::default();
    facts
        .external_api_argument_effect_facts
        .push(ExternalApiArgumentEffectFact {
            method_name: "new".to_string(),
            receiver_type: Some("ImportConfig".to_string()),
            arg_count: None,
            parameter_index: 0,
            parameter_type: Some(JavaType::String),
            parameter_nullability: NullabilityState::Unknown,
            argument_mutability: ReceiverMutabilityFact::ReadOnly,
            alias_policy: ExternalArgumentAliasPolicy::Escapes,
            provenance: FactProvenance::UserOverride,
        });
    facts
        .external_api_argument_effect_facts
        .push(ExternalApiArgumentEffectFact {
            method_name: "new".to_string(),
            receiver_type: Some("ImportConfig".to_string()),
            arg_count: None,
            parameter_index: 1,
            parameter_type: Some(callback_ty.clone()),
            parameter_nullability: NullabilityState::MaybeNull,
            argument_mutability: ReceiverMutabilityFact::ReadOnly,
            alias_policy: ExternalArgumentAliasPolicy::Escapes,
            provenance: FactProvenance::UserOverride,
        });

    let constructed_ty = JavaType::Named {
        name: "ImportConfig".to_string(),
        args: Vec::new(),
    };
    let shapes = new_java2rust::call_plan::external_constructor_arg_shapes(
        &constructed_ty,
        "new",
        3,
        &facts,
    )
    .expect("one modeled constructor argument should shape the constructor call");

    assert_eq!(shapes[0].java_ty, JavaType::String);
    assert_eq!(shapes[1].java_ty, optional_java_type(callback_ty));
    assert_eq!(shapes[2].java_ty, JavaType::Unknown("new:arg2".to_string()));
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
fn records_three_method_recursion_cycle_as_finite_mutual_edges() {
    let semantic = analyze(
        r#"
class ThreeCycle {
    int c(int x) { return a(x); }
    int a(int x) { return b(x); }
    int b(int x) { return c(x); }
}
"#,
    );

    let a = semantic.facts.recursion_facts_for("ThreeCycle", "a");
    assert!(
        a.iter()
            .any(|fact| { fact.calls.method_name == "b" && fact.kind == RecursionKind::Mutual })
    );

    let b = semantic.facts.recursion_facts_for("ThreeCycle", "b");
    assert!(
        b.iter()
            .any(|fact| { fact.calls.method_name == "c" && fact.kind == RecursionKind::Mutual })
    );

    let c = semantic.facts.recursion_facts_for("ThreeCycle", "c");
    assert!(
        c.iter()
            .any(|fact| { fact.calls.method_name == "a" && fact.kind == RecursionKind::Mutual })
    );

    assert_eq!(semantic.facts.recursion_facts.len(), 3);
}

#[test]
fn does_not_record_recursion_facts_for_acyclic_call_chains() {
    let semantic = analyze(
        r#"
class AcyclicCalls {
    int a(int x) { return b(x); }
    int b(int x) { return c(x); }
    int c(int x) { return x; }
}
"#,
    );

    assert!(semantic.facts.recursion_facts.is_empty());
}

#[test]
fn records_constructor_delegation_recursion_as_finite_edges() {
    let semantic = analyze(
        r#"
class ConstructorCycle {
    ConstructorCycle() { this(1); }
    ConstructorCycle(int x) { this(); }
}
"#,
    );

    let facts = &semantic.facts.constructor_recursion_facts;
    assert_eq!(facts.len(), 2, "{facts:?}");
    assert!(facts.iter().all(|fact| {
        fact.kind == RecursionKind::Mutual
            && fact.constructor.type_name == "ConstructorCycle"
            && fact.calls.type_name == "ConstructorCycle"
    }));

    let constructors = facts
        .iter()
        .map(|fact| fact.constructor.rust_name.as_str())
        .collect::<HashSet<_>>();
    let calls = facts
        .iter()
        .map(|fact| fact.calls.rust_name.as_str())
        .collect::<HashSet<_>>();
    assert_eq!(constructors, calls);
    assert_eq!(constructors.len(), 2);
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
fn translation_no_longer_reports_pending_assignment_in_condition_diagnostics() {
    let translation = translate_source(&idiom_gap("WhileAssignmentNotNull.java"));

    assert!(
        !translation
            .diagnostics
            .has_code("control_flow_rewrite_pending"),
        "{:?}",
        translation.diagnostics
    );
    assert!(
        !translation
            .rust
            .contains("assignment-in-condition normalization")
    );
}

#[test]
fn translation_no_longer_reports_pending_tail_recursive_rewrite_diagnostics() {
    let translation = translate_source(&idiom_gap("TailRecursiveAccumulator.java"));

    assert!(
        !translation
            .diagnostics
            .has_code("control_flow_rewrite_pending"),
        "{:?}",
        translation.diagnostics
    );
    assert!(
        !translation
            .rust
            .contains("tail-recursive loop normalization")
    );
    assert!(translation.rust.contains("loop {"), "{}", translation.rust);
}

#[test]
fn recursion_policy_warns_when_direct_recursion_is_not_safe_tail_loop_shape() {
    let source = r#"
class NonTailRecursive {
    int factorial(int n) {
        if (n <= 1) {
            return 1;
        }
        return n * factorial(n - 1);
    }
}
"#;
    let semantic = analyze(source);

    assert!(semantic.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "recursion_tco_fallback"
            && diagnostic.message.contains("NonTailRecursive::factorial")
            && diagnostic
                .message
                .contains("does not match the safe accumulator loop shape")
    }));
    assert!(semantic.control_flow_plans.is_empty());
}

#[test]
fn recursion_policy_warns_for_mutual_tail_style_recursion_fallback() {
    let semantic = analyze(&idiom_gap("MutualRecursion.java"));

    assert!(semantic.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "recursion_tco_fallback"
            && diagnostic.message.contains("MutualRecursion::even")
            && diagnostic.message.contains("mutual recursion")
    }));
    assert!(semantic.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "recursion_tco_fallback"
            && diagnostic.message.contains("MutualRecursion::odd")
            && diagnostic.message.contains("mutual recursion")
    }));
}

#[test]
fn recursion_policy_warns_for_receiver_recursive_calls_not_proven_this() {
    let source = r#"
class ReceiverRecursive {
    int count(ReceiverRecursive next, int n) {
        if (n <= 0) {
            return 0;
        }
        return next.count(this, n - 1);
    }
}
"#;
    let semantic = analyze(source);

    assert!(semantic.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "recursion_tco_fallback"
            && diagnostic.message.contains("ReceiverRecursive::count")
            && diagnostic.message.contains("not proven to be this")
    }));
}

#[test]
fn recursion_policy_warns_when_exception_wrapped_tail_loop_is_not_lowered() {
    let source = r#"
class Problem extends Exception {}
class ThrowingTailRecursive {
    long factorialLoop(int n, long acc) throws Problem {
        if (n <= 1) {
            return acc;
        }
        return factorialLoop(n - 1, acc * n);
    }
}
"#;
    let semantic = analyze(source);

    assert!(semantic.control_flow_plans.iter().any(|plan| {
        plan.method.type_name == "ThrowingTailRecursive"
            && plan.method.method_name == "factorialLoop"
            && plan
                .rewrites
                .iter()
                .any(|rewrite| matches!(rewrite, ControlFlowRewritePlan::TailRecursiveLoop(_)))
    }));
    assert!(semantic.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "recursion_tco_fallback"
            && diagnostic
                .message
                .contains("ThrowingTailRecursive::factorialLoop")
            && diagnostic
                .message
                .contains("exception lowering wraps the method in Result")
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
