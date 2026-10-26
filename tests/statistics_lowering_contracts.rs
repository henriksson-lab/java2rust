#[path = "common/mod.rs"]
mod common;

#[test]
fn interface_view_prerequisites_preserve_generic_callbacks_wildcards_and_native_bindings() {
    let source = include_str!(
        "corpus/statistics/descriptive-acceptance/InterfaceViewPrerequisitesAudit.java"
    );
    common::verify_java_sources_zero_contract(
        "interfaceprerequisites.InterfaceViewPrerequisitesAudit",
        &[(
            "tests/corpus/statistics/descriptive-acceptance/InterfaceViewPrerequisitesAudit.java",
            source,
        )],
    );
    common::cargo_run_translated_batch_sources(
        "interface_view_prerequisites",
        &[new_java2rust::BatchSource::new(
            "interfaceprerequisites/InterfaceViewPrerequisitesAudit.java",
            source,
        )],
        r#"
trait Exit { fn value(self) -> i32; }
impl Exit for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Exit for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
fn main() { assert_eq!(translated::InterfaceViewPrerequisitesAudit::run().value(),0); }
"#,
    );
}

#[test]
fn runtime_class_assignability_preserves_native_functional_ancestry_across_files() {
    let sources = [
        (
            "tests/corpus/statistics/descriptive-acceptance/NativeClassBase.java",
            include_str!("corpus/statistics/descriptive-acceptance/NativeClassBase.java"),
        ),
        (
            "tests/corpus/statistics/descriptive-acceptance/NativeClassAssignabilityAudit.java",
            include_str!(
                "corpus/statistics/descriptive-acceptance/NativeClassAssignabilityAudit.java"
            ),
        ),
    ];
    common::verify_java_sources_zero_contract(
        "nativeclassassignability.NativeClassAssignabilityAudit",
        &sources,
    );
    let batch = sources
        .iter()
        .map(|(path, source)| new_java2rust::BatchSource::new(*path, *source))
        .collect::<Vec<_>>();
    common::cargo_run_translated_batch_sources(
        "native_class_assignability",
        &batch,
        r#"
trait Exit { fn value(self) -> i32; }
impl Exit for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Exit for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
fn main() { assert_eq!(translated::NativeClassAssignabilityAudit::run().value(),0); }
"#,
    );
}

#[test]
fn runtime_class_assignability_preserves_ancestry_arrays_names_and_null_order() {
    let source =
        include_str!("corpus/statistics/descriptive-acceptance/ClassAssignabilityAudit.java");
    common::verify_java_sources_zero_contract(
        "classassignability.ClassAssignabilityAudit",
        &[(
            "tests/corpus/statistics/descriptive-acceptance/ClassAssignabilityAudit.java",
            source,
        )],
    );
    common::cargo_run_translated_batch_sources(
        "class_assignability",
        &[new_java2rust::BatchSource::new(
            "classassignability/ClassAssignabilityAudit.java",
            source,
        )],
        r#"
trait Exit { fn value(self) -> i32; }
impl Exit for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Exit for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
fn main() { assert_eq!(translated::ClassAssignabilityAudit::run().value(),0); }
"#,
    );
}

#[test]
fn nullable_generic_receivers_preserve_mutation_reborrows_and_argument_order() {
    common::verify_java_fixture_zero_contract(
        "nullable_generic_receivers",
        "nullablegenericreceiver.NullableGenericReceiverAudit",
        "tests/corpus/statistics/descriptive-acceptance/NullableGenericReceiverAudit.java",
        include_str!("corpus/statistics/descriptive-acceptance/NullableGenericReceiverAudit.java"),
        r#"
trait Exit { fn value(self) -> i32; }
impl Exit for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Exit for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn contract() { assert_eq!(NullableGenericReceiverAudit::run().value(),0); }
"#,
    );
}

#[test]
fn checked_array_fill_preserves_callback_order_reentrancy_and_failures() {
    let source = include_str!("corpus/statistics/descriptive-acceptance/CheckedFillAudit.java");
    common::verify_java_sources_zero_contract(
        "checkedfill.CheckedFillAudit",
        &[(
            "tests/corpus/statistics/descriptive-acceptance/CheckedFillAudit.java",
            source,
        )],
    );
    common::cargo_run_translated_batch_sources(
        "checked_array_fill",
        &[new_java2rust::BatchSource::new(
            "checkedfill/CheckedFillAudit.java",
            source,
        )],
        r#"
trait Exit { fn value(self) -> i32; }
impl Exit for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Exit for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
fn main() { assert_eq!(translated::CheckedFillAudit::run().value(),0); }
"#,
    );
}

#[test]
fn static_interface_factories_preserve_shared_array_arguments_and_state() {
    common::verify_java_fixture_zero_contract(
        "static_interface_storage",
        "staticinterfacestorage.StaticInterfaceStorageAudit",
        "tests/corpus/statistics/descriptive-acceptance/StaticInterfaceStorageAudit.java",
        include_str!("corpus/statistics/descriptive-acceptance/StaticInterfaceStorageAudit.java"),
        r#"
trait Exit { fn value(self) -> i32; }
impl Exit for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Exit for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn contract() { assert_eq!(StaticInterfaceStorageAudit::run().value(),0); }
"#,
    );
}

#[test]
fn typed_source_sam_cast_preserves_defaults_retention_identity_and_exceptions() {
    common::verify_java_fixture_zero_contract(
        "typed_source_sam_cast",
        "typedsourcesamcast.TypedSourceSamCastAudit",
        "tests/corpus/statistics/descriptive-acceptance/TypedSourceSamCastAudit.java",
        include_str!("corpus/statistics/descriptive-acceptance/TypedSourceSamCastAudit.java"),
        r#"
trait Exit { fn value(self) -> i32; }
impl Exit for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Exit for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn contract() { assert_eq!(TypedSourceSamCastAudit::run().value(),0); }
"#,
    );
}

#[test]
fn interface_destination_anonymous_ancestor_slots_preserves_identity_state_and_effects() {
    common::verify_java_fixture_zero_contract(
        "anonymous_ancestor_slots",
        "anonymousancestorslots.AnonymousAncestorSlotsAudit",
        "tests/corpus/statistics/descriptive-acceptance/AnonymousAncestorSlotsAudit.java",
        include_str!("corpus/statistics/descriptive-acceptance/AnonymousAncestorSlotsAudit.java"),
        r#"
trait Exit { fn value(self) -> i32; }
impl Exit for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Exit for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn contract() { assert_eq!(AnonymousAncestorSlotsAudit::run().value(),0); }
"#,
    );
}

#[test]
fn interface_destination_native_consumer_destinations_preserves_identity_state_and_effects() {
    common::verify_java_fixture_zero_contract(
        "native_consumer_destinations",
        "nativeconsumerdestinations.NativeConsumerDestinationsAudit",
        "tests/corpus/statistics/descriptive-acceptance/NativeConsumerDestinationsAudit.java",
        include_str!(
            "corpus/statistics/descriptive-acceptance/NativeConsumerDestinationsAudit.java"
        ),
        r#"
trait Exit { fn value(self) -> i32; }
impl Exit for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Exit for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn contract() { assert_eq!(NativeConsumerDestinationsAudit::run().value(),0); }
"#,
    );
}

#[test]
fn interface_destination_generic_non_native_qualifiers_preserves_identity_state_and_effects() {
    common::verify_java_fixture_zero_contract(
        "generic_non_native_qualifiers",
        "genericnonnativequalifiers.GenericNonNativeQualifierAudit",
        "tests/corpus/statistics/descriptive-acceptance/GenericNonNativeQualifierAudit.java",
        include_str!(
            "corpus/statistics/descriptive-acceptance/GenericNonNativeQualifierAudit.java"
        ),
        r#"
trait Exit { fn value(self) -> i32; }
impl Exit for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Exit for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn contract() { assert_eq!(GenericNonNativeQualifierAudit::run().value(),0); }
"#,
    );
}

#[test]
fn canonical_generic_qualifiers_reach_reference_consumers_before_emission() {
    common::verify_java_fixture_zero_contract(
        "generic_qualifier_saturation",
        "genericqualifiersaturation.GenericQualifierSaturationAudit",
        "tests/corpus/statistics/descriptive-acceptance/GenericQualifierSaturationAudit.java",
        include_str!(
            "corpus/statistics/descriptive-acceptance/GenericQualifierSaturationAudit.java"
        ),
        r#"
trait Exit { fn value(self) -> i32; }
impl Exit for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Exit for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn contract() { assert_eq!(GenericQualifierSaturationAudit::run().value(),0); }
"#,
    );
}

#[test]
fn source_sam_references_keep_generic_domains_aliases_and_checked_effects() {
    common::verify_java_fixture_zero_contract(
        "source_sam_domains",
        "sourcesamdomains.SourceSamDomainsAudit",
        "tests/corpus/statistics/descriptive-acceptance/SourceSamDomainsAudit.java",
        include_str!("corpus/statistics/descriptive-acceptance/SourceSamDomainsAudit.java"),
        r#"
trait Exit { fn value(self) -> i32; }
impl Exit for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Exit for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn contract() { assert_eq!(SourceSamDomainsAudit::run().value(),0); }
"#,
    );
}

#[test]
fn overloaded_factories_keep_static_and_argument_array_identity() {
    common::verify_java_fixture_zero_contract(
        "overloaded_factory_storage",
        "overloadedfactorystorage.OverloadedFactoryStorageAudit",
        "tests/corpus/statistics/descriptive-acceptance/OverloadedFactoryStorageAudit.java",
        include_str!("corpus/statistics/descriptive-acceptance/OverloadedFactoryStorageAudit.java"),
        r#"
trait Exit { fn value(self) -> i32; }
impl Exit for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Exit for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn contract() { assert_eq!(OverloadedFactoryStorageAudit::run().value(),0); }
"#,
    );
}

#[test]
fn reference_allocation_imported_constructor_and_nested_static_owners_stay_distinct() {
    let input = include_str!(
        "corpus/statistics/descriptive-acceptance/reference-factories/inputsum/Sum.java"
    );
    let result = include_str!(
        "corpus/statistics/descriptive-acceptance/reference-factories/resultsum/Sum.java"
    );
    let probe = include_str!(
        "corpus/statistics/descriptive-acceptance/reference-factories/ReferenceFactoryProbe.java"
    );
    let same = include_str!(
        "corpus/statistics/descriptive-acceptance/reference-factories/SamePackageFactoryProbe.java"
    );
    common::verify_java_sources_zero_contract(
        "resultsum.SamePackageFactoryProbe",
        &[
            (
                "tests/corpus/statistics/descriptive-acceptance/reference-factories/inputsum/Sum.java",
                input,
            ),
            (
                "tests/corpus/statistics/descriptive-acceptance/reference-factories/resultsum/Sum.java",
                result,
            ),
            (
                "tests/corpus/statistics/descriptive-acceptance/reference-factories/SamePackageFactoryProbe.java",
                same,
            ),
        ],
    );
    common::verify_java_sources_zero_contract(
        "referencefactories.ReferenceFactoryProbe",
        &[
            (
                "tests/corpus/statistics/descriptive-acceptance/reference-factories/inputsum/Sum.java",
                input,
            ),
            (
                "tests/corpus/statistics/descriptive-acceptance/reference-factories/resultsum/Sum.java",
                result,
            ),
            (
                "tests/corpus/statistics/descriptive-acceptance/reference-factories/ReferenceFactoryProbe.java",
                probe,
            ),
        ],
    );
    common::cargo_run_translated_batch_sources(
        "reference_factory_owners",
        &[
            new_java2rust::BatchSource::new("inputsum/Sum.java", input),
            new_java2rust::BatchSource::new("resultsum/Sum.java", result),
            new_java2rust::BatchSource::new("referencefactories/ReferenceFactoryProbe.java", probe),
            new_java2rust::BatchSource::new("resultsum/SamePackageFactoryProbe.java", same),
        ],
        r#"
trait Exit { fn value(self) -> i32; }
impl Exit for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Exit for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
fn main() { assert_eq!(translated::ReferenceFactoryProbe::run().value(),0); assert_eq!(translated::SamePackageFactoryProbe::run().value(),0); }
"#,
    );
}

#[test]
fn reference_allocation_static_array_identity_preserves_canonical_storage_and_effects() {
    common::verify_java_fixture_zero_contract(
        "static_array_identity",
        "staticarrayidentity.StaticArrayIdentityAudit",
        "tests/corpus/statistics/descriptive-acceptance/StaticArrayIdentityAudit.java",
        include_str!("corpus/statistics/descriptive-acceptance/StaticArrayIdentityAudit.java"),
        r#"
trait Exit { fn value(self) -> i32; }
impl Exit for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Exit for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn contract() { assert_eq!(StaticArrayIdentityAudit::run().value(),0); }
"#,
    );
}

#[test]
fn reference_allocation_constructor_references_preserves_canonical_storage_and_effects() {
    common::verify_java_fixture_zero_contract(
        "constructor_references",
        "constructorreference.ConstructorReferenceAudit",
        "tests/corpus/statistics/descriptive-acceptance/ConstructorReferenceAudit.java",
        include_str!("corpus/statistics/descriptive-acceptance/ConstructorReferenceAudit.java"),
        r#"
trait Exit { fn value(self) -> i32; }
impl Exit for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Exit for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn contract() { assert_eq!(ConstructorReferenceAudit::run().value(),0); }
"#,
    );
}

#[test]
fn cross_file_accessors_preserve_private_storage_aliases_nulls_and_constant_widths() {
    let sources = [
        new_java2rust::BatchSource::new(
            "accessorboundaries/Cell.java",
            include_str!("corpus/statistics/descriptive-acceptance/accessor-boundaries/Cell.java"),
        ),
        new_java2rust::BatchSource::new(
            "accessorboundaries/Holder.java",
            include_str!(
                "corpus/statistics/descriptive-acceptance/accessor-boundaries/Holder.java"
            ),
        ),
        new_java2rust::BatchSource::new(
            "accessorboundaries/AccessProbe.java",
            include_str!(
                "corpus/statistics/descriptive-acceptance/accessor-boundaries/AccessProbe.java"
            ),
        ),
    ];
    common::verify_java_sources_zero_contract(
        "accessorboundaries.AccessProbe",
        &[
            (
                "tests/corpus/statistics/descriptive-acceptance/accessor-boundaries/Cell.java",
                include_str!(
                    "corpus/statistics/descriptive-acceptance/accessor-boundaries/Cell.java"
                ),
            ),
            (
                "tests/corpus/statistics/descriptive-acceptance/accessor-boundaries/Holder.java",
                include_str!(
                    "corpus/statistics/descriptive-acceptance/accessor-boundaries/Holder.java"
                ),
            ),
            (
                "tests/corpus/statistics/descriptive-acceptance/accessor-boundaries/AccessProbe.java",
                include_str!(
                    "corpus/statistics/descriptive-acceptance/accessor-boundaries/AccessProbe.java"
                ),
            ),
        ],
    );
    common::cargo_run_translated_batch_sources(
        "accessor_boundaries",
        &sources,
        r#"
trait Exit { fn value(self) -> i32; }
impl Exit for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Exit for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
fn main() { assert_eq!(translated::AccessProbe::run().value(),0); }
"#,
    );
}

#[test]
fn collection_actions_request_native_support_without_imports() {
    common::verify_java_fixture_zero_contract(
        "implicit_collection_action",
        "implicitcollectionaction.ImplicitCollectionActionAudit",
        "tests/corpus/statistics/descriptive-acceptance/ImplicitCollectionActionAudit.java",
        include_str!("corpus/statistics/descriptive-acceptance/ImplicitCollectionActionAudit.java"),
        r#"
trait Exit { fn value(self) -> i32; }
impl Exit for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Exit for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn action() { assert_eq!(ImplicitCollectionActionAudit::run().value(),0); }
"#,
    );
}

#[test]
fn collection_for_each_keeps_callable_evaluation_nulls_aliases_and_checked_iteration() {
    let source =
        include_str!("corpus/statistics/descriptive-acceptance/CollectionForEachAudit.java");
    common::verify_java_sources_zero_contract(
        "collectionforeach.CollectionForEachAudit",
        &[(
            "tests/corpus/statistics/descriptive-acceptance/CollectionForEachAudit.java",
            source,
        )],
    );
    common::cargo_run_translated_batch_sources(
        "collection_for_each",
        &[new_java2rust::BatchSource::new(
            "collectionforeach/CollectionForEachAudit.java",
            source,
        )],
        r#"
trait Exit { fn value(self) -> i32; }
impl Exit for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Exit for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
fn main() { assert_eq!(translated::CollectionForEachAudit::run().value(),0); }
"#,
    );
}

#[test]
fn sibling_nested_declarations_keep_identity_in_one_compilation_unit() {
    common::verify_java_fixture_zero_contract(
        "sibling_nested_identity",
        "siblingbuilders.SiblingNestedBuilderAudit",
        "tests/corpus/statistics/descriptive-acceptance/SiblingNestedBuilderAudit.java",
        include_str!("corpus/statistics/descriptive-acceptance/SiblingNestedBuilderAudit.java"),
        r#"
trait Exit { fn value(self) -> i32; }
impl Exit for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Exit for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn identity() { assert_eq!(SiblingNestedBuilderAudit::run().value(),0); }
"#,
    );
}

#[test]
fn sibling_nested_builders_keep_lexical_types_and_escaping_callback_state() {
    let sources = [
        new_java2rust::BatchSource::new(
            "nestedbuilders/Alpha.java",
            include_str!("corpus/statistics/descriptive-acceptance/nested-builders/Alpha.java"),
        ),
        new_java2rust::BatchSource::new(
            "nestedbuilders/Beta.java",
            include_str!("corpus/statistics/descriptive-acceptance/nested-builders/Beta.java"),
        ),
        new_java2rust::BatchSource::new(
            "nestedbuilders/NestedBuilderProbe.java",
            include_str!(
                "corpus/statistics/descriptive-acceptance/nested-builders/NestedBuilderProbe.java"
            ),
        ),
    ];
    common::verify_java_sources_zero_contract(
        "nestedbuilders.NestedBuilderProbe",
        &[
            (
                "tests/corpus/statistics/descriptive-acceptance/nested-builders/Alpha.java",
                sources[0].source(),
            ),
            (
                "tests/corpus/statistics/descriptive-acceptance/nested-builders/Beta.java",
                sources[1].source(),
            ),
            (
                "tests/corpus/statistics/descriptive-acceptance/nested-builders/NestedBuilderProbe.java",
                sources[2].source(),
            ),
        ],
    );
    common::cargo_run_translated_batch_sources(
        "sibling_nested_builders",
        &sources,
        r#"
trait Exit { fn value(self) -> i32; }
impl Exit for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Exit for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
fn main(){assert_eq!(translated::NestedBuilderProbe::run().value(),0);}
"#,
    );
}

#[test]
fn bounded_static_overloads_keep_selected_declaration_across_files() {
    let sources = [
        new_java2rust::BatchSource::new(
            "genericoverloads/Feed.java",
            include_str!("corpus/statistics/descriptive-acceptance/generic-overloads/Feed.java"),
        ),
        new_java2rust::BatchSource::new(
            "genericoverloads/GenericFeedProbe.java",
            include_str!(
                "corpus/statistics/descriptive-acceptance/generic-overloads/GenericFeedProbe.java"
            ),
        ),
    ];
    common::verify_java_sources_zero_contract(
        "genericoverloads.GenericFeedProbe",
        &[
            (
                "tests/corpus/statistics/descriptive-acceptance/generic-overloads/Feed.java",
                sources[0].source(),
            ),
            (
                "tests/corpus/statistics/descriptive-acceptance/generic-overloads/GenericFeedProbe.java",
                sources[1].source(),
            ),
        ],
    );
    common::cargo_run_translated_batch_sources(
        "bounded_static_overloads",
        &sources,
        r#"
trait Exit { fn value(self) -> i32; }
impl Exit for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Exit for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
fn main(){assert_eq!(translated::GenericFeedProbe::run().value(),0);}
"#,
    );
}

#[test]
fn inherited_generic_and_primitive_overloads_share_trait_slot_names() {
    common::verify_java_fixture_zero_contract(
        "inherited_generic_overload",
        "inheritedgenericoverload.InheritedGenericOverloadProbe",
        "tests/corpus/statistics/descriptive-acceptance/InheritedGenericOverloadProbe.java",
        include_str!("corpus/statistics/descriptive-acceptance/InheritedGenericOverloadProbe.java"),
        r#"
trait Exit { fn value(self) -> i32; }
impl Exit for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Exit for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn slots(){assert_eq!(InheritedGenericOverloadProbe::run().value(),0);}
"#,
    );
}

#[test]
fn super_implementations_keep_selected_argument_effect_and_carrier_write_contracts() {
    common::verify_java_fixture_zero_contract(
        "super_implementation",
        "superimplementation.SuperImplementationAudit",
        "tests/corpus/statistics/descriptive-acceptance/SuperImplementationAudit.java",
        include_str!("corpus/statistics/descriptive-acceptance/SuperImplementationAudit.java"),
        r#"
trait Exit { fn value(self) -> i32; }
impl Exit for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Exit for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn super_implementation() { assert_eq!(SuperImplementationAudit::run().value(),0); }
"#,
    );
}

#[test]
fn hierarchy_constructors_preserve_delegated_values_carriers_and_copy_state() {
    common::verify_java_fixture_zero_contract(
        "hierarchy_constructor",
        "hierarchyconstructor.HierarchyConstructorAudit",
        "tests/corpus/statistics/descriptive-acceptance/HierarchyConstructorAudit.java",
        include_str!("corpus/statistics/descriptive-acceptance/HierarchyConstructorAudit.java"),
        r#"
trait Exit { fn value(self) -> i32; }
impl Exit for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Exit for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn hierarchy_constructor() { assert_eq!(HierarchyConstructorAudit::run().value(),0); }
"#,
    );
}

#[test]
fn stored_source_callbacks_keep_factory_identity_and_reentrant_state() {
    common::verify_java_fixture_zero_contract(
        "stored_source_callback",
        "storedcallback.StoredSourceCallbackAudit",
        "tests/corpus/statistics/descriptive-acceptance/StoredSourceCallbackAudit.java",
        include_str!("corpus/statistics/descriptive-acceptance/StoredSourceCallbackAudit.java"),
        r#"
trait Exit { fn value(self) -> i32; }
impl Exit for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Exit for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn source_callback() { assert_eq!(StoredSourceCallbackAudit::run().value(),0); }
"#,
    );
}

#[test]
fn generic_consumer_bounds_keep_state_identity_nulls_and_checked_invocation() {
    common::verify_java_fixture_zero_contract(
        "generic_consumer_bound",
        "consumerbound.GenericConsumerBoundAudit",
        "tests/corpus/statistics/descriptive-acceptance/GenericConsumerBoundAudit.java",
        include_str!("corpus/statistics/descriptive-acceptance/GenericConsumerBoundAudit.java"),
        r#"
trait Exit { fn value(self) -> i32; }
impl Exit for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Exit for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn generic_consumer() { assert_eq!(GenericConsumerBoundAudit::run().value(),0); }
#[test] fn local_and_synchronized_protocols_use_checked_callback_abi() {
    let calls = std::rc::Rc::new(std::cell::Cell::new(0));
    let retained = calls.clone();
    let mut local = DoubleConsumer::from_shared_fallible_fn(move |value| {
        retained.set(retained.get() + 1);
        if value < 0.0 { Err(JavaException::new("IllegalArgumentException", "callback")) }
        else { Ok(()) }
    });
    let identity = JavaReferenceIdentity::java_reference_identity(&local);
    __JavaBoundDoubleConsumer::__java_accept_result(&mut local, 1.0).unwrap();
    assert!(__JavaBoundDoubleConsumer::__java_accept_result(&mut local, -1.0).is_err());
    assert_eq!(calls.get(), 2);
    assert_eq!(identity, JavaReferenceIdentity::java_reference_identity(&local));
    let mut sync = __JavaSyncDoubleConsumer::from_shared_fallible_fn(|_| {
        Err(JavaException::new("IllegalStateException", "sync"))
    });
    assert!(__JavaBoundDoubleConsumer::__java_accept_result(&mut sync, 1.0).is_err());
}
"#,
    );
}

#[test]
fn primitive_consumer_views_preserve_source_state_identity_and_composition() {
    common::verify_java_fixture_zero_contract(
        "primitive_consumer_views",
        "consumerview.PrimitiveConsumerViewAudit",
        "tests/corpus/statistics/descriptive-acceptance/PrimitiveConsumerViewAudit.java",
        include_str!("corpus/statistics/descriptive-acceptance/PrimitiveConsumerViewAudit.java"),
        r#"
trait Exit { fn value(self) -> i32; }
impl Exit for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Exit for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn state_and_identity() { assert_eq!(PrimitiveConsumerViewAudit::run().value(), 0); }
"#,
    );
}

#[test]
fn native_bytebuffer_preserves_backing_array_and_position_aliases() {
    common::verify_java_fixture_zero_contract(
        "bytebuffer_backing",
        "bufferbacking.ByteBufferBackingAudit",
        "tests/corpus/statistics/descriptive-acceptance/ByteBufferBackingAudit.java",
        include_str!("corpus/statistics/descriptive-acceptance/ByteBufferBackingAudit.java"),
        r#"
trait Exit { fn value(self) -> i32; }
impl Exit for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Exit for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn backing() { assert_eq!(ByteBufferBackingAudit::run().value(), 0); }
"#,
    );
}

#[test]
fn source_hierarchy_factories_reuse_carrier_upcasts_and_identity() {
    common::verify_java_fixture_zero_contract(
        "hierarchy_factory",
        "HierarchyFactoryAudit",
        "tests/corpus/statistics/descriptive-acceptance/HierarchyFactoryAudit.java",
        include_str!("corpus/statistics/descriptive-acceptance/HierarchyFactoryAudit.java"),
        r#"
trait Exit { fn value(self) -> i32; }
impl Exit for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Exit for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn hierarchy() { assert_eq!(HierarchyFactoryAudit::run().value(), 0); }
"#,
    );
}

#[test]
fn source_generic_arrays_identity_preserve_aliases() {
    common::verify_java_fixture_zero_contract(
        "source_array_identity",
        "SourceArrayIdentityAudit",
        "tests/corpus/statistics/descriptive-acceptance/SourceArrayIdentityAudit.java",
        include_str!("corpus/statistics/descriptive-acceptance/SourceArrayIdentityAudit.java"),
        r#"
trait Exit { fn value(self) -> i32; }
impl Exit for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Exit for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn aliases() { assert_eq!(SourceArrayIdentityAudit::run().value(), 0); }
"#,
    );
}

#[test]
fn source_generic_arrays_sync_preserve_aliases() {
    common::verify_java_fixture_zero_contract(
        "source_array_sync",
        "SourceArraySyncAudit",
        "tests/corpus/statistics/descriptive-acceptance/SourceArraySyncAudit.java",
        include_str!("corpus/statistics/descriptive-acceptance/SourceArraySyncAudit.java"),
        r#"
trait Exit { fn value(self) -> i32; }
impl Exit for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Exit for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn aliases() { assert_eq!(SourceArraySyncAudit::run().value(), 0); }
"#,
    );
}

#[test]
fn source_callback_anonymous_receivers_share_the_interface_contract() {
    common::verify_java_fixture_zero_contract(
        "anonymous_receiver_contract",
        "AnonymousReceiverContractAudit",
        "tests/corpus/statistics/descriptive-acceptance/AnonymousReceiverContractAudit.java",
        include_str!(
            "corpus/statistics/descriptive-acceptance/AnonymousReceiverContractAudit.java"
        ),
        r#"
trait Exit { fn value(self) -> i32; }
impl Exit for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Exit for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn contract() { assert_eq!(AnonymousReceiverContractAudit::run().value(), 0); }
"#,
    );
}

#[test]
fn source_callback_field_references_capture_the_selected_reference() {
    common::verify_java_fixture_zero_contract(
        "field_reference_capture",
        "FieldReferenceCaptureAudit",
        "tests/corpus/statistics/descriptive-acceptance/FieldReferenceCaptureAudit.java",
        include_str!("corpus/statistics/descriptive-acceptance/FieldReferenceCaptureAudit.java"),
        r#"
trait Exit { fn value(self) -> i32; }
impl Exit for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Exit for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn capture() { assert_eq!(FieldReferenceCaptureAudit::run().value(), 0); }
"#,
    );
}

#[test]
fn inherited_native_results_retain_callbacks_and_default_dispatch() {
    common::verify_java_fixture_zero_contract(
        "inherited_native_result",
        "InheritedNativeResultAudit",
        "tests/corpus/statistics/descriptive-acceptance/InheritedNativeResultAudit.java",
        include_str!("corpus/statistics/descriptive-acceptance/InheritedNativeResultAudit.java"),
        r#"
trait Exit { fn value(self) -> i32; }
impl Exit for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Exit for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn inherited() { assert_eq!(InheritedNativeResultAudit::run().value(), 0); }
"#,
    );
}

#[test]
fn conditional_array_reads_widen_after_checked_evaluation() {
    common::verify_java_fixture_zero_contract(
        "conditional_array_widening",
        "ConditionalArrayWideningAudit",
        "tests/corpus/statistics/ranking-acceptance/ConditionalArrayWideningAudit.java",
        include_str!("corpus/statistics/ranking-acceptance/ConditionalArrayWideningAudit.java"),
        r#"
trait Exit { fn value(self) -> i32; }
impl Exit for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Exit for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn widening() { assert_eq!(ConditionalArrayWideningAudit::run().value(), 0); }
"#,
    );
}

#[test]
fn erased_hash_dispatch_preserves_source_overrides_state_and_exceptions() {
    common::verify_java_fixture_zero_contract(
        "erased_hash_dispatch",
        "ErasedHashDispatchAudit",
        "tests/corpus/statistics/ranking-acceptance/ErasedHashDispatchAudit.java",
        include_str!("corpus/statistics/ranking-acceptance/ErasedHashDispatchAudit.java"),
        r#"
trait Exit { fn value(self) -> i32; }
impl Exit for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Exit for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn hashing() { assert_eq!(ErasedHashDispatchAudit::run().value(), 0); }
"#,
    );
}

#[test]
fn erased_enum_hashes_preserve_the_typed_identity_hash() {
    common::verify_java_fixture_zero_contract(
        "erased_enum_hash",
        "EnumErasedHashAudit",
        "tests/corpus/statistics/ranking-acceptance/EnumErasedHashAudit.java",
        include_str!("corpus/statistics/ranking-acceptance/EnumErasedHashAudit.java"),
        r#"
trait Exit { fn value(self) -> i32; }
impl Exit for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Exit for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn hashing() { assert_eq!(EnumErasedHashAudit::run().value(), 0); }
"#,
    );
}

#[test]
fn enum_objects_preserve_identity_erasure_and_binary_class_metadata() {
    common::verify_java_fixture_zero_contract(
        "enum_object_contract",
        "audit.enums.EnumObjectAudit",
        "tests/corpus/statistics/ranking-acceptance/EnumObjectAudit.java",
        include_str!("corpus/statistics/ranking-acceptance/EnumObjectAudit.java"),
        r#"
trait Exit { fn value(self) -> i32; }
impl Exit for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Exit for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn objects() { assert_eq!(EnumObjectAudit::run().value(), 0); }
"#,
    );
}

#[test]
fn enum_comparisons_stage_snapshots_before_checked_invocation() {
    common::verify_java_fixture_zero_contract(
        "enum_comparison_order",
        "EnumCompareAudit",
        "tests/corpus/statistics/ranking-acceptance/EnumCompareAudit.java",
        include_str!("corpus/statistics/ranking-acceptance/EnumCompareAudit.java"),
        r#"
trait Exit { fn value(self) -> i32; }
impl Exit for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Exit for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn comparison() {
    let plain: i32 = EnumCompareAudit::plain();
    assert_eq!(plain, -2);
    assert_eq!(EnumCompareAudit::run().value(), 0);
}
"#,
    );
}

#[test]
fn enum_name_results_keep_string_contracts_and_catchable_null_receivers() {
    common::verify_java_fixture_zero_contract(
        "enum_name_strings",
        "EnumNameStringAudit",
        "tests/corpus/statistics/ranking-acceptance/EnumNameStringAudit.java",
        include_str!("corpus/statistics/ranking-acceptance/EnumNameStringAudit.java"),
        r#"
trait Exit { fn value(self) -> i32; }
impl Exit for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Exit for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn names() { assert_eq!(EnumNameStringAudit::run().value(), 0); }
"#,
    );
}

#[test]
fn enum_values_attach_fresh_array_storage_and_preserve_aliases() {
    common::verify_java_fixture_zero_contract(
        "enum_fresh_arrays",
        "EnumFreshArrayAudit",
        "tests/corpus/statistics/ranking-acceptance/EnumFreshArrayAudit.java",
        include_str!("corpus/statistics/ranking-acceptance/EnumFreshArrayAudit.java"),
        r#"
trait Exit { fn value(self) -> i32; }
impl Exit for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Exit for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn arrays() { assert_eq!(EnumFreshArrayAudit::run().value(), 0); }
"#,
    );
}

#[test]
fn retained_local_callbacks_allow_recursion_and_recover_after_exceptions() {
    common::verify_java_fixture_zero_contract(
        "functional_reentry",
        "FunctionalReentryAudit",
        "tests/corpus/statistics/composition-audit/FunctionalReentryAudit.java",
        include_str!("corpus/statistics/composition-audit/FunctionalReentryAudit.java"),
        r#"
trait Exit { fn value(self) -> i32; }
impl Exit for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Exit for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn reentry() { assert_eq!(FunctionalReentryAudit::run().value(), 0); }
"#,
    );
}

#[test]
fn generic_composition_preserves_factory_order_and_operator_parent_contracts() {
    common::verify_java_fixture_zero_contract(
        "generic_composition_order",
        "GenericCompositionOrderAudit",
        "tests/corpus/statistics/generic-composition-audit/GenericCompositionOrderAudit.java",
        include_str!(
            "corpus/statistics/generic-composition-audit/GenericCompositionOrderAudit.java"
        ),
        r#"
trait Exit { fn value(self) -> i32; }
impl Exit for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Exit for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn order() { assert_eq!(GenericCompositionOrderAudit::run().value(), 0); }
"#,
    );
}

#[test]
fn generic_composition_source_overrides_require_polymorphic_dispatch() {
    let translated = new_java2rust::translate_source(
        r#"
import java.util.function.Function;
class Override implements Function<String, String> {
    public String apply(String value) { return value; }
    public <V> Function<String, V> andThen(Function<? super String, ? extends V> after) { return null; }
}
class UseOverride { static Function<String, String> create() { return new Override(); } }
"#,
    );
    assert!(
        translated.rust.contains(
            "native generic functional composition override requires polymorphic dispatch"
        ),
        "{}",
        translated.rust
    );
}

#[test]
fn generic_composition_preserves_intermediate_arrays_and_source_state() {
    common::verify_java_fixture_zero_contract(
        "generic_composition_state",
        "GenericCompositionAudit",
        "tests/corpus/statistics/generic-composition-audit/GenericCompositionAudit.java",
        include_str!("corpus/statistics/generic-composition-audit/GenericCompositionAudit.java"),
        r#"
trait Exit { fn value(self) -> i32; }
impl Exit for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Exit for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn composition() { assert_eq!(GenericCompositionAudit::run().value(), 0); }
"#,
    );
}

#[test]
fn generic_composition_preserves_nullable_locals_and_escaping_suppliers() {
    common::verify_java_fixture_zero_contract(
        "generic_composition_domains",
        "GenericCompositionDomainsAudit",
        "tests/corpus/statistics/generic-composition-audit/GenericCompositionDomainsAudit.java",
        include_str!(
            "corpus/statistics/generic-composition-audit/GenericCompositionDomainsAudit.java"
        ),
        r#"
trait Exit { fn value(self) -> i32; }
impl Exit for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Exit for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn domains() { assert_eq!(GenericCompositionDomainsAudit::run().value(), 0); }
"#,
    );
}

#[test]
fn source_default_helpers_preserve_declared_callback_contracts() {
    common::verify_java_fixture_zero_contract(
        "declared_default_sam",
        "DeclaredDefaultSamAudit",
        "tests/corpus/statistics/composition-audit/DeclaredDefaultSamAudit.java",
        include_str!("corpus/statistics/composition-audit/DeclaredDefaultSamAudit.java"),
        r#"
trait Exit { fn value(self) -> i32; }
impl Exit for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Exit for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn declared_callback() { assert_eq!(DeclaredDefaultSamAudit::run().value(), 0); }
"#,
    );
}

#[test]
fn callback_array_iteration_preserves_carriers_aliases_and_retained_elements() {
    common::verify_java_fixture_zero_contract(
        "callback_array_composition",
        "PrimitiveCompositionArrayAudit",
        "tests/corpus/statistics/primitive-composition-arrays/PrimitiveCompositionArrayAudit.java",
        include_str!(
            "corpus/statistics/primitive-composition-arrays/PrimitiveCompositionArrayAudit.java"
        ),
        r#"
trait Exit { fn value(self) -> i32; }
impl Exit for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Exit for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn retained_array_callbacks() { assert_eq!(PrimitiveCompositionArrayAudit::run().value(), 0); }
"#,
    );
}

#[test]
fn primitive_composition_preserves_order_identity_state_and_exceptions() {
    common::verify_java_fixture_zero_contract(
        "primitive_composition",
        "FixedCompositionAudit",
        "tests/corpus/statistics/composition-audit/FixedCompositionAudit.java",
        include_str!("corpus/statistics/composition-audit/FixedCompositionAudit.java"),
        r#"
trait Exit { fn value(self) -> i32; }
impl Exit for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Exit for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn composition() { assert_eq!(FixedCompositionAudit::run().value(), 0); }
"#,
    );
}

#[test]
fn callback_local_domains_preserve_null_aliases_and_source_effect_order() {
    common::verify_java_fixture_zero_contract(
        "callback_local_domains",
        "FunctionalLambdaDomainsAudit",
        "tests/corpus/statistics/composition-audit/FunctionalLambdaDomainsAudit.java",
        include_str!("corpus/statistics/composition-audit/FunctionalLambdaDomainsAudit.java"),
        r#"
trait Exit { fn value(self) -> i32; }
impl Exit for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Exit for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn domains() { assert_eq!(FunctionalLambdaDomainsAudit::run().value(), 0); }
"#,
    );
}

#[test]
fn native_functional_parent_views_preserve_identity_nulls_and_array_domains() {
    common::verify_java_fixture_zero_contract(
        "functional_parent_view",
        "FunctionalParentViewProbe",
        "tests/corpus/statistics/FunctionalParentViewProbe.java",
        include_str!("corpus/statistics/FunctionalParentViewProbe.java"),
        r#"
trait Exit { fn value(self) -> i32; }
impl Exit for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Exit for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn views() { assert_eq!(FunctionalParentViewProbe::run().value(), 0); }
"#,
    );
}

#[test]
fn imported_enum_fields_keep_nullable_storage_across_batch_translation() {
    let fixtures =
        common::load_java_fixtures("tests/corpus/statistics/nullable-enum-fields").unwrap();
    let paths: Vec<_> = fixtures
        .iter()
        .map(|fixture| {
            format!(
                "tests/corpus/statistics/nullable-enum-fields/{}",
                fixture.relative_path.display()
            )
        })
        .collect();
    let java_sources: Vec<_> = paths
        .iter()
        .zip(&fixtures)
        .map(|(path, fixture)| (path.as_str(), fixture.source.as_str()))
        .collect();
    common::verify_java_sources_zero_contract("neutral.client.ImportedEnumProbe", &java_sources);
    let sources: Vec<_> = fixtures
        .iter()
        .map(|fixture| {
            new_java2rust::BatchSource::new(
                fixture.relative_path.to_str().unwrap(),
                &fixture.source,
            )
        })
        .collect();
    common::cargo_run_translated_batch_sources(
        "imported_enum_fields",
        &sources,
        r#"
use translated::ImportedEnumProbe;
trait Exit { fn value(self) -> i32; }
impl Exit for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Exit for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
fn main() { assert_eq!(ImportedEnumProbe::run().value(), 0); }
"#,
    );
}

#[test]
fn functional_array_domains_preserve_source_inputs_results_and_retained_aliases() {
    common::verify_java_fixture_zero_contract(
        "functional_array_domain",
        "FunctionalArrayDomainProbe",
        "tests/corpus/statistics/FunctionalArrayDomainProbe.java",
        include_str!("corpus/statistics/FunctionalArrayDomainProbe.java"),
        r#"
trait Exit { fn value(self) -> i32; }
impl Exit for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Exit for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn aliases() { assert_eq!(FunctionalArrayDomainProbe::run().value(), 0); }
"#,
    );
}

#[test]
fn nullable_enum_constructor_inputs_keep_field_comparison_storage() {
    common::verify_java_fixture_zero_contract(
        "nullable_enum_field",
        "NullableEnumFieldProbe",
        "tests/corpus/statistics/NullableEnumFieldProbe.java",
        include_str!("corpus/statistics/NullableEnumFieldProbe.java"),
        r#"
trait Exit { fn value(self) -> i32; }
impl Exit for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Exit for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn comparisons() { assert_eq!(NullableEnumFieldProbe::run().value(), 0); }
"#,
    );
}

#[test]
fn source_owned_random_lookalikes_keep_their_declared_state_and_validation() {
    common::verify_java_fixture_zero_contract(
        "source_random",
        "SourceRandomProbe",
        "tests/corpus/statistics/SourceRandomProbe.java",
        include_str!("corpus/statistics/SourceRandomProbe.java"),
        r#"
trait Exit { fn value(self) -> i32; }
impl Exit for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Exit for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn source_state() { assert_eq!(SourceRandomProbe::run().value(), 0); }
"#,
    );
}

#[test]
fn splittable_random_preserves_seeded_draws_aliases_callbacks_and_invalid_state() {
    common::verify_java_fixture_zero_contract(
        "splittable_random",
        "SplittableRandomProbe",
        "tests/corpus/statistics/SplittableRandomProbe.java",
        include_str!("corpus/statistics/SplittableRandomProbe.java"),
        r#"
trait Exit { fn value(self) -> i32; }
impl Exit for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Exit for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn draws() { assert_eq!(SplittableRandomProbe::run().value(), 0); }
"#,
    );
}

#[test]
fn bound_expression_references_evaluate_once_and_retain_the_selected_receiver() {
    common::verify_java_fixture_zero_contract(
        "bound_expression_reference",
        "BoundExpressionReferenceProbe",
        "tests/corpus/statistics/BoundExpressionReferenceProbe.java",
        include_str!("corpus/statistics/BoundExpressionReferenceProbe.java"),
        r#"
trait Exit { fn value(self) -> i32; }
impl Exit for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Exit for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn bound_receivers() { assert_eq!(BoundExpressionReferenceProbe::run().value(), 0); }
"#,
    );
}

#[test]
fn source_owned_array_sort_keeps_its_selected_algorithm_and_callback_arity() {
    common::verify_java_fixture_zero_contract(
        "source_array_sort",
        "SourceArraySortProbe",
        "tests/corpus/statistics/SourceArraySortProbe.java",
        include_str!("corpus/statistics/SourceArraySortProbe.java"),
        r#"
trait Exit { fn value(self) -> i32; }
impl Exit for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Exit for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn source_algorithm() { assert_eq!(SourceArraySortProbe::run().value(), 0); }
"#,
    );
}

#[test]
fn reference_array_copies_keep_shallow_identity_padding_and_call_return_contracts() {
    common::verify_java_fixture_zero_contract(
        "reference_array_copies",
        "ReferenceArrayCopiesProbe",
        "tests/corpus/statistics/ReferenceArrayCopiesProbe.java",
        include_str!("corpus/statistics/ReferenceArrayCopiesProbe.java"),
        r#"
trait Exit { fn value(self) -> i32; }
impl Exit for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Exit for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn copies() { assert_eq!(ReferenceArrayCopiesProbe::run().value(), 0); }
"#,
    );
}

#[test]
fn reference_array_comparators_preserve_aliases_stability_and_errors() {
    common::verify_java_fixture_zero_contract(
        "reference_array_sort",
        "ReferenceArraySortProbe",
        "tests/corpus/statistics/ReferenceArraySortProbe.java",
        include_str!("corpus/statistics/ReferenceArraySortProbe.java"),
        r#"
trait Exit { fn value(self) -> i32; }
impl Exit for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Exit for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn ordering() { assert_eq!(ReferenceArraySortProbe::run().value(), 0); }
"#,
    );
}

#[test]
fn source_owned_string_factory_keeps_its_declared_argument_and_result_contract() {
    common::verify_java_fixture_zero_contract(
        "source_string_factory",
        "SourceStringFactoryProbe",
        "tests/corpus/statistics/SourceStringFactoryProbe.java",
        include_str!("corpus/statistics/SourceStringFactoryProbe.java"),
        r#"
trait Exit { fn value(self) -> i32; }
impl Exit for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Exit for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn source_contract() { assert_eq!(SourceStringFactoryProbe::run().value(), 0); }
"#,
    );
}

#[test]
fn native_string_producers_finish_before_optional_and_borrowed_destinations() {
    common::verify_java_fixture_zero_contract(
        "native_string_slots",
        "NativeStringSlotsProbe",
        "tests/corpus/statistics/NativeStringSlotsProbe.java",
        include_str!("corpus/statistics/NativeStringSlotsProbe.java"),
        r#"
trait Exit { fn value(self) -> i32; }
impl Exit for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Exit for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn string_contracts() { assert_eq!(NativeStringSlotsProbe::run().value(), 0); }
"#,
    );
}
