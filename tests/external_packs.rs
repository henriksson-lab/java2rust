use new_java2rust::external_facts::parse_external_facts;
use new_java2rust::external_packs::{
    ExternalPackApiEffect, ExternalPackClosedHierarchy, ExternalPackManifest,
    ExternalPackModeledApi, ExternalPackModeledConstructor, ExternalPackModeledField,
    ExternalPackObjectSafety, ExternalPackObjectSafetyViolation, ExternalPackUnsupportedApi,
    apache_commons_codec_pack_facts, apache_commons_collections_pack_facts,
    apache_commons_compress_pack_facts, apache_commons_config_pack_facts,
    apache_commons_lang_pack_facts, apache_commons_math_optim_pack_facts,
    apache_commons_math_pack_facts, apache_commons_math_stats_pack_facts, apache_http_pack_facts,
    audit_external_pack_fact_boundaries, audit_external_pack_manifest_coverage, avro_pack_facts,
    barclay_argparser_pack_facts, barclay_help_pack_facts, barclay_utils_pack_facts,
    broad_http_pack_facts, builtin_external_library_packs, builtin_external_pack_facts,
    builtin_external_pack_manifest, builtin_external_pack_manifest_for_provider_name,
    builtin_external_pack_manifests, bwa_pack_facts, disq_write_pack_facts,
    external_api_effect_for_receiver_with_arity, external_constructor_for,
    external_crate_api_mapping_for_receiver, external_crate_constructor_mapping_for,
    external_field_for, external_library_pack_for_type, fastutil_pack_facts, genomicsdb_pack_facts,
    google_bigquery_pack_facts, google_bigquery_storage_pack_facts, google_cloud_nio_pack_facts,
    google_storage_pack_facts, guava_concurrent_pack_facts, guava_immutable_collect_pack_facts,
    hadoop_conf_pack_facts, hdf5_pack_facts, hipparchus_distribution_pack_facts,
    htsjdk_bcf_pack_facts, htsjdk_nested_helper_pack_facts, htsjdk_sam_bam_pack_facts,
    htsjdk_sam_cram_pack_facts, htsjdk_sam_header_pack_facts, htsjdk_sam_metrics_pack_facts,
    htsjdk_sam_reader_pack_facts, htsjdk_sam_record_pack_facts, htsjdk_sam_reference_pack_facts,
    htsjdk_sam_writer_pack_facts, htsjdk_samtools_util_pack_facts,
    htsjdk_tribble_feature_pack_facts, htsjdk_tribble_index_pack_facts,
    htsjdk_variantcontext_pack_facts, htsjdk_vcf_pack_facts, intel_gkl_pack_facts,
    jackson_databind_pack_facts, jama_pack_facts, java_sql_pack_facts, jgrapht_pack_facts,
    kryo_pack_facts, kryo_serializers_pack_facts, opencsv_pack_facts, org_json_pack_facts,
    parse_external_pack_manifest, picard_markduplicates_pack_facts, picard_metrics_pack_facts,
    picard_programgroups_pack_facts, picard_vcf_pack_facts, reflections_pack_facts,
    representation_facts_for_type, scala_tuple_pack_facts, spark_pack_facts, sqlite_pack_facts,
    unsupported_external_api_for_receiver,
};
use new_java2rust::facts::{
    ClosedHierarchyFact, ExceptionFact, ExceptionKind, ExternalApiArgumentEffectFact,
    ExternalApiCoverage, ExternalApiEffectFact, ExternalApiRole, ExternalArgumentAliasPolicy,
    ExternalConstructorFact, ExternalCrateApiMappingFact, ExternalCrateConstructorMappingFact,
    ExternalFieldFact, ExternalLibraryMode, FactProvenance, FactSet, FieldKey,
    FieldNullabilityFact, FieldNullabilityProvenance, HierarchyOverride, HierarchyStrategy,
    NullabilityState, ObjectSafetyFact, ObjectSafetyViolation, ReceiverMutabilityFact,
    TypeCapabilityOverride, UnsupportedExternalApiFact, UnsupportedExternalConstructorFact,
    UnsupportedExternalFieldFact,
};
use new_java2rust::java_ir::{JavaType, PrimitiveType};
use new_java2rust::{parser, resolve, semantic_analysis};

#[test]
fn builtin_external_pack_manifests_declare_boundaries_and_crate_hooks() {
    let manifests = builtin_external_pack_manifests();
    let variantcontext = manifests
        .iter()
        .find(|manifest| manifest.pack_name == "htsjdk.variant.variantcontext")
        .expect("variantcontext manifest should exist");
    assert!(variantcontext.owns_type("htsjdk.variant.variantcontext.VariantContext"));
    assert_eq!(variantcontext.mode, ExternalLibraryMode::ExternalFactModel);
    assert_eq!(
        variantcontext.coverage_for_api(
            "htsjdk.variant.variantcontext.VariantContext",
            "fullyDecode"
        ),
        ExternalApiCoverage::Unsupported
    );
    assert_eq!(
        variantcontext.coverage_for_api("htsjdk.variant.variantcontext.VariantContext", "getID"),
        ExternalApiCoverage::Semantic
    );
    let get_contig = variantcontext
        .modeled_api("htsjdk.variant.variantcontext.VariantContext", "getContig")
        .expect("modeled VariantContext getters should be explicit manifest entries");
    assert_eq!(get_contig.coverage, ExternalApiCoverage::Semantic);
    assert_eq!(
        variantcontext.coverage_for_constructor("VariantContextBuilder", 0),
        ExternalApiCoverage::Semantic
    );
    assert!(
        variantcontext
            .object_strategies
            .iter()
            .any(|strategy| strategy.type_name == "htsjdk.variant.variantcontext.VariantContext"),
        "object representation policy should be declared in the pack manifest"
    );
    assert!(
        variantcontext.type_capabilities.iter().any(|capability| {
            capability.type_name == "htsjdk.variant.variantcontext.GenotypesContext"
                && capability.sequence_like == Some("htsjdk.variant.variantcontext.Genotype")
        }),
        "collection element shape policy should be declared in the pack manifest"
    );
    assert!(
        variantcontext.argument_effects.iter().any(|effect| {
            effect.receiver_type == "htsjdk.variant.variantcontext.VariantContextBuilder"
                && effect.method_name == "chr"
                && effect.parameter_index == 0
                && effect.parameter_type == Some("String")
        }),
        "argument ownership/nullability policy should be declared in the pack manifest"
    );
    assert!(
        variantcontext.exceptions.iter().any(|exception| {
            exception.type_name == "htsjdk.variant.variantcontext.VariantContextException"
                && exception.kind == ExceptionKind::Unchecked
        }),
        "exception classes should be declared in the pack manifest"
    );
    assert!(
        variantcontext.api_effects.iter().any(|effect| {
            effect.receiver_type == "Allele"
                && effect.method_name == "getBases"
                && effect.role == ExternalApiRole::Getter
                && effect.return_type == Some("byte[]")
                && effect.return_nullability == NullabilityState::DefinitelyNonNull
        }),
        "VariantContext Allele typed getter behavior should be declared in the pack manifest"
    );
    assert!(
        variantcontext.api_effects.iter().any(|effect| {
            effect.receiver_type == "Allele"
                && effect.method_name == "isReference"
                && effect.role == ExternalApiRole::BooleanGetter
                && effect.return_type == Some("boolean")
        }),
        "VariantContext Allele predicate behavior should be declared in the pack manifest"
    );
    assert!(
        variantcontext.api_effects.iter().any(|effect| {
            effect.receiver_type == "VariantContext"
                && effect.method_name == "getAlleles"
                && effect.role == ExternalApiRole::Getter
                && effect.return_type == Some("List<Allele>")
                && effect.return_nullability == NullabilityState::DefinitelyNonNull
        }),
        "VariantContext allele collection getter behavior should be declared in the pack manifest"
    );
    assert!(
        variantcontext.api_effects.iter().any(|effect| {
            effect.receiver_type == "VariantContext"
                && effect.method_name == "getAttributes"
                && effect.return_type == Some("Map<String,Object>")
        }),
        "VariantContext attribute map getter behavior should be declared in the pack manifest"
    );
    assert!(
        variantcontext.api_effects.iter().any(|effect| {
            effect.receiver_type == "VariantContext"
                && effect.method_name == "hasAttribute"
                && effect.role == ExternalApiRole::BooleanGetter
                && effect.return_type == Some("boolean")
        }),
        "VariantContext predicate behavior should be declared in the pack manifest"
    );
    assert!(
        variantcontext.api_effects.iter().any(|effect| {
            effect.receiver_type == "VariantContext"
                && effect.method_name == "getAttribute"
                && effect.return_type == Some("Object")
                && effect.return_nullability == NullabilityState::MaybeNull
        }),
        "VariantContext nullable attribute getter behavior should be declared in the pack manifest"
    );
    assert!(
        variantcontext.api_effects.iter().any(|effect| {
            effect.receiver_type == "VariantContextBuilder"
                && effect.method_name == "chr"
                && effect.role == ExternalApiRole::FluentMutator
                && effect.receiver_mutability == ReceiverMutabilityFact::MutatesReceiver
                && effect.return_type == Some("VariantContextBuilder")
        }),
        "VariantContextBuilder fluent mutator behavior should be declared in the pack manifest"
    );
    assert!(
        variantcontext.api_effects.iter().any(|effect| {
            effect.receiver_type == "VariantContextBuilder"
                && effect.method_name == "make"
                && effect.role == ExternalApiRole::Getter
                && effect.return_type == Some("VariantContext")
        }),
        "VariantContextBuilder make behavior should be declared in the pack manifest"
    );
    assert!(
        variantcontext.api_effects.iter().any(|effect| {
            effect.receiver_type == "GenotypeBuilder"
                && effect.method_name == "DP"
                && effect.role == ExternalApiRole::FluentMutator
                && effect.return_type == Some("GenotypeBuilder")
        }),
        "GenotypeBuilder fluent mutator behavior should be declared in the pack manifest"
    );
    assert!(
        variantcontext.api_effects.iter().any(|effect| {
            effect.receiver_type == "GenotypeBuilder"
                && effect.method_name == "make"
                && effect.role == ExternalApiRole::Getter
                && effect.return_type == Some("Genotype")
        }),
        "GenotypeBuilder make behavior should be declared in the pack manifest"
    );
    assert!(
        variantcontext.api_effects.iter().any(|effect| {
            effect.receiver_type == "Genotype"
                && effect.method_name == "getSampleName"
                && effect.role == ExternalApiRole::Getter
                && effect.return_type == Some("String")
                && effect.return_nullability == NullabilityState::DefinitelyNonNull
        }),
        "Genotype value getter behavior should be declared in the pack manifest"
    );
    assert!(
        variantcontext.api_effects.iter().any(|effect| {
            effect.receiver_type == "Genotype"
                && effect.method_name == "hasAD"
                && effect.role == ExternalApiRole::BooleanGetter
                && effect.return_type == Some("boolean")
        }),
        "Genotype predicate behavior should be declared in the pack manifest"
    );
    assert!(
        variantcontext.api_effects.iter().any(|effect| {
            effect.receiver_type == "GenotypesContext"
                && effect.method_name == "get"
                && effect.role == ExternalApiRole::IndexedGetter
                && effect.return_type == Some("Genotype")
        }),
        "GenotypesContext indexed access behavior should be declared in the pack manifest"
    );
    assert!(
        variantcontext.api_effects.iter().any(|effect| {
            effect.receiver_type == "GenotypesContext"
                && effect.method_name == "stream"
                && effect.role == ExternalApiRole::Getter
                && effect.return_type == Some("Stream<Genotype>")
        }),
        "GenotypesContext stream behavior should be declared in the pack manifest"
    );
    assert!(
        variantcontext.api_effects.iter().any(|effect| {
            effect.receiver_type == "VariantContext"
                && effect.method_name == "getGenotype"
                && effect.return_type == Some("Genotype")
                && effect.return_nullability == NullabilityState::MaybeNull
        }),
        "GATK-shaped VariantContext sample lookup behavior should be declared in the pack manifest"
    );
    assert!(
        variantcontext.api_effects.iter().any(|effect| {
            effect.receiver_type == "VariantContext"
                && effect.method_name == "getAttributeAsInt"
                && effect.return_type == Some("int")
        }),
        "GATK-shaped VariantContext integer attribute getter behavior should be declared in the pack manifest"
    );
    assert!(
        variantcontext.api_effects.iter().any(|effect| {
            effect.receiver_type == "VariantContextBuilder"
                && effect.method_name == "rmAttribute"
                && effect.role == ExternalApiRole::FluentMutator
                && effect.return_type == Some("VariantContextBuilder")
        }),
        "GATK-shaped VariantContextBuilder attribute removal behavior should be declared in the pack manifest"
    );
    assert!(
        variantcontext.api_effects.iter().any(|effect| {
            effect.receiver_type == "Genotype"
                && effect.method_name == "getDP"
                && effect.return_type == Some("int")
        }),
        "GATK-shaped Genotype depth getter behavior should be declared in the pack manifest"
    );
    assert!(
        variantcontext.api_effects.iter().any(|effect| {
            effect.receiver_type == "Genotype"
                && effect.method_name == "getAllele"
                && effect.role == ExternalApiRole::IndexedGetter
                && effect.return_type == Some("Allele")
        }),
        "GATK-shaped Genotype indexed allele getter behavior should be declared in the pack manifest"
    );
    assert!(
        variantcontext.api_effects.iter().any(|effect| {
            effect.receiver_type == "VariantContext"
                && effect.method_name == "getNAlleles"
                && effect.return_type == Some("int")
        }),
        "GATK-shaped VariantContext allele-count getter behavior should be declared in the pack manifest"
    );
    assert!(
        variantcontext.api_effects.iter().any(|effect| {
            effect.receiver_type == "Allele"
                && effect.method_name == "isSymbolic"
                && effect.role == ExternalApiRole::BooleanGetter
        }),
        "GATK-shaped Allele symbolic predicate behavior should be declared in the pack manifest"
    );
    assert!(
        variantcontext.api_effects.iter().any(|effect| {
            effect.receiver_type == "Genotype"
                && effect.method_name == "hasDP"
                && effect.role == ExternalApiRole::BooleanGetter
        }),
        "GATK-shaped Genotype depth predicate behavior should be declared in the pack manifest"
    );
    assert!(
        variantcontext.api_effects.iter().any(|effect| {
            effect.receiver_type == "Genotype"
                && effect.method_name == "getLikelihoods"
                && effect.return_type == Some("GenotypeLikelihoods")
                && effect.return_nullability == NullabilityState::MaybeNull
        }),
        "GATK-shaped Genotype likelihood getter behavior should be declared in the pack manifest"
    );
    assert!(
        variantcontext.api_effects.iter().any(|effect| {
            effect.receiver_type == "GenotypesContext"
                && effect.method_name == "add"
                && effect.role == ExternalApiRole::Mutator
                && effect.receiver_mutability == ReceiverMutabilityFact::MutatesReceiver
                && effect.return_type == Some("boolean")
        }),
        "GATK-shaped GenotypesContext add behavior should be declared in the pack manifest"
    );

    let hdf5 = manifests
        .iter()
        .find(|manifest| manifest.pack_name == "hdf5")
        .expect("hdf5 manifest should exist");
    assert!(hdf5.owns_type("HDF5File.OpenMode"));
    assert_eq!(hdf5.mode, ExternalLibraryMode::RustCrateMapping);
    assert_eq!(hdf5.default_coverage, ExternalApiCoverage::Mapped);
    assert_eq!(hdf5.rust_target, Some("hdf5_pure_rust"));
    assert_eq!(
        hdf5.coverage_for_constructor("HDF5File", 1),
        ExternalApiCoverage::Mapped
    );
    assert_eq!(
        hdf5.coverage_for_field("HDF5File.OpenMode", "CREATE"),
        ExternalApiCoverage::Mapped
    );
    let hdf5_write = hdf5
        .modeled_api("HDF5File", "writeDoubleArray")
        .expect("mapped HDF5 runtime methods should be explicit manifest entries");
    assert_eq!(hdf5_write.coverage, ExternalApiCoverage::Mapped);
    assert!(
        hdf5.api_effects.iter().any(|effect| {
            effect.receiver_type == "HDF5File"
                && effect.method_name == "readDoubleArray"
                && effect.return_type == Some("double[]")
                && effect.thrown == ["HDF5Error"]
        }),
        "crate-mapped method behavior should be declared in the pack manifest"
    );
    let hdf5_facts = hdf5_pack_facts();
    let hdf5_write_mapping = hdf5_facts
        .external_crate_api_mapping_for_receiver("HDF5File", "writeDoubleArray", None)
        .expect("mapped HDF5 method should have a generated facade-symbol fact");
    assert_eq!(hdf5_write_mapping.rust_name, "write_double_array");
    let hdf5_constructor_mapping = hdf5_facts
        .external_crate_constructor_mapping_for("HDF5File", 1)
        .expect("mapped HDF5 constructor should have a generated facade-symbol fact");
    assert_eq!(hdf5_constructor_mapping.rust_name, "new");
    assert!(
        hdf5.field_nullability.iter().any(|field| {
            field.type_name == "HDF5File.OpenMode"
                && field.field_name == "CREATE"
                && field.state == NullabilityState::DefinitelyNonNull
        }),
        "field nullability policy should be declared in the pack manifest"
    );

    let sqlite = manifests
        .iter()
        .find(|manifest| manifest.pack_name == "sqlite")
        .expect("sqlite manifest should exist");
    assert!(sqlite.owns_type("org.sqlite.SQLiteConnection"));
    assert!(sqlite.owns_type("SQLiteResultSet"));
    assert!(sqlite.owns_type("SQLException"));
    assert_eq!(sqlite.rust_target, Some("sqlite"));
    let sqlite_connect = sqlite
        .modeled_api("SQLiteConfig", "createConnection")
        .expect("mapped SQLite facade methods should be explicit manifest entries");
    assert_eq!(sqlite_connect.coverage, ExternalApiCoverage::Mapped);
    let sqlite_execute_query = sqlite
        .modeled_api("SQLiteStatement", "executeQuery")
        .expect("SQLite query execution should be an explicit manifest entry");
    assert_eq!(sqlite_execute_query.coverage, ExternalApiCoverage::Mapped);
    let sqlite_statement_cleanup = sqlite
        .modeled_api("SQLiteStatement", "clearParameters")
        .expect("SQLite statement cleanup should be an explicit manifest entry");
    assert_eq!(
        sqlite_statement_cleanup.coverage,
        ExternalApiCoverage::Mapped
    );
    let sqlite_result_getter = sqlite
        .modeled_api("SQLiteResultSet", "getString")
        .expect("SQLite result-set scalar reads should be explicit manifest entries");
    assert_eq!(sqlite_result_getter.coverage, ExternalApiCoverage::Mapped);
    let sqlite_long_getter = sqlite
        .modeled_api("SQLiteResultSet", "getLong")
        .expect("SQLite long result-set reads should be explicit manifest entries");
    assert_eq!(sqlite_long_getter.coverage, ExternalApiCoverage::Mapped);
    assert!(
        sqlite.api_effects.iter().any(|effect| {
            effect.receiver_type == "SQLiteConfig"
                && effect.method_name == "createConnection"
                && effect.return_type == Some("SQLiteConnection")
                && effect.return_nullability == NullabilityState::DefinitelyNonNull
                && effect.thrown == ["SQLException"]
        }),
        "SQLite crate-mapped method behavior should be declared in the pack manifest"
    );
    assert!(
        sqlite.api_effects.iter().any(|effect| {
            effect.receiver_type == "SQLiteStatement"
                && effect.method_name == "executeQuery"
                && effect.return_type == Some("SQLiteResultSet")
                && effect.return_nullability == NullabilityState::DefinitelyNonNull
                && effect.thrown == ["SQLException"]
        }),
        "SQLite result-set handle creation should be declared in the pack manifest"
    );
    assert!(
        sqlite.api_effects.iter().any(|effect| {
            effect.receiver_type == "SQLiteResultSet"
                && effect.method_name == "getString"
                && effect.return_type == Some("String")
                && effect.return_nullability == NullabilityState::MaybeNull
                && effect.thrown == ["SQLException"]
        }),
        "SQLite nullable string reads should be declared in the pack manifest"
    );
    assert!(
        sqlite.api_effects.iter().any(|effect| {
            effect.receiver_type == "SQLiteResultSet"
                && effect.method_name == "getLong"
                && effect.return_type == Some("long")
                && effect.return_nullability == NullabilityState::DefinitelyNonNull
                && effect.thrown == ["SQLException"]
        }),
        "SQLite long reads should be declared in the pack manifest"
    );
    assert!(
        sqlite.api_effects.iter().any(|effect| {
            effect.receiver_type == "SQLiteResultSet"
                && effect.method_name == "getBoolean"
                && effect.return_type == Some("boolean")
                && effect.return_nullability == NullabilityState::DefinitelyNonNull
                && effect.thrown == ["SQLException"]
        }),
        "SQLite boolean reads should be declared in the pack manifest"
    );
    assert!(
        sqlite
            .exceptions
            .iter()
            .any(|exception| exception.type_name == "SQLException"
                && exception.kind == ExceptionKind::Checked),
        "SQLite facade errors should be declared in the pack manifest"
    );
    let sqlite_facts = sqlite_pack_facts();
    let sqlite_connect_mapping = sqlite_facts
        .external_crate_api_mapping_for_receiver("SQLiteConfig", "createConnection", None)
        .expect("mapped SQLite method should have a generated facade-symbol fact");
    assert_eq!(sqlite_connect_mapping.rust_name, "create_connection");
    let sqlite_bind_mapping = sqlite_facts
        .external_crate_api_mapping_for_receiver("SQLiteStatement", "bindString", None)
        .expect("mapped SQLite bind method should have a generated facade-symbol fact");
    assert_eq!(sqlite_bind_mapping.rust_name, "bind_string");
    let sqlite_clear_mapping = sqlite_facts
        .external_crate_api_mapping_for_receiver("SQLiteStatement", "clearParameters", None)
        .expect(
            "mapped SQLite statement lifecycle method should have a generated facade-symbol fact",
        );
    assert_eq!(sqlite_clear_mapping.rust_name, "clear_parameters");
    let sqlite_get_string_mapping = sqlite_facts
        .external_crate_api_mapping_for_receiver("SQLiteResultSet", "getString", None)
        .expect("mapped SQLite result-set getter should have a generated facade-symbol fact");
    assert_eq!(sqlite_get_string_mapping.rust_name, "get_string");
    let sqlite_get_long_mapping = sqlite_facts
        .external_crate_api_mapping_for_receiver("SQLiteResultSet", "getLong", None)
        .expect("mapped SQLite long getter should have a generated facade-symbol fact");
    assert_eq!(sqlite_get_long_mapping.rust_name, "get_long");
    let sqlite_get_boolean_mapping = sqlite_facts
        .external_crate_api_mapping_for_receiver("SQLiteResultSet", "getBoolean", None)
        .expect("mapped SQLite boolean getter should have a generated facade-symbol fact");
    assert_eq!(sqlite_get_boolean_mapping.rust_name, "get_boolean");
    let sqlite_constructor_mapping = sqlite_facts
        .external_crate_constructor_mapping_for("SQLiteConfig", 0)
        .expect("mapped SQLite constructor should have a generated facade-symbol fact");
    assert_eq!(sqlite_constructor_mapping.rust_name, "new");

    let tribble = manifests
        .iter()
        .find(|manifest| manifest.pack_name == "htsjdk.tribble.feature")
        .expect("tribble feature manifest should exist");
    assert!(
        tribble.api_effects.iter().any(|effect| {
            effect.receiver_type == "NamedFeature"
                && effect.method_name == "getName"
                && effect.return_nullability == NullabilityState::MaybeNull
        }),
        "Tribble feature method behavior should be declared in the pack manifest"
    );

    let vcf = manifests
        .iter()
        .find(|manifest| manifest.pack_name == "htsjdk.variant.vcf")
        .expect("VCF manifest should exist");
    assert!(
        vcf.api_effects.iter().any(|effect| {
            effect.receiver_type == "VCFHeader"
                && effect.method_name == "getGenotypeSamples"
                && effect.role == ExternalApiRole::Getter
                && effect.return_nullability == NullabilityState::DefinitelyNonNull
        }),
        "VCF getter behavior should be declared in the pack manifest"
    );
    assert!(
        vcf.api_effects.iter().any(|effect| {
            effect.receiver_type == "VCFHeader"
                && effect.method_name == "addMetaDataLine"
                && effect.role == ExternalApiRole::Mutator
                && effect.receiver_mutability == ReceiverMutabilityFact::MutatesReceiver
        }),
        "VCF mutator behavior should be declared in the pack manifest"
    );

    let sam_header = manifests
        .iter()
        .find(|manifest| manifest.pack_name == "htsjdk.samtools.header")
        .expect("SAM header manifest should exist");
    assert!(
        sam_header.api_effects.iter().any(|effect| {
            effect.receiver_type == "SAMSequenceDictionary"
                && effect.method_name == "getSequence"
                && effect.role == ExternalApiRole::IndexedGetter
                && effect.return_nullability == NullabilityState::DefinitelyNonNull
        }),
        "SAM header indexed getter behavior should be declared in the pack manifest"
    );
    assert!(
        sam_header.api_effects.iter().any(|effect| {
            effect.receiver_type == "SAMFileHeader"
                && effect.method_name == "setSequenceDictionary"
                && effect.role == ExternalApiRole::Mutator
                && effect.receiver_mutability == ReceiverMutabilityFact::MutatesReceiver
        }),
        "SAM header mutator behavior should be declared in the pack manifest"
    );
    assert!(
        sam_header.api_effects.iter().any(|effect| {
            effect.receiver_type == "SAMReadGroupRecord"
                && effect.method_name == "getSample"
                && effect.role == ExternalApiRole::Getter
                && effect.return_nullability == NullabilityState::MaybeNull
        }),
        "SAM read-group nullable getter behavior should be declared in the pack manifest"
    );

    let samtools_util = manifests
        .iter()
        .find(|manifest| manifest.pack_name == "htsjdk.samtools.util")
        .expect("SAM tools util manifest should exist");
    assert!(
        samtools_util.api_effects.iter().any(|effect| {
            effect.receiver_type == "Interval"
                && effect.method_name == "getName"
                && effect.role == ExternalApiRole::Getter
                && effect.return_nullability == NullabilityState::MaybeNull
        }),
        "SAM tools interval nullable getter behavior should be declared in the pack manifest"
    );
    assert!(
        samtools_util.api_effects.iter().any(|effect| {
            effect.receiver_type == "Interval"
                && effect.method_name == "intersects"
                && effect.role == ExternalApiRole::BooleanGetter
                && effect.return_nullability == NullabilityState::DefinitelyNonNull
        }),
        "SAM tools interval predicate behavior should be declared in the pack manifest"
    );
    assert!(
        samtools_util.api_effects.iter().any(|effect| {
            effect.receiver_type == "PeekableIterator"
                && effect.method_name == "close"
                && effect.role == ExternalApiRole::SideEffect
                && effect.return_nullability == NullabilityState::Unknown
        }),
        "SAM tools iterator close behavior should be declared in the pack manifest"
    );
    assert!(
        samtools_util.api_effects.iter().any(|effect| {
            effect.receiver_type == "PeekableIterator"
                && effect.method_name == "peek"
                && effect.role == ExternalApiRole::Getter
                && effect.return_nullability == NullabilityState::DefinitelyNonNull
        }),
        "SAM tools iterator peek behavior should be declared in the pack manifest"
    );
}

#[test]
fn builtin_external_library_pack_lookup_uses_manifest_boundaries() {
    let variantcontext =
        external_library_pack_for_type("htsjdk.variant.variantcontext.VariantContext")
            .expect("qualified VariantContext type should resolve through manifest ownership");
    assert_eq!(variantcontext.pack_name, "htsjdk.variant.variantcontext");
    assert_eq!(variantcontext.mode, ExternalLibraryMode::ExternalFactModel);

    let matrix = external_library_pack_for_type("Jama.Matrix")
        .expect("qualified Jama Matrix type should resolve through manifest ownership");
    assert_eq!(matrix.pack_name, "Jama");
    assert_eq!(matrix.mode, ExternalLibraryMode::ExternalFactModel);

    let hdf5 = external_library_pack_for_type("HDF5File")
        .expect("simple HDF5 facade type should resolve through manifest ownership");
    assert_eq!(hdf5.pack_name, "hdf5");
    assert_eq!(hdf5.mode, ExternalLibraryMode::RustCrateMapping);

    assert!(
        external_library_pack_for_type("UnownedDependencyType").is_none(),
        "unknown external types must not be claimed by a builtin pack"
    );
}

#[test]
fn builtin_external_member_lookup_uses_cached_pack_facts() {
    let variant_get_id = external_api_effect_for_receiver_with_arity(
        Some("htsjdk.variant.variantcontext.VariantContext"),
        "getID",
        Some(0),
    )
    .expect("VariantContext.getID should resolve through builtin pack member lookup");
    assert_eq!(
        variant_get_id.provenance,
        FactProvenance::ExternalLibraryPack
    );
    assert_eq!(variant_get_id.return_type, Some(JavaType::String));

    let unsupported = unsupported_external_api_for_receiver(
        Some("htsjdk.variant.variantcontext.VariantContext"),
        "fullyDecode",
    )
    .expect("VariantContext.fullyDecode should resolve as unsupported pack coverage");
    assert_eq!(unsupported.pack_name, "htsjdk.variant.variantcontext");

    let hdf5_ctor = external_constructor_for(
        "HDF5File",
        &[
            JavaType::Unknown("_".to_string()),
            JavaType::Unknown("_".to_string()),
        ],
    )
    .expect("HDF5File two-argument constructor should resolve through builtin pack member lookup");
    assert_eq!(hdf5_ctor.provenance, FactProvenance::ExternalLibraryPack);

    let hdf5_ctor_mapping = external_crate_constructor_mapping_for("HDF5File", 2)
        .expect("HDF5 constructor should have a crate mapping");
    assert_eq!(hdf5_ctor_mapping.rust_name, "new");

    let hdf5_snake_ctor = external_constructor_for(
        "h_d_f5_file",
        &[
            JavaType::Unknown("_".to_string()),
            JavaType::Unknown("_".to_string()),
        ],
    )
    .expect("builtin constructor lookup should normalize receiver spellings");
    assert_eq!(hdf5_snake_ctor.type_name, "HDF5File");

    let hdf5_snake_ctor_mapping = external_crate_constructor_mapping_for("h_d_f5_file", 2)
        .expect("builtin crate-constructor lookup should normalize receiver spellings");
    assert_eq!(hdf5_snake_ctor_mapping.rust_name, "new");

    let exists_mapping = external_crate_api_mapping_for_receiver("HDF5File", "exists", Some(1))
        .expect("HDF5 exists should have a crate mapping");
    assert_eq!(exists_mapping.rust_name, "exists");

    let open_mode = external_field_for("HDF5File", "OpenMode")
        .expect("HDF5File.OpenMode should resolve through builtin field lookup");
    assert_eq!(open_mode.provenance, FactProvenance::ExternalLibraryPack);
}

#[test]
fn builtin_external_pack_facts_are_generated_from_manifest_boundaries() {
    let facts = builtin_external_library_packs();

    for manifest in builtin_external_pack_manifests() {
        for owned_type in manifest.owned_type_patterns {
            let pack = facts
                .external_library_pack_for_type(owned_type)
                .unwrap_or_else(|| panic!("{owned_type} should have a generated pack fact"));
            assert_eq!(pack.pack_name, manifest.pack_name);
            assert_eq!(pack.mode, manifest.mode);
            assert_eq!(pack.coverage, manifest.default_coverage);
            assert_eq!(pack.rust_target.as_deref(), manifest.rust_target);
            assert_eq!(pack.provenance, FactProvenance::ExternalLibraryPack);
        }
        for api in manifest.unsupported_apis {
            let unsupported = facts
                .unsupported_external_api_for_receiver(api.receiver_type, api.method_name)
                .unwrap_or_else(|| {
                    panic!(
                        "{}.{} should have a generated unsupported API fact",
                        api.receiver_type, api.method_name
                    )
                });
            assert_eq!(unsupported.pack_name, manifest.pack_name);
            assert_eq!(unsupported.reason, api.reason);
            assert_eq!(unsupported.provenance, FactProvenance::ExternalLibraryPack);
        }
        for strategy in manifest.object_strategies {
            let override_fact = facts
                .hierarchy_overrides
                .iter()
                .find(|fact| fact.type_name == strategy.type_name)
                .unwrap_or_else(|| {
                    panic!(
                        "{} should have a generated object strategy fact from {}",
                        strategy.type_name, manifest.pack_name
                    )
                });
            assert_eq!(
                override_fact.provenance,
                FactProvenance::ExternalLibraryPack
            );
        }
        for capability in manifest.type_capabilities {
            let override_fact = facts
                .type_capability_overrides
                .iter()
                .find(|fact| fact.type_name == capability.type_name)
                .unwrap_or_else(|| {
                    panic!(
                        "{} should have a generated type capability fact from {}",
                        capability.type_name, manifest.pack_name
                    )
                });
            assert_eq!(
                override_fact.provenance,
                FactProvenance::ExternalLibraryPack
            );
        }
        for field in manifest.field_nullability {
            let field_fact = facts
                .field_nullability_facts
                .iter()
                .find(|fact| {
                    fact.field.type_name == field.type_name
                        && fact.field.field_name == field.field_name
                })
                .unwrap_or_else(|| {
                    panic!(
                        "{}.{} should have a generated field nullability fact from {}",
                        field.type_name, field.field_name, manifest.pack_name
                    )
                });
            assert_eq!(field_fact.state, field.state);
            assert_eq!(
                field_fact.provenance,
                FieldNullabilityProvenance::ExternalLibraryPack
            );
        }
        for argument in manifest.argument_effects {
            let argument_fact = facts
                .external_api_argument_effect_for_receiver(
                    Some(argument.receiver_type),
                    argument.method_name,
                    argument.parameter_index,
                )
                .unwrap_or_else(|| {
                    panic!(
                        "{}.{} argument {} should have a generated argument effect fact from {}",
                        argument.receiver_type,
                        argument.method_name,
                        argument.parameter_index,
                        manifest.pack_name
                    )
                });
            assert_eq!(
                argument_fact.provenance,
                FactProvenance::ExternalLibraryPack
            );
        }
        for exception in manifest.exceptions {
            let exception_fact = facts
                .exception_facts
                .iter()
                .find(|fact| fact.type_name == exception.type_name)
                .unwrap_or_else(|| {
                    panic!(
                        "{} should have a generated exception fact from {}",
                        exception.type_name, manifest.pack_name
                    )
                });
            assert_eq!(exception_fact.kind, exception.kind);
            assert_eq!(
                exception_fact.provenance,
                FactProvenance::ExternalLibraryPack
            );
        }
        for effect in manifest.api_effects {
            let effect_fact = facts
                .external_api_effect_for_receiver_with_arity(
                    Some(effect.receiver_type),
                    effect.method_name,
                    effect.arg_count,
                )
                .unwrap_or_else(|| {
                    panic!(
                        "{}.{} should have a generated API effect fact from {}",
                        effect.receiver_type, effect.method_name, manifest.pack_name
                    )
                });
            assert_eq!(effect_fact.role, effect.role);
            assert_eq!(effect_fact.receiver_mutability, effect.receiver_mutability);
            assert_eq!(effect_fact.return_nullability, effect.return_nullability);
            assert_eq!(effect_fact.provenance, FactProvenance::ExternalLibraryPack);
        }
        for constructor in manifest.modeled_constructors {
            if constructor.coverage == ExternalApiCoverage::Unsupported {
                let unsupported = facts
                    .unsupported_external_constructor_for(constructor.type_name, constructor.arg_count)
                    .unwrap_or_else(|| {
                        panic!(
                            "{} constructor with {} argument(s) should have a generated unsupported constructor fact from {}",
                            constructor.type_name,
                            constructor.arg_count,
                            manifest.pack_name
                        )
                    });
                assert_eq!(unsupported.pack_name, manifest.pack_name);
                assert_eq!(unsupported.provenance, FactProvenance::ExternalLibraryPack);
                continue;
            }
            let constructor_fact = facts
                .external_constructor_facts
                .iter()
                .find(|fact| {
                    fact.type_name == constructor.type_name
                        && fact.arg_types.len() == constructor.arg_count
                })
                .unwrap_or_else(|| {
                    panic!(
                        "{} constructor with {} argument(s) should have a generated constructor fact from {}",
                        constructor.type_name,
                        constructor.arg_count,
                        manifest.pack_name
                    )
                });
            assert_eq!(
                constructor_fact.provenance,
                FactProvenance::ExternalLibraryPack
            );
        }
        for field in manifest.modeled_fields {
            if field.coverage == ExternalApiCoverage::Unsupported {
                let unsupported = facts
                    .unsupported_external_field_for(field.type_name, field.field_name)
                    .unwrap_or_else(|| {
                        panic!(
                            "{}.{} should have a generated unsupported field fact from {}",
                            field.type_name, field.field_name, manifest.pack_name
                        )
                    });
                assert_eq!(unsupported.pack_name, manifest.pack_name);
                assert_eq!(unsupported.provenance, FactProvenance::ExternalLibraryPack);
                continue;
            }
            let field_fact = facts
                .external_field_facts
                .iter()
                .find(|fact| {
                    fact.field.type_name == field.type_name
                        && fact.field.field_name == field.field_name
                })
                .unwrap_or_else(|| {
                    panic!(
                        "{}.{} should have a generated field fact from {}",
                        field.type_name, field.field_name, manifest.pack_name
                    )
                });
            assert_eq!(field_fact.provenance, FactProvenance::ExternalLibraryPack);
        }
    }
}

#[test]
fn builtin_external_pack_facts_can_be_selected_by_pack_name() {
    let htsjdk = builtin_external_pack_facts("htsjdk.variant.variantcontext")
        .expect("variantcontext pack should be selectable");
    assert!(
        htsjdk
            .external_library_pack_for_type("htsjdk.variant.variantcontext.VariantContext")
            .is_some()
    );
    assert!(
        htsjdk
            .external_api_effect_for_receiver(
                Some("htsjdk.variant.variantcontext.VariantContext"),
                "getContig"
            )
            .is_some()
    );

    let hdf5 = builtin_external_pack_facts("hdf5").expect("hdf5 pack should be selectable");
    assert_eq!(
        hdf5.external_library_pack_for_type("HDF5File")
            .unwrap()
            .mode,
        ExternalLibraryMode::RustCrateMapping
    );

    assert!(builtin_external_pack_facts("missing-pack").is_none());
}

#[test]
fn builtin_representation_facts_can_be_looked_up_without_loading_api_coverage() {
    let facts = representation_facts_for_type("htsjdk.variant.variantcontext.VariantContext");

    assert!(
        facts
            .external_library_pack_for_type("htsjdk.variant.variantcontext.VariantContext")
            .is_some(),
        "representation lookup should still include the pack boundary row"
    );
    assert!(
        facts.hierarchy_overrides.iter().any(|fact| {
            fact.type_name == "htsjdk.variant.variantcontext.VariantContext"
                && fact.provenance == FactProvenance::ExternalLibraryPack
        }),
        "representation lookup should expose object strategy facts for the target type"
    );
    assert!(
        facts.external_api_effect_facts.is_empty()
            && facts.external_constructor_facts.is_empty()
            && facts.external_field_facts.is_empty(),
        "representation lookup must not clone full method/constructor/field API coverage"
    );

    let missing = representation_facts_for_type("UnownedDependencyType");
    assert!(missing.external_library_packs.is_empty());
    assert!(missing.hierarchy_overrides.is_empty());
    assert!(missing.type_capability_overrides.is_empty());
}

#[test]
fn builtin_external_pack_manifests_can_be_selected_by_pack_or_provider_name() {
    let exact = builtin_external_pack_manifest("hdf5").expect("hdf5 manifest should be selectable");
    assert_eq!(exact.pack_name, "hdf5");
    assert_eq!(exact.mode, ExternalLibraryMode::RustCrateMapping);

    let provider = builtin_external_pack_manifest_for_provider_name("builtin-pack:hdf5")
        .expect("builtin-pack provider names should resolve through external_packs");
    assert_eq!(provider.pack_name, exact.pack_name);
    assert_eq!(provider.rust_target, exact.rust_target);

    let direct_provider =
        builtin_external_pack_manifest_for_provider_name("htsjdk.variant.variantcontext")
            .expect("direct built-in provider names should resolve through external_packs");
    assert_eq!(direct_provider.pack_name, "htsjdk.variant.variantcontext");

    assert!(builtin_external_pack_manifest("missing-pack").is_none());
    assert!(builtin_external_pack_manifest_for_provider_name("manifest:/tmp/pack.txt").is_none());
}

#[test]
fn builtin_pack_manifests_explicitly_classify_modeled_member_facts() {
    let manifests = builtin_external_pack_manifests();

    for manifest in &manifests {
        let pack_name = manifest.pack_name;
        let facts = builtin_external_pack_facts(pack_name)
            .unwrap_or_else(|| panic!("{pack_name} facts should be selectable by manifest name"));
        let audit = audit_external_pack_manifest_coverage(manifest, &facts);
        assert!(
            audit.is_empty(),
            "{pack_name} manifest/fact coverage audit should be clean: {:?}",
            audit
        );
        let boundary_audit = audit_external_pack_fact_boundaries(&facts);
        assert!(
            boundary_audit.is_empty(),
            "{pack_name} manifest/fact boundary audit should be clean: {:?}",
            boundary_audit
        );
        assert!(
            !manifest.modeled_apis.is_empty()
                || !manifest.modeled_constructors.is_empty()
                || !manifest.modeled_fields.is_empty(),
            "{pack_name} should not rely only on pack-level default coverage"
        );

        for effect in facts.external_api_effect_facts.iter().filter(|effect| {
            effect.provenance == FactProvenance::ExternalLibraryPack
                && effect.receiver_type.is_some()
        }) {
            let receiver = effect.receiver_type.as_deref().unwrap();
            let modeled = manifest
                .modeled_api(receiver, &effect.method_name)
                .unwrap_or_else(|| {
                    panic!(
                        "{}.{} in {pack_name} has a pack-owned effect fact but no manifest coverage entry",
                        receiver, effect.method_name
                    )
            });
            assert_ne!(modeled.coverage, ExternalApiCoverage::Unsupported);
        }

        for constructor in facts
            .external_constructor_facts
            .iter()
            .filter(|constructor| constructor.provenance == FactProvenance::ExternalLibraryPack)
        {
            let modeled = manifest
                .modeled_constructor(&constructor.type_name, constructor.arg_types.len())
                .unwrap_or_else(|| {
                    panic!(
                        "{} constructor with {} argument(s) in {pack_name} has a pack-owned constructor fact but no manifest coverage entry",
                        constructor.type_name,
                        constructor.arg_types.len()
                    )
                });
            assert_ne!(modeled.coverage, ExternalApiCoverage::Unsupported);
        }

        for field in facts
            .external_field_facts
            .iter()
            .filter(|field| field.provenance == FactProvenance::ExternalLibraryPack)
        {
            let modeled = manifest
                .modeled_field(&field.field.type_name, &field.field.field_name)
                .unwrap_or_else(|| {
                    panic!(
                        "{}.{} in {pack_name} has a pack-owned field fact but no manifest coverage entry",
                        field.field.type_name, field.field.field_name
                    )
                });
            assert_ne!(modeled.coverage, ExternalApiCoverage::Unsupported);
        }
    }
}

#[test]
fn pack_owned_domain_getters_take_precedence_over_generic_stdlib_provenance() {
    for (receiver, method) in [
        ("SAMRecord", "getReadName"),
        ("SAMRecord", "getStart"),
        ("ReferenceSequence", "getBases"),
        ("Feature", "getContig"),
        ("Locatable", "getStart"),
    ] {
        let pack_fact =
            new_java2rust::external_packs::external_api_effect_for_receiver(Some(receiver), method)
                .unwrap_or_else(|| panic!("{receiver}.{method} should be pack-dispatched"));
        assert_eq!(pack_fact.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(
            new_java2rust::stdlib::external_api_method_policy(Some(receiver), method, 0).provenance,
            FactProvenance::ConservativeFallback,
            "{receiver}.{method} should be owned by its external pack, not generic stdlib"
        );
    }

    assert_eq!(
        new_java2rust::stdlib::external_api_method_policy(
            Some("UnlinkedReferenceSequence"),
            "getBases",
            0,
        )
        .provenance,
        FactProvenance::BuiltinExternalModel,
        "unlinked libraries should keep reusable stdlib getter inference"
    );
}

#[test]
fn pack_owned_fields_and_constructors_take_precedence_over_generic_stdlib_provenance() {
    for (type_name, field_name) in [
        ("SamReaderFactory.Option", "CACHE_FILE_BASED_INDEXES"),
        ("QueryJobConfiguration.Priority", "BATCH"),
        ("HDF5File.OpenMode", "READ_ONLY"),
        ("DataFormat", "AVRO"),
        ("VCFHeader", "REFERENCE_KEY"),
    ] {
        let pack_fact = new_java2rust::external_packs::external_field_for(type_name, field_name)
            .unwrap_or_else(|| panic!("{type_name}.{field_name} should be pack-dispatched"));
        assert_eq!(pack_fact.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(
            new_java2rust::stdlib::external_field_provenance(type_name, field_name),
            FactProvenance::ConservativeFallback,
            "{type_name}.{field_name} should be owned by its external pack, not generic stdlib"
        );
    }

    for (type_name, args) in [
        ("VariantContextBuilder", Vec::new()),
        (
            "HDF5File",
            vec![JavaType::Named {
                name: "File".to_string(),
                args: Vec::new(),
            }],
        ),
        ("SQLiteConfig", Vec::new()),
    ] {
        let pack_fact = new_java2rust::external_packs::external_constructor_for(type_name, &args)
            .unwrap_or_else(|| panic!("{type_name}/{} should be pack-dispatched", args.len()));
        assert_eq!(pack_fact.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(
            new_java2rust::stdlib::external_constructor_provenance(type_name, &args),
            FactProvenance::ConservativeFallback,
            "{type_name}/{} should be owned by its external pack, not generic stdlib",
            args.len()
        );
    }

    for (type_name, field_name) in [
        ("Option", "CACHE_FILE_BASED_INDEXES"),
        ("Attribute", "PRIMITIVE"),
        ("Priority", "BATCH"),
    ] {
        assert!(
            new_java2rust::external_packs::external_field_for(type_name, field_name).is_none(),
            "ambiguous bare {type_name}.{field_name} should not be claimed by a narrow pack"
        );
        assert_eq!(
            new_java2rust::stdlib::external_field_provenance(type_name, field_name),
            FactProvenance::BuiltinExternalModel,
            "ambiguous bare {type_name}.{field_name} should keep shared stdlib fallback"
        );
    }

    assert_eq!(
        new_java2rust::stdlib::external_constructor_provenance("URL", &[JavaType::String],),
        FactProvenance::BuiltinExternalModel,
        "JDK constructors that are not pack-owned should keep stdlib provenance"
    );
}

#[test]
fn external_pack_manifest_coverage_audit_reports_unclassified_pack_facts() {
    let manifest = ExternalPackManifest {
        pack_name: "dep.audit",
        owned_type_patterns: &["dep.audit"],
        mode: ExternalLibraryMode::ExternalFactModel,
        default_coverage: ExternalApiCoverage::Conservative,
        rust_target: None,
        object_strategies: &[],
        closed_hierarchy: &[ExternalPackClosedHierarchy {
            type_name: "dep.audit.KnownClosed",
            closed: true,
        }],
        object_safety: &[ExternalPackObjectSafety {
            type_name: "dep.audit.KnownObjectSafety",
            violation: ExternalPackObjectSafetyViolation::SelfReferentialReturn {
                method_name: "copy",
            },
        }],
        type_capabilities: &[],
        field_nullability: &[],
        argument_effects: &[],
        exceptions: &[new_java2rust::external_packs::ExternalPackException {
            type_name: "dep.audit.KnownException",
            kind: ExceptionKind::Checked,
        }],
        api_effects: &[ExternalPackApiEffect {
            receiver_type: "dep.audit.Record",
            method_name: "known",
            arg_count: None,
            rust_name: None,
            role: ExternalApiRole::Getter,
            receiver_mutability: ReceiverMutabilityFact::ReadOnly,
            return_type: Some("String"),
            return_nullability: NullabilityState::DefinitelyNonNull,
            thrown: &[],
        }],
        modeled_apis: &[
            ExternalPackModeledApi {
                receiver_type: "dep.audit.Record",
                method_name: "known",
                coverage: ExternalApiCoverage::Semantic,
            },
            ExternalPackModeledApi {
                receiver_type: "dep.audit.Record",
                method_name: "unsupported",
                coverage: ExternalApiCoverage::Unsupported,
            },
            ExternalPackModeledApi {
                receiver_type: "dep.audit.Record",
                method_name: "noEffectRow",
                coverage: ExternalApiCoverage::Semantic,
            },
        ],
        modeled_constructors: &[
            ExternalPackModeledConstructor {
                type_name: "dep.audit.Record",
                arg_count: 0,
                rust_type_name: None,
                rust_name: None,
                coverage: ExternalApiCoverage::Semantic,
            },
            ExternalPackModeledConstructor {
                type_name: "dep.audit.NativeHandle",
                arg_count: 0,
                rust_type_name: None,
                rust_name: None,
                coverage: ExternalApiCoverage::Unsupported,
            },
        ],
        modeled_fields: &[
            ExternalPackModeledField {
                type_name: "dep.audit.Record",
                field_name: "KNOWN",
                coverage: ExternalApiCoverage::Semantic,
            },
            ExternalPackModeledField {
                type_name: "dep.audit.Record",
                field_name: "RAW_HANDLE",
                coverage: ExternalApiCoverage::Unsupported,
            },
        ],
        unsupported_apis: &[],
    };
    let mut facts = FactSet::default();
    facts.external_api_effect_facts.push(ExternalApiEffectFact {
        method_name: "missing".to_string(),
        receiver_type: Some("dep.audit.Record".to_string()),
        arg_count: None,
        role: ExternalApiRole::Getter,
        receiver_mutability: ReceiverMutabilityFact::ReadOnly,
        return_type: None,
        return_nullability: NullabilityState::Unknown,
        thrown: Vec::new(),
        provenance: FactProvenance::ExternalLibraryPack,
    });
    facts.external_api_effect_facts.push(ExternalApiEffectFact {
        method_name: "known".to_string(),
        receiver_type: Some("dep.audit.Record".to_string()),
        arg_count: None,
        role: ExternalApiRole::Getter,
        receiver_mutability: ReceiverMutabilityFact::ReadOnly,
        return_type: Some(JavaType::String),
        return_nullability: NullabilityState::MaybeNull,
        thrown: Vec::new(),
        provenance: FactProvenance::ExternalLibraryPack,
    });
    facts.external_api_effect_facts.push(ExternalApiEffectFact {
        method_name: "noEffectRow".to_string(),
        receiver_type: Some("dep.audit.Record".to_string()),
        arg_count: None,
        role: ExternalApiRole::Getter,
        receiver_mutability: ReceiverMutabilityFact::ReadOnly,
        return_type: None,
        return_nullability: NullabilityState::Unknown,
        thrown: Vec::new(),
        provenance: FactProvenance::ExternalLibraryPack,
    });
    facts
        .external_api_argument_effect_facts
        .push(ExternalApiArgumentEffectFact {
            method_name: "missing".to_string(),
            receiver_type: Some("dep.audit.Record".to_string()),
            parameter_index: 0,
            parameter_type: Some(JavaType::String),
            parameter_nullability: NullabilityState::DefinitelyNonNull,
            argument_mutability: ReceiverMutabilityFact::ReadOnly,
            alias_policy: ExternalArgumentAliasPolicy::Borrowed,
            provenance: FactProvenance::ExternalLibraryPack,
        });
    facts
        .external_api_argument_effect_facts
        .push(ExternalApiArgumentEffectFact {
            method_name: "unsupported".to_string(),
            receiver_type: Some("dep.audit.Record".to_string()),
            parameter_index: 0,
            parameter_type: Some(JavaType::String),
            parameter_nullability: NullabilityState::DefinitelyNonNull,
            argument_mutability: ReceiverMutabilityFact::ReadOnly,
            alias_policy: ExternalArgumentAliasPolicy::Borrowed,
            provenance: FactProvenance::ExternalLibraryPack,
        });
    facts
        .external_constructor_facts
        .push(ExternalConstructorFact {
            type_name: "dep.audit.Record".to_string(),
            arg_types: vec![JavaType::String],
            provenance: FactProvenance::ExternalLibraryPack,
        });
    facts.external_field_facts.push(ExternalFieldFact {
        field: FieldKey {
            type_name: "dep.audit.Record".to_string(),
            field_name: "MISSING".to_string(),
        },
        field_type: None,
        provenance: FactProvenance::ExternalLibraryPack,
    });
    facts
        .unsupported_external_api_facts
        .push(UnsupportedExternalApiFact {
            pack_name: "dep.audit".to_string(),
            receiver_type: "dep.audit.Record".to_string(),
            method_name: "unsupported".to_string(),
            reason: "stale reason".to_string(),
            provenance: FactProvenance::ExternalLibraryPack,
        });
    facts
        .unsupported_external_api_facts
        .push(UnsupportedExternalApiFact {
            pack_name: "dep.audit".to_string(),
            receiver_type: "dep.audit.Record".to_string(),
            method_name: "missingUnsupported".to_string(),
            reason: "stale reason".to_string(),
            provenance: FactProvenance::ExternalLibraryPack,
        });
    facts
        .unsupported_external_constructor_facts
        .push(UnsupportedExternalConstructorFact {
            pack_name: "dep.audit".to_string(),
            type_name: "dep.audit.NativeHandle".to_string(),
            arg_count: 0,
            reason: "stale reason".to_string(),
            provenance: FactProvenance::ExternalLibraryPack,
        });
    facts
        .unsupported_external_constructor_facts
        .push(UnsupportedExternalConstructorFact {
            pack_name: "dep.audit".to_string(),
            type_name: "dep.audit.UnmodeledNativeHandle".to_string(),
            arg_count: 0,
            reason: "native handle".to_string(),
            provenance: FactProvenance::ExternalLibraryPack,
        });
    facts
        .unsupported_external_field_facts
        .push(UnsupportedExternalFieldFact {
            pack_name: "dep.audit".to_string(),
            field: FieldKey {
                type_name: "dep.audit.Record".to_string(),
                field_name: "RAW_HANDLE".to_string(),
            },
            reason: "stale reason".to_string(),
            provenance: FactProvenance::ExternalLibraryPack,
        });
    facts
        .unsupported_external_field_facts
        .push(UnsupportedExternalFieldFact {
            pack_name: "dep.audit".to_string(),
            field: FieldKey {
                type_name: "dep.audit.Record".to_string(),
                field_name: "UNMODELED_RAW_HANDLE".to_string(),
            },
            reason: "native handle".to_string(),
            provenance: FactProvenance::ExternalLibraryPack,
        });
    facts.hierarchy_overrides.push(HierarchyOverride {
        type_name: "dep.audit.UnmodeledObject".to_string(),
        strategy: HierarchyStrategy::ErasedObject,
        provenance: FactProvenance::ExternalLibraryPack,
    });
    facts.closed_hierarchy_facts.push(ClosedHierarchyFact {
        type_name: "dep.audit.UnmodeledClosed".to_string(),
        closed: true,
        provenance: FactProvenance::ExternalLibraryPack,
    });
    facts.closed_hierarchy_facts.push(ClosedHierarchyFact {
        type_name: "dep.audit.KnownClosed".to_string(),
        closed: false,
        provenance: FactProvenance::ExternalLibraryPack,
    });
    facts.object_safety_facts.push(ObjectSafetyFact {
        type_name: "dep.audit.UnmodeledObjectSafety".to_string(),
        object_safe: false,
        violations: vec![ObjectSafetyViolation::UnsupportedMember],
        provenance: FactProvenance::ExternalLibraryPack,
    });
    facts.object_safety_facts.push(ObjectSafetyFact {
        type_name: "dep.audit.KnownObjectSafety".to_string(),
        object_safe: false,
        violations: vec![ObjectSafetyViolation::NestedSelfReferentialReturn {
            method_name: "copyAll".to_string(),
        }],
        provenance: FactProvenance::ExternalLibraryPack,
    });
    facts
        .type_capability_overrides
        .push(TypeCapabilityOverride {
            type_name: "dep.audit.UnmodeledCapability".to_string(),
            copy: None,
            clone: Some(true),
            string_like: None,
            sequence_like: None,
            implements: Vec::new(),
            extends: None,
            final_type: Some(true),
            object_safe_trait: None,
            provenance: FactProvenance::ExternalLibraryPack,
        });
    facts.field_nullability_facts.push(FieldNullabilityFact {
        field: FieldKey {
            type_name: "dep.audit.Record".to_string(),
            field_name: "unmodeledNullability".to_string(),
        },
        state: NullabilityState::DefinitelyNonNull,
        provenance: FieldNullabilityProvenance::ExternalLibraryPack,
    });
    facts.exception_facts.push(ExceptionFact {
        type_name: "dep.audit.MissingException".to_string(),
        kind: ExceptionKind::Checked,
        provenance: FactProvenance::ExternalLibraryPack,
    });
    facts.exception_facts.push(ExceptionFact {
        type_name: "dep.audit.KnownException".to_string(),
        kind: ExceptionKind::Unchecked,
        provenance: FactProvenance::ExternalLibraryPack,
    });
    facts
        .external_crate_api_mapping_facts
        .push(ExternalCrateApiMappingFact {
            receiver_type: "dep.audit.Record".to_string(),
            method_name: "known".to_string(),
            arg_count: None,
            rust_name: "known".to_string(),
            provenance: FactProvenance::ExternalLibraryPack,
        });

    let audit = audit_external_pack_manifest_coverage(&manifest, &facts);
    assert!(audit.has_code("external_pack_manifest_missing_api_coverage"));
    assert!(audit.has_code("external_pack_manifest_api_effect_mismatch"));
    assert!(audit.has_code("external_pack_manifest_missing_api_effect_entry"));
    assert!(audit.has_code("external_pack_manifest_missing_argument_effect_coverage"));
    assert!(audit.has_code("external_pack_manifest_unsupported_argument_effect_emitted"));
    assert!(audit.has_code("external_pack_manifest_missing_constructor_coverage"));
    assert!(audit.has_code("external_pack_manifest_missing_field_coverage"));
    assert!(audit.has_code("external_pack_manifest_missing_unsupported_api_coverage"));
    assert!(audit.has_code("external_pack_manifest_unsupported_api_mismatch"));
    assert!(audit.has_code("external_pack_manifest_missing_unsupported_constructor_coverage"));
    assert!(audit.has_code("external_pack_manifest_unsupported_constructor_mismatch"));
    assert!(audit.has_code("external_pack_manifest_missing_unsupported_field_coverage"));
    assert!(audit.has_code("external_pack_manifest_unsupported_field_mismatch"));
    assert!(audit.has_code("external_pack_manifest_missing_object_strategy_coverage"));
    assert!(audit.has_code("external_pack_manifest_missing_closed_hierarchy_coverage"));
    assert!(audit.has_code("external_pack_manifest_closed_hierarchy_mismatch"));
    assert!(audit.has_code("external_pack_manifest_missing_object_safety_coverage"));
    assert!(audit.has_code("external_pack_manifest_object_safety_mismatch"));
    assert!(audit.has_code("external_pack_manifest_missing_type_capability_coverage"));
    assert!(audit.has_code("external_pack_manifest_missing_field_nullability_coverage"));
    assert!(audit.has_code("external_pack_manifest_missing_exception_coverage"));
    assert!(audit.has_code("external_pack_manifest_exception_kind_mismatch"));
    assert!(audit.has_code("external_pack_manifest_unexpected_crate_api_mapping"));
}

#[test]
fn external_pack_manifest_coverage_audits_crate_mapping_facts() {
    let manifest = ExternalPackManifest {
        pack_name: "dep.crate",
        owned_type_patterns: &["dep.crate"],
        mode: ExternalLibraryMode::RustCrateMapping,
        default_coverage: ExternalApiCoverage::Mapped,
        rust_target: Some("dep_crate"),
        object_strategies: &[],
        closed_hierarchy: &[],
        object_safety: &[],
        type_capabilities: &[],
        field_nullability: &[],
        argument_effects: &[],
        exceptions: &[],
        api_effects: &[ExternalPackApiEffect {
            receiver_type: "dep.crate.Reader",
            method_name: "readRecord",
            arg_count: Some(1),
            rust_name: Some("read_record"),
            role: ExternalApiRole::Getter,
            receiver_mutability: ReceiverMutabilityFact::ReadOnly,
            return_type: Some("dep.crate.Record"),
            return_nullability: NullabilityState::MaybeNull,
            thrown: &[],
        }],
        modeled_apis: &[
            ExternalPackModeledApi {
                receiver_type: "dep.crate.Reader",
                method_name: "readRecord",
                coverage: ExternalApiCoverage::Mapped,
            },
            ExternalPackModeledApi {
                receiver_type: "dep.crate.Reader",
                method_name: "noEffectRow",
                coverage: ExternalApiCoverage::Mapped,
            },
            ExternalPackModeledApi {
                receiver_type: "dep.crate.Reader",
                method_name: "nativeOnly",
                coverage: ExternalApiCoverage::Unsupported,
            },
        ],
        modeled_constructors: &[
            ExternalPackModeledConstructor {
                type_name: "dep.crate.Reader",
                arg_count: 0,
                rust_type_name: Some("Reader"),
                rust_name: Some("open"),
                coverage: ExternalApiCoverage::Mapped,
            },
            ExternalPackModeledConstructor {
                type_name: "dep.crate.NativeOnly",
                arg_count: 1,
                rust_type_name: None,
                rust_name: None,
                coverage: ExternalApiCoverage::Unsupported,
            },
        ],
        modeled_fields: &[],
        unsupported_apis: &[],
    };
    let mut facts = FactSet::default();
    facts
        .external_crate_api_mapping_facts
        .push(ExternalCrateApiMappingFact {
            receiver_type: "dep.crate.Reader".to_string(),
            method_name: "readRecord".to_string(),
            arg_count: Some(1),
            rust_name: "wrong_read".to_string(),
            provenance: FactProvenance::ExternalLibraryPack,
        });
    facts
        .external_crate_api_mapping_facts
        .push(ExternalCrateApiMappingFact {
            receiver_type: "dep.crate.Reader".to_string(),
            method_name: "noEffectRow".to_string(),
            arg_count: Some(0),
            rust_name: "no_effect_row".to_string(),
            provenance: FactProvenance::ExternalLibraryPack,
        });
    facts
        .external_crate_api_mapping_facts
        .push(ExternalCrateApiMappingFact {
            receiver_type: "dep.crate.Reader".to_string(),
            method_name: "nativeOnly".to_string(),
            arg_count: None,
            rust_name: "native_only".to_string(),
            provenance: FactProvenance::ExternalLibraryPack,
        });
    facts
        .external_crate_api_mapping_facts
        .push(ExternalCrateApiMappingFact {
            receiver_type: "dep.crate.Reader".to_string(),
            method_name: "missing".to_string(),
            arg_count: None,
            rust_name: "missing".to_string(),
            provenance: FactProvenance::ExternalLibraryPack,
        });
    facts
        .external_crate_constructor_mapping_facts
        .push(ExternalCrateConstructorMappingFact {
            type_name: "dep.crate.Reader".to_string(),
            arg_count: 0,
            rust_type_name: Some("Reader".to_string()),
            rust_name: "wrong_open".to_string(),
            provenance: FactProvenance::ExternalLibraryPack,
        });
    facts
        .external_crate_constructor_mapping_facts
        .push(ExternalCrateConstructorMappingFact {
            type_name: "dep.crate.NativeOnly".to_string(),
            arg_count: 1,
            rust_type_name: None,
            rust_name: "new".to_string(),
            provenance: FactProvenance::ExternalLibraryPack,
        });
    facts
        .external_crate_constructor_mapping_facts
        .push(ExternalCrateConstructorMappingFact {
            type_name: "dep.crate.Reader".to_string(),
            arg_count: 2,
            rust_type_name: None,
            rust_name: "from_parts".to_string(),
            provenance: FactProvenance::ExternalLibraryPack,
        });

    let audit = audit_external_pack_manifest_coverage(&manifest, &facts);
    assert!(audit.has_code("external_pack_manifest_crate_api_mapping_mismatch"));
    assert!(audit.has_code("external_pack_manifest_missing_crate_api_mapping_entry"));
    assert!(audit.has_code("external_pack_manifest_unsupported_crate_api_mapping_emitted"));
    assert!(audit.has_code("external_pack_manifest_missing_crate_api_mapping_coverage"));
    assert!(audit.has_code("external_pack_manifest_crate_constructor_mapping_mismatch"));
    assert!(audit.has_code("external_pack_manifest_unsupported_crate_constructor_mapping_emitted"));
    assert!(audit.has_code("external_pack_manifest_missing_crate_constructor_mapping_coverage"));
}

#[test]
fn external_pack_fact_boundary_audit_reports_unowned_manifest_members() {
    let mut facts = FactSet::default();
    facts
        .external_library_packs
        .push(new_java2rust::facts::ExternalLibraryPackFact {
            pack_name: "dep.boundary".to_string(),
            package_prefix: "dep.boundary".to_string(),
            mode: ExternalLibraryMode::ExternalFactModel,
            coverage: ExternalApiCoverage::Semantic,
            rust_target: None,
            provenance: FactProvenance::ExternalLibraryPack,
        });
    facts.external_api_effect_facts.push(ExternalApiEffectFact {
        method_name: "read".to_string(),
        receiver_type: Some("other.Reader".to_string()),
        arg_count: None,
        role: ExternalApiRole::Getter,
        receiver_mutability: ReceiverMutabilityFact::ReadOnly,
        return_type: None,
        return_nullability: NullabilityState::Unknown,
        thrown: Vec::new(),
        provenance: FactProvenance::ExternalLibraryPack,
    });
    facts
        .external_api_argument_effect_facts
        .push(ExternalApiArgumentEffectFact {
            method_name: "read".to_string(),
            receiver_type: Some("other.Reader".to_string()),
            parameter_index: 0,
            parameter_type: Some(JavaType::String),
            parameter_nullability: NullabilityState::DefinitelyNonNull,
            argument_mutability: ReceiverMutabilityFact::ReadOnly,
            alias_policy: ExternalArgumentAliasPolicy::Borrowed,
            provenance: FactProvenance::ExternalLibraryPack,
        });
    facts
        .external_constructor_facts
        .push(ExternalConstructorFact {
            type_name: "other.Reader".to_string(),
            arg_types: Vec::new(),
            provenance: FactProvenance::ExternalLibraryPack,
        });
    facts.external_field_facts.push(ExternalFieldFact {
        field: FieldKey {
            type_name: "other.Reader".to_string(),
            field_name: "RAW".to_string(),
        },
        field_type: None,
        provenance: FactProvenance::ExternalLibraryPack,
    });
    facts.hierarchy_overrides.push(HierarchyOverride {
        type_name: "other.ReaderObject".to_string(),
        strategy: HierarchyStrategy::ErasedObject,
        provenance: FactProvenance::ExternalLibraryPack,
    });
    facts.closed_hierarchy_facts.push(ClosedHierarchyFact {
        type_name: "other.ReaderClosed".to_string(),
        closed: true,
        provenance: FactProvenance::ExternalLibraryPack,
    });
    facts.object_safety_facts.push(ObjectSafetyFact {
        type_name: "other.ReaderObjectSafety".to_string(),
        object_safe: false,
        violations: vec![ObjectSafetyViolation::UnsupportedMember],
        provenance: FactProvenance::ExternalLibraryPack,
    });
    facts
        .type_capability_overrides
        .push(TypeCapabilityOverride {
            type_name: "other.ReaderCapability".to_string(),
            copy: None,
            clone: Some(true),
            string_like: None,
            sequence_like: None,
            implements: Vec::new(),
            extends: None,
            final_type: Some(true),
            object_safe_trait: None,
            provenance: FactProvenance::ExternalLibraryPack,
        });
    facts.field_nullability_facts.push(FieldNullabilityFact {
        field: FieldKey {
            type_name: "other.Reader".to_string(),
            field_name: "name".to_string(),
        },
        state: NullabilityState::DefinitelyNonNull,
        provenance: FieldNullabilityProvenance::ExternalLibraryPack,
    });
    facts.exception_facts.push(ExceptionFact {
        type_name: "other.ReaderException".to_string(),
        kind: ExceptionKind::Checked,
        provenance: FactProvenance::ExternalLibraryPack,
    });
    facts
        .external_crate_api_mapping_facts
        .push(ExternalCrateApiMappingFact {
            receiver_type: "other.Reader".to_string(),
            method_name: "read".to_string(),
            arg_count: None,
            rust_name: "read".to_string(),
            provenance: FactProvenance::ExternalLibraryPack,
        });
    facts
        .external_crate_constructor_mapping_facts
        .push(ExternalCrateConstructorMappingFact {
            type_name: "other.Reader".to_string(),
            arg_count: 0,
            rust_type_name: Some("Reader".to_string()),
            rust_name: "new".to_string(),
            provenance: FactProvenance::ExternalLibraryPack,
        });

    let audit = audit_external_pack_fact_boundaries(&facts);
    assert!(audit.has_code("external_pack_manifest_unowned_api_fact"));
    assert!(audit.has_code("external_pack_manifest_unowned_argument_effect_fact"));
    assert!(audit.has_code("external_pack_manifest_unowned_constructor_fact"));
    assert!(audit.has_code("external_pack_manifest_unowned_field_fact"));
    assert!(audit.has_code("external_pack_manifest_unowned_object_strategy_fact"));
    assert!(audit.has_code("external_pack_manifest_unowned_closed_hierarchy_fact"));
    assert!(audit.has_code("external_pack_manifest_unowned_object_safety_fact"));
    assert!(audit.has_code("external_pack_manifest_unowned_type_capability_fact"));
    assert!(audit.has_code("external_pack_manifest_unowned_field_nullability_fact"));
    assert!(audit.has_code("external_pack_manifest_unowned_exception_fact"));
    assert!(audit.has_code("external_pack_manifest_unowned_crate_api_mapping_fact"));
    assert!(audit.has_code("external_pack_manifest_unowned_crate_constructor_mapping_fact"));
}

#[test]
fn parsed_external_pack_manifest_reports_members_outside_declared_boundaries() {
    let parsed = parse_external_pack_manifest(
        r#"
        external-pack dep.boundary dep.boundary rust-crate-mapping mapped
        external-pack-api dep.boundary other.Reader read mapped getter read-only maybe-null args=0 return=Object rust=read
        external-pack-argument dep.boundary other.Reader read 0 definitely-non-null read-only borrowed type=String
        external-pack-constructor dep.boundary other.Reader 0 mapped rust-type=Reader rust=open
        external-pack-field dep.boundary other.Reader RAW mapped
        external-pack-field-nullability dep.boundary other.Reader name definitely-non-null
        external-pack-object dep.boundary other.Reader erased-object
        external-pack-closed dep.boundary other.Reader closed
        external-pack-object-safety dep.boundary other.Reader unsupported-member
        external-pack-capability dep.boundary other.Reader clone=true
        external-pack-exception dep.boundary other.ReaderException checked
        external-pack-unsupported dep.boundary other.Reader rawDecode runtime codec state
        external-pack-constructor dep.boundary other.NativeHandle 1 unsupported
        external-pack-field dep.boundary other.Reader RAW_HANDLE unsupported
        "#,
    );

    for code in [
        "external_pack_manifest_unowned_api_fact",
        "external_pack_manifest_unowned_argument_effect_fact",
        "external_pack_manifest_unowned_crate_api_mapping_fact",
        "external_pack_manifest_unowned_constructor_fact",
        "external_pack_manifest_unowned_crate_constructor_mapping_fact",
        "external_pack_manifest_unowned_field_fact",
        "external_pack_manifest_unowned_unsupported_api_fact",
        "external_pack_manifest_unowned_unsupported_constructor_fact",
        "external_pack_manifest_unowned_unsupported_field_fact",
        "external_pack_manifest_unowned_object_strategy_fact",
        "external_pack_manifest_unowned_closed_hierarchy_fact",
        "external_pack_manifest_unowned_object_safety_fact",
        "external_pack_manifest_unowned_type_capability_fact",
        "external_pack_manifest_unowned_field_nullability_fact",
        "external_pack_manifest_unowned_exception_fact",
    ] {
        assert!(
            parsed.diagnostics.has_code(code),
            "parsed manifest should report {code}: {:?}",
            parsed.diagnostics
        );
    }
}

#[test]
fn external_pack_manifest_coverage_lookup_is_member_kind_symmetric() {
    let manifest = ExternalPackManifest {
        pack_name: "dep.boundary",
        owned_type_patterns: &["dep.boundary"],
        mode: ExternalLibraryMode::ExternalFactModel,
        default_coverage: ExternalApiCoverage::Conservative,
        rust_target: None,
        object_strategies: &[],
        closed_hierarchy: &[],
        object_safety: &[],
        type_capabilities: &[],
        field_nullability: &[],
        argument_effects: &[],
        exceptions: &[],
        api_effects: &[],
        modeled_apis: &[ExternalPackModeledApi {
            receiver_type: "dep.boundary.Record",
            method_name: "contig",
            coverage: ExternalApiCoverage::Semantic,
        }],
        modeled_constructors: &[
            ExternalPackModeledConstructor {
                type_name: "dep.boundary.Record",
                arg_count: 1,
                rust_type_name: None,
                rust_name: None,
                coverage: ExternalApiCoverage::Semantic,
            },
            ExternalPackModeledConstructor {
                type_name: "dep.boundary.NativeHandle",
                arg_count: 0,
                rust_type_name: None,
                rust_name: None,
                coverage: ExternalApiCoverage::Unsupported,
            },
        ],
        modeled_fields: &[
            ExternalPackModeledField {
                type_name: "dep.boundary.Record",
                field_name: "KIND",
                coverage: ExternalApiCoverage::Semantic,
            },
            ExternalPackModeledField {
                type_name: "dep.boundary.Record",
                field_name: "RAW_HANDLE",
                coverage: ExternalApiCoverage::Unsupported,
            },
        ],
        unsupported_apis: &[ExternalPackUnsupportedApi {
            receiver_type: "dep.boundary.Record",
            method_name: "decodeWithRuntimeState",
            reason: "runtime-backed decode is an explicit boundary",
        }],
    };

    assert_eq!(
        manifest.coverage_for_api("dep.boundary.Record", "contig"),
        ExternalApiCoverage::Semantic
    );
    assert_eq!(
        manifest.coverage_for_api("dep.boundary.Record", "decodeWithRuntimeState"),
        ExternalApiCoverage::Unsupported
    );
    assert_eq!(
        manifest.coverage_for_api("dep.boundary.Record", "unclassified"),
        ExternalApiCoverage::Conservative
    );
    assert_eq!(
        manifest.coverage_for_constructor("dep.boundary.Record", 1),
        ExternalApiCoverage::Semantic
    );
    assert_eq!(
        manifest.coverage_for_constructor("dep.boundary.NativeHandle", 0),
        ExternalApiCoverage::Unsupported
    );
    assert_eq!(
        manifest.coverage_for_constructor("dep.boundary.Record", 2),
        ExternalApiCoverage::Conservative
    );
    assert_eq!(
        manifest.coverage_for_field("dep.boundary.Record", "KIND"),
        ExternalApiCoverage::Semantic
    );
    assert_eq!(
        manifest.coverage_for_field("dep.boundary.Record", "RAW_HANDLE"),
        ExternalApiCoverage::Unsupported
    );
    assert_eq!(
        manifest.coverage_for_field("dep.boundary.Record", "UNCLASSIFIED"),
        ExternalApiCoverage::Conservative
    );
}

#[test]
fn external_pack_manifest_parser_emits_pack_boundary_and_member_facts() {
    let parsed = parse_external_pack_manifest(
        r#"
        external-pack dep.variant dep.variant external-fact-model semantic target=crate::external::dep_variant
        external-pack-object dep.variant dep.variant.Record erased-object
        external-pack-object dep.variant dep.variant.Builder dyn-trait DepVariantBuilder
        external-pack-object dep.variant dep.variant.Kind closed-enum DepVariantKind SNV,INDEL
        external-pack-object dep.variant dep.variant.Value concrete
        external-pack-closed dep.variant dep.variant.Kind closed
        external-pack-capability dep.variant dep.variant.Record clone=true final=true
        external-pack-capability dep.variant dep.variant.Collection sequence=Record final=true
        external-pack-capability dep.variant dep.variant.NamedRecord implements=Record,Named
        external-pack-capability dep.variant dep.variant.Named object-safe=true extends=Record
        external-pack-object-safety dep.variant dep.variant.GenericFactory nested-self-return copyAll
        external-pack-object-safety dep.variant dep.variant.GenericFactory nested-self-param acceptAll values
        external-pack-field-nullability dep.variant dep.variant.Record TYPE definitely-non-null
        external-pack-argument dep.variant dep.variant.Builder setContig 0 definitely-non-null read-only borrowed type=String
        external-pack-exception dep.variant dep.variant.DecodeError checked
        external-pack-api dep.variant dep.variant.Record getContig semantic getter read-only definitely-non-null return=String
        external-pack-api dep.variant dep.variant.Record hasAttribute semantic boolean-getter read-only definitely-non-null return=boolean
        external-pack-api dep.variant dep.variant.Builder setContig semantic fluent-mutator mutates-receiver definitely-non-null return=dep.variant.Builder
        external-pack-api dep.variant dep.variant.Reader read mapped getter read-only maybe-null args=1 return=dep.variant.Record throws=DepIoError
        external-pack-constructor dep.variant dep.variant.Builder 0 semantic
        external-pack-constructor dep.variant dep.variant.LegacyReader 1 unsupported
        external-pack-field dep.variant dep.variant.Record TYPE semantic
        external-pack-field dep.variant dep.variant.Record RAW_HANDLE unsupported
        external-pack-unsupported dep.variant dep.variant.Reader rawDecode requires runtime codec state
        "#,
    );
    assert!(
        parsed.diagnostics.is_empty(),
        "manifest parse should be clean: {:?}",
        parsed.diagnostics
    );

    let pack = parsed
        .facts
        .external_library_pack_for_type("dep.variant.Record")
        .expect("parsed package prefix should produce a pack boundary fact");
    assert_eq!(pack.pack_name, "dep.variant");
    assert_eq!(pack.mode, ExternalLibraryMode::ExternalFactModel);
    assert_eq!(pack.coverage, ExternalApiCoverage::Semantic);
    assert_eq!(
        pack.rust_target.as_deref(),
        Some("crate::external::dep_variant")
    );
    assert_eq!(pack.provenance, FactProvenance::ExternalLibraryPack);

    let record_strategy = parsed
        .facts
        .hierarchy_overrides
        .iter()
        .find(|fact| fact.type_name == "dep.variant.Record")
        .expect("external-pack-object should emit an object strategy fact");
    assert_eq!(record_strategy.strategy, HierarchyStrategy::ErasedObject);
    assert_eq!(
        record_strategy.provenance,
        FactProvenance::ExternalLibraryPack
    );
    let builder_strategy = parsed
        .facts
        .hierarchy_overrides
        .iter()
        .find(|fact| fact.type_name == "dep.variant.Builder")
        .expect("dyn-trait strategy should be parsed");
    assert_eq!(
        builder_strategy.strategy,
        HierarchyStrategy::DynTrait {
            trait_name: "DepVariantBuilder".to_string()
        }
    );
    let kind_strategy = parsed
        .facts
        .hierarchy_overrides
        .iter()
        .find(|fact| fact.type_name == "dep.variant.Kind")
        .expect("closed-enum strategy should be parsed");
    assert_eq!(
        kind_strategy.strategy,
        HierarchyStrategy::ClosedEnum {
            enum_name: "DepVariantKind".to_string(),
            variants: vec!["SNV".to_string(), "INDEL".to_string()]
        }
    );
    let value_strategy = parsed
        .facts
        .hierarchy_overrides
        .iter()
        .find(|fact| fact.type_name == "dep.variant.Value")
        .expect("concrete strategy should be parsed");
    assert_eq!(value_strategy.strategy, HierarchyStrategy::ConcreteStruct);
    let kind_closed = parsed
        .facts
        .closed_hierarchy_facts
        .iter()
        .find(|fact| fact.type_name == "dep.variant.Kind")
        .expect("external-pack-closed should emit a closed-hierarchy fact");
    assert!(kind_closed.closed);
    assert_eq!(kind_closed.provenance, FactProvenance::ExternalLibraryPack);

    let record_capability = parsed
        .facts
        .type_capability_overrides
        .iter()
        .find(|fact| fact.type_name == "dep.variant.Record")
        .expect("external-pack-capability should emit a capability fact");
    assert_eq!(record_capability.clone, Some(true));
    assert_eq!(record_capability.final_type, Some(true));
    assert_eq!(
        record_capability.provenance,
        FactProvenance::ExternalLibraryPack
    );
    assert!(
        parsed.facts.is_clone_type(&JavaType::Named {
            name: "dep.variant.record".to_string(),
            args: Vec::new(),
        }),
        "capability facts should be addressable by resolver-canonical qualified spelling"
    );
    assert!(
        parsed.facts.is_final_type(&JavaType::Named {
            name: "dep.variant.record".to_string(),
            args: Vec::new(),
        }),
        "final capability lookup should normalize qualified type spellings"
    );
    assert_eq!(
        parsed.facts.sequence_element_type_for(&JavaType::Named {
            name: "dep.variant.Collection".to_string(),
            args: Vec::new(),
        }),
        Some(JavaType::Named {
            name: "Record".to_string(),
            args: Vec::new(),
        })
    );
    assert_eq!(
        parsed.facts.sequence_element_type_for(&JavaType::Named {
            name: "dep.variant.collection".to_string(),
            args: Vec::new(),
        }),
        Some(JavaType::Named {
            name: "Record".to_string(),
            args: Vec::new(),
        }),
        "sequence capability lookup should normalize qualified type spellings"
    );
    assert_eq!(
        parsed
            .facts
            .object_safe_trait_fact_for_name("dep.variant.named"),
        Some(true),
        "object-safe capability lookup should normalize qualified type spellings"
    );
    assert_eq!(
        parsed
            .facts
            .closed_hierarchy_fact_for_name("dep.variant.kind"),
        Some(true),
        "closed-hierarchy lookup should normalize qualified type spellings"
    );
    assert!(
        parsed
            .facts
            .has_object_representation_facts(&JavaType::Named {
                name: "dep.variant.record".to_string(),
                args: Vec::new(),
            }),
        "representation fact presence should normalize qualified type spellings"
    );
    let named_record = parsed
        .facts
        .type_capability_overrides
        .iter()
        .find(|fact| fact.type_name == "dep.variant.NamedRecord")
        .expect("implements capability should be parsed");
    assert_eq!(
        named_record.implements,
        vec![
            JavaType::Named {
                name: "Record".to_string(),
                args: Vec::new(),
            },
            JavaType::Named {
                name: "Named".to_string(),
                args: Vec::new(),
            }
        ]
    );
    let named = parsed
        .facts
        .type_capability_overrides
        .iter()
        .find(|fact| fact.type_name == "dep.variant.Named")
        .expect("extends and object-safe capability should be parsed");
    assert_eq!(named.object_safe_trait, Some(true));
    assert_eq!(
        named.extends,
        Some(JavaType::Named {
            name: "Record".to_string(),
            args: Vec::new(),
        })
    );
    let generic_factory_safety = parsed
        .facts
        .object_safety_facts
        .iter()
        .find(|fact| fact.type_name == "dep.variant.GenericFactory")
        .expect("external-pack-object-safety should emit an object-safety fact");
    assert_eq!(
        generic_factory_safety.provenance,
        FactProvenance::ExternalLibraryPack
    );
    assert!(!generic_factory_safety.object_safe);
    assert_eq!(
        generic_factory_safety.violations,
        vec![
            ObjectSafetyViolation::NestedSelfReferentialReturn {
                method_name: "copyAll".to_string(),
            },
            ObjectSafetyViolation::NestedSelfReferentialParameter {
                method_name: "acceptAll".to_string(),
                param_name: "values".to_string(),
            },
        ]
    );
    let generic_factory_capability = parsed
        .facts
        .type_capability_overrides
        .iter()
        .find(|fact| {
            fact.type_name == "dep.variant.GenericFactory" && fact.object_safe_trait == Some(false)
        })
        .expect("external-pack-object-safety should also emit object-safe=false capability");
    assert_eq!(
        generic_factory_capability.provenance,
        FactProvenance::ExternalLibraryPack
    );
    let type_field = parsed
        .facts
        .field_nullability_facts
        .iter()
        .find(|fact| {
            fact.field.type_name == "dep.variant.Record" && fact.field.field_name == "TYPE"
        })
        .expect("external-pack-field-nullability should emit a field nullability fact");
    assert_eq!(type_field.state, NullabilityState::DefinitelyNonNull);
    assert_eq!(
        type_field.provenance,
        FieldNullabilityProvenance::ExternalLibraryPack
    );
    let set_contig_argument = parsed
        .facts
        .external_api_argument_effect_for_receiver(Some("dep.variant.Builder"), "setContig", 0)
        .expect("external-pack-argument should emit an argument effect fact");
    assert_eq!(set_contig_argument.parameter_type, Some(JavaType::String));
    assert_eq!(
        set_contig_argument.parameter_nullability,
        NullabilityState::DefinitelyNonNull
    );
    assert_eq!(
        set_contig_argument.argument_mutability,
        ReceiverMutabilityFact::ReadOnly
    );
    assert_eq!(
        set_contig_argument.alias_policy,
        ExternalArgumentAliasPolicy::Borrowed
    );
    assert_eq!(
        set_contig_argument.provenance,
        FactProvenance::ExternalLibraryPack
    );
    let decode_error = parsed
        .facts
        .exception_facts
        .iter()
        .find(|fact| fact.type_name == "dep.variant.DecodeError")
        .expect("external-pack-exception should emit an exception fact");
    assert_eq!(decode_error.kind, ExceptionKind::Checked);
    assert_eq!(decode_error.provenance, FactProvenance::ExternalLibraryPack);
    assert_eq!(
        parsed.facts.exception_kind_for("dep.variant.decode_error"),
        Some(ExceptionKind::Checked),
        "exception facts should be addressable by resolver-canonical qualified spelling"
    );

    let getter = parsed
        .facts
        .external_api_effect_for_receiver(Some("dep.variant.Record"), "getContig")
        .expect("modeled manifest API should produce an external API effect fact");
    assert_eq!(getter.role, ExternalApiRole::Getter);
    assert_eq!(getter.receiver_mutability, ReceiverMutabilityFact::ReadOnly);
    assert_eq!(
        getter.return_nullability,
        NullabilityState::DefinitelyNonNull
    );
    assert_eq!(getter.return_type, Some(JavaType::String));
    assert_eq!(getter.provenance, FactProvenance::ExternalLibraryPack);

    let reader = parsed
        .facts
        .external_api_effect_for_receiver_with_arity(Some("dep.variant.Reader"), "read", Some(1))
        .expect("mapped manifest API should still be represented as a fact");
    assert_eq!(reader.arg_count, Some(1));
    assert_eq!(
        reader.thrown,
        vec![JavaType::Named {
            name: "DepIoError".to_string(),
            args: Vec::new(),
        }]
    );
    assert_eq!(
        parsed
            .facts
            .external_crate_api_mapping_for_receiver("dep.variant.Reader", "read", Some(1))
            .map(|mapping| mapping.rust_name.as_str()),
        None,
        "external fact-model packs should not produce crate mappings"
    );

    let constructor = parsed
        .facts
        .external_constructor_for("dep.variant.Builder", &[])
        .expect("manifest constructor entry should produce constructor fact");
    assert_eq!(constructor.provenance, FactProvenance::ExternalLibraryPack);

    let field = parsed
        .facts
        .external_field_for("dep.variant.Record", "TYPE")
        .expect("manifest field entry should produce field fact");
    assert_eq!(field.provenance, FactProvenance::ExternalLibraryPack);

    let unsupported = parsed
        .facts
        .unsupported_external_api_for_receiver("dep.variant.Reader", "rawDecode")
        .expect("unsupported manifest entry should produce explicit diagnostic fact");
    assert_eq!(unsupported.pack_name, "dep.variant");
    assert_eq!(unsupported.reason, "requires runtime codec state");

    let unsupported_constructor = parsed
        .facts
        .unsupported_external_constructor_for("dep.variant.LegacyReader", 1)
        .expect("unsupported manifest constructor should produce explicit diagnostic fact");
    assert_eq!(unsupported_constructor.pack_name, "dep.variant");
    assert_eq!(
        unsupported_constructor.reason,
        "declared unsupported by external pack manifest"
    );

    let unsupported_field = parsed
        .facts
        .unsupported_external_field_for("dep.variant.Record", "RAW_HANDLE")
        .expect("unsupported manifest field should produce explicit diagnostic fact");
    assert_eq!(unsupported_field.pack_name, "dep.variant");
    assert_eq!(
        unsupported_field.reason,
        "declared unsupported by external pack manifest"
    );
}

#[test]
fn external_pack_manifest_parser_feeds_semantic_analysis_without_lowering_rules() {
    let parsed = parse_external_pack_manifest(
        r#"
        external-pack dep.io dep.io rust-crate-mapping mapped target=dep_io
        external-pack dep.io Reader rust-crate-mapping mapped target=dep_io
        external-pack-api dep.io Reader read mapped getter read-only maybe-null return=Record throws=DepIoError rust=read_record
        external-pack-constructor dep.io Reader 0 mapped rust-type=ReaderHandle rust=open
        "#,
    );
    assert!(parsed.diagnostics.is_empty());

    let source = r#"
package app;
import dep.io.Reader;
class UseDependency {
    Object load(Reader reader) {
        return reader.read();
    }
}
"#;
    let program = parser::parse_java_source(source).program;
    let resolved = resolve::resolve_program(&program, Default::default());
    let semantic =
        semantic_analysis::analyze_program(&resolved.program, &resolved.facts, &parsed.facts);

    let read = semantic
        .facts
        .external_api_effect_for_receiver(Some("Reader"), "read")
        .expect("parsed pack API should be consumed as an external fact");
    assert_eq!(
        read.return_type,
        Some(JavaType::Named {
            name: "Record".to_string(),
            args: Vec::new(),
        })
    );
    assert_eq!(read.provenance, FactProvenance::ExternalLibraryPack);
    let read_mapping = semantic
        .facts
        .external_crate_api_mapping_for_receiver("Reader", "read", None)
        .expect("parsed mapped pack API should produce a facade mapping fact");
    assert_eq!(read_mapping.rust_name, "read_record");
    let constructor_mapping = semantic
        .facts
        .external_crate_constructor_mapping_for("Reader", 0)
        .expect("parsed mapped pack constructor should produce a facade mapping fact");
    assert_eq!(
        constructor_mapping.rust_type_name.as_deref(),
        Some("ReaderHandle")
    );
    assert_eq!(constructor_mapping.rust_name, "open");

    let method_effect = semantic
        .facts
        .method_effect_for("UseDependency", "load")
        .expect("external thrown type should propagate into enclosing method effects");
    assert_eq!(
        method_effect.thrown,
        vec![JavaType::Named {
            name: "DepIoError".to_string(),
            args: Vec::new(),
        }]
    );
}

#[test]
fn parsed_external_pack_manifest_owns_missing_and_unsupported_diagnostics() {
    let parsed = parse_external_pack_manifest(
        r#"
        external-pack dep.runtime dep.runtime external-fact-model semantic
        external-pack dep.runtime Reader external-fact-model semantic
        external-pack-unsupported dep.runtime Reader rawDecode requires runtime state
        external-pack-api dep.runtime Reader inlineRawDecode unsupported getter unknown unknown
        external-pack-constructor dep.runtime Reader 1 unsupported
        external-pack-field dep.runtime Reader UNSUPPORTED unsupported
        "#,
    );
    assert!(
        parsed.diagnostics.is_empty(),
        "manifest parse should be clean: {:?}",
        parsed.diagnostics
    );

    let source = r#"
package app;
import dep.runtime.Reader;
class UseParsedPackCoverage {
    Object unsupported(Reader reader) {
        return reader.rawDecode();
    }
    Object inlineUnsupported(Reader reader) {
        return reader.inlineRawDecode();
    }
    Object missingMethod(Reader reader) {
        return reader.unmodeled();
    }
    Object missingConstructor() {
        return new Reader("path", 7);
    }
    Object unsupportedConstructor() {
        return new Reader("path");
    }
    Object missingField(Reader reader) {
        return reader.UNMODELED;
    }
    Object unsupportedField(Reader reader) {
        return reader.UNSUPPORTED;
    }
}
"#;
    let program = parser::parse_java_source(source).program;
    let resolved = resolve::resolve_program(&program, Default::default());
    let semantic =
        semantic_analysis::analyze_program(&resolved.program, &resolved.facts, &parsed.facts);

    assert!(
        semantic.diagnostics.iter().any(|diagnostic| diagnostic.code
            == "external_pack_unsupported_api"
            && diagnostic.message.contains("Reader.rawDecode")
            && diagnostic.message.contains("dep.runtime")),
        "unsupported parsed-pack APIs should produce pack-owned diagnostics: {:?}",
        semantic.diagnostics
    );
    assert!(
        semantic.diagnostics.iter().any(|diagnostic| diagnostic.code
            == "external_pack_unsupported_api"
            && diagnostic.message.contains("Reader.inlineRawDecode")
            && diagnostic.message.contains("dep.runtime")),
        "inline unsupported parsed-pack API coverage should produce the same pack-owned diagnostic: {:?}",
        semantic.diagnostics
    );
    assert!(
        semantic
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "external_pack_missing_api"
                && diagnostic.message.contains("Reader.unmodeled")
                && diagnostic.message.contains("dep.runtime")),
        "missing parsed-pack APIs should produce pack-owned diagnostics: {:?}",
        semantic.diagnostics
    );
    assert!(
        semantic.diagnostics.iter().any(|diagnostic| diagnostic.code
            == "external_pack_missing_constructor"
            && diagnostic.message.contains("Reader constructor")
            && diagnostic.message.contains("2 argument")
            && diagnostic.message.contains("dep.runtime")),
        "missing parsed-pack constructors should produce pack-owned diagnostics: {:?}",
        semantic.diagnostics
    );
    assert!(
        semantic.diagnostics.iter().any(|diagnostic| diagnostic.code
            == "external_pack_unsupported_constructor"
            && diagnostic.message.contains("Reader constructor")
            && diagnostic.message.contains("1 argument")
            && diagnostic.message.contains("dep.runtime")),
        "unsupported parsed-pack constructors should produce pack-owned diagnostics: {:?}",
        semantic.diagnostics
    );
    assert!(
        semantic.diagnostics.iter().any(|diagnostic| diagnostic.code
            == "external_pack_missing_field"
            && diagnostic.message.contains("Reader.UNMODELED")
            && diagnostic.message.contains("dep.runtime")),
        "missing parsed-pack fields should produce pack-owned diagnostics: {:?}",
        semantic.diagnostics
    );
    assert!(
        semantic.diagnostics.iter().any(|diagnostic| diagnostic.code
            == "external_pack_unsupported_field"
            && diagnostic.message.contains("Reader.UNSUPPORTED")
            && diagnostic.message.contains("dep.runtime")),
        "unsupported parsed-pack fields should produce pack-owned diagnostics: {:?}",
        semantic.diagnostics
    );
    assert!(
        semantic
            .facts
            .external_api_effect_for_receiver(Some("Reader"), "rawDecode")
            .is_none()
    );
    assert!(
        semantic
            .facts
            .external_api_effect_for_receiver(Some("Reader"), "unmodeled")
            .is_none()
    );
    assert!(
        semantic
            .facts
            .external_constructor_for(
                "Reader",
                &[JavaType::String, JavaType::Primitive(PrimitiveType::Int)]
            )
            .is_none()
    );
    assert!(
        semantic
            .facts
            .external_constructor_for("Reader", &[JavaType::String])
            .is_none()
    );
    assert!(
        semantic
            .facts
            .external_field_for("Reader", "UNMODELED")
            .is_none()
    );
    assert!(
        semantic
            .facts
            .external_field_for("Reader", "UNSUPPORTED")
            .is_none()
    );
}

#[test]
fn external_pack_manifest_parser_reports_invalid_manifest_lines() {
    let parsed = parse_external_pack_manifest(
        r#"
        external-pack broken.pkg broken.pkg unknown-mode semantic
        external-pack-api broken.pkg broken.Type get semantic getter impossible-mutability
        external-pack-api broken.pkg broken.Type raw unsupported impossible-role read-only
        external-pack-api broken.pkg broken.Type rawWithRust unsupported getter read-only unknown rust=raw_with_rust
        external-pack-constructor broken.pkg broken.Type many semantic
        external-pack-constructor broken.pkg broken.Type 1 unsupported rust=open_raw
        external-pack-field-nullability broken.pkg broken.Type NAME impossible-nullability
        external-pack-argument broken.pkg broken.Type put zero definitely-non-null read-only borrowed
        external-pack-exception broken.pkg broken.Type maybe
        "#,
    );

    assert!(parsed.diagnostics.has_errors());
    assert_eq!(parsed.diagnostics.len(), 9);
    assert!(
        parsed
            .diagnostics
            .has_code("external_pack_manifest_parse_error")
    );
}

#[test]
fn htsjdk_variantcontext_pack_declares_owned_external_boundary() {
    let facts = htsjdk_variantcontext_pack_facts();

    let pack = facts
        .external_library_pack_for_type("htsjdk.variant.variantcontext.VariantContext")
        .expect("variantcontext package should be owned by the external pack");
    assert_eq!(pack.pack_name, "htsjdk.variant.variantcontext");
    assert_eq!(pack.mode, ExternalLibraryMode::ExternalFactModel);
    assert_eq!(pack.coverage, ExternalApiCoverage::Semantic);
    assert_eq!(pack.provenance, FactProvenance::ExternalLibraryPack);

    assert!(
        facts.has_object_representation_facts(&JavaType::Named {
            name: "htsjdk.variant.variantcontext.VariantContext".to_string(),
            args: Vec::new(),
        }),
        "variantcontext values should stay explicit external objects/facades"
    );
}

#[test]
fn htsjdk_variantcontext_pack_does_not_claim_map_entry_methods_in_enhanced_for() {
    let parsed = parser::parse_java_source(
        r#"
import htsjdk.variant.variantcontext.Allele;
import java.util.List;
import java.util.Map;

class Haplotype {}

class VariantContextMapEntryLoop {
    void run(Map<Allele, List<Haplotype>> alleleMapper) {
        for (final Map.Entry<Allele, List<Haplotype>> allele : alleleMapper.entrySet()) {
            allele.getKey();
            allele.getValue();
        }
    }
}
"#,
    );
    let resolved = resolve::resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let semantic = semantic_analysis::analyze_program(
        &resolved.program,
        &resolved.facts,
        &htsjdk_variantcontext_pack_facts(),
    );

    assert!(
        semantic.diagnostics.iter().all(|diagnostic| {
            !(diagnostic.code == "external_pack_missing_api"
                && (diagnostic.message.contains("Allele.getKey")
                    || diagnostic.message.contains("Allele.getValue")))
        }),
        "Map.Entry accessors should not be attributed to the Allele external pack: {:?}",
        semantic.diagnostics
    );
}

#[test]
fn htsjdk_variantcontext_pack_models_builder_and_getter_effects() {
    let facts = htsjdk_variantcontext_pack_facts();

    let internal_codec = JavaType::Named {
        name: "InternalCodecException".to_string(),
        args: Vec::new(),
    };
    let runtime_exception = JavaType::Named {
        name: "RuntimeException".to_string(),
        args: Vec::new(),
    };
    let reachable = facts.reachable_hierarchy_edges_for_type(&internal_codec);
    assert!(
        reachable
            .iter()
            .any(|edge| edge.sub_name == "InternalCodecException"
                && edge.super_name == "RuntimeException"),
        "external exception hierarchy should be declared by the pack"
    );
    assert!(
        facts
            .reachable_hierarchy_edges_for_type(&JavaType::Named {
                name: "htsjdk.tribble.TribbleException.InternalCodecException".to_string(),
                args: Vec::new(),
            })
            .iter()
            .any(
                |edge| edge.sub_name == "htsjdk.tribble.TribbleException.InternalCodecException"
                    && edge.super_name == "RuntimeException"
            ),
        "fully-qualified external exception hierarchy should be declared by the pack"
    );
    assert_eq!(
        facts.exception_kind_for("InternalCodecException"),
        Some(ExceptionKind::Unchecked)
    );
    assert_eq!(
        facts
            .reachable_hierarchy_edges_for_type(&runtime_exception)
            .len(),
        0,
        "RuntimeException itself should stay a root rather than a pack-owned facade"
    );

    let genotype = JavaType::Named {
        name: "Genotype".to_string(),
        args: Vec::new(),
    };
    let variant_context = JavaType::Named {
        name: "VariantContext".to_string(),
        args: Vec::new(),
    };
    let dictionary = JavaType::Named {
        name: "SAMSequenceDictionary".to_string(),
        args: Vec::new(),
    };
    let allele_list = JavaType::Named {
        name: "List".to_string(),
        args: vec![JavaType::Named {
            name: "Allele".to_string(),
            args: Vec::new(),
        }],
    };

    for (type_name, args) in [
        ("VariantContextComparator", vec![dictionary]),
        ("GenotypeBuilder", Vec::new()),
        ("GenotypeBuilder", vec![genotype.clone()]),
        (
            "GenotypeBuilder",
            vec![JavaType::String, allele_list.clone()],
        ),
        ("VariantContextBuilder", Vec::new()),
        ("VariantContextBuilder", vec![variant_context]),
        (
            "VariantContextBuilder",
            vec![
                JavaType::String,
                JavaType::String,
                JavaType::Primitive(PrimitiveType::Int),
                JavaType::Primitive(PrimitiveType::Int),
                allele_list,
            ],
        ),
    ] {
        let constructor = facts
            .external_constructor_for(type_name, &args)
            .expect("variantcontext builder constructor should be pack-owned by arity");
        assert_eq!(constructor.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(
            new_java2rust::stdlib::external_constructor_provenance(type_name, &args),
            FactProvenance::ConservativeFallback,
            "{type_name}/{:?} should be owned by the variantcontext external pack, not stdlib",
            args.len()
        );
    }

    let variant_type = facts
        .external_field_for("VariantContext", "Type")
        .expect("VariantContext.Type should be pack-owned");
    assert_eq!(variant_type.provenance, FactProvenance::ExternalLibraryPack);
    assert_eq!(
        variant_type.field_type,
        Some(JavaType::Named {
            name: "Type".to_string(),
            args: Vec::new(),
        })
    );
    assert_eq!(
        new_java2rust::stdlib::external_field_provenance("VariantContext", "Type"),
        FactProvenance::ConservativeFallback,
        "VariantContext.Type precision should come from the variantcontext pack, not stdlib"
    );
    for field_name in ["SNP", "MNP", "SYMBOLIC"] {
        let variant_type_constant = facts
            .external_field_for("Type", field_name)
            .unwrap_or_else(|| panic!("VariantContext.Type.{field_name} should be pack-owned"));
        assert_eq!(
            variant_type_constant.provenance,
            FactProvenance::ExternalLibraryPack
        );
        assert_eq!(
            variant_type_constant.field_type,
            Some(JavaType::Named {
                name: "Type".to_string(),
                args: Vec::new(),
            })
        );
    }
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
        let field = facts
            .external_field_for("Allele", field_name)
            .unwrap_or_else(|| panic!("Allele.{field_name} should be pack-owned"));
        assert_eq!(field.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(field.field_type, Some(expected_type));
        let dispatched = new_java2rust::external_packs::external_field_for("Allele", field_name)
            .unwrap_or_else(|| panic!("Allele.{field_name} should be globally pack-dispatched"));
        assert_eq!(dispatched.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(
            new_java2rust::stdlib::external_field_provenance("Allele", field_name),
            FactProvenance::ConservativeFallback,
            "Allele.{field_name} precision should come from the variantcontext pack, not stdlib"
        );
    }
    let max_diploid_alt_alleles = facts
        .external_field_for(
            "GenotypeLikelihoods",
            "MAX_DIPLOID_ALT_ALLELES_THAT_CAN_BE_GENOTYPED",
        )
        .expect("GenotypeLikelihoods max alt allele limit should be pack-owned");
    assert_eq!(
        max_diploid_alt_alleles.provenance,
        FactProvenance::ExternalLibraryPack
    );
    assert_eq!(
        max_diploid_alt_alleles.field_type,
        Some(JavaType::Primitive(PrimitiveType::Int))
    );
    let dispatched_max_diploid_alt_alleles = new_java2rust::external_packs::external_field_for(
        "GenotypeLikelihoods",
        "MAX_DIPLOID_ALT_ALLELES_THAT_CAN_BE_GENOTYPED",
    )
    .expect("GenotypeLikelihoods max alt allele limit should be globally pack-dispatched");
    assert_eq!(
        dispatched_max_diploid_alt_alleles.provenance,
        FactProvenance::ExternalLibraryPack
    );
    assert_eq!(
        new_java2rust::stdlib::external_field_provenance(
            "GenotypeLikelihoods",
            "MAX_DIPLOID_ALT_ALLELES_THAT_CAN_BE_GENOTYPED"
        ),
        FactProvenance::ConservativeFallback,
        "GenotypeLikelihoods max alt allele limit precision should come from the variantcontext pack, not stdlib"
    );
    let no_genotypes = facts
        .external_field_for("GenotypesContext", "NO_GENOTYPES")
        .expect("GenotypesContext.NO_GENOTYPES should be pack-owned");
    assert_eq!(no_genotypes.provenance, FactProvenance::ExternalLibraryPack);
    assert_eq!(
        no_genotypes.field_type,
        Some(JavaType::Named {
            name: "GenotypesContext".to_string(),
            args: Vec::new(),
        })
    );
    let dispatched_no_genotypes =
        new_java2rust::external_packs::external_field_for("GenotypesContext", "NO_GENOTYPES")
            .expect("GenotypesContext.NO_GENOTYPES should be globally pack-dispatched");
    assert_eq!(
        dispatched_no_genotypes.provenance,
        FactProvenance::ExternalLibraryPack
    );
    assert_eq!(
        new_java2rust::stdlib::external_field_provenance("GenotypesContext", "NO_GENOTYPES"),
        FactProvenance::ConservativeFallback,
        "GenotypesContext.NO_GENOTYPES precision should come from the variantcontext pack, not stdlib"
    );
    for type_name in [
        "CommonInfo",
        "htsjdk.variant.variantcontext.CommonInfo",
        "VariantContext",
        "htsjdk.variant.variantcontext.VariantContext",
    ] {
        let field = facts
            .external_field_for(type_name, "NO_LOG10_PERROR")
            .unwrap_or_else(|| panic!("{type_name}.NO_LOG10_PERROR should be pack-owned"));
        assert_eq!(field.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(
            field.field_type,
            Some(JavaType::Primitive(PrimitiveType::Double))
        );
        let dispatched =
            new_java2rust::external_packs::external_field_for(type_name, "NO_LOG10_PERROR")
                .unwrap_or_else(|| {
                    panic!("{type_name}.NO_LOG10_PERROR should be globally pack-dispatched")
                });
        assert_eq!(dispatched.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(
            new_java2rust::stdlib::external_field_provenance(type_name, "NO_LOG10_PERROR"),
            FactProvenance::ConservativeFallback,
            "{type_name}.NO_LOG10_PERROR precision should come from the variantcontext pack, not stdlib"
        );
    }
    let unphased_separator = facts
        .external_field_for("Genotype", "UNPHASED_ALLELE_SEPARATOR")
        .expect("Genotype.UNPHASED_ALLELE_SEPARATOR should be pack-owned");
    assert_eq!(
        unphased_separator.provenance,
        FactProvenance::ExternalLibraryPack
    );
    assert_eq!(
        unphased_separator.field_type,
        Some(JavaType::Primitive(PrimitiveType::Char))
    );
    let dispatched_unphased_separator =
        new_java2rust::external_packs::external_field_for("Genotype", "UNPHASED_ALLELE_SEPARATOR")
            .expect("Genotype.UNPHASED_ALLELE_SEPARATOR should be globally pack-dispatched");
    assert_eq!(
        dispatched_unphased_separator.provenance,
        FactProvenance::ExternalLibraryPack
    );
    assert_eq!(
        new_java2rust::stdlib::external_field_provenance("Genotype", "UNPHASED_ALLELE_SEPARATOR"),
        FactProvenance::ConservativeFallback,
        "Genotype.UNPHASED_ALLELE_SEPARATOR precision should come from the variantcontext pack, not stdlib"
    );
    for field_name in ["INDEL", "NO_VARIATION"] {
        let field = facts
            .external_field_for("Type", field_name)
            .unwrap_or_else(|| panic!("Type.{field_name} should be pack-owned"));
        assert_eq!(field.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(
            field.field_type,
            Some(JavaType::Named {
                name: "Type".to_string(),
                args: Vec::new(),
            })
        );
        let dispatched = new_java2rust::external_packs::external_field_for("Type", field_name)
            .unwrap_or_else(|| panic!("Type.{field_name} should be globally pack-dispatched"));
        assert_eq!(dispatched.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(
            new_java2rust::stdlib::external_field_provenance("Type", field_name),
            FactProvenance::ConservativeFallback,
            "Type.{field_name} precision should come from the variantcontext pack, not stdlib"
        );
    }
    for field_name in ["alleleIndex1", "alleleIndex2"] {
        let field = facts
            .external_field_for("GenotypeLikelihoodsAllelePair", field_name)
            .unwrap_or_else(|| {
                panic!("GenotypeLikelihoodsAllelePair.{field_name} should be pack-owned")
            });
        assert_eq!(field.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(
            field.field_type,
            Some(JavaType::Primitive(PrimitiveType::Int))
        );
        let dispatched = new_java2rust::external_packs::external_field_for(
            "GenotypeLikelihoodsAllelePair",
            field_name,
        )
        .unwrap_or_else(|| {
            panic!("GenotypeLikelihoodsAllelePair.{field_name} should be globally pack-dispatched")
        });
        assert_eq!(dispatched.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(
            new_java2rust::stdlib::external_field_provenance(
                "GenotypeLikelihoodsAllelePair",
                field_name
            ),
            FactProvenance::ConservativeFallback,
            "GenotypeLikelihoodsAllelePair.{field_name} precision should come from the variantcontext pack, not stdlib"
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
        let field = facts
            .external_field_for("JexlVCMatchExp", field_name)
            .unwrap_or_else(|| panic!("JexlVCMatchExp.{field_name} should be pack-owned"));
        assert_eq!(field.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(field.field_type, Some(expected_type));
        let dispatched =
            new_java2rust::external_packs::external_field_for("JexlVCMatchExp", field_name)
                .unwrap_or_else(|| {
                    panic!("JexlVCMatchExp.{field_name} should be globally pack-dispatched")
                });
        assert_eq!(dispatched.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(
            new_java2rust::stdlib::external_field_provenance("JexlVCMatchExp", field_name),
            FactProvenance::ConservativeFallback,
            "JexlVCMatchExp.{field_name} precision should come from the variantcontext pack, not stdlib"
        );
    }
    for field_name in ["TREAT_AS_MATCH", "TREAT_AS_MISMATCH"] {
        let field = facts
            .external_field_for("JexlMissingValueTreatment", field_name)
            .unwrap_or_else(|| {
                panic!("JexlMissingValueTreatment.{field_name} should be pack-owned")
            });
        assert_eq!(field.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(
            field.field_type,
            Some(JavaType::Named {
                name: "JexlMissingValueTreatment".to_string(),
                args: Vec::new(),
            })
        );
        let dispatched = new_java2rust::external_packs::external_field_for(
            "JexlMissingValueTreatment",
            field_name,
        )
        .unwrap_or_else(|| {
            panic!("JexlMissingValueTreatment.{field_name} should be globally pack-dispatched")
        });
        assert_eq!(dispatched.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(
            new_java2rust::stdlib::external_field_provenance(
                "JexlMissingValueTreatment",
                field_name
            ),
            FactProvenance::ConservativeFallback,
            "JexlMissingValueTreatment.{field_name} precision should come from the variantcontext pack, not stdlib"
        );
    }

    let allele_create = facts
        .external_api_effect_for_receiver(Some("Allele"), "create")
        .expect("Allele.create should be pack-owned");
    assert_eq!(
        allele_create.provenance,
        FactProvenance::ExternalLibraryPack
    );
    assert_eq!(allele_create.role, ExternalApiRole::Getter);
    assert_eq!(
        allele_create.return_type,
        Some(JavaType::Named {
            name: "Allele".to_string(),
            args: Vec::new(),
        })
    );
    assert_eq!(
        allele_create.return_nullability,
        NullabilityState::DefinitelyNonNull
    );
    assert_eq!(
        new_java2rust::stdlib::external_api_method_policy(Some("Allele"), "create", 1).provenance,
        FactProvenance::ConservativeFallback,
        "Allele.create precision should come from the variantcontext pack, not stdlib"
    );
    let allele_base_string = facts
        .external_api_effect_for_receiver(Some("Allele"), "getBaseString")
        .expect("Allele.getBaseString should be pack-owned");
    assert_eq!(
        allele_base_string.provenance,
        FactProvenance::ExternalLibraryPack
    );
    assert_eq!(allele_base_string.return_type, Some(JavaType::String));
    assert_eq!(
        new_java2rust::stdlib::external_api_method_policy(Some("Allele"), "getBaseString", 0)
            .provenance,
        FactProvenance::ConservativeFallback,
        "Allele.getBaseString precision should come from the variantcontext pack, not stdlib"
    );
    for (receiver, method) in [
        ("Allele", "getBases"),
        ("Allele", "length"),
        ("VariantContext", "getContig"),
        ("VariantContext", "getStart"),
    ] {
        assert!(
            facts
                .external_api_effect_for_receiver(Some(receiver), method)
                .is_some(),
            "{receiver}.{method} should be pack-owned"
        );
        assert_eq!(
            new_java2rust::stdlib::external_api_method_policy(Some(receiver), method, 0).provenance,
            FactProvenance::ConservativeFallback,
            "{receiver}.{method} precision should come from the variantcontext pack, not stdlib"
        );
    }
    assert_eq!(
        new_java2rust::stdlib::external_api_method_policy(Some("Allele"), "equals", 1).provenance,
        FactProvenance::BuiltinExternalModel,
        "Object-inherited methods stay generic builtin even when a pack also models the receiver"
    );
    assert_eq!(
        new_java2rust::stdlib::external_api_method_policy(Some("ExternalSequence"), "getBases", 0)
            .provenance,
        FactProvenance::BuiltinExternalModel,
        "unknown-library domain getters should keep the generic stdlib fallback"
    );

    let alleles = facts
        .external_api_effect_for_receiver(Some("VariantContextBuilder"), "alleles")
        .expect("builder setter should be modeled by the pack");
    assert_eq!(alleles.role, ExternalApiRole::FluentMutator);
    assert_eq!(
        alleles.receiver_mutability,
        ReceiverMutabilityFact::MutatesReceiver
    );
    assert_eq!(alleles.provenance, FactProvenance::ExternalLibraryPack);

    let make = facts
        .external_api_effect_for_receiver(Some("VariantContextBuilder"), "make")
        .expect("builder make should be modeled by the pack");
    assert_eq!(make.role, ExternalApiRole::Getter);
    assert_eq!(
        make.return_type,
        Some(JavaType::Named {
            name: "VariantContext".to_string(),
            args: Vec::new(),
        })
    );
    assert_eq!(make.return_nullability, NullabilityState::DefinitelyNonNull);

    let attribute = facts
        .external_api_effect_for_receiver(Some("VariantContext"), "getAttribute")
        .expect("nullable attributes should be modeled by the pack");
    assert_eq!(attribute.role, ExternalApiRole::Getter);
    assert_eq!(
        attribute.return_type,
        Some(JavaType::Named {
            name: "Object".to_string(),
            args: Vec::new(),
        })
    );
    assert_eq!(attribute.return_nullability, NullabilityState::MaybeNull);

    let validation = facts
        .external_api_effect_for_receiver(Some("VariantContext"), "validateAlternateAlleles")
        .expect("VariantContext validation checks should be modeled by the pack");
    assert_eq!(validation.role, ExternalApiRole::SideEffect);
    assert_eq!(
        validation.receiver_mutability,
        ReceiverMutabilityFact::ReadOnly
    );
    assert_eq!(
        validation.thrown,
        vec![JavaType::Named {
            name: "InternalCodecException".to_string(),
            args: Vec::new(),
        }]
    );

    let strict_validation = facts
        .external_api_effect_for_receiver(Some("VariantContext"), "extraStrictValidation")
        .expect("VariantContext aggregate validation should be modeled by the pack");
    assert_eq!(strict_validation.role, ExternalApiRole::SideEffect);
    for (method_name, parameter_index, parameter_type) in [
        ("extraStrictValidation", 0, "Allele"),
        ("extraStrictValidation", 1, "Allele"),
        ("extraStrictValidation", 2, "Set<String>"),
        ("validateReferenceBases", 0, "Allele"),
        ("validateReferenceBases", 1, "Allele"),
        ("validateRSIDs", 0, "Set<String>"),
    ] {
        let argument = facts
            .external_api_argument_effect_for_receiver(
                Some("VariantContext"),
                method_name,
                parameter_index,
            )
            .unwrap_or_else(|| {
                panic!(
                    "VariantContext.{method_name} parameter {parameter_index} should have a pack-owned argument boundary fact"
                )
            });
        assert_eq!(
            argument.parameter_type,
            Some(parser::parse_type(parameter_type))
        );
        assert_eq!(
            argument.parameter_nullability,
            NullabilityState::DefinitelyNonNull
        );
        assert_eq!(
            argument.argument_mutability,
            ReceiverMutabilityFact::ReadOnly
        );
        assert_eq!(argument.alias_policy, ExternalArgumentAliasPolicy::Borrowed);
        assert_eq!(argument.provenance, FactProvenance::ExternalLibraryPack);
    }

    let variant_alleles = facts
        .external_api_effect_for_receiver(Some("VariantContext"), "getAlleles")
        .expect("allele collection getter should be modeled by the pack");
    assert_eq!(
        variant_alleles.return_type,
        Some(JavaType::Named {
            name: "List".to_string(),
            args: vec![JavaType::Named {
                name: "Allele".to_string(),
                args: Vec::new(),
            }],
        })
    );

    let indexed_allele = facts
        .external_api_effect_for_receiver(Some("VariantContext"), "getAllele")
        .expect("indexed allele getter should be modeled by the pack");
    assert_eq!(indexed_allele.role, ExternalApiRole::IndexedGetter);
    assert_eq!(
        indexed_allele.return_type,
        Some(JavaType::Named {
            name: "Allele".to_string(),
            args: Vec::new(),
        })
    );
    assert_eq!(
        indexed_allele.return_nullability,
        NullabilityState::DefinitelyNonNull
    );
}

#[test]
fn variantcontext_exception_catches_use_pack_hierarchy_facts() {
    let source = r#"
import htsjdk.variant.variantcontext.VariantContext;
import htsjdk.tribble.TribbleException.InternalCodecException;

class VariantValidationCatchOrder {
    int validate(VariantContext variant) {
        try {
            variant.validateAlternateAlleles();
            variant.extraStrictValidation(null, null);
            return 1;
        } catch (InternalCodecException specific) {
            return 2;
        } catch (RuntimeException broader) {
            return 3;
        }
    }
}
"#;
    let parsed = parser::parse_java_source(source);
    let resolved = resolve::resolve_program(&parsed.program, parsed.diagnostics);
    let semantic = semantic_analysis::analyze_program(
        &resolved.program,
        &resolved.facts,
        &htsjdk_variantcontext_pack_facts(),
    );

    let effect = semantic
        .facts
        .external_api_effect_for_receiver(Some("VariantContext"), "validateAlternateAlleles")
        .expect("external validation call should keep its declared throw effect");
    assert_eq!(
        effect.thrown,
        vec![JavaType::Named {
            name: "InternalCodecException".to_string(),
            args: Vec::new(),
        }]
    );
    let strict_effect = semantic
        .facts
        .external_api_effect_for_receiver(Some("VariantContext"), "extraStrictValidation")
        .expect("external strict validation call should keep its declared throw effect");
    assert_eq!(
        strict_effect.thrown,
        vec![JavaType::Named {
            name: "InternalCodecException".to_string(),
            args: Vec::new(),
        }]
    );

    let catches = semantic
        .facts
        .catch_facts_for("VariantValidationCatchOrder", "validate");
    assert_eq!(catches.len(), 2);
    assert_eq!(
        catches[0].assignable_from,
        vec![JavaType::Named {
            name: "InternalCodecException".to_string(),
            args: Vec::new(),
        }]
    );
    assert!(
        catches[1].assignable_from.contains(&JavaType::Named {
            name: "InternalCodecException".to_string(),
            args: Vec::new(),
        }),
        "broader catch should be assignable from the pack-owned specific exception"
    );
    assert!(
        semantic.diagnostics.iter().all(|diagnostic| {
            diagnostic.code != "external_pack_missing_api"
                && diagnostic.code != "external_api_unsupported"
        }),
        "VariantContext exception flow should be covered by pack facts, got {:?}",
        semantic.diagnostics
    );
}

#[test]
fn semantic_analysis_uses_variantcontext_pack_for_builder_and_read_flow() {
    let source = r#"
import htsjdk.variant.variantcontext.Allele;
import htsjdk.variant.variantcontext.CommonInfo;
import htsjdk.variant.variantcontext.Genotype;
import htsjdk.variant.variantcontext.GenotypeBuilder;
import htsjdk.variant.variantcontext.GenotypesContext;
import htsjdk.variant.variantcontext.VariantContext;
import htsjdk.variant.variantcontext.VariantContextBuilder;
import java.util.Arrays;
import java.util.List;

class VariantContextPilotUse {
    VariantContext build(String contig, int start, String sample) {
        Allele reference = Allele.create("A", true);
        Allele alternate = Allele.create("T", false);
        List<Allele> alleles = Arrays.asList(reference, alternate);
        Genotype genotype = new GenotypeBuilder(sample, alleles).DP(12).GQ(30).make();
        GenotypesContext genotypes = GenotypesContext.create();
        genotypes.add(genotype);
        return new VariantContextBuilder()
            .chr(contig)
            .start(start)
            .stop(start)
            .alleles(alleles)
            .genotypes(genotypes)
            .attribute("DP", 12)
            .make();
    }

    int summarize(VariantContext variant) {
        Object maybeDepth = variant.getAttribute("DP");
        return variant.getStart() + variant.getEnd() + variant.getNAlleles()
            + variant.getGenotypes().size();
    }

    String firstAllele(VariantContext variant) {
        return variant.getAllele(0).getBaseString();
    }
}
"#;
    let parsed = parser::parse_java_source(source);
    let resolved = resolve::resolve_program(&parsed.program, parsed.diagnostics);
    let semantic = semantic_analysis::analyze_program(
        &resolved.program,
        &resolved.facts,
        &htsjdk_variantcontext_pack_facts(),
    );

    assert!(
        semantic.diagnostics.iter().all(|diagnostic| {
            diagnostic.code != "external_pack_missing_api"
                && diagnostic.code != "external_api_unsupported"
        }),
        "VariantContext pilot flow should be covered by pack facts, got {:?}",
        semantic.diagnostics
    );

    let allele_create = semantic
        .facts
        .external_api_effect_for_receiver(Some("Allele"), "create")
        .expect("Allele.create should be pack-owned in semantic facts");
    assert_eq!(
        allele_create.provenance,
        FactProvenance::ExternalLibraryPack
    );
    assert_eq!(
        allele_create.return_type,
        Some(JavaType::Named {
            name: "Allele".to_string(),
            args: Vec::new(),
        })
    );
    assert_eq!(
        allele_create.return_nullability,
        NullabilityState::DefinitelyNonNull
    );

    for method_name in ["chr", "start", "stop", "alleles", "genotypes", "attribute"] {
        let effect = semantic
            .facts
            .external_api_effect_for_receiver(Some("VariantContextBuilder"), method_name)
            .unwrap_or_else(|| {
                panic!("VariantContextBuilder.{method_name} should be modeled by the pack")
            });
        assert_eq!(effect.role, ExternalApiRole::FluentMutator);
        assert_eq!(
            effect.receiver_mutability,
            ReceiverMutabilityFact::MutatesReceiver
        );
        assert_eq!(
            effect.return_type,
            Some(JavaType::Named {
                name: "VariantContextBuilder".to_string(),
                args: Vec::new(),
            })
        );
    }

    let make = semantic
        .facts
        .external_api_effect_for_receiver(Some("VariantContextBuilder"), "make")
        .expect("VariantContextBuilder.make should be modeled by the pack");
    assert_eq!(
        make.return_type,
        Some(JavaType::Named {
            name: "VariantContext".to_string(),
            args: Vec::new(),
        })
    );
    assert_eq!(make.return_nullability, NullabilityState::DefinitelyNonNull);

    let nullable_attribute = semantic
        .facts
        .external_api_effect_for_receiver(Some("VariantContext"), "getAttribute")
        .expect("VariantContext.getAttribute should be modeled by the pack");
    assert_eq!(
        nullable_attribute.return_nullability,
        NullabilityState::MaybeNull
    );

    let genotypes = semantic
        .facts
        .external_api_effect_for_receiver(Some("VariantContext"), "getGenotypes")
        .expect("VariantContext.getGenotypes should be modeled by the pack");
    assert_eq!(
        genotypes.return_type,
        Some(JavaType::Named {
            name: "GenotypesContext".to_string(),
            args: Vec::new(),
        })
    );

    let element = semantic
        .facts
        .sequence_element_type_for(&JavaType::Named {
            name: "GenotypesContext".to_string(),
            args: Vec::new(),
        })
        .expect("GenotypesContext should remain sequence-like through semantic analysis");
    assert_eq!(
        element,
        JavaType::Named {
            name: "Genotype".to_string(),
            args: Vec::new(),
        }
    );
}

#[test]
fn semantic_analysis_uses_variantcontext_pack_for_read_flow() {
    let source = r#"
import htsjdk.variant.variantcontext.Allele;
import htsjdk.variant.variantcontext.GenotypesContext;
import htsjdk.variant.variantcontext.VariantContext;
import htsjdk.variant.variantcontext.VariantContext.Type;
import java.util.List;

class VariantContextReadFlowUse {
    String identity(VariantContext variant) {
        return variant.getContig()
            + variant.getID()
            + variant.getType().name()
            + variant.getAttributeAsString("CSQ", "missing");
    }

    int counts(VariantContext variant) {
        return variant.getStart()
            + variant.getEnd()
            + variant.getNAlleles()
            + variant.getNSamples()
            + variant.getGenotypes().size();
    }

    Object collections(VariantContext variant) {
        List<Allele> alleles = variant.getAlleles();
        GenotypesContext genotypes = variant.getGenotypes();
        return alleles.toString()
            + genotypes.getSampleNames()
            + variant.getAttributeAsStringList("ANN");
    }

    boolean predicates(VariantContext variant, Allele allele) {
        return variant.isSNP()
            || variant.isIndel()
            || variant.isBiallelic()
            || variant.hasAttribute("DP")
            || variant.hasAllele(allele);
    }

    Object types() {
        return Type.values();
    }
}
"#;
    let parsed = parser::parse_java_source(source);
    let resolved = resolve::resolve_program(&parsed.program, parsed.diagnostics);
    let semantic = semantic_analysis::analyze_program(
        &resolved.program,
        &resolved.facts,
        &htsjdk_variantcontext_pack_facts(),
    );

    assert!(
        semantic.diagnostics.iter().all(|diagnostic| {
            diagnostic.code != "external_pack_missing_api"
                && diagnostic.code != "external_api_unsupported"
        }),
        "VariantContext read flow should be covered by pack facts, got {:?}",
        semantic.diagnostics
    );

    for method_name in ["getContig", "getID"] {
        let effect = semantic
            .facts
            .external_api_effect_for_receiver(Some("VariantContext"), method_name)
            .unwrap_or_else(|| panic!("VariantContext.{method_name} should be modeled"));
        assert_eq!(effect.role, ExternalApiRole::Getter);
        assert_eq!(
            effect.return_nullability,
            NullabilityState::DefinitelyNonNull
        );
        assert_eq!(effect.provenance, FactProvenance::ExternalLibraryPack);
    }

    let attribute_as_string = semantic
        .facts
        .external_api_effect_for_receiver(Some("VariantContext"), "getAttributeAsString")
        .expect("VariantContext.getAttributeAsString should be modeled");
    assert_eq!(attribute_as_string.role, ExternalApiRole::Getter);
    assert_eq!(attribute_as_string.return_type, Some(JavaType::String));
    assert_eq!(
        attribute_as_string.return_nullability,
        NullabilityState::MaybeNull
    );
    assert_eq!(
        attribute_as_string.provenance,
        FactProvenance::ExternalLibraryPack
    );

    let variant_type = semantic
        .facts
        .external_api_effect_for_receiver(Some("VariantContext"), "getType")
        .expect("VariantContext.getType should be modeled");
    assert_eq!(
        variant_type.return_type,
        Some(JavaType::Named {
            name: "Type".to_string(),
            args: Vec::new(),
        })
    );

    let enum_values = semantic
        .facts
        .external_api_effect_for_receiver(Some("Type"), "values")
        .expect("VariantContext.Type.values should be modeled");
    assert_eq!(enum_values.role, ExternalApiRole::Getter);
    assert_eq!(
        enum_values.return_type,
        Some(JavaType::Array(Box::new(JavaType::Named {
            name: "Type".to_string(),
            args: Vec::new(),
        })))
    );
    assert_eq!(
        enum_values.return_nullability,
        NullabilityState::DefinitelyNonNull
    );

    let string_list = semantic
        .facts
        .external_api_effect_for_receiver(Some("VariantContext"), "getAttributeAsStringList")
        .expect("VariantContext string-list attribute getter should be modeled");
    assert_eq!(
        string_list.return_type,
        Some(JavaType::Named {
            name: "List".to_string(),
            args: vec![JavaType::String],
        })
    );
    assert_eq!(
        string_list.return_nullability,
        NullabilityState::DefinitelyNonNull
    );

    let genotypes = semantic
        .facts
        .external_api_effect_for_receiver(Some("VariantContext"), "getGenotypes")
        .expect("VariantContext.getGenotypes should be modeled");
    assert_eq!(
        genotypes.return_type,
        Some(JavaType::Named {
            name: "GenotypesContext".to_string(),
            args: Vec::new(),
        })
    );

    for method_name in [
        "isSNP",
        "isIndel",
        "isBiallelic",
        "hasAttribute",
        "hasAllele",
    ] {
        let predicate = semantic
            .facts
            .external_api_effect_for_receiver(Some("VariantContext"), method_name)
            .unwrap_or_else(|| panic!("VariantContext.{method_name} should be modeled"));
        assert_eq!(predicate.role, ExternalApiRole::BooleanGetter);
        assert_eq!(
            predicate.return_type,
            Some(JavaType::Primitive(PrimitiveType::Boolean))
        );
    }
}

#[test]
fn semantic_analysis_uses_variantcontext_pack_for_genotype_flow() {
    let source = r#"
import htsjdk.variant.variantcontext.Allele;
import htsjdk.variant.variantcontext.Genotype;
import htsjdk.variant.variantcontext.GenotypeBuilder;
import htsjdk.variant.variantcontext.GenotypesContext;
import htsjdk.variant.variantcontext.VariantContext;
import java.util.Arrays;
import java.util.Iterator;

class VariantContextGenotypeFlowUse {
    Genotype build(String sample, Allele reference, Allele alternate) {
        return new GenotypeBuilder(sample, Arrays.asList(reference, alternate))
            .DP(10)
            .GQ(20)
            .make();
    }

    GenotypesContext store(Genotype genotype) {
        GenotypesContext genotypes = GenotypesContext.create();
        genotypes.add(genotype);
        genotypes.replace(genotype);
        return genotypes.copy();
    }

    Genotype sampleLookup(VariantContext variant, GenotypesContext genotypes) {
        Genotype bySample = variant.getGenotype("SAMPLE");
        Genotype byIndex = genotypes.get(0);
        return bySample.sameGenotype(byIndex) ? bySample : byIndex;
    }

    Object iteration(GenotypesContext genotypes) {
        Iterator<Genotype> iterator = genotypes.iterator();
        Object stream = genotypes.stream();
        Object ordered = genotypes.iterateInSampleNameOrder();
        Object spliterator = genotypes.spliterator();
        return iterator.toString() + stream + ordered + spliterator + genotypes.getSampleNames();
    }
}
"#;
    let parsed = parser::parse_java_source(source);
    let resolved = resolve::resolve_program(&parsed.program, parsed.diagnostics);
    let semantic = semantic_analysis::analyze_program(
        &resolved.program,
        &resolved.facts,
        &htsjdk_variantcontext_pack_facts(),
    );

    assert!(
        semantic.diagnostics.iter().all(|diagnostic| {
            diagnostic.code != "external_pack_missing_api"
                && diagnostic.code != "external_api_unsupported"
        }),
        "VariantContext genotype flow should be covered by pack facts, got {:?}",
        semantic.diagnostics
    );

    let make = semantic
        .facts
        .external_api_effect_for_receiver(Some("GenotypeBuilder"), "make")
        .expect("GenotypeBuilder.make should be modeled by the pack");
    assert_eq!(
        make.return_type,
        Some(JavaType::Named {
            name: "Genotype".to_string(),
            args: Vec::new(),
        })
    );
    assert_eq!(make.return_nullability, NullabilityState::DefinitelyNonNull);

    for method_name in ["DP", "GQ"] {
        let effect = semantic
            .facts
            .external_api_effect_for_receiver(Some("GenotypeBuilder"), method_name)
            .unwrap_or_else(|| panic!("GenotypeBuilder.{method_name} should be modeled"));
        assert_eq!(effect.role, ExternalApiRole::FluentMutator);
        assert_eq!(
            effect.receiver_mutability,
            ReceiverMutabilityFact::MutatesReceiver
        );
    }

    for method_name in ["add", "replace"] {
        let effect = semantic
            .facts
            .external_api_effect_for_receiver(Some("GenotypesContext"), method_name)
            .unwrap_or_else(|| panic!("GenotypesContext.{method_name} should be modeled"));
        assert_eq!(effect.role, ExternalApiRole::Mutator);
        assert_eq!(
            effect.receiver_mutability,
            ReceiverMutabilityFact::MutatesReceiver
        );
    }

    let copy = semantic
        .facts
        .external_api_effect_for_receiver(Some("GenotypesContext"), "copy")
        .expect("GenotypesContext.copy should be modeled by the pack");
    assert_eq!(
        copy.return_type,
        Some(JavaType::Named {
            name: "GenotypesContext".to_string(),
            args: Vec::new(),
        })
    );
    assert_eq!(copy.return_nullability, NullabilityState::DefinitelyNonNull);

    let sample = semantic
        .facts
        .external_api_effect_for_receiver(Some("VariantContext"), "getGenotype")
        .expect("VariantContext sample genotype lookup should be modeled by the pack");
    assert_eq!(
        sample.return_type,
        Some(JavaType::Named {
            name: "Genotype".to_string(),
            args: Vec::new(),
        })
    );

    let indexed = semantic
        .facts
        .external_api_effect_for_receiver(Some("GenotypesContext"), "get")
        .expect("GenotypesContext indexed genotype lookup should be modeled by the pack");
    assert_eq!(indexed.role, ExternalApiRole::IndexedGetter);
    assert_eq!(
        indexed.return_type,
        Some(JavaType::Named {
            name: "Genotype".to_string(),
            args: Vec::new(),
        })
    );

    for method_name in [
        "iterator",
        "stream",
        "iterateInSampleNameOrder",
        "spliterator",
    ] {
        let effect = semantic
            .facts
            .external_api_effect_for_receiver(Some("GenotypesContext"), method_name)
            .unwrap_or_else(|| panic!("GenotypesContext.{method_name} should be modeled"));
        assert_eq!(effect.role, ExternalApiRole::Getter);
        assert_eq!(
            effect.return_nullability,
            NullabilityState::DefinitelyNonNull
        );
    }

    let element = semantic
        .facts
        .sequence_element_type_for(&JavaType::Named {
            name: "GenotypesContext".to_string(),
            args: Vec::new(),
        })
        .expect("GenotypesContext should remain sequence-like in genotype flow");
    assert_eq!(
        element,
        JavaType::Named {
            name: "Genotype".to_string(),
            args: Vec::new(),
        }
    );
}

#[test]
fn htsjdk_variantcontext_pack_models_sequence_like_genotypes_context() {
    let facts = htsjdk_variantcontext_pack_facts();
    let element = facts
        .sequence_element_type_for(&JavaType::Named {
            name: "GenotypesContext".to_string(),
            args: Vec::new(),
        })
        .expect("GenotypesContext should be sequence-like");

    assert_eq!(
        element,
        JavaType::Named {
            name: "Genotype".to_string(),
            args: Vec::new(),
        }
    );
}

#[test]
fn htsjdk_bcf_pack_declares_codec_boundary() {
    let facts = htsjdk_bcf_pack_facts();

    let pack = facts
        .external_library_pack_for_type("htsjdk.variant.bcf2.BCF2Codec")
        .expect("BCF2Codec should be owned by the BCF pack");
    assert_eq!(pack.pack_name, "htsjdk.variant.bcf2");
    assert_eq!(pack.mode, ExternalLibraryMode::ExternalFactModel);
    assert_eq!(pack.coverage, ExternalApiCoverage::Semantic);
    assert_eq!(pack.provenance, FactProvenance::ExternalLibraryPack);

    let constructor = facts
        .external_constructor_for("BCF2Codec", &[])
        .expect("BCF2Codec/0 constructor should be pack-owned");
    assert_eq!(constructor.provenance, FactProvenance::ExternalLibraryPack);
    assert_eq!(
        new_java2rust::stdlib::external_constructor_provenance("BCF2Codec", &[]),
        FactProvenance::ConservativeFallback,
        "BCF2Codec/0 precision should come from the BCF pack, not stdlib"
    );
}

#[test]
fn htsjdk_vcf_pack_declares_header_metadata_boundary() {
    let facts = htsjdk_vcf_pack_facts();

    let pack = facts
        .external_library_pack_for_type("htsjdk.variant.vcf.VCFHeader")
        .expect("VCF package should be owned by the external pack");
    assert_eq!(pack.pack_name, "htsjdk.variant.vcf");
    assert_eq!(pack.mode, ExternalLibraryMode::ExternalFactModel);
    assert_eq!(pack.coverage, ExternalApiCoverage::Semantic);
    assert_eq!(pack.provenance, FactProvenance::ExternalLibraryPack);

    let header = facts
        .external_constructor_for("VCFHeader", &[])
        .expect("VCFHeader constructor should be pack-owned");
    assert_eq!(header.provenance, FactProvenance::ExternalLibraryPack);
    for (type_name, args) in [
        (
            "VCFInfoHeaderLine",
            vec![JavaType::String, JavaType::String, JavaType::String],
        ),
        (
            "VCFFormatHeaderLine",
            vec![JavaType::String, JavaType::String, JavaType::String],
        ),
        (
            "VCFFilterHeaderLine",
            vec![JavaType::String, JavaType::String],
        ),
        ("VCFHeaderLine", vec![JavaType::String, JavaType::String]),
        ("VCFHeader", Vec::new()),
        ("VCFCodec", Vec::new()),
        (
            "VCFRecordCodec",
            vec![
                JavaType::Named {
                    name: "VCFHeader".to_string(),
                    args: Vec::new(),
                },
                JavaType::Primitive(new_java2rust::java_ir::PrimitiveType::Boolean),
            ],
        ),
    ] {
        assert_eq!(
            new_java2rust::stdlib::external_constructor_provenance(type_name, &args),
            FactProvenance::ConservativeFallback,
            "{type_name}/{} constructor precision should come from the VCF pack, not stdlib fallback",
            args.len()
        );
    }

    let reader = facts
        .external_constructor_for(
            "VCFFileReader",
            &[JavaType::Named {
                name: "Path".to_string(),
                args: Vec::new(),
            }],
        )
        .expect("VCFFileReader path constructor should be pack-owned");
    assert_eq!(reader.provenance, FactProvenance::ExternalLibraryPack);

    let simple_header_line = facts
        .external_constructor_for(
            "VCFSimpleHeaderLine",
            &[
                JavaType::String,
                JavaType::Named {
                    name: "Map".to_string(),
                    args: vec![JavaType::String, JavaType::String],
                },
            ],
        )
        .expect("VCFSimpleHeaderLine mapping constructor should be pack-owned");
    assert_eq!(
        simple_header_line.provenance,
        FactProvenance::ExternalLibraryPack
    );

    let named_simple_header_line = facts
        .external_constructor_for(
            "VCFSimpleHeaderLine",
            &[JavaType::String, JavaType::String, JavaType::String],
        )
        .expect("VCFSimpleHeaderLine named constructor should be pack-owned");
    assert_eq!(
        named_simple_header_line.provenance,
        FactProvenance::ExternalLibraryPack
    );

    let file_header = facts
        .external_api_effect_for_receiver(Some("VCFFileReader"), "getFileHeader")
        .expect("VCFFileReader header getter should be pack-owned");
    assert_eq!(
        file_header.return_type,
        Some(JavaType::Named {
            name: "VCFHeader".to_string(),
            args: Vec::new(),
        })
    );

    let iterator = facts
        .external_api_effect_for_receiver(Some("VCFFileReader"), "iterator")
        .expect("VCFFileReader iterator should be pack-owned");
    assert_eq!(
        iterator.return_type,
        Some(JavaType::Named {
            name: "CloseableIterator".to_string(),
            args: vec![JavaType::Named {
                name: "VariantContext".to_string(),
                args: Vec::new(),
            }],
        })
    );
}

#[test]
fn htsjdk_vcf_pack_models_header_getters_and_mutators() {
    let facts = htsjdk_vcf_pack_facts();

    let samples = facts
        .external_api_effect_for_receiver(Some("VCFHeader"), "getGenotypeSamples")
        .expect("VCFHeader samples getter should be modeled by the pack");
    assert_eq!(samples.role, ExternalApiRole::Getter);
    assert_eq!(
        samples.receiver_mutability,
        ReceiverMutabilityFact::ReadOnly
    );
    assert_eq!(
        samples.return_nullability,
        NullabilityState::DefinitelyNonNull
    );
    assert_eq!(samples.provenance, FactProvenance::ExternalLibraryPack);

    let ordered_samples = facts
        .external_api_effect_for_receiver(Some("VCFHeader"), "getSampleNamesInOrder")
        .expect("VCFHeader ordered sample getter should be modeled by the pack");
    assert_eq!(
        ordered_samples.return_type,
        Some(JavaType::Named {
            name: "List".to_string(),
            args: vec![JavaType::String],
        })
    );
    assert_eq!(
        ordered_samples.return_nullability,
        NullabilityState::DefinitelyNonNull
    );

    let sorted_metadata = facts
        .external_api_effect_for_receiver(Some("VCFHeader"), "getMetaDataInSortedOrder")
        .expect("VCFHeader sorted metadata getter should be modeled by the pack");
    assert_eq!(
        sorted_metadata.return_type,
        Some(JavaType::Named {
            name: "Set".to_string(),
            args: vec![JavaType::Named {
                name: "VCFHeaderLine".to_string(),
                args: Vec::new(),
            }],
        })
    );

    let genotype_count = facts
        .external_api_effect_for_receiver(Some("VCFHeader"), "getNGenotypeSamples")
        .expect("VCFHeader genotype sample count should be modeled by the pack");
    assert_eq!(
        genotype_count.return_type,
        Some(JavaType::Primitive(
            new_java2rust::java_ir::PrimitiveType::Int
        ))
    );

    for (field_name, expected_ty) in [
        ("PASSES_FILTERS_v4", JavaType::String),
        ("MISSING_VALUE_v4", JavaType::String),
        ("EMPTY_ID_FIELD", JavaType::String),
        ("EMPTY_INFO_FIELD", JavaType::String),
        ("GENOTYPE_ALLELE_DEPTHS", JavaType::String),
        ("SVTYPE", JavaType::String),
        ("NULL_ALLELE", JavaType::String),
        ("UNFILTERED", JavaType::String),
        ("DEPTH_KEY", JavaType::String),
        ("ALLELE_NUMBER_KEY", JavaType::String),
        ("ALLELE_COUNT_KEY", JavaType::String),
        ("ALLELE_FREQUENCY_KEY", JavaType::String),
        ("END_KEY", JavaType::String),
        ("GENOTYPE_KEY", JavaType::String),
        ("RMS_MAPPING_QUALITY_KEY", JavaType::String),
        ("DBSNP_KEY", JavaType::String),
        ("GENOTYPE_FILTER_KEY", JavaType::String),
        ("GENOTYPE_PL_KEY", JavaType::String),
        ("GENOTYPE_POSTERIORS_KEY", JavaType::String),
        ("ID_FIELD_SEPARATOR", JavaType::String),
        ("INFO_FIELD_ARRAY_SEPARATOR", JavaType::String),
        (
            "INFO_FIELD_ARRAY_SEPARATOR_CHAR",
            JavaType::Primitive(PrimitiveType::Char),
        ),
        ("MAX_GENOTYPE_QUAL", JavaType::Primitive(PrimitiveType::Int)),
    ] {
        let field = facts
            .external_field_for("VCFConstants", field_name)
            .unwrap_or_else(|| panic!("VCFConstants.{field_name} should be pack-owned"));
        assert_eq!(field.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(field.field_type, Some(expected_ty));
        let dispatched =
            new_java2rust::external_packs::external_field_for("VCFConstants", field_name)
                .unwrap_or_else(|| {
                    panic!("VCFConstants.{field_name} should be globally pack-dispatched")
                });
        assert_eq!(dispatched.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(
            new_java2rust::stdlib::external_field_provenance("VCFConstants", field_name),
            FactProvenance::ConservativeFallback,
            "VCFConstants.{field_name} precision should come from the VCF pack, not stdlib"
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
        let field = facts
            .external_field_for("VCFHeader", field_name)
            .unwrap_or_else(|| panic!("VCFHeader.{field_name} should be pack-owned"));
        assert_eq!(field.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(field.field_type, Some(expected_ty));
        assert_eq!(
            new_java2rust::stdlib::external_field_provenance("VCFHeader", field_name),
            FactProvenance::ConservativeFallback,
            "VCFHeader.{field_name} precision should come from the VCF pack, not stdlib"
        );
    }

    let supported_header_line_type = facts
        .external_field_for("VCFCompoundHeaderLine", "SupportedHeaderLineType")
        .expect("VCFCompoundHeaderLine.SupportedHeaderLineType should be pack-owned");
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
        new_java2rust::stdlib::external_field_provenance(
            "VCFCompoundHeaderLine",
            "SupportedHeaderLineType"
        ),
        FactProvenance::ConservativeFallback,
        "VCFCompoundHeaderLine.SupportedHeaderLineType precision should come from the VCF pack, not stdlib"
    );

    for field_name in ["FORMAT", "INFO"] {
        let field = facts
            .external_field_for("SupportedHeaderLineType", field_name)
            .unwrap_or_else(|| panic!("SupportedHeaderLineType.{field_name} should be pack-owned"));
        assert_eq!(field.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(
            field.field_type,
            Some(JavaType::Named {
                name: "SupportedHeaderLineType".to_string(),
                args: Vec::new(),
            })
        );
        let dispatched = new_java2rust::external_packs::external_field_for(
            "VCFCompoundHeaderLine.SupportedHeaderLineType",
            field_name,
        )
        .unwrap_or_else(|| {
            panic!("SupportedHeaderLineType.{field_name} should be globally pack-dispatched")
        });
        assert_eq!(dispatched.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(
            new_java2rust::stdlib::external_field_provenance("SupportedHeaderLineType", field_name),
            FactProvenance::ConservativeFallback,
            "SupportedHeaderLineType.{field_name} precision should come from the VCF pack, not stdlib"
        );
    }

    let writer_output_type = facts
        .external_field_for("VariantContextWriterBuilder", "OutputType")
        .expect("VariantContextWriterBuilder.OutputType should be pack-owned");
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
        new_java2rust::stdlib::external_field_provenance(
            "VariantContextWriterBuilder",
            "OutputType"
        ),
        FactProvenance::ConservativeFallback,
        "VariantContextWriterBuilder.OutputType precision should come from the VCF pack, not stdlib"
    );
    let default_options = facts
        .external_field_for("VariantContextWriterBuilder", "DEFAULT_OPTIONS")
        .expect("VariantContextWriterBuilder.DEFAULT_OPTIONS should be pack-owned");
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
    let dispatched = new_java2rust::external_packs::external_field_for(
        "VariantContextWriterBuilder",
        "DEFAULT_OPTIONS",
    )
    .expect("VariantContextWriterBuilder.DEFAULT_OPTIONS should be globally pack-dispatched");
    assert_eq!(dispatched.provenance, FactProvenance::ExternalLibraryPack);
    assert_eq!(
        new_java2rust::stdlib::external_field_provenance(
            "VariantContextWriterBuilder",
            "DEFAULT_OPTIONS"
        ),
        FactProvenance::ConservativeFallback,
        "VariantContextWriterBuilder.DEFAULT_OPTIONS precision should come from the VCF pack, not stdlib"
    );

    let writer_builder = facts
        .external_constructor_for("VariantContextWriterBuilder", &[])
        .expect("VariantContextWriterBuilder zero-arg constructor should be pack-owned");
    assert_eq!(
        writer_builder.provenance,
        FactProvenance::ExternalLibraryPack
    );

    for method_name in [
        "clearOptions",
        "setCreateMD5",
        "setOption",
        "setOutputFileType",
        "setReferenceDictionary",
        "setOutputPath",
    ] {
        let effect = facts
            .external_api_effect_for_receiver(Some("VariantContextWriterBuilder"), method_name)
            .unwrap_or_else(|| {
                panic!(
                    "VariantContextWriterBuilder.{method_name} should be modeled by the VCF pack"
                )
            });
        assert_eq!(effect.role, ExternalApiRole::FluentMutator);
        assert_eq!(
            effect.receiver_mutability,
            ReceiverMutabilityFact::MutatesReceiver
        );
        assert_eq!(
            effect.return_type,
            Some(JavaType::Named {
                name: "VariantContextWriterBuilder".to_string(),
                args: Vec::new(),
            })
        );
    }

    let output_type = facts
        .external_api_effect_for_receiver(
            Some("VariantContextWriterBuilder"),
            "determineOutputTypeFromFile",
        )
        .expect("VariantContextWriterBuilder output-type helper should be modeled");
    assert_eq!(output_type.role, ExternalApiRole::Getter);
    assert_eq!(
        output_type.return_type,
        Some(JavaType::Named {
            name: "OutputType".to_string(),
            args: Vec::new(),
        })
    );

    let build = facts
        .external_api_effect_for_receiver(Some("VariantContextWriterBuilder"), "build")
        .expect("VariantContextWriterBuilder.build should be modeled");
    assert_eq!(build.role, ExternalApiRole::Getter);
    assert_eq!(
        build.return_type,
        Some(JavaType::Named {
            name: "VariantContextWriter".to_string(),
            args: Vec::new(),
        })
    );

    for method_name in ["setHeader", "writeHeader", "add", "close"] {
        let effect = facts
            .external_api_effect_for_receiver(Some("VariantContextWriter"), method_name)
            .unwrap_or_else(|| panic!("VariantContextWriter.{method_name} should be modeled"));
        assert_eq!(effect.role, ExternalApiRole::Mutator);
        assert_eq!(
            effect.receiver_mutability,
            ReceiverMutabilityFact::MutatesReceiver
        );
    }

    let check_error = facts
        .external_api_effect_for_receiver(Some("VariantContextWriter"), "checkError")
        .expect("VariantContextWriter.checkError should be modeled");
    assert_eq!(check_error.role, ExternalApiRole::BooleanGetter);
    assert_eq!(
        check_error.return_type,
        Some(JavaType::Primitive(PrimitiveType::Boolean))
    );

    let sequence_dictionary = facts
        .external_api_effect_for_receiver(Some("VCFHeader"), "getSequenceDictionary")
        .expect("VCFHeader sequence dictionary getter should be modeled by the pack");
    assert_eq!(sequence_dictionary.role, ExternalApiRole::Getter);
    assert_eq!(
        sequence_dictionary.return_nullability,
        NullabilityState::DefinitelyNonNull
    );

    let info_line = facts
        .external_api_effect_for_receiver(Some("VCFHeader"), "getInfoHeaderLine")
        .expect("VCFHeader keyed info-line getter should be modeled by the pack");
    assert_eq!(
        info_line.return_type,
        Some(JavaType::Named {
            name: "VCFInfoHeaderLine".to_string(),
            args: Vec::new(),
        })
    );
    assert_eq!(info_line.return_nullability, NullabilityState::MaybeNull);

    let has_info_line = facts
        .external_api_effect_for_receiver(Some("VCFHeader"), "hasInfoLine")
        .expect("VCFHeader info-line predicate should be modeled by the pack");
    assert_eq!(has_info_line.role, ExternalApiRole::BooleanGetter);

    let format_line = facts
        .external_api_effect_for_receiver(Some("VCFHeader"), "getFormatHeaderLine")
        .expect("VCFHeader keyed format-line getter should be modeled by the pack");
    assert_eq!(
        format_line.return_type,
        Some(JavaType::Named {
            name: "VCFFormatHeaderLine".to_string(),
            args: Vec::new(),
        })
    );
    assert_eq!(format_line.return_nullability, NullabilityState::MaybeNull);

    let comparator = facts
        .external_api_effect_for_receiver(Some("VCFHeader"), "getVCFRecordComparator")
        .expect("VCFHeader record comparator getter should be modeled by the pack");
    assert_eq!(
        comparator.return_type,
        Some(JavaType::Named {
            name: "Comparator".to_string(),
            args: vec![JavaType::Named {
                name: "VariantContext".to_string(),
                args: Vec::new(),
            }],
        })
    );

    let metadata_line = facts
        .external_api_effect_for_receiver(Some("VCFHeader"), "getMetaDataLine")
        .expect("VCFHeader keyed metadata getter should be modeled by the pack");
    assert_eq!(
        metadata_line.return_type,
        Some(JavaType::Named {
            name: "VCFHeaderLine".to_string(),
            args: Vec::new(),
        })
    );
    assert_eq!(
        metadata_line.return_nullability,
        NullabilityState::MaybeNull
    );

    let add_line = facts
        .external_api_effect_for_receiver(Some("VCFHeader"), "addMetaDataLine")
        .expect("VCFHeader metadata mutation should be modeled by the pack");
    assert_eq!(add_line.role, ExternalApiRole::Mutator);
    assert_eq!(
        add_line.receiver_mutability,
        ReceiverMutabilityFact::MutatesReceiver
    );

    let set_dictionary = facts
        .external_api_effect_for_receiver(Some("VCFHeader"), "setSequenceDictionary")
        .expect("VCFHeader sequence dictionary setter should be modeled by the pack");
    assert_eq!(set_dictionary.role, ExternalApiRole::Mutator);

    let format_string = facts
        .external_api_effect_for_receiver(Some("VCFHeaderVersion"), "getFormatString")
        .expect("VCFHeaderVersion format-string getter should be modeled by the pack");
    assert_eq!(format_string.return_type, Some(JavaType::String));

    let version_string = facts
        .external_api_effect_for_receiver(Some("VCFHeaderVersion"), "getVersionString")
        .expect("VCFHeaderVersion version-string getter should be modeled by the pack");
    assert_eq!(version_string.return_type, Some(JavaType::String));
}

#[test]
fn semantic_analysis_uses_vcf_pack_for_header_metadata_calls() {
    let semantic = analyze(
        r#"
import htsjdk.variant.vcf.VCFHeader;
import htsjdk.variant.vcf.VCFFileReader;
import htsjdk.variant.vcf.VCFHeaderLine;
import htsjdk.variant.vcf.VCFHeaderLineType;
import htsjdk.variant.vcf.VCFHeaderVersion;
import htsjdk.variant.vcf.VCFFormatHeaderLine;
import htsjdk.variant.vcf.VCFInfoHeaderLine;
import htsjdk.variant.vcf.VCFSimpleHeaderLine;
import htsjdk.variant.vcf.VCFConstants;
import htsjdk.variant.variantcontext.writer.Options;
import htsjdk.variant.variantcontext.writer.VariantContextWriter;
import htsjdk.variant.variantcontext.writer.VariantContextWriterBuilder;
import htsjdk.variant.variantcontext.VariantContext;
import htsjdk.samtools.SAMSequenceDictionary;
import java.io.File;
import java.nio.file.Path;
import java.util.Map;

class VcfPackUse {
    VCFHeader header() {
        VCFHeader header = new VCFHeader();
        header.addMetaDataLine(new VCFHeaderLine("source", "tool"));
        header.addMetaDataLine(new VCFInfoHeaderLine("DP", 1, VCFHeaderLineType.Integer, "Depth"));
        header.setSequenceDictionary(header.getSequenceDictionary());
        return header;
    }

    Object samples(VCFHeader header) {
        return header.getGenotypeSamples();
    }

    Object orderedSamples(VCFHeader header) {
        return header.getSampleNamesInOrder();
    }

    Object sortedMetadata(VCFHeader header) {
        return header.getMetaDataInSortedOrder();
    }

    int genotypeCount(VCFHeader header) {
        return header.getNGenotypeSamples();
    }

    Object dictionary(VCFHeader header) {
        return header.getSequenceDictionary();
    }

    Object infoLine(VCFHeader header) {
        return header.getInfoHeaderLine("DP");
    }

    boolean hasInfoLine(VCFHeader header) {
        return header.hasInfoLine("DP");
    }

    boolean hasFormatLine(VCFHeader header) {
        return header.hasFormatLine("GT");
    }

    boolean hasGenotypingData(VCFHeader header) {
        return header.hasGenotypingData();
    }

    Object formatLine(VCFHeader header) {
        return header.getFormatHeaderLine("GT");
    }

    Object comparator(VCFHeader header) {
        return header.getVCFRecordComparator();
    }

    Object metadataLine(VCFHeader header) {
        return header.getMetaDataLine("source");
    }

    Object otherHeaderLine(VCFHeader header) {
        return header.getOtherHeaderLine("source");
    }

    String version(VCFHeaderVersion version) {
        return version.getFormatString() + version.getVersionString();
    }

    boolean fixedCount(VCFInfoHeaderLine infoLine, VCFFormatHeaderLine formatLine) {
        return infoLine.isFixedCount() || formatLine.isFixedCount();
    }

    Object parsedType(String name) {
        return VCFHeaderLineType.valueOf(name);
    }

    VCFFileReader reader(Path sourcePath) {
        return new VCFFileReader(sourcePath);
    }

    VCFSimpleHeaderLine simpleLine(String key, Map<String, String> attributes) {
        return new VCFSimpleHeaderLine(key, attributes);
    }

    VCFSimpleHeaderLine namedSimpleLine(String key, String name, String description) {
        return new VCFSimpleHeaderLine(key, name, description);
    }

    VCFHeader readerHeader(VCFFileReader reader) {
        return reader.getFileHeader();
    }

    Object readerIterator(VCFFileReader reader) {
        return reader.iterator();
    }

    String emptyMarkers() {
        return VCFConstants.EMPTY_ID_FIELD + VCFConstants.EMPTY_INFO_FIELD;
    }

    VariantContextWriter writer(File output, SAMSequenceDictionary dictionary) {
        VariantContextWriterBuilder builder = new VariantContextWriterBuilder();
        return builder.clearOptions()
            .setReferenceDictionary(dictionary)
            .setOutputPath(output.toPath())
            .setOutputFileType(builder.determineOutputTypeFromFile(output))
            .setOption(Options.INDEX_ON_THE_FLY)
            .setCreateMD5(false)
            .build();
    }

    void writeVariant(VariantContextWriter writer, VCFHeader header, VariantContext variant) {
        writer.setHeader(header);
        writer.writeHeader(header);
        writer.add(variant);
        if (writer.checkError()) {
            throw new IllegalStateException("writer failed");
        }
        writer.close();
    }
}
"#,
    );

    let samples = semantic
        .facts
        .external_api_effect_for_receiver(Some("VCFHeader"), "getGenotypeSamples")
        .unwrap();
    assert_eq!(samples.provenance, FactProvenance::ExternalLibraryPack);
    assert_eq!(
        samples.return_nullability,
        NullabilityState::DefinitelyNonNull
    );

    let ordered_samples = semantic
        .facts
        .external_api_effect_for_receiver(Some("VCFHeader"), "getSampleNamesInOrder")
        .expect("VCFHeader ordered sample getter should be modeled by the pack");
    assert_eq!(
        ordered_samples.return_type,
        Some(JavaType::Named {
            name: "List".to_string(),
            args: vec![JavaType::String],
        })
    );
    assert_eq!(
        ordered_samples.provenance,
        FactProvenance::ExternalLibraryPack
    );

    let sorted_metadata = semantic
        .facts
        .external_api_effect_for_receiver(Some("VCFHeader"), "getMetaDataInSortedOrder")
        .expect("VCFHeader sorted metadata getter should be modeled by the pack");
    assert_eq!(
        sorted_metadata.return_nullability,
        NullabilityState::DefinitelyNonNull
    );

    let genotype_count = semantic
        .facts
        .external_api_effect_for_receiver(Some("VCFHeader"), "getNGenotypeSamples")
        .expect("VCFHeader genotype sample count should be modeled by the pack");
    assert_eq!(
        genotype_count.return_type,
        Some(JavaType::Primitive(
            new_java2rust::java_ir::PrimitiveType::Int
        ))
    );

    let info_line = semantic
        .facts
        .external_api_effect_for_receiver(Some("VCFHeader"), "getInfoHeaderLine")
        .expect("VCFHeader info-line lookup should be modeled by the pack");
    assert_eq!(info_line.return_nullability, NullabilityState::MaybeNull);
    assert_eq!(info_line.provenance, FactProvenance::ExternalLibraryPack);

    let has_info_line = semantic
        .facts
        .external_api_effect_for_receiver(Some("VCFHeader"), "hasInfoLine")
        .expect("VCFHeader info-line predicate should be modeled by the pack");
    assert_eq!(has_info_line.role, ExternalApiRole::BooleanGetter);

    for method_name in ["hasFormatLine", "hasGenotypingData"] {
        let predicate = semantic
            .facts
            .external_api_effect_for_receiver(Some("VCFHeader"), method_name)
            .expect("VCFHeader predicate should be modeled by the pack");
        assert_eq!(predicate.role, ExternalApiRole::BooleanGetter);
        assert_eq!(
            predicate.return_nullability,
            NullabilityState::DefinitelyNonNull
        );
    }

    let format_line = semantic
        .facts
        .external_api_effect_for_receiver(Some("VCFHeader"), "getFormatHeaderLine")
        .expect("VCFHeader format-line lookup should be modeled by the pack");
    assert_eq!(
        format_line.return_type,
        Some(JavaType::Named {
            name: "VCFFormatHeaderLine".to_string(),
            args: Vec::new(),
        })
    );

    let metadata_line = semantic
        .facts
        .external_api_effect_for_receiver(Some("VCFHeader"), "getMetaDataLine")
        .expect("VCFHeader metadata lookup should be modeled by the pack");
    assert_eq!(
        metadata_line.return_type,
        Some(JavaType::Named {
            name: "VCFHeaderLine".to_string(),
            args: Vec::new(),
        })
    );

    let other_header_line = semantic
        .facts
        .external_api_effect_for_receiver(Some("VCFHeader"), "getOtherHeaderLine")
        .expect("VCFHeader other-line lookup should be modeled by the pack");
    assert_eq!(
        other_header_line.return_type,
        Some(JavaType::Named {
            name: "VCFHeaderLine".to_string(),
            args: Vec::new(),
        })
    );
    assert_eq!(
        other_header_line.return_nullability,
        NullabilityState::MaybeNull
    );

    let add_line = semantic
        .facts
        .external_api_effect_for_receiver(Some("VCFHeader"), "addMetaDataLine")
        .unwrap();
    assert_eq!(add_line.role, ExternalApiRole::Mutator);
    assert_eq!(add_line.provenance, FactProvenance::ExternalLibraryPack);

    let set_dictionary = semantic
        .facts
        .external_api_effect_for_receiver(Some("VCFHeader"), "setSequenceDictionary")
        .expect("VCFHeader sequence dictionary setter should be modeled by the pack");
    assert_eq!(set_dictionary.role, ExternalApiRole::Mutator);

    let format_string = semantic
        .facts
        .external_api_effect_for_receiver(Some("VCFHeaderVersion"), "getFormatString")
        .expect("VCFHeaderVersion format-string getter should be modeled by the pack");
    assert_eq!(format_string.return_type, Some(JavaType::String));

    for receiver in ["VCFInfoHeaderLine", "VCFFormatHeaderLine"] {
        let fixed_count = semantic
            .facts
            .external_api_effect_for_receiver(Some(receiver), "isFixedCount")
            .expect("VCF compound header fixed-count predicate should be modeled by the pack");
        assert_eq!(fixed_count.role, ExternalApiRole::BooleanGetter);
        assert_eq!(
            fixed_count.return_nullability,
            NullabilityState::DefinitelyNonNull
        );
    }

    let parsed_type = semantic
        .facts
        .external_api_effect_for_receiver(Some("VCFHeaderLineType"), "valueOf")
        .expect("VCF header-line enum parser should be modeled by the pack");
    assert_eq!(
        parsed_type.return_type,
        Some(JavaType::Named {
            name: "VCFHeaderLineType".to_string(),
            args: Vec::new(),
        })
    );
    assert_eq!(
        parsed_type.return_nullability,
        NullabilityState::DefinitelyNonNull
    );

    let version_string = semantic
        .facts
        .external_api_effect_for_receiver(Some("VCFHeaderVersion"), "getVersionString")
        .expect("VCFHeaderVersion version-string getter should be modeled by the pack");
    assert_eq!(version_string.return_type, Some(JavaType::String));

    let comparator = semantic
        .facts
        .external_api_effect_for_receiver(Some("VCFHeader"), "getVCFRecordComparator")
        .expect("VCFHeader record comparator getter should be modeled by the pack");
    assert_eq!(comparator.role, ExternalApiRole::Getter);

    let header = semantic
        .facts
        .external_constructor_for("VCFHeader", &[])
        .unwrap();
    assert_eq!(header.provenance, FactProvenance::ExternalLibraryPack);

    let reader = semantic
        .facts
        .external_constructor_for(
            "VCFFileReader",
            &[JavaType::Named {
                name: "Path".to_string(),
                args: Vec::new(),
            }],
        )
        .expect("VCFFileReader constructor should be modeled by the pack");
    assert_eq!(reader.provenance, FactProvenance::ExternalLibraryPack);

    let simple_header_line = semantic
        .facts
        .external_constructor_for(
            "VCFSimpleHeaderLine",
            &[
                JavaType::String,
                JavaType::Named {
                    name: "Map".to_string(),
                    args: vec![JavaType::String, JavaType::String],
                },
            ],
        )
        .expect("VCFSimpleHeaderLine constructor should be modeled by the pack");
    assert_eq!(
        simple_header_line.provenance,
        FactProvenance::ExternalLibraryPack
    );

    let named_simple_header_line = semantic
        .facts
        .external_constructor_for(
            "VCFSimpleHeaderLine",
            &[JavaType::String, JavaType::String, JavaType::String],
        )
        .expect("VCFSimpleHeaderLine three-string constructor should be modeled by the pack");
    assert_eq!(
        named_simple_header_line.provenance,
        FactProvenance::ExternalLibraryPack
    );

    let reader_header = semantic
        .facts
        .external_api_effect_for_receiver(Some("VCFFileReader"), "getFileHeader")
        .expect("VCFFileReader header getter should be modeled by the pack");
    assert_eq!(
        reader_header.return_type,
        Some(JavaType::Named {
            name: "VCFHeader".to_string(),
            args: Vec::new(),
        })
    );

    let reader_iterator = semantic
        .facts
        .external_api_effect_for_receiver(Some("VCFFileReader"), "iterator")
        .expect("VCFFileReader iterator should be modeled by the pack");
    assert_eq!(
        reader_iterator.return_type,
        Some(JavaType::Named {
            name: "CloseableIterator".to_string(),
            args: vec![JavaType::Named {
                name: "VariantContext".to_string(),
                args: Vec::new(),
            }],
        })
    );

    let writer_builder = semantic
        .facts
        .external_constructor_for("VariantContextWriterBuilder", &[])
        .expect("VariantContextWriterBuilder constructor should be modeled by the VCF pack");
    assert_eq!(
        writer_builder.provenance,
        FactProvenance::ExternalLibraryPack
    );

    for method_name in [
        "clearOptions",
        "setReferenceDictionary",
        "setOutputPath",
        "setOutputFileType",
        "setOption",
        "setCreateMD5",
    ] {
        let builder_method = semantic
            .facts
            .external_api_effect_for_receiver(Some("VariantContextWriterBuilder"), method_name)
            .unwrap_or_else(|| {
                panic!("VariantContextWriterBuilder.{method_name} should be modeled")
            });
        assert_eq!(builder_method.role, ExternalApiRole::FluentMutator);
        assert_eq!(
            builder_method.receiver_mutability,
            ReceiverMutabilityFact::MutatesReceiver
        );
    }

    let determine_output_type = semantic
        .facts
        .external_api_effect_for_receiver(
            Some("VariantContextWriterBuilder"),
            "determineOutputTypeFromFile",
        )
        .expect("VariantContextWriterBuilder output type helper should be modeled");
    assert_eq!(
        determine_output_type.return_type,
        Some(JavaType::Named {
            name: "OutputType".to_string(),
            args: Vec::new(),
        })
    );

    let build = semantic
        .facts
        .external_api_effect_for_receiver(Some("VariantContextWriterBuilder"), "build")
        .expect("VariantContextWriterBuilder build should be modeled");
    assert_eq!(
        build.return_type,
        Some(JavaType::Named {
            name: "VariantContextWriter".to_string(),
            args: Vec::new(),
        })
    );

    for method_name in ["setHeader", "writeHeader", "add", "close"] {
        let writer_method = semantic
            .facts
            .external_api_effect_for_receiver(Some("VariantContextWriter"), method_name)
            .unwrap_or_else(|| panic!("VariantContextWriter.{method_name} should be modeled"));
        assert_eq!(writer_method.role, ExternalApiRole::Mutator);
        assert_eq!(
            writer_method.receiver_mutability,
            ReceiverMutabilityFact::MutatesReceiver
        );
    }

    let check_error = semantic
        .facts
        .external_api_effect_for_receiver(Some("VariantContextWriter"), "checkError")
        .expect("VariantContextWriter.checkError should be modeled");
    assert_eq!(check_error.role, ExternalApiRole::BooleanGetter);
    assert_eq!(
        check_error.return_type,
        Some(JavaType::Primitive(PrimitiveType::Boolean))
    );

    assert!(
        !semantic
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "external_pack_missing_api"
                && diagnostic.message.contains("VariantContextWriterBuilder")),
        "modeled VCF writer builder calls should not produce pack-missing diagnostics: {:?}",
        semantic.diagnostics
    );
}

#[test]
fn apache_commons_math_optim_pack_declares_gatk_shaped_constructor_boundary() {
    let facts = apache_commons_math_optim_pack_facts();

    let pack = facts
        .external_library_pack_for_type("SearchInterval")
        .expect("SearchInterval should be owned by the Apache Commons Math optimization pack");
    assert_eq!(pack.pack_name, "org.apache.commons.math3.optim.univariate");
    assert_eq!(pack.mode, ExternalLibraryMode::ExternalFactModel);
    assert_eq!(pack.coverage, ExternalApiCoverage::Semantic);
    assert_eq!(pack.provenance, FactProvenance::ExternalLibraryPack);

    let interval = facts
        .external_constructor_for(
            "SearchInterval",
            &[
                JavaType::Primitive(new_java2rust::java_ir::PrimitiveType::Double),
                JavaType::Primitive(new_java2rust::java_ir::PrimitiveType::Double),
                JavaType::Primitive(new_java2rust::java_ir::PrimitiveType::Double),
            ],
        )
        .expect("SearchInterval(double,double,double) should be pack-owned");
    assert_eq!(interval.provenance, FactProvenance::ExternalLibraryPack);

    let objective = facts
        .external_constructor_for(
            "UnivariateObjectiveFunction",
            &[JavaType::Unknown("method_reference".to_string())],
        )
        .expect("UnivariateObjectiveFunction(function reference) should be pack-owned");
    assert_eq!(objective.provenance, FactProvenance::ExternalLibraryPack);

    let goal_type = facts
        .external_library_pack_for_type("GoalType")
        .expect("GoalType should be owned with adjacent optimization facts");
    assert_eq!(
        goal_type.pack_name,
        "org.apache.commons.math3.optim.univariate"
    );
    assert_eq!(goal_type.provenance, FactProvenance::ExternalLibraryPack);

    for field_name in ["MAXIMIZE", "MINIMIZE"] {
        let field = facts
            .external_field_for("GoalType", field_name)
            .expect("GoalType enum constants should be pack-owned");
        assert_eq!(field.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(
            field.field_type,
            Some(JavaType::Named {
                name: "GoalType".to_string(),
                args: Vec::new(),
            })
        );

        let dispatched = new_java2rust::external_packs::external_field_for("GoalType", field_name)
            .expect("global external pack dispatch should find GoalType constants");
        assert_eq!(dispatched.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(
            new_java2rust::stdlib::external_field_provenance("GoalType", field_name),
            FactProvenance::ConservativeFallback,
            "GoalType constants should come from the optimization pack, not generic stdlib"
        );
    }
}

#[test]
fn semantic_analysis_uses_apache_commons_math_optim_pack_for_method_references() {
    let semantic = analyze(
        r#"
import java.util.function.DoubleUnaryOperator;
import java.util.function.Function;
import org.apache.commons.math3.optim.univariate.SearchInterval;
import org.apache.commons.math3.optim.univariate.UnivariateObjectiveFunction;

class OptimizationBoundaryUse {
    SearchInterval interval(double min, double max, double guess) {
        return new SearchInterval(min, max, guess);
    }

    UnivariateObjectiveFunction boxedObjective(Function<Double, Double> function) {
        return new UnivariateObjectiveFunction(function::apply);
    }

    UnivariateObjectiveFunction primitiveObjective(DoubleUnaryOperator function) {
        return new UnivariateObjectiveFunction(function::applyAsDouble);
    }
}
"#,
    );

    assert!(
        semantic
            .diagnostics
            .iter()
            .all(|diagnostic| diagnostic.code != "external_pack_missing_constructor"),
        "modeled Apache Commons Math optimization constructors should not report pack gaps"
    );

    let interval = semantic
        .facts
        .external_constructor_for(
            "SearchInterval",
            &[
                JavaType::Primitive(new_java2rust::java_ir::PrimitiveType::Double),
                JavaType::Primitive(new_java2rust::java_ir::PrimitiveType::Double),
                JavaType::Primitive(new_java2rust::java_ir::PrimitiveType::Double),
            ],
        )
        .expect("semantic analysis should use the optimization pack for SearchInterval");
    assert_eq!(interval.provenance, FactProvenance::ExternalLibraryPack);

    let objective = semantic
        .facts
        .external_constructor_for(
            "UnivariateObjectiveFunction",
            &[JavaType::Unknown("method_reference".to_string())],
        )
        .expect("semantic analysis should use the optimization pack for function wrappers");
    assert_eq!(objective.provenance, FactProvenance::ExternalLibraryPack);
}

#[test]
fn picard_programgroups_pack_declares_help_category_boundary() {
    let facts = picard_programgroups_pack_facts();

    let pack = facts
        .external_library_pack_for_type("VariantEvaluationProgramGroup")
        .expect("Picard variant program group should be owned by the program-groups pack");
    assert_eq!(pack.pack_name, "picard.cmdline.programgroups");
    assert_eq!(pack.mode, ExternalLibraryMode::ExternalFactModel);
    assert_eq!(pack.coverage, ExternalApiCoverage::Semantic);
    assert_eq!(pack.provenance, FactProvenance::ExternalLibraryPack);

    for type_name in [
        "DiagnosticsAndQCProgramGroup",
        "IntervalsManipulationProgramGroup",
        "OtherProgramGroup",
        "ReadDataManipulationProgramGroup",
        "ReferenceProgramGroup",
        "VariantEvaluationProgramGroup",
        "VariantFilteringProgramGroup",
        "VariantManipulationProgramGroup",
    ] {
        assert_eq!(
            new_java2rust::stdlib::external_constructor_provenance(type_name, &[]),
            FactProvenance::ConservativeFallback,
            "{type_name}/0 constructor precision should come from the Picard pack, not stdlib fallback"
        );

        let constructor = facts
            .external_constructor_for(type_name, &[])
            .unwrap_or_else(|| panic!("{type_name}/0 should be pack-owned"));
        assert_eq!(constructor.provenance, FactProvenance::ExternalLibraryPack);

        let name = facts
            .external_api_effect_for_receiver(Some(type_name), "getName")
            .unwrap_or_else(|| panic!("{type_name}.getName should be pack-owned"));
        assert_eq!(name.return_type, Some(JavaType::String));
        assert_eq!(name.return_nullability, NullabilityState::DefinitelyNonNull);

        let description = facts
            .external_api_effect_for_receiver(Some(type_name), "getDescription")
            .unwrap_or_else(|| panic!("{type_name}.getDescription should be pack-owned"));
        assert_eq!(description.return_type, Some(JavaType::String));
        assert_eq!(
            description.return_nullability,
            NullabilityState::DefinitelyNonNull
        );
    }
}

#[test]
fn picard_markduplicates_pack_declares_duplicate_tagging_policy_boundary() {
    let facts = picard_markduplicates_pack_facts();

    let pack = facts
        .external_library_pack_for_type("MarkDuplicates")
        .expect("MarkDuplicates should be owned by the Picard markduplicates pack");
    assert_eq!(pack.pack_name, "picard.sam.markduplicates");
    assert_eq!(pack.mode, ExternalLibraryMode::ExternalFactModel);
    assert_eq!(pack.coverage, ExternalApiCoverage::Semantic);
    assert_eq!(pack.provenance, FactProvenance::ExternalLibraryPack);

    let duplicate_tagging_policy = facts
        .external_field_for("MarkDuplicates", "DuplicateTaggingPolicy")
        .expect("MarkDuplicates.DuplicateTaggingPolicy should be pack-owned");
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
        new_java2rust::stdlib::external_field_provenance(
            "MarkDuplicates",
            "DuplicateTaggingPolicy"
        ),
        FactProvenance::ConservativeFallback,
        "MarkDuplicates.DuplicateTaggingPolicy precision should come from the Picard markduplicates pack, not stdlib"
    );

    for field_name in ["DontTag", "OpticalOnly", "All"] {
        let field = facts
            .external_field_for("DuplicateTaggingPolicy", field_name)
            .unwrap_or_else(|| panic!("DuplicateTaggingPolicy.{field_name} should be pack-owned"));
        assert_eq!(field.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(
            field.field_type,
            Some(JavaType::Named {
                name: "DuplicateTaggingPolicy".to_string(),
                args: Vec::new(),
            })
        );
        let dispatched =
            new_java2rust::external_packs::external_field_for("DuplicateTaggingPolicy", field_name)
                .unwrap_or_else(|| {
                    panic!("DuplicateTaggingPolicy.{field_name} should be globally pack-dispatched")
                });
        assert_eq!(dispatched.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(
            new_java2rust::stdlib::external_field_provenance("DuplicateTaggingPolicy", field_name),
            FactProvenance::ConservativeFallback,
            "DuplicateTaggingPolicy.{field_name} precision should come from the Picard markduplicates pack, not stdlib"
        );
    }

    let read_ends_pack = facts
        .external_library_pack_for_type("ReadEnds")
        .expect("ReadEnds should be owned by the Picard markduplicates pack");
    assert_eq!(read_ends_pack.pack_name, "picard.sam.markduplicates");
    for field_name in ["FF", "FR", "RF", "RR", "F", "R"] {
        let field = facts
            .external_field_for("ReadEnds", field_name)
            .unwrap_or_else(|| panic!("ReadEnds.{field_name} should be pack-owned"));
        assert_eq!(field.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(
            field.field_type,
            Some(JavaType::Primitive(PrimitiveType::Byte))
        );
        let dispatched = new_java2rust::external_packs::external_field_for("ReadEnds", field_name)
            .unwrap_or_else(|| panic!("ReadEnds.{field_name} should be globally dispatched"));
        assert_eq!(dispatched.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(
            new_java2rust::stdlib::external_field_provenance("ReadEnds", field_name),
            FactProvenance::ConservativeFallback,
            "ReadEnds.{field_name} precision should come from the Picard markduplicates pack, not stdlib"
        );
    }

    for (type_name, arg_len) in [("OpticalDuplicateFinder", 0), ("OpticalDuplicateFinder", 3)] {
        let args = vec![JavaType::Unknown("Object".to_string()); arg_len];
        let constructor = facts
            .external_constructor_for(type_name, &args)
            .unwrap_or_else(|| panic!("{type_name}/{arg_len} should be pack-owned"));
        assert_eq!(constructor.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(
            new_java2rust::stdlib::external_constructor_provenance(type_name, &args),
            FactProvenance::ConservativeFallback,
            "{type_name}/{arg_len} precision should come from the Picard markduplicates pack, not stdlib"
        );
    }
}

#[test]
fn picard_metrics_pack_models_duplication_metrics_public_fields() {
    let facts = picard_metrics_pack_facts();
    let pack = facts
        .external_library_pack_for_type("picard.sam.DuplicationMetrics")
        .expect("DuplicationMetrics should be owned by the Picard metrics pack");
    assert_eq!(pack.pack_name, "picard.analysis.metrics");
    assert_eq!(pack.mode, ExternalLibraryMode::ExternalFactModel);
    assert_eq!(pack.coverage, ExternalApiCoverage::Semantic);

    for (field_name, expected_ty) in [
        ("LIBRARY", JavaType::String),
        (
            "UNPAIRED_READS_EXAMINED",
            JavaType::Primitive(PrimitiveType::Long),
        ),
        (
            "READ_PAIRS_EXAMINED",
            JavaType::Primitive(PrimitiveType::Long),
        ),
        (
            "SECONDARY_OR_SUPPLEMENTARY_RDS",
            JavaType::Primitive(PrimitiveType::Long),
        ),
        ("UNMAPPED_READS", JavaType::Primitive(PrimitiveType::Long)),
        (
            "UNPAIRED_READ_DUPLICATES",
            JavaType::Primitive(PrimitiveType::Long),
        ),
        (
            "READ_PAIR_DUPLICATES",
            JavaType::Primitive(PrimitiveType::Long),
        ),
        (
            "READ_PAIR_OPTICAL_DUPLICATES",
            JavaType::Primitive(PrimitiveType::Long),
        ),
        (
            "ESTIMATED_LIBRARY_SIZE",
            JavaType::Primitive(PrimitiveType::Long),
        ),
        (
            "PERCENT_DUPLICATION",
            JavaType::Primitive(PrimitiveType::Double),
        ),
    ] {
        let field = facts
            .external_field_for("DuplicationMetrics", field_name)
            .unwrap_or_else(|| panic!("DuplicationMetrics.{field_name} should be pack-owned"));
        assert_eq!(field.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(field.field_type, Some(expected_ty.clone()));

        let dispatched =
            new_java2rust::external_packs::external_field_for("DuplicationMetrics", field_name)
                .unwrap_or_else(|| {
                    panic!("DuplicationMetrics.{field_name} should be globally pack-dispatched")
                });
        assert_eq!(dispatched.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(dispatched.field_type, Some(expected_ty));
        assert_eq!(
            new_java2rust::stdlib::external_field_provenance("DuplicationMetrics", field_name),
            FactProvenance::ConservativeFallback,
            "DuplicationMetrics.{field_name} precision should come from the Picard metrics pack, not stdlib"
        );
    }
}

#[test]
fn semantic_analysis_uses_picard_programgroups_pack_for_help_category_table() {
    let semantic = analyze(
        r#"
import java.util.HashMap;
import java.util.Map;

class HelpCategoryUse {
    Map<String, String> categories() {
        Map<String, String> categories = new HashMap<>();
        categories.put(new picard.cmdline.programgroups.DiagnosticsAndQCProgramGroup().getName(), "tools");
        categories.put(new picard.cmdline.programgroups.VariantEvaluationProgramGroup().getName(), "tools");
        categories.put(new picard.cmdline.programgroups.VariantFilteringProgramGroup().getName(), "tools");
        categories.put(new picard.cmdline.programgroups.VariantManipulationProgramGroup().getDescription(), "tools");
        return categories;
    }
}
"#,
    );

    assert!(
        semantic.diagnostics.iter().all(|diagnostic| diagnostic.code
            != "external_pack_missing_constructor"
            && diagnostic.code != "external_pack_missing_api"),
        "modeled Picard program-group constructors and getters should not report pack gaps"
    );

    for type_name in [
        "DiagnosticsAndQCProgramGroup",
        "VariantEvaluationProgramGroup",
        "VariantFilteringProgramGroup",
        "VariantManipulationProgramGroup",
    ] {
        let constructor = semantic
            .facts
            .external_constructor_for(type_name, &[])
            .unwrap_or_else(|| panic!("{type_name}/0 should be explained by the Picard pack"));
        assert_eq!(constructor.provenance, FactProvenance::ExternalLibraryPack);
    }

    let name = semantic
        .facts
        .external_api_effect_for_receiver(Some("VariantEvaluationProgramGroup"), "getName")
        .expect("program-group getName should be explained by the Picard pack");
    assert_eq!(name.return_type, Some(JavaType::String));
    assert_eq!(name.provenance, FactProvenance::ExternalLibraryPack);

    let description = semantic
        .facts
        .external_api_effect_for_receiver(Some("VariantManipulationProgramGroup"), "getDescription")
        .expect("program-group getDescription should be explained by the Picard pack");
    assert_eq!(description.return_type, Some(JavaType::String));
    assert_eq!(description.provenance, FactProvenance::ExternalLibraryPack);
}

#[test]
fn avro_pack_declares_schema_parser_boundary() {
    let facts = avro_pack_facts();

    let pack = facts
        .external_library_pack_for_type("Parser")
        .expect("Schema.Parser should be owned by the Avro pack");
    assert_eq!(pack.pack_name, "org.apache.avro");
    assert_eq!(pack.mode, ExternalLibraryMode::ExternalFactModel);
    assert_eq!(pack.coverage, ExternalApiCoverage::Semantic);
    assert_eq!(pack.provenance, FactProvenance::ExternalLibraryPack);

    let parser = facts
        .external_constructor_for("Parser", &[])
        .expect("Schema.Parser/0 should be pack-owned while nested names are collapsed");
    assert_eq!(parser.provenance, FactProvenance::ExternalLibraryPack);

    let datum_reader = facts
        .external_constructor_for(
            "GenericDatumReader",
            &[JavaType::Named {
                name: "Schema".to_string(),
                args: Vec::new(),
            }],
        )
        .expect("GenericDatumReader(Schema) should be pack-owned");
    assert_eq!(datum_reader.provenance, FactProvenance::ExternalLibraryPack);
    assert_eq!(
        new_java2rust::stdlib::external_constructor_provenance(
            "GenericDatumReader",
            &[JavaType::Named {
                name: "Schema".to_string(),
                args: Vec::new(),
            }],
        ),
        FactProvenance::ConservativeFallback,
        "Avro GenericDatumReader constructor precision should come from the pack, not stdlib fallback"
    );

    let parse = facts
        .external_api_effect_for_receiver(Some("Parser"), "parse")
        .expect("Schema.Parser.parse should be pack-owned");
    assert_eq!(
        parse.return_type,
        Some(JavaType::Named {
            name: "Schema".to_string(),
            args: Vec::new(),
        })
    );
    assert_eq!(
        parse.return_nullability,
        NullabilityState::DefinitelyNonNull
    );
}

#[test]
fn semantic_analysis_uses_avro_pack_for_schema_parser_workflow() {
    let semantic = analyze(
        r#"
class AvroSchemaParserUse {
    Schema parse(String schemaJson) {
        return new org.apache.avro.Schema.Parser().parse(schemaJson);
    }

    Object reader(String schemaJson) {
        return new GenericDatumReader(new org.apache.avro.Schema.Parser().parse(schemaJson));
    }
}
"#,
    );

    assert!(
        semantic.diagnostics.iter().all(|diagnostic| diagnostic.code
            != "external_pack_missing_constructor"
            && diagnostic.code != "external_pack_missing_api"),
        "modeled Avro parser constructors and parse should not report pack gaps"
    );

    let parser = semantic
        .facts
        .external_constructor_for("Parser", &[])
        .expect("semantic analysis should use the Avro pack for Schema.Parser");
    assert_eq!(parser.provenance, FactProvenance::ExternalLibraryPack);

    let parse = semantic
        .facts
        .external_api_effect_for_receiver(Some("Parser"), "parse")
        .expect("semantic analysis should use the Avro pack for Parser.parse");
    assert_eq!(
        parse.return_type,
        Some(JavaType::Named {
            name: "Schema".to_string(),
            args: Vec::new(),
        })
    );
    assert_eq!(parse.provenance, FactProvenance::ExternalLibraryPack);

    let datum_reader = semantic
        .facts
        .external_constructor_for(
            "GenericDatumReader",
            &[JavaType::Named {
                name: "Schema".to_string(),
                args: Vec::new(),
            }],
        )
        .expect("semantic analysis should use the Avro pack for GenericDatumReader");
    assert_eq!(datum_reader.provenance, FactProvenance::ExternalLibraryPack);
}

#[test]
fn barclay_help_pack_declares_doc_exception_boundary() {
    let facts = barclay_help_pack_facts();

    let pack = facts
        .external_library_pack_for_type("DocException")
        .expect("DocException should be owned by the Barclay help pack");
    assert_eq!(pack.pack_name, "org.broadinstitute.barclay.help");
    assert_eq!(pack.mode, ExternalLibraryMode::ExternalFactModel);
    assert_eq!(pack.coverage, ExternalApiCoverage::Semantic);
    assert_eq!(pack.provenance, FactProvenance::ExternalLibraryPack);

    let constructor = facts
        .external_constructor_for(
            "DocException",
            &[
                JavaType::String,
                JavaType::Named {
                    name: "IOException".to_string(),
                    args: Vec::new(),
                },
            ],
        )
        .expect("DocException(String, Throwable) should be pack-owned");
    assert_eq!(constructor.provenance, FactProvenance::ExternalLibraryPack);
    assert_eq!(
        facts.exception_kind_for("DocException"),
        Some(ExceptionKind::Unchecked)
    );

    for field_name in [
        "ARGUMENTS_POSITIONAL",
        "ARGUMENT_DEFAULT_VALUE",
        "ARGUMENT_TYPE",
    ] {
        let field = facts
            .external_field_for("TemplateProperties", field_name)
            .unwrap_or_else(|| panic!("TemplateProperties.{field_name} should be pack-owned"));
        assert_eq!(field.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(field.field_type, Some(JavaType::String));
        let dispatched =
            new_java2rust::external_packs::external_field_for("TemplateProperties", field_name)
                .unwrap_or_else(|| {
                    panic!("TemplateProperties.{field_name} should be globally pack-dispatched")
                });
        assert_eq!(dispatched.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(
            new_java2rust::stdlib::external_field_provenance("TemplateProperties", field_name),
            FactProvenance::ConservativeFallback,
            "TemplateProperties.{field_name} precision should come from the Barclay help pack, not stdlib"
        );
    }
}

#[test]
fn semantic_analysis_uses_barclay_help_pack_for_doc_exception_wrapping() {
    let semantic = analyze(
        r#"
class BarclayDocExceptionUse {
    void wrap(IOException io, TemplateException template) {
        throw new DocException("io", io);
    }

    void wrapTemplate(IOException io, TemplateException template) {
        throw new DocException("template", template);
    }
}
"#,
    );

    assert!(
        semantic
            .diagnostics
            .iter()
            .all(|diagnostic| diagnostic.code != "external_pack_missing_constructor"),
        "modeled Barclay DocException constructors should not report pack gaps"
    );

    for arg_type in ["IOException", "TemplateException"] {
        let constructor = semantic
            .facts
            .external_constructor_for(
                "DocException",
                &[
                    JavaType::String,
                    JavaType::Named {
                        name: arg_type.to_string(),
                        args: Vec::new(),
                    },
                ],
            )
            .unwrap_or_else(|| {
                panic!("DocException(String,{arg_type}) should be explained by the Barclay pack")
            });
        assert_eq!(constructor.provenance, FactProvenance::ExternalLibraryPack);
    }
}

#[test]
fn barclay_argparser_pack_declares_command_line_boundary() {
    let facts = barclay_argparser_pack_facts();

    let pack = facts
        .external_library_pack_for_type("CommandLineArgumentParser")
        .expect("CommandLineArgumentParser should be owned by the Barclay argparser pack");
    assert_eq!(pack.pack_name, "org.broadinstitute.barclay.argparser");
    assert_eq!(pack.mode, ExternalLibraryMode::ExternalFactModel);
    assert_eq!(pack.coverage, ExternalApiCoverage::Semantic);
    assert_eq!(pack.provenance, FactProvenance::ExternalLibraryPack);

    for (type_name, arg_len) in [
        ("CommandLineException", 1),
        ("CommandLineException", 2),
        ("CommandLineException.BadArgumentValue", 1),
        ("CommandLineException.BadArgumentValue", 2),
        ("CommandLineException.BadArgumentValue", 3),
        ("BadArgumentValue", 1),
        ("BadArgumentValue", 2),
        ("BadArgumentValue", 3),
        ("CommandLineException.MissingArgument", 1),
        ("CommandLineException.MissingArgument", 2),
        ("MissingArgument", 1),
        ("MissingArgument", 2),
        ("CommandLineArgumentParser", 3),
    ] {
        let args = vec![JavaType::String; arg_len];
        let constructor = facts
            .external_constructor_for(type_name, &args)
            .unwrap_or_else(|| panic!("{type_name}/{arg_len} should be pack-owned"));
        assert_eq!(constructor.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(
            new_java2rust::stdlib::external_constructor_provenance(type_name, &args),
            FactProvenance::ConservativeFallback,
            "{type_name}/{arg_len} precision should come from the Barclay argparser pack, not stdlib fallback"
        );
    }

    let show_hidden = facts
        .external_field_for("SpecialArgumentsCollection", "SHOW_HIDDEN")
        .expect("SpecialArgumentsCollection.SHOW_HIDDEN should be pack-owned");
    assert_eq!(show_hidden.provenance, FactProvenance::ExternalLibraryPack);
    assert_eq!(
        show_hidden.field_type,
        Some(JavaType::Primitive(PrimitiveType::Boolean))
    );
    let dispatched_show_hidden = new_java2rust::external_packs::external_field_for(
        "SpecialArgumentsCollection",
        "SHOW_HIDDEN",
    )
    .expect("SpecialArgumentsCollection.SHOW_HIDDEN should be globally pack-dispatched");
    assert_eq!(
        dispatched_show_hidden.provenance,
        FactProvenance::ExternalLibraryPack
    );
    assert_eq!(
        new_java2rust::stdlib::external_field_provenance(
            "SpecialArgumentsCollection",
            "SHOW_HIDDEN"
        ),
        FactProvenance::ConservativeFallback,
        "SpecialArgumentsCollection.SHOW_HIDDEN precision should come from the Barclay argparser pack, not stdlib"
    );
}

#[test]
fn semantic_analysis_uses_barclay_argparser_pack_for_nested_exception_constructors() {
    let semantic = analyze(
        r#"
import org.broadinstitute.barclay.argparser.CommandLineException;

class BarclayArgparserUse {
    RuntimeException badValue(String name, String value) {
        return new CommandLineException.BadArgumentValue(name);
    }

    RuntimeException missing(String name, String message) {
        return new CommandLineException.MissingArgument(name, message);
    }
}
"#,
    );

    for (type_name, arg_len) in [
        ("CommandLineException.BadArgumentValue", 1),
        ("CommandLineException.MissingArgument", 2),
    ] {
        let args = vec![JavaType::String; arg_len];
        let constructor = semantic
            .facts
            .external_constructor_for(type_name, &args)
            .unwrap_or_else(|| panic!("{type_name}/{arg_len} should be pack-owned"));
        assert_eq!(constructor.provenance, FactProvenance::ExternalLibraryPack);
    }

    assert!(
        !semantic
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "external_pack_missing_constructor"),
        "modeled nested Barclay exception constructors should not produce missing-pack diagnostics: {:?}",
        semantic.diagnostics
    );
}

#[test]
fn barclay_utils_pack_declares_class_finder_boundary() {
    let facts = barclay_utils_pack_facts();

    let pack = facts
        .external_library_pack_for_type("ClassFinder")
        .expect("ClassFinder should be owned by the Barclay utils pack");
    assert_eq!(pack.pack_name, "org.broadinstitute.barclay.utils");
    assert_eq!(pack.mode, ExternalLibraryMode::ExternalFactModel);
    assert_eq!(pack.coverage, ExternalApiCoverage::Semantic);
    assert_eq!(pack.provenance, FactProvenance::ExternalLibraryPack);

    let constructor = facts
        .external_constructor_for("ClassFinder", &[])
        .expect("ClassFinder/0 should be pack-owned");
    assert_eq!(constructor.provenance, FactProvenance::ExternalLibraryPack);
    assert_eq!(
        new_java2rust::stdlib::external_constructor_provenance("ClassFinder", &[]),
        FactProvenance::ConservativeFallback,
        "ClassFinder/0 precision should come from the Barclay utils pack, not stdlib fallback"
    );
}

#[test]
fn scala_tuple_pack_declares_tuple_boundaries() {
    let facts = scala_tuple_pack_facts();

    for (type_name, arity, getters) in [
        ("Tuple2", 2, &["_1", "_2"][..]),
        ("Tuple3", 3, &["_1", "_2", "_3"][..]),
        ("Tuple4", 4, &["_1", "_2", "_3", "_4"][..]),
    ] {
        let pack = facts
            .external_library_pack_for_type(type_name)
            .unwrap_or_else(|| panic!("{type_name} should be owned by the Scala tuple pack"));
        assert_eq!(pack.pack_name, "scala");
        assert_eq!(pack.mode, ExternalLibraryMode::ExternalFactModel);
        assert_eq!(pack.coverage, ExternalApiCoverage::Semantic);
        assert_eq!(pack.provenance, FactProvenance::ExternalLibraryPack);

        let args = vec![JavaType::Primitive(new_java2rust::java_ir::PrimitiveType::Int); arity];
        let constructor = facts
            .external_constructor_for(type_name, &args)
            .unwrap_or_else(|| panic!("{type_name}/{arity} should be pack-owned"));
        assert_eq!(constructor.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(
            new_java2rust::stdlib::external_constructor_provenance(type_name, &args),
            FactProvenance::ConservativeFallback,
            "{type_name}/{arity} precision should come from the Scala tuple pack, not stdlib"
        );

        for getter in getters {
            let effect = facts
                .external_api_effect_for_receiver(Some(type_name), getter)
                .unwrap_or_else(|| panic!("{type_name}.{getter} should be pack-owned"));
            assert_eq!(effect.role, ExternalApiRole::Getter);
            assert_eq!(effect.receiver_mutability, ReceiverMutabilityFact::ReadOnly);
            assert_eq!(effect.return_nullability, NullabilityState::MaybeNull);
        }
    }
}

#[test]
fn semantic_analysis_uses_scala_tuple_pack_for_tuple4_workflow() {
    let semantic = analyze(
        r#"
class ScalaTuple4Use {
    Object build(int period, int length, int observed, int required) {
        Tuple4<Integer, Integer, Integer, Integer> combo =
            new Tuple4<>(period, length, observed, required);
        return combo._4();
    }
}
"#,
    );

    assert!(
        semantic.diagnostics.iter().all(|diagnostic| diagnostic.code
            != "external_pack_missing_constructor"
            && diagnostic.code != "external_pack_missing_api"),
        "modeled Scala Tuple4 constructors and accessors should not report pack gaps"
    );

    let constructor = semantic
        .facts
        .external_constructor_for(
            "Tuple4",
            &[
                JavaType::Primitive(new_java2rust::java_ir::PrimitiveType::Int),
                JavaType::Primitive(new_java2rust::java_ir::PrimitiveType::Int),
                JavaType::Primitive(new_java2rust::java_ir::PrimitiveType::Int),
                JavaType::Primitive(new_java2rust::java_ir::PrimitiveType::Int),
            ],
        )
        .expect("semantic analysis should use the Scala tuple pack for Tuple4/4");
    assert_eq!(constructor.provenance, FactProvenance::ExternalLibraryPack);

    let getter = semantic
        .facts
        .external_api_effect_for_receiver(Some("Tuple4"), "_4")
        .expect("semantic analysis should use the Scala tuple pack for Tuple4._4");
    assert_eq!(getter.provenance, FactProvenance::ExternalLibraryPack);
}

#[test]
fn kryo_serializers_pack_declares_unmodifiable_collections_boundary() {
    let facts = kryo_serializers_pack_facts();

    let pack = facts
        .external_library_pack_for_type("UnmodifiableCollectionsSerializer")
        .expect("UnmodifiableCollectionsSerializer should be owned by the Kryo serializers pack");
    assert_eq!(pack.pack_name, "de.javakaffee.kryoserializers");
    assert_eq!(pack.mode, ExternalLibraryMode::ExternalFactModel);
    assert_eq!(pack.coverage, ExternalApiCoverage::Semantic);
    assert_eq!(pack.provenance, FactProvenance::ExternalLibraryPack);

    let constructor = facts
        .external_constructor_for("UnmodifiableCollectionsSerializer", &[])
        .expect("UnmodifiableCollectionsSerializer/0 should be pack-owned");
    assert_eq!(constructor.provenance, FactProvenance::ExternalLibraryPack);
}

#[test]
fn kryo_pack_declares_core_serializer_runtime_constructors() {
    let facts = kryo_pack_facts();

    let pack = facts
        .external_library_pack_for_type("Kryo")
        .expect("Kryo should be owned by the core Kryo pack");
    assert_eq!(pack.pack_name, "com.esotericsoftware.kryo");
    assert_eq!(pack.mode, ExternalLibraryMode::ExternalFactModel);
    assert_eq!(pack.coverage, ExternalApiCoverage::Semantic);
    assert_eq!(pack.provenance, FactProvenance::ExternalLibraryPack);
    let serializer_pack = facts
        .external_library_pack_for_type("com.esotericsoftware.kryo.Serializer")
        .expect("qualified Kryo Serializer should be owned by the core Kryo pack");
    assert_eq!(serializer_pack.pack_name, "com.esotericsoftware.kryo");
    assert!(
        facts.external_library_pack_for_type("Serializer").is_none(),
        "the Kryo pack should not claim the ambiguous bare Serializer type"
    );
    assert!(
        facts.has_object_representation_facts(&JavaType::Named {
            name: "com.esotericsoftware.kryo.Serializer".to_string(),
            args: Vec::new(),
        }),
        "qualified Kryo Serializer should be an explicit erased external boundary"
    );

    for (type_name, args) in [
        ("Kryo", Vec::new()),
        ("com.esotericsoftware.kryo.Kryo", Vec::new()),
        (
            "Input",
            vec![JavaType::Named {
                name: "InputStream".to_string(),
                args: Vec::new(),
            }],
        ),
        (
            "Output",
            vec![JavaType::Named {
                name: "OutputStream".to_string(),
                args: Vec::new(),
            }],
        ),
        (
            "FieldSerializer",
            vec![
                JavaType::Named {
                    name: "Kryo".to_string(),
                    args: Vec::new(),
                },
                JavaType::Named {
                    name: "Class".to_string(),
                    args: Vec::new(),
                },
            ],
        ),
        ("ImmutableMapSerializer", Vec::new()),
    ] {
        let constructor = facts
            .external_constructor_for(type_name, &args)
            .unwrap_or_else(|| panic!("{type_name}/{} should be pack-owned", args.len()));
        assert_eq!(constructor.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(
            new_java2rust::stdlib::external_constructor_provenance(type_name, &args),
            FactProvenance::ConservativeFallback,
            "{type_name}/{} should not be a generic stdlib fallback",
            args.len()
        );
    }

    for (method_name, expected_ty) in [
        ("readBoolean", JavaType::Primitive(PrimitiveType::Boolean)),
        ("readByte", JavaType::Primitive(PrimitiveType::Byte)),
        ("readShort", JavaType::Primitive(PrimitiveType::Short)),
        ("readInt", JavaType::Primitive(PrimitiveType::Int)),
        ("readLong", JavaType::Primitive(PrimitiveType::Long)),
        ("readFloat", JavaType::Primitive(PrimitiveType::Float)),
        (
            "readBytes",
            JavaType::Array(Box::new(JavaType::Primitive(PrimitiveType::Byte))),
        ),
        ("readString", JavaType::String),
    ] {
        let effect = facts
            .external_api_effect_for_receiver(Some("Input"), method_name)
            .unwrap_or_else(|| panic!("Input.{method_name} should be pack-owned"));
        assert_eq!(effect.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(effect.role, ExternalApiRole::Getter);
        assert_eq!(
            effect.receiver_mutability,
            ReceiverMutabilityFact::MutatesReceiver
        );
        assert_eq!(effect.return_type, Some(expected_ty));
    }

    let read_object = facts
        .external_api_effect_for_receiver(Some("Kryo"), "readClassAndObject")
        .expect("Kryo.readClassAndObject should be pack-owned");
    assert_eq!(read_object.provenance, FactProvenance::ExternalLibraryPack);
    assert_eq!(read_object.role, ExternalApiRole::Getter);
    assert_eq!(
        read_object.return_type,
        Some(JavaType::Named {
            name: "Object".to_string(),
            args: Vec::new(),
        })
    );
    assert_eq!(read_object.return_nullability, NullabilityState::MaybeNull);

    let read_string = facts
        .external_api_effect_for_receiver(Some("Input"), "readString")
        .expect("Input.readString should be pack-owned");
    assert_eq!(read_string.return_nullability, NullabilityState::MaybeNull);

    let write_object = facts
        .external_api_effect_for_receiver(Some("Kryo"), "writeClassAndObject")
        .expect("Kryo.writeClassAndObject should be pack-owned");
    assert_eq!(write_object.provenance, FactProvenance::ExternalLibraryPack);
    assert_eq!(write_object.role, ExternalApiRole::SideEffect);

    for method_name in [
        "writeBoolean",
        "writeByte",
        "writeShort",
        "writeInt",
        "writeLong",
        "writeFloat",
        "writeBytes",
        "writeString",
        "writeAscii",
        "close",
    ] {
        let effect = facts
            .external_api_effect_for_receiver(Some("Output"), method_name)
            .unwrap_or_else(|| panic!("Output.{method_name} should be pack-owned"));
        assert_eq!(effect.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(effect.role, ExternalApiRole::SideEffect);
        assert_eq!(effect.receiver_mutability, ReceiverMutabilityFact::ReadOnly);
    }
}

#[test]
fn semantic_analysis_uses_kryo_pack_for_primitive_io() {
    let semantic = analyze(
        r#"
import com.esotericsoftware.kryo.io.Input;
import com.esotericsoftware.kryo.io.Output;
import java.io.InputStream;
import java.io.OutputStream;

class KryoPrimitiveIoUse {
    void roundTrip(InputStream inputStream, OutputStream outputStream) {
        Input input = new Input(inputStream);
        Output output = new Output(outputStream);
        boolean flag = input.readBoolean();
        byte tag = input.readByte();
        short code = input.readShort();
        int count = input.readInt();
        long total = input.readLong();
        float score = input.readFloat();
        byte[] payload = input.readBytes(4);
        String label = input.readString();
        output.writeBoolean(flag);
        output.writeByte(tag);
        output.writeShort(code);
        output.writeInt(count);
        output.writeLong(total);
        output.writeFloat(score);
        output.writeBytes(payload);
        output.writeString(label);
        output.writeAscii(label);
    }
}
"#,
    );

    assert!(
        semantic.diagnostics.iter().all(|diagnostic| diagnostic.code
            != "external_pack_missing_constructor"
            && diagnostic.code != "external_pack_missing_api"),
        "modeled Kryo primitive IO constructor and methods should not report pack gaps: {:?}",
        semantic.diagnostics
    );

    for (receiver, method_name) in [
        ("Input", "readBoolean"),
        ("Input", "readByte"),
        ("Input", "readShort"),
        ("Input", "readInt"),
        ("Input", "readLong"),
        ("Input", "readFloat"),
        ("Input", "readBytes"),
        ("Input", "readString"),
        ("Output", "writeBoolean"),
        ("Output", "writeByte"),
        ("Output", "writeShort"),
        ("Output", "writeInt"),
        ("Output", "writeLong"),
        ("Output", "writeFloat"),
        ("Output", "writeBytes"),
        ("Output", "writeString"),
        ("Output", "writeAscii"),
    ] {
        let effect = semantic
            .facts
            .external_api_effect_for_receiver(Some(receiver), method_name)
            .unwrap_or_else(|| panic!("semantic analysis should model {receiver}.{method_name}"));
        assert_eq!(effect.provenance, FactProvenance::ExternalLibraryPack);
    }
}

#[test]
fn semantic_analysis_uses_kryo_pack_for_object_serialization_and_close() {
    let semantic = analyze(
        r#"
import com.esotericsoftware.kryo.Kryo;
import com.esotericsoftware.kryo.io.Input;
import com.esotericsoftware.kryo.io.Output;

import java.util.Set;

class KryoObjectSerializationUse {
    Set<String> readNames(Kryo kryo, Input input) {
        return (Set<String>) kryo.readClassAndObject(input);
    }

    void writeNames(Kryo kryo, Output output, Set<String> names) {
        kryo.writeClassAndObject(output, names);
        output.close();
    }
}
"#,
    );

    assert!(
        semantic
            .diagnostics
            .iter()
            .all(|diagnostic| diagnostic.code != "external_pack_missing_api"),
        "modeled Kryo object serialization and close methods should not report pack gaps: {:?}",
        semantic.diagnostics
    );

    let read_object = semantic
        .facts
        .external_api_effect_for_receiver(Some("Kryo"), "readClassAndObject")
        .expect("semantic analysis should model Kryo.readClassAndObject");
    assert_eq!(read_object.provenance, FactProvenance::ExternalLibraryPack);
    assert_eq!(read_object.role, ExternalApiRole::Getter);
    assert_eq!(read_object.return_nullability, NullabilityState::MaybeNull);
    assert_eq!(
        read_object.return_type,
        Some(JavaType::Named {
            name: "Object".to_string(),
            args: Vec::new(),
        })
    );

    for (receiver, method_name) in [("Kryo", "writeClassAndObject"), ("Output", "close")] {
        let effect = semantic
            .facts
            .external_api_effect_for_receiver(Some(receiver), method_name)
            .unwrap_or_else(|| panic!("semantic analysis should model {receiver}.{method_name}"));
        assert_eq!(effect.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(effect.role, ExternalApiRole::SideEffect);
    }
}

#[test]
fn semantic_analysis_uses_kryo_serializers_pack_for_collection_serializer() {
    let semantic = analyze(
        r#"
class KryoCollectionSerializerUse {
    void register(Kryo kryo, Class<?> mapClass) {
        kryo.register(mapClass, new UnmodifiableCollectionsSerializer());
    }
}
"#,
    );

    assert!(
        semantic.diagnostics.iter().all(|diagnostic| diagnostic.code
            != "external_pack_missing_constructor"
            && diagnostic.code != "external_pack_missing_api"),
        "modeled Kryo serializer constructors should not report pack gaps"
    );

    let constructor = semantic
        .facts
        .external_constructor_for("UnmodifiableCollectionsSerializer", &[])
        .expect("UnmodifiableCollectionsSerializer/0 should be explained by the pack");
    assert_eq!(constructor.provenance, FactProvenance::ExternalLibraryPack);
}

#[test]
fn opencsv_pack_declares_reader_writer_boundary() {
    let facts = opencsv_pack_facts();

    let reader_pack = facts
        .external_library_pack_for_type("CSVReader")
        .expect("CSVReader should be owned by the OpenCSV pack");
    assert_eq!(reader_pack.pack_name, "com.opencsv");
    assert_eq!(reader_pack.mode, ExternalLibraryMode::ExternalFactModel);
    assert_eq!(reader_pack.coverage, ExternalApiCoverage::Semantic);
    assert_eq!(reader_pack.provenance, FactProvenance::ExternalLibraryPack);

    for (type_name, args) in [
        (
            "CSVReader",
            vec![JavaType::Named {
                name: "Reader".to_string(),
                args: Vec::new(),
            }],
        ),
        (
            "CSVReader",
            vec![
                JavaType::Named {
                    name: "Reader".to_string(),
                    args: Vec::new(),
                },
                JavaType::Primitive(new_java2rust::java_ir::PrimitiveType::Char),
                JavaType::Primitive(new_java2rust::java_ir::PrimitiveType::Char),
                JavaType::Primitive(new_java2rust::java_ir::PrimitiveType::Char),
            ],
        ),
        (
            "CSVWriter",
            vec![
                JavaType::Named {
                    name: "Writer".to_string(),
                    args: Vec::new(),
                },
                JavaType::Primitive(new_java2rust::java_ir::PrimitiveType::Char),
            ],
        ),
        (
            "CSVWriter",
            vec![
                JavaType::Named {
                    name: "Writer".to_string(),
                    args: Vec::new(),
                },
                JavaType::Primitive(new_java2rust::java_ir::PrimitiveType::Char),
                JavaType::Primitive(new_java2rust::java_ir::PrimitiveType::Char),
                JavaType::Primitive(new_java2rust::java_ir::PrimitiveType::Char),
            ],
        ),
    ] {
        let constructor = facts
            .external_constructor_for(type_name, &args)
            .unwrap_or_else(|| panic!("{type_name}/{} should be pack-owned", args.len()));
        assert_eq!(constructor.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(
            new_java2rust::stdlib::external_constructor_provenance(type_name, &args),
            FactProvenance::ConservativeFallback,
            "{type_name}/{} should not be a generic stdlib fallback",
            args.len()
        );
    }

    let read_next = facts
        .external_api_effect_for_receiver_with_arity(Some("CSVReader"), "readNext", Some(0))
        .expect("CSVReader.readNext should be pack-modeled");
    assert_eq!(read_next.provenance, FactProvenance::ExternalLibraryPack);
    assert_eq!(
        read_next.receiver_mutability,
        ReceiverMutabilityFact::MutatesReceiver
    );
    assert_eq!(read_next.return_nullability, NullabilityState::MaybeNull);
    assert_eq!(
        read_next.return_type,
        Some(JavaType::Array(Box::new(JavaType::String)))
    );

    let write_next = facts
        .external_api_effect_for_receiver_with_arity(Some("CSVWriter"), "writeNext", Some(2))
        .expect("CSVWriter.writeNext should be pack-modeled");
    assert_eq!(write_next.provenance, FactProvenance::ExternalLibraryPack);
    assert_eq!(
        write_next.receiver_mutability,
        ReceiverMutabilityFact::MutatesReceiver
    );
}

#[test]
fn semantic_analysis_uses_opencsv_pack_for_gatk_shaped_csv_io() {
    let semantic = analyze(
        r#"
class OpenCsvIoUse {
    void run(Reader reader, Writer writer, char separator, char quote, char escape, String[] row, java.util.List<String[]> rows) {
        CSVReader csvReader = new CSVReader(reader, separator, quote, escape);
        CSVWriter csvWriter = new CSVWriter(writer, separator, quote, escape);
        String[] next = csvReader.readNext();
        java.util.List<String[]> allRows = csvReader.readAll();
        csvWriter.writeNext(row, false);
        csvWriter.writeAll(rows);
        csvWriter.flush();
        csvReader.close();
        csvWriter.close();
    }
}
"#,
    );

    assert!(
        semantic.diagnostics.iter().all(|diagnostic| diagnostic.code
            != "external_pack_missing_constructor"
            && diagnostic.code != "external_pack_missing_api"),
        "modeled OpenCSV constructors and APIs should not report pack gaps"
    );

    let constructor = semantic
        .facts
        .external_constructor_for(
            "CSVReader",
            &[
                JavaType::Named {
                    name: "Reader".to_string(),
                    args: Vec::new(),
                },
                JavaType::Primitive(new_java2rust::java_ir::PrimitiveType::Char),
                JavaType::Primitive(new_java2rust::java_ir::PrimitiveType::Char),
                JavaType::Primitive(new_java2rust::java_ir::PrimitiveType::Char),
            ],
        )
        .expect("semantic analysis should use the OpenCSV pack for CSVReader/4");
    assert_eq!(constructor.provenance, FactProvenance::ExternalLibraryPack);

    let read_next = semantic
        .facts
        .external_api_effect_for_receiver_with_arity(Some("CSVReader"), "readNext", Some(0))
        .expect("semantic analysis should use the OpenCSV pack for CSVReader.readNext");
    assert_eq!(read_next.provenance, FactProvenance::ExternalLibraryPack);
    assert_eq!(read_next.return_nullability, NullabilityState::MaybeNull);
}

#[test]
fn org_json_pack_declares_object_array_boundary() {
    let facts = org_json_pack_facts();

    let pack = facts
        .external_library_pack_for_type("JSONObject")
        .expect("JSONObject should be owned by the org.json pack");
    assert_eq!(pack.pack_name, "org.json");
    assert_eq!(pack.mode, ExternalLibraryMode::ExternalFactModel);
    assert_eq!(pack.coverage, ExternalApiCoverage::Semantic);
    assert_eq!(pack.provenance, FactProvenance::ExternalLibraryPack);

    for (type_name, args) in [
        ("JSONObject", Vec::new()),
        (
            "JSONObject",
            vec![JavaType::Named {
                name: "JSONTokener".to_string(),
                args: Vec::new(),
            }],
        ),
        ("JSONArray", Vec::new()),
        (
            "JSONArray",
            vec![JavaType::Named {
                name: "JSONTokener".to_string(),
                args: Vec::new(),
            }],
        ),
        (
            "JSONTokener",
            vec![JavaType::Named {
                name: "InputStream".to_string(),
                args: Vec::new(),
            }],
        ),
    ] {
        let constructor = facts
            .external_constructor_for(type_name, &args)
            .unwrap_or_else(|| panic!("{type_name}/{} should be pack-owned", args.len()));
        assert_eq!(constructor.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(
            new_java2rust::stdlib::external_constructor_provenance(type_name, &args),
            FactProvenance::ConservativeFallback,
            "{type_name}/{} should not be a generic stdlib fallback",
            args.len()
        );
    }

    let put = facts
        .external_api_effect_for_receiver_with_arity(Some("JSONObject"), "put", Some(2))
        .expect("JSONObject.put should be pack-modeled");
    assert_eq!(put.provenance, FactProvenance::ExternalLibraryPack);
    assert_eq!(
        put.receiver_mutability,
        ReceiverMutabilityFact::MutatesReceiver
    );
    assert_eq!(
        put.return_type,
        Some(JavaType::Named {
            name: "JSONObject".to_string(),
            args: Vec::new(),
        })
    );

    let opt = facts
        .external_api_effect_for_receiver_with_arity(Some("JSONObject"), "opt", Some(1))
        .expect("JSONObject.opt should be pack-modeled");
    assert_eq!(opt.return_nullability, NullabilityState::MaybeNull);

    let has = facts
        .external_api_effect_for_receiver_with_arity(Some("JSONObject"), "has", Some(1))
        .expect("JSONObject.has should be pack-modeled");
    assert_eq!(
        has.return_type,
        Some(JavaType::Primitive(PrimitiveType::Boolean))
    );

    let array_put = facts
        .external_api_effect_for_receiver_with_arity(Some("JSONArray"), "put", Some(1))
        .expect("JSONArray.put should be pack-modeled");
    assert_eq!(
        array_put.receiver_mutability,
        ReceiverMutabilityFact::MutatesReceiver
    );

    let next_value = facts
        .external_api_effect_for_receiver_with_arity(Some("JSONTokener"), "nextValue", Some(0))
        .expect("JSONTokener.nextValue should be pack-modeled");
    assert_eq!(
        next_value.receiver_mutability,
        ReceiverMutabilityFact::MutatesReceiver
    );
    assert_eq!(next_value.return_nullability, NullabilityState::MaybeNull);
}

#[test]
fn semantic_analysis_uses_org_json_pack_for_gatk_shaped_json_code() {
    let semantic = analyze(
        r#"
class JsonRuntimeUse {
    void run(InputStream input, String sampleName) {
        JSONTokener tokener = new JSONTokener(input);
        JSONObject object = new JSONObject(tokener);
        object.put("sample", sampleName);
        boolean hasSample = object.has("sample");
        String sample = object.getString("sample");
        String fallback = object.optString("missing");
        Object maybeValue = object.opt("missing");
        int count = object.length();
        JSONArray array = new JSONArray();
        array.put(object);
        Object first = array.get(0);
        String firstName = array.getString(0);
        int arrayLength = array.length();
        Object parsed = tokener.nextValue();
    }
}
"#,
    );

    assert!(
        semantic.diagnostics.iter().all(|diagnostic| diagnostic.code
            != "external_pack_missing_constructor"
            && diagnostic.code != "external_pack_missing_api"),
        "modeled org.json constructors and APIs should not report pack gaps"
    );

    let constructor = semantic
        .facts
        .external_constructor_for(
            "JSONObject",
            &[JavaType::Named {
                name: "JSONTokener".to_string(),
                args: Vec::new(),
            }],
        )
        .expect("semantic analysis should use the org.json pack for JSONObject/1");
    assert_eq!(constructor.provenance, FactProvenance::ExternalLibraryPack);

    let put = semantic
        .facts
        .external_api_effect_for_receiver_with_arity(Some("JSONObject"), "put", Some(2))
        .expect("semantic analysis should model JSONObject.put");
    assert_eq!(put.provenance, FactProvenance::ExternalLibraryPack);
    assert_eq!(
        put.receiver_mutability,
        ReceiverMutabilityFact::MutatesReceiver
    );

    let opt = semantic
        .facts
        .external_api_effect_for_receiver_with_arity(Some("JSONObject"), "opt", Some(1))
        .expect("semantic analysis should model JSONObject.opt");
    assert_eq!(opt.return_nullability, NullabilityState::MaybeNull);
}

#[test]
fn reflections_pack_declares_conservative_scanning_boundary() {
    let facts = reflections_pack_facts();

    let pack = facts
        .external_library_pack_for_type("Reflections")
        .expect("Reflections should be owned by the Reflections pack");
    assert_eq!(pack.pack_name, "org.reflections");
    assert_eq!(pack.mode, ExternalLibraryMode::ExternalFactModel);
    assert_eq!(pack.coverage, ExternalApiCoverage::Conservative);
    assert_eq!(pack.provenance, FactProvenance::ExternalLibraryPack);

    let constructor_args = [JavaType::String];
    let constructor = facts
        .external_constructor_for("Reflections", &constructor_args)
        .expect("Reflections/1 should be pack-owned");
    assert_eq!(constructor.provenance, FactProvenance::ExternalLibraryPack);
    assert_eq!(
        new_java2rust::stdlib::external_constructor_provenance("Reflections", &constructor_args),
        FactProvenance::ConservativeFallback,
        "Reflections/1 should not be a generic stdlib fallback"
    );

    let subtypes = facts
        .external_api_effect_for_receiver_with_arity(Some("Reflections"), "getSubTypesOf", Some(1))
        .expect("Reflections.getSubTypesOf should be pack-modeled");
    assert_eq!(subtypes.provenance, FactProvenance::ExternalLibraryPack);
    assert_eq!(
        subtypes.receiver_mutability,
        ReceiverMutabilityFact::ReadOnly
    );
    assert_eq!(
        subtypes.return_nullability,
        NullabilityState::DefinitelyNonNull
    );
    assert_eq!(
        subtypes.return_type,
        Some(JavaType::Named {
            name: "Set".to_string(),
            args: vec![JavaType::Named {
                name: "Class".to_string(),
                args: Vec::new(),
            }],
        })
    );

    let methods = facts
        .external_api_effect_for_receiver_with_arity(
            Some("Reflections"),
            "getMethodsAnnotatedWith",
            Some(1),
        )
        .expect("Reflections.getMethodsAnnotatedWith should be pack-modeled");
    assert_eq!(
        methods.receiver_mutability,
        ReceiverMutabilityFact::ReadOnly
    );
    assert_eq!(
        methods.return_nullability,
        NullabilityState::DefinitelyNonNull
    );
}

#[test]
fn semantic_analysis_uses_reflections_pack_for_gatk_shaped_scans() {
    let semantic = analyze(
        r#"
class ReflectionsRuntimeUse {
    void run(String packageName, Class baseClass, Class annotationClass) {
        Reflections reflections = new Reflections(packageName);
        java.util.Set subtypes = reflections.getSubTypesOf(baseClass);
        java.util.Set annotatedTypes = reflections.getTypesAnnotatedWith(annotationClass);
        java.util.Set annotatedMethods = reflections.getMethodsAnnotatedWith(annotationClass);
        java.util.Set annotatedFields = reflections.getFieldsAnnotatedWith(annotationClass);
    }
}
"#,
    );

    assert!(
        semantic.diagnostics.iter().all(|diagnostic| diagnostic.code
            != "external_pack_missing_constructor"
            && diagnostic.code != "external_pack_missing_api"),
        "modeled Reflections constructors and discovery APIs should not report pack gaps"
    );

    let constructor = semantic
        .facts
        .external_constructor_for("Reflections", &[JavaType::String])
        .expect("semantic analysis should use the Reflections pack for Reflections/1");
    assert_eq!(constructor.provenance, FactProvenance::ExternalLibraryPack);

    let subtypes = semantic
        .facts
        .external_api_effect_for_receiver_with_arity(Some("Reflections"), "getSubTypesOf", Some(1))
        .expect("semantic analysis should model Reflections.getSubTypesOf");
    assert_eq!(subtypes.provenance, FactProvenance::ExternalLibraryPack);
    assert_eq!(
        subtypes.receiver_mutability,
        ReceiverMutabilityFact::ReadOnly
    );
}

#[test]
fn hadoop_conf_pack_declares_configuration_boundary() {
    let facts = hadoop_conf_pack_facts();

    let pack = facts
        .external_library_pack_for_type("Configuration")
        .expect("Configuration should be owned by the Hadoop conf pack");
    assert_eq!(pack.pack_name, "org.apache.hadoop.conf");
    assert_eq!(pack.mode, ExternalLibraryMode::ExternalFactModel);
    assert_eq!(pack.coverage, ExternalApiCoverage::Semantic);
    assert_eq!(pack.provenance, FactProvenance::ExternalLibraryPack);

    let constructor = facts
        .external_constructor_for("Configuration", &[])
        .expect("Configuration/0 should be pack-owned");
    assert_eq!(constructor.provenance, FactProvenance::ExternalLibraryPack);
    assert_eq!(
        new_java2rust::stdlib::external_constructor_provenance("Configuration", &[]),
        FactProvenance::ConservativeFallback,
        "Configuration/0 should not be a generic stdlib fallback"
    );

    let get_required = facts
        .external_api_effect_for_receiver_with_arity(Some("Configuration"), "get", Some(1))
        .expect("Configuration.get(String) should be pack-modeled");
    assert_eq!(get_required.provenance, FactProvenance::ExternalLibraryPack);
    assert_eq!(
        get_required.receiver_mutability,
        ReceiverMutabilityFact::ReadOnly
    );
    assert_eq!(get_required.return_nullability, NullabilityState::MaybeNull);
    assert_eq!(get_required.return_type, Some(JavaType::String));

    let get_defaulted = facts
        .external_api_effect_for_receiver_with_arity(Some("Configuration"), "get", Some(2))
        .expect("Configuration.get(String,String) should be pack-modeled");
    assert_eq!(
        get_defaulted.return_nullability,
        NullabilityState::DefinitelyNonNull
    );

    let set = facts
        .external_api_effect_for_receiver_with_arity(Some("Configuration"), "set", Some(2))
        .expect("Configuration.set should be pack-modeled");
    assert_eq!(set.provenance, FactProvenance::ExternalLibraryPack);
    assert_eq!(
        set.receiver_mutability,
        ReceiverMutabilityFact::MutatesReceiver
    );

    let get_boolean = facts
        .external_api_effect_for_receiver_with_arity(Some("Configuration"), "getBoolean", Some(2))
        .expect("Configuration.getBoolean should be pack-modeled");
    assert_eq!(
        get_boolean.return_type,
        Some(JavaType::Primitive(
            new_java2rust::java_ir::PrimitiveType::Boolean
        ))
    );
    assert_eq!(
        get_boolean.return_nullability,
        NullabilityState::DefinitelyNonNull
    );
}

#[test]
fn semantic_analysis_uses_hadoop_conf_pack_for_gatk_shaped_configuration_io() {
    let semantic = analyze(
        r#"
class HadoopConfigurationUse {
    void run() {
        Configuration configuration = new Configuration();
        configuration.set("sample.name", "NA12878");
        configuration.setBoolean("emit.filtered", true);
        configuration.setInt("max.records", 100);
        String sample = configuration.get("sample.name");
        String fallback = configuration.get("missing", "default");
        boolean emit = configuration.getBoolean("emit.filtered", false);
        int maxRecords = configuration.getInt("max.records", 10);
    }
}
"#,
    );

    assert!(
        semantic.diagnostics.iter().all(|diagnostic| diagnostic.code
            != "external_pack_missing_constructor"
            && diagnostic.code != "external_pack_missing_api"),
        "modeled Hadoop Configuration constructors and APIs should not report pack gaps"
    );

    let constructor = semantic
        .facts
        .external_constructor_for("Configuration", &[])
        .expect("semantic analysis should use the Hadoop conf pack for Configuration/0");
    assert_eq!(constructor.provenance, FactProvenance::ExternalLibraryPack);

    let get = semantic
        .facts
        .external_api_effect_for_receiver_with_arity(Some("Configuration"), "get", Some(1))
        .expect("semantic analysis should use the Hadoop conf pack for Configuration.get");
    assert_eq!(get.provenance, FactProvenance::ExternalLibraryPack);
    assert_eq!(get.return_nullability, NullabilityState::MaybeNull);

    let set = semantic
        .facts
        .external_api_effect_for_receiver_with_arity(Some("Configuration"), "set", Some(2))
        .expect("semantic analysis should use the Hadoop conf pack for Configuration.set");
    assert_eq!(
        set.receiver_mutability,
        ReceiverMutabilityFact::MutatesReceiver
    );
}

#[test]
fn apache_commons_config_pack_declares_builder_boundary_without_claiming_configuration() {
    let facts = apache_commons_config_pack_facts();

    let pack = facts
        .external_library_pack_for_type("Configurations")
        .expect("Configurations should be owned by the Apache Commons Configuration pack");
    assert_eq!(pack.pack_name, "org.apache.commons.configuration2");
    assert_eq!(pack.mode, ExternalLibraryMode::ExternalFactModel);
    assert_eq!(pack.coverage, ExternalApiCoverage::Semantic);
    assert_eq!(pack.provenance, FactProvenance::ExternalLibraryPack);
    assert!(
        facts
            .external_library_pack_for_type("Configuration")
            .is_none(),
        "Apache Commons Configuration must not claim the ambiguous bare Configuration type"
    );

    let constructor = facts
        .external_constructor_for("Configurations", &[])
        .expect("Configurations/0 should be pack-owned");
    assert_eq!(constructor.provenance, FactProvenance::ExternalLibraryPack);
    assert_eq!(
        new_java2rust::stdlib::external_constructor_provenance("Configurations", &[]),
        FactProvenance::ConservativeFallback,
        "Configurations/0 should not be a generic stdlib fallback"
    );

    let properties = facts
        .external_api_effect_for_receiver(Some("Configurations"), "properties")
        .expect("Configurations.properties should be pack-modeled");
    assert_eq!(properties.provenance, FactProvenance::ExternalLibraryPack);
    assert_eq!(
        properties.receiver_mutability,
        ReceiverMutabilityFact::ReadOnly
    );
    assert_eq!(
        properties.return_nullability,
        NullabilityState::DefinitelyNonNull
    );
    assert_eq!(
        properties.return_type,
        Some(JavaType::Named {
            name: "PropertiesConfiguration".to_string(),
            args: Vec::new(),
        })
    );

    let get_string = facts
        .external_api_effect_for_receiver(Some("PropertiesConfiguration"), "getString")
        .expect("PropertiesConfiguration.getString should be pack-modeled");
    assert_eq!(get_string.provenance, FactProvenance::ExternalLibraryPack);
    assert_eq!(get_string.return_type, Some(JavaType::String));
    assert_eq!(get_string.return_nullability, NullabilityState::MaybeNull);

    let set_property = facts
        .external_api_effect_for_receiver(Some("PropertiesConfiguration"), "setProperty")
        .expect("PropertiesConfiguration.setProperty should be pack-modeled");
    assert_eq!(
        set_property.receiver_mutability,
        ReceiverMutabilityFact::MutatesReceiver
    );
}

#[test]
fn semantic_analysis_uses_apache_commons_config_pack_for_gatk_shaped_loading() {
    let semantic = analyze(
        r#"
class ApacheCommonsConfigurationUse {
    void run(File propertiesFile) {
        Configurations configurations = new Configurations();
        PropertiesConfiguration properties = configurations.properties(propertiesFile);
        String sampleName = properties.getString("sample.name");
        boolean emit = properties.getBoolean("emit.filtered");
        int maxRecords = properties.getInt("max.records");
        properties.setProperty("last.sample", sampleName);
    }
}
"#,
    );

    assert!(
        semantic.diagnostics.iter().all(|diagnostic| diagnostic.code
            != "external_pack_missing_constructor"
            && diagnostic.code != "external_pack_missing_api"),
        "modeled Apache Commons Configuration constructors and APIs should not report pack gaps"
    );

    let constructor = semantic
        .facts
        .external_constructor_for("Configurations", &[])
        .expect("semantic analysis should use the Apache Commons Configuration pack");
    assert_eq!(constructor.provenance, FactProvenance::ExternalLibraryPack);

    let properties = semantic
        .facts
        .external_api_effect_for_receiver(Some("Configurations"), "properties")
        .expect("semantic analysis should model Configurations.properties");
    assert_eq!(properties.provenance, FactProvenance::ExternalLibraryPack);
    assert_eq!(
        properties.return_nullability,
        NullabilityState::DefinitelyNonNull
    );

    let get_string = semantic
        .facts
        .external_api_effect_for_receiver(Some("PropertiesConfiguration"), "getString")
        .expect("semantic analysis should model PropertiesConfiguration.getString");
    assert_eq!(get_string.return_nullability, NullabilityState::MaybeNull);
}

#[test]
fn spark_pack_declares_runtime_boundary() {
    let facts = spark_pack_facts();

    let pack = facts
        .external_library_pack_for_type("JavaSparkContext")
        .expect("JavaSparkContext should be owned by the Spark pack");
    assert_eq!(pack.pack_name, "org.apache.spark");
    assert_eq!(pack.mode, ExternalLibraryMode::ExternalFactModel);
    assert_eq!(pack.coverage, ExternalApiCoverage::Semantic);
    assert_eq!(pack.provenance, FactProvenance::ExternalLibraryPack);

    for (type_name, args) in [
        ("SparkConf", Vec::new()),
        (
            "HashPartitioner",
            vec![JavaType::Primitive(
                new_java2rust::java_ir::PrimitiveType::Int,
            )],
        ),
        (
            "JavaSparkContext",
            vec![JavaType::Named {
                name: "SparkConf".to_string(),
                args: Vec::new(),
            }],
        ),
        (
            "JavaRDD",
            vec![
                JavaType::Named {
                    name: "Object".to_string(),
                    args: Vec::new(),
                },
                JavaType::Named {
                    name: "Object".to_string(),
                    args: Vec::new(),
                },
            ],
        ),
        ("PartitionGroup", vec![JavaType::String]),
        (
            "RowMatrix",
            vec![JavaType::Named {
                name: "Object".to_string(),
                args: Vec::new(),
            }],
        ),
    ] {
        let constructor = facts
            .external_constructor_for(type_name, &args)
            .unwrap_or_else(|| panic!("{type_name}/{} should be pack-owned", args.len()));
        assert_eq!(constructor.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(
            new_java2rust::stdlib::external_constructor_provenance(type_name, &args),
            FactProvenance::ConservativeFallback,
            "{type_name}/{} should not be a generic stdlib fallback",
            args.len()
        );
    }

    let set_app_name = facts
        .external_api_effect_for_receiver(Some("SparkConf"), "setAppName")
        .expect("SparkConf.setAppName should be pack-modeled");
    assert_eq!(set_app_name.provenance, FactProvenance::ExternalLibraryPack);
    assert_eq!(
        set_app_name.receiver_mutability,
        ReceiverMutabilityFact::MutatesReceiver
    );
    assert_eq!(
        set_app_name.return_type,
        Some(JavaType::Named {
            name: "SparkConf".to_string(),
            args: Vec::new(),
        })
    );

    let parallelize = facts
        .external_api_effect_for_receiver(Some("JavaSparkContext"), "parallelize")
        .expect("JavaSparkContext.parallelize should be pack-modeled");
    assert_eq!(parallelize.provenance, FactProvenance::ExternalLibraryPack);
    assert_eq!(
        parallelize.receiver_mutability,
        ReceiverMutabilityFact::ReadOnly
    );
    assert_eq!(
        parallelize.return_type,
        Some(JavaType::Named {
            name: "JavaRDD".to_string(),
            args: Vec::new(),
        })
    );

    let hadoop_configuration = facts
        .external_api_effect_for_receiver(Some("JavaSparkContext"), "hadoopConfiguration")
        .expect("JavaSparkContext.hadoopConfiguration should be pack-modeled");
    assert_eq!(
        hadoop_configuration.return_type,
        Some(JavaType::Named {
            name: "Configuration".to_string(),
            args: Vec::new(),
        })
    );

    let context = facts
        .external_api_effect_for_receiver(Some("JavaRDD"), "context")
        .expect("JavaRDD.context should be pack-modeled");
    assert_eq!(
        context.return_type,
        Some(JavaType::Named {
            name: "JavaSparkContext".to_string(),
            args: Vec::new(),
        })
    );

    let get_num_partitions = facts
        .external_api_effect_for_receiver(Some("JavaRDD"), "getNumPartitions")
        .expect("JavaRDD.getNumPartitions should be pack-modeled");
    assert_eq!(
        get_num_partitions.return_type,
        Some(JavaType::Primitive(
            new_java2rust::java_ir::PrimitiveType::Int
        ))
    );

    let raw_rdd = facts
        .external_api_effect_for_receiver(Some("JavaRDD"), "rdd")
        .expect("JavaRDD.rdd should be pack-modeled");
    assert_eq!(
        raw_rdd.return_type,
        Some(JavaType::Named {
            name: "Object".to_string(),
            args: Vec::new(),
        })
    );

    let map_partitions = facts
        .external_api_effect_for_receiver_with_arity(Some("JavaRDD"), "mapPartitions", Some(1))
        .expect("JavaRDD.mapPartitions/1 should be pack-modeled");
    assert_eq!(
        map_partitions.return_type,
        Some(JavaType::Named {
            name: "JavaRDD".to_string(),
            args: Vec::new(),
        })
    );

    let map_partitions_preserving = facts
        .external_api_effect_for_receiver_with_arity(Some("JavaRDD"), "mapPartitions", Some(2))
        .expect("JavaRDD.mapPartitions/2 should be pack-modeled");
    assert_eq!(
        map_partitions_preserving.return_type,
        Some(JavaType::Named {
            name: "JavaRDD".to_string(),
            args: Vec::new(),
        })
    );

    let map_partitions_with_index = facts
        .external_api_effect_for_receiver_with_arity(
            Some("JavaRDD"),
            "mapPartitionsWithIndex",
            Some(2),
        )
        .expect("JavaRDD.mapPartitionsWithIndex/2 should be pack-modeled");
    assert_eq!(
        map_partitions_with_index.return_type,
        Some(JavaType::Named {
            name: "JavaRDD".to_string(),
            args: Vec::new(),
        })
    );

    let map_to_pair = facts
        .external_api_effect_for_receiver_with_arity(Some("JavaRDD"), "mapToPair", Some(1))
        .expect("JavaRDD.mapToPair/1 should be pack-modeled");
    assert_eq!(
        map_to_pair.return_type,
        Some(JavaType::Named {
            name: "JavaPairRDD".to_string(),
            args: Vec::new(),
        })
    );

    let map_partitions_to_pair = facts
        .external_api_effect_for_receiver_with_arity(
            Some("JavaRDD"),
            "mapPartitionsToPair",
            Some(1),
        )
        .expect("JavaRDD.mapPartitionsToPair/1 should be pack-modeled");
    assert_eq!(
        map_partitions_to_pair.return_type,
        Some(JavaType::Named {
            name: "JavaPairRDD".to_string(),
            args: Vec::new(),
        })
    );

    let map_partitions_to_pair_preserving = facts
        .external_api_effect_for_receiver_with_arity(
            Some("JavaRDD"),
            "mapPartitionsToPair",
            Some(2),
        )
        .expect("JavaRDD.mapPartitionsToPair/2 should be pack-modeled");
    assert_eq!(
        map_partitions_to_pair_preserving.return_type,
        Some(JavaType::Named {
            name: "JavaPairRDD".to_string(),
            args: Vec::new(),
        })
    );

    let zip_partitions = facts
        .external_api_effect_for_receiver_with_arity(Some("JavaRDD"), "zipPartitions", Some(2))
        .expect("JavaRDD.zipPartitions/2 should be pack-modeled");
    assert_eq!(
        zip_partitions.return_type,
        Some(JavaType::Named {
            name: "JavaRDD".to_string(),
            args: Vec::new(),
        })
    );

    let sort_by_key_1 = facts
        .external_api_effect_for_receiver_with_arity(Some("JavaPairRDD"), "sortByKey", Some(1))
        .expect("JavaPairRDD.sortByKey/1 should be pack-modeled");
    assert_eq!(
        sort_by_key_1.return_type,
        Some(JavaType::Named {
            name: "JavaPairRDD".to_string(),
            args: Vec::new(),
        })
    );

    let sort_by_key_3 = facts
        .external_api_effect_for_receiver_with_arity(Some("JavaPairRDD"), "sortByKey", Some(3))
        .expect("JavaPairRDD.sortByKey/3 should be pack-modeled");
    assert_eq!(
        sort_by_key_3.return_type,
        Some(JavaType::Named {
            name: "JavaPairRDD".to_string(),
            args: Vec::new(),
        })
    );

    let keys = facts
        .external_api_effect_for_receiver_with_arity(Some("JavaPairRDD"), "keys", Some(0))
        .expect("JavaPairRDD.keys/0 should be pack-modeled");
    assert_eq!(
        keys.return_type,
        Some(JavaType::Named {
            name: "JavaRDD".to_string(),
            args: Vec::new(),
        })
    );

    let pair_map_partitions = facts
        .external_api_effect_for_receiver_with_arity(
            Some("JavaPairRDD"),
            "mapPartitionsToPair",
            Some(1),
        )
        .expect("JavaPairRDD.mapPartitionsToPair/1 should be pack-modeled");
    assert_eq!(
        pair_map_partitions.return_type,
        Some(JavaType::Named {
            name: "JavaPairRDD".to_string(),
            args: Vec::new(),
        })
    );

    let pair_map_partitions_to_rdd = facts
        .external_api_effect_for_receiver_with_arity(Some("JavaPairRDD"), "mapPartitions", Some(1))
        .expect("JavaPairRDD.mapPartitions/1 should be pack-modeled");
    assert_eq!(
        pair_map_partitions_to_rdd.return_type,
        Some(JavaType::Named {
            name: "JavaRDD".to_string(),
            args: Vec::new(),
        })
    );

    let pair_filter = facts
        .external_api_effect_for_receiver_with_arity(Some("JavaPairRDD"), "filter", Some(1))
        .expect("JavaPairRDD.filter/1 should be pack-modeled");
    assert_eq!(
        pair_filter.return_type,
        Some(JavaType::Named {
            name: "JavaPairRDD".to_string(),
            args: Vec::new(),
        })
    );

    let pair_flat_map = facts
        .external_api_effect_for_receiver_with_arity(Some("JavaPairRDD"), "flatMap", Some(1))
        .expect("JavaPairRDD.flatMap/1 should be pack-modeled");
    assert_eq!(
        pair_flat_map.return_type,
        Some(JavaType::Named {
            name: "JavaRDD".to_string(),
            args: Vec::new(),
        })
    );

    let pair_map = facts
        .external_api_effect_for_receiver_with_arity(Some("JavaPairRDD"), "map", Some(1))
        .expect("JavaPairRDD.map/1 should be pack-modeled");
    assert_eq!(
        pair_map.return_type,
        Some(JavaType::Named {
            name: "JavaRDD".to_string(),
            args: Vec::new(),
        })
    );

    let reduce_by_key = facts
        .external_api_effect_for_receiver_with_arity(Some("JavaPairRDD"), "reduceByKey", Some(1))
        .expect("JavaPairRDD.reduceByKey/1 should be pack-modeled");
    assert_eq!(
        reduce_by_key.return_type,
        Some(JavaType::Named {
            name: "JavaPairRDD".to_string(),
            args: Vec::new(),
        })
    );

    let combine_by_key = facts
        .external_api_effect_for_receiver_with_arity(Some("JavaPairRDD"), "combineByKey", Some(3))
        .expect("JavaPairRDD.combineByKey/3 should be pack-modeled");
    assert_eq!(
        combine_by_key.return_type,
        Some(JavaType::Named {
            name: "JavaPairRDD".to_string(),
            args: Vec::new(),
        })
    );

    let combine_by_key_partitioned = facts
        .external_api_effect_for_receiver_with_arity(Some("JavaPairRDD"), "combineByKey", Some(6))
        .expect("JavaPairRDD.combineByKey/6 should be pack-modeled");
    assert_eq!(
        combine_by_key_partitioned.return_type,
        Some(JavaType::Named {
            name: "JavaPairRDD".to_string(),
            args: Vec::new(),
        })
    );

    let pair_partition_by = facts
        .external_api_effect_for_receiver_with_arity(Some("JavaPairRDD"), "partitionBy", Some(1))
        .expect("JavaPairRDD.partitionBy/1 should be pack-modeled");
    assert_eq!(
        pair_partition_by.return_type,
        Some(JavaType::Named {
            name: "JavaPairRDD".to_string(),
            args: Vec::new(),
        })
    );

    let destroy = facts
        .external_api_effect_for_receiver(Some("Broadcast"), "destroy")
        .expect("Broadcast.destroy should be pack-modeled");
    assert_eq!(destroy.role, ExternalApiRole::SideEffect);
    assert_eq!(
        destroy.receiver_mutability,
        ReceiverMutabilityFact::MutatesReceiver
    );

    for method_name in ["value", "getValue"] {
        let effect = facts
            .external_api_effect_for_receiver(Some("Broadcast"), method_name)
            .unwrap_or_else(|| panic!("Broadcast.{method_name} should be pack-modeled"));
        assert_eq!(
            effect.return_type,
            Some(JavaType::Named {
                name: "Object".to_string(),
                args: Vec::new(),
            })
        );
        assert_eq!(
            effect.return_nullability,
            NullabilityState::DefinitelyNonNull
        );
    }

    let cache = facts
        .external_api_effect_for_receiver(Some("JavaRDD"), "cache")
        .expect("JavaRDD.cache should be pack-modeled");
    assert_eq!(
        cache.receiver_mutability,
        ReceiverMutabilityFact::MutatesReceiver
    );

    let unpersist = facts
        .external_api_effect_for_receiver(Some("JavaRDD"), "unpersist")
        .expect("JavaRDD.unpersist should be pack-modeled");
    assert_eq!(
        unpersist.receiver_mutability,
        ReceiverMutabilityFact::MutatesReceiver
    );
    assert_eq!(
        unpersist.return_type,
        Some(JavaType::Named {
            name: "JavaRDD".to_string(),
            args: Vec::new(),
        })
    );

    let save_as_text_file = facts
        .external_api_effect_for_receiver(Some("JavaRDD"), "saveAsTextFile")
        .expect("JavaRDD.saveAsTextFile should be pack-modeled");
    assert_eq!(save_as_text_file.role, ExternalApiRole::SideEffect);

    let count = facts
        .external_api_effect_for_receiver(Some("JavaRDD"), "count")
        .expect("JavaRDD.count should be pack-modeled");
    assert_eq!(
        count.return_type,
        Some(JavaType::Primitive(
            new_java2rust::java_ir::PrimitiveType::Long
        ))
    );
}

#[test]
fn semantic_analysis_uses_spark_pack_for_gatk_shaped_runtime_code() {
    let semantic = analyze(
        r#"
class SparkRuntimeUse {
    void run(java.util.List<String> values, Object rawRdd, Object tag, int partitions) {
        SparkConf conf = new SparkConf().setAppName("gatk").setMaster("local");
        JavaSparkContext context = new JavaSparkContext(conf);
        JavaRDD reads = new JavaRDD(rawRdd, tag);
        JavaRDD parallelized = context.parallelize(values);
        JavaRDD cached = parallelized.cache();
        JavaRDD filtered = cached.filter(value -> true);
        JavaRDD mappedPartitions = filtered.mapPartitions(iterator -> iterator);
        JavaRDD mappedPreservingPartitions = filtered.mapPartitions(iterator -> iterator, true);
        JavaRDD indexedPartitions = mappedPartitions.mapPartitionsWithIndex((index, iterator) -> iterator, true);
        JavaPairRDD pairs = indexedPartitions.mapToPair(read -> null);
        JavaPairRDD partitionPairs = indexedPartitions.mapPartitionsToPair(iterator -> iterator);
        JavaPairRDD partitionPairsPreserving = indexedPartitions.mapPartitionsToPair(iterator -> iterator, false);
        JavaPairRDD sortedPairs = pairs.sortByKey(null);
        JavaPairRDD sortedPairsWithReducers = partitionPairs.sortByKey(null, true, partitions);
        JavaPairRDD pairPartitions = sortedPairsWithReducers.mapPartitionsToPair(iterator -> iterator);
        JavaPairRDD reducedPairs = partitionPairsPreserving.reduceByKey((left, right) -> left);
        JavaPairRDD combinedPairs = reducedPairs.combineByKey(value -> value, (left, value) -> left, (left, right) -> left);
        JavaPairRDD partitionedCombinedPairs = reducedPairs.combineByKey(value -> value, (left, value) -> left, (left, right) -> left, new HashPartitioner(partitions), false, null);
        JavaPairRDD filteredPairs = combinedPairs.filter(pair -> true);
        JavaPairRDD hashPartitionedPairs = reducedPairs.partitionBy(new HashPartitioner(partitions));
        JavaRDD pairPartitionValues = hashPartitionedPairs.mapPartitions(iterator -> iterator);
        JavaRDD pairFlatMapped = filteredPairs.flatMap(pair -> null);
        JavaRDD pairMapped = partitionedCombinedPairs.map(pair -> null);
        JavaRDD keys = pairPartitions.keys();
        JavaRDD zipped = indexedPartitions.zipPartitions(filtered, (left, right) -> left);
        JavaRDD repartitioned = filtered.repartition(partitions);
        JavaRDD partitioned = repartitioned.partitionBy(new HashPartitioner(partitions));
        JavaSparkContext owner = partitioned.context();
        org.apache.hadoop.conf.Configuration hadoopConf = owner.hadoopConfiguration();
        Object raw = partitioned.rdd();
        int partitionCount = partitioned.getNumPartitions();
        Broadcast handle = context.broadcast("reads");
        Object broadcastValue = handle.value();
        Object broadcastValueAlias = handle.getValue();
        handle.destroy();
        partitioned.saveAsTextFile("out");
        partitioned.unpersist();
        long count = partitioned.count();
        java.util.List collected = partitioned.collect();
        RowMatrix matrix = new RowMatrix(rawRdd);
        long rows = matrix.numRows();
        PartitionGroup group = new PartitionGroup("worker");
        context.stop();
    }
}
"#,
    );

    assert!(
        semantic.diagnostics.iter().all(|diagnostic| diagnostic.code
            != "external_pack_missing_constructor"
            && diagnostic.code != "external_pack_missing_api"),
        "modeled Spark constructors and APIs should not report pack gaps"
    );

    let constructor = semantic
        .facts
        .external_constructor_for(
            "JavaSparkContext",
            &[JavaType::Named {
                name: "SparkConf".to_string(),
                args: Vec::new(),
            }],
        )
        .expect("semantic analysis should use the Spark pack for JavaSparkContext/1");
    assert_eq!(constructor.provenance, FactProvenance::ExternalLibraryPack);

    let parallelize = semantic
        .facts
        .external_api_effect_for_receiver(Some("JavaSparkContext"), "parallelize")
        .expect("semantic analysis should model JavaSparkContext.parallelize");
    assert_eq!(parallelize.provenance, FactProvenance::ExternalLibraryPack);

    let cache = semantic
        .facts
        .external_api_effect_for_receiver(Some("JavaRDD"), "cache")
        .expect("semantic analysis should model JavaRDD.cache");
    assert_eq!(
        cache.receiver_mutability,
        ReceiverMutabilityFact::MutatesReceiver
    );
}

#[test]
fn bwa_pack_declares_native_wrapper_boundary() {
    let facts = bwa_pack_facts();

    let pack = facts
        .external_library_pack_for_type("BwaMemAligner")
        .expect("BwaMemAligner should be owned by the BWA pack");
    assert_eq!(pack.pack_name, "org.broadinstitute.hellbender.utils.bwa");
    assert_eq!(pack.mode, ExternalLibraryMode::ExternalFactModel);
    assert_eq!(pack.coverage, ExternalApiCoverage::Semantic);
    assert_eq!(pack.provenance, FactProvenance::ExternalLibraryPack);

    let alignment_pack = facts
        .external_library_pack_for_type("BwaMemAlignment")
        .expect("BwaMemAlignment should be owned by the BWA pack");
    assert_eq!(
        alignment_pack.pack_name,
        "org.broadinstitute.hellbender.utils.bwa"
    );

    let alignment_constructor_args = [
        JavaType::Primitive(PrimitiveType::Int),
        JavaType::Primitive(PrimitiveType::Int),
        JavaType::Primitive(PrimitiveType::Int),
        JavaType::Primitive(PrimitiveType::Int),
        JavaType::Primitive(PrimitiveType::Int),
        JavaType::Primitive(PrimitiveType::Int),
        JavaType::Primitive(PrimitiveType::Int),
        JavaType::Primitive(PrimitiveType::Int),
        JavaType::Primitive(PrimitiveType::Int),
        JavaType::Primitive(PrimitiveType::Int),
        JavaType::String,
        JavaType::String,
        JavaType::String,
        JavaType::Primitive(PrimitiveType::Int),
        JavaType::Primitive(PrimitiveType::Int),
        JavaType::Primitive(PrimitiveType::Int),
    ];
    let alignment_constructor = facts
        .external_constructor_for("BwaMemAlignment", &alignment_constructor_args)
        .expect("BwaMemAlignment/16 should be pack-owned for serializer reconstruction");
    assert_eq!(
        alignment_constructor.provenance,
        FactProvenance::ExternalLibraryPack
    );

    for (type_name, args) in [
        ("BwaMemIndex", vec![JavaType::String]),
        (
            "BwaMemAligner",
            vec![JavaType::Named {
                name: "BwaMemIndex".to_string(),
                args: Vec::new(),
            }],
        ),
    ] {
        let constructor = facts
            .external_constructor_for(type_name, &args)
            .unwrap_or_else(|| panic!("{type_name}/{} should be pack-owned", args.len()));
        assert_eq!(constructor.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(
            new_java2rust::stdlib::external_constructor_provenance(type_name, &args),
            FactProvenance::ConservativeFallback,
            "{type_name}/{} should not be a generic stdlib fallback",
            args.len()
        );
    }

    let field = facts
        .external_field_for("BwaMemAligner", "MEM_F_ALL")
        .expect("BwaMemAligner.MEM_F_ALL should be pack-modeled");
    assert_eq!(field.provenance, FactProvenance::ExternalLibraryPack);
    assert_eq!(
        field.field_type,
        Some(JavaType::Primitive(
            new_java2rust::java_ir::PrimitiveType::Int
        ))
    );
    assert_eq!(
        new_java2rust::stdlib::external_field_provenance("BwaMemAligner", "MEM_F_ALL"),
        FactProvenance::ConservativeFallback,
        "BwaMemAligner.MEM_F_ALL should not be a generic stdlib field fallback"
    );

    for method_name in [
        "getSamFlag",
        "getRefId",
        "getRefStart",
        "getRefEnd",
        "getSeqStart",
        "getSeqEnd",
        "getMapQual",
        "getNMismatches",
        "getAlignerScore",
        "getSuboptimalScore",
        "getMateRefId",
        "getMateRefStart",
        "getTemplateLen",
    ] {
        let effect = facts
            .external_api_effect_for_receiver(Some("BwaMemAlignment"), method_name)
            .unwrap_or_else(|| panic!("BwaMemAlignment.{method_name} should be pack-modeled"));
        assert_eq!(effect.role, ExternalApiRole::Getter);
        assert_eq!(
            effect.return_type,
            Some(JavaType::Primitive(PrimitiveType::Int))
        );
    }

    for (method_name, expected_nullability) in [
        ("getCigar", NullabilityState::DefinitelyNonNull),
        ("getMDTag", NullabilityState::MaybeNull),
        ("getXATag", NullabilityState::MaybeNull),
    ] {
        let effect = facts
            .external_api_effect_for_receiver(Some("BwaMemAlignment"), method_name)
            .unwrap_or_else(|| panic!("BwaMemAlignment.{method_name} should be pack-modeled"));
        assert_eq!(effect.return_type, Some(JavaType::String));
        assert_eq!(effect.return_nullability, expected_nullability);
    }

    for method_name in ["setIntraCtgOptions", "setZDropOption"] {
        let effect = facts
            .external_api_effect_for_receiver(Some("BwaMemAligner"), method_name)
            .unwrap_or_else(|| panic!("BwaMemAligner.{method_name} should be pack-modeled"));
        assert_eq!(effect.role, ExternalApiRole::Mutator);
        assert_eq!(
            effect.receiver_mutability,
            ReceiverMutabilityFact::MutatesReceiver
        );
    }

    let align_seqs = facts
        .external_api_effect_for_receiver(Some("BwaMemAligner"), "alignSeqs")
        .expect("BwaMemAligner.alignSeqs should be pack-modeled");
    assert_eq!(align_seqs.role, ExternalApiRole::Getter);
    assert_eq!(
        align_seqs.receiver_mutability,
        ReceiverMutabilityFact::ReadOnly
    );
    assert_eq!(
        align_seqs.return_type,
        Some(JavaType::Named {
            name: "List".to_string(),
            args: vec![JavaType::Named {
                name: "List".to_string(),
                args: vec![JavaType::Named {
                    name: "BwaMemAlignment".to_string(),
                    args: Vec::new(),
                }],
            }],
        })
    );
}

#[test]
fn semantic_analysis_uses_bwa_pack_for_native_wrappers_and_flags() {
    let semantic = analyze(
        r#"
class BwaNativeWrapperUse {
    Object run(String indexImage, java.util.List<byte[]> sequences, int zDrop) {
        BwaMemIndex index = new BwaMemIndex(indexImage);
        BwaMemAligner aligner = new BwaMemAligner(index);
        int all = BwaMemAligner.MEM_F_ALL;
        aligner.setIntraCtgOptions();
        aligner.setZDropOption(zDrop);
        return aligner.alignSeqs(sequences);
    }
}
"#,
    );

    assert!(
        semantic.diagnostics.iter().all(|diagnostic| diagnostic.code
            != "external_pack_missing_constructor"
            && diagnostic.code != "external_pack_missing_field"
            && diagnostic.code != "external_pack_missing_api"),
        "modeled BWA constructors, fields, and methods should not report pack gaps: {:?}",
        semantic.diagnostics
    );

    let constructor = semantic
        .facts
        .external_constructor_for("BwaMemAligner", &[JavaType::Unknown("_".to_string())])
        .expect("semantic analysis should use the BWA pack for BwaMemAligner/1");
    assert_eq!(constructor.provenance, FactProvenance::ExternalLibraryPack);

    let align_seqs = semantic
        .facts
        .external_api_effect_for_receiver(Some("BwaMemAligner"), "alignSeqs")
        .expect("semantic analysis should use the BWA pack for alignSeqs");
    assert_eq!(
        align_seqs.return_nullability,
        NullabilityState::DefinitelyNonNull
    );
}

#[test]
fn jgrapht_pack_declares_graph_algorithm_boundary() {
    let facts = jgrapht_pack_facts();

    let pack = facts
        .external_library_pack_for_type("ConnectivityInspector")
        .expect("ConnectivityInspector should be owned by the JGraphT pack");
    assert_eq!(pack.pack_name, "org.jgrapht");
    assert_eq!(pack.mode, ExternalLibraryMode::ExternalFactModel);
    assert_eq!(pack.coverage, ExternalApiCoverage::Semantic);
    assert_eq!(pack.provenance, FactProvenance::ExternalLibraryPack);

    for (type_name, args) in [
        ("IntegerComponentNameProvider", Vec::new()),
        ("SimpleGraph", vec![JavaType::Unknown("_".to_string())]),
        (
            "ConnectivityInspector",
            vec![JavaType::Unknown("_".to_string())],
        ),
        ("CycleDetector", vec![JavaType::Unknown("_".to_string())]),
        (
            "DefaultDirectedWeightedGraph",
            vec![JavaType::Unknown("_".to_string())],
        ),
        (
            "DOTExporter",
            vec![
                JavaType::Unknown("_".to_string()),
                JavaType::Unknown("_".to_string()),
                JavaType::Unknown("_".to_string()),
                JavaType::Unknown("_".to_string()),
                JavaType::Unknown("_".to_string()),
            ],
        ),
    ] {
        let constructor = facts
            .external_constructor_for(type_name, &args)
            .unwrap_or_else(|| panic!("{type_name}/{} should be pack-owned", args.len()));
        assert_eq!(constructor.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(
            new_java2rust::stdlib::external_constructor_provenance(type_name, &args),
            FactProvenance::ConservativeFallback,
            "{type_name}/{} should not be a generic stdlib fallback",
            args.len()
        );
    }

    let connected_sets = facts
        .external_api_effect_for_receiver_with_arity(
            Some("ConnectivityInspector"),
            "connectedSets",
            Some(0),
        )
        .expect("ConnectivityInspector.connectedSets should be pack-modeled");
    assert_eq!(
        connected_sets.return_type,
        Some(JavaType::Named {
            name: "List".to_string(),
            args: vec![JavaType::Named {
                name: "Set".to_string(),
                args: vec![JavaType::Named {
                    name: "Object".to_string(),
                    args: Vec::new(),
                }],
            }],
        })
    );
    assert_eq!(
        connected_sets.return_nullability,
        NullabilityState::DefinitelyNonNull
    );

    let detect_cycles = facts
        .external_api_effect_for_receiver_with_arity(Some("CycleDetector"), "detectCycles", Some(0))
        .expect("CycleDetector.detectCycles should be pack-modeled");
    assert_eq!(
        detect_cycles.return_type,
        Some(JavaType::Primitive(PrimitiveType::Boolean))
    );

    let add_vertex = facts
        .external_api_effect_for_receiver_with_arity(Some("SimpleGraph"), "addVertex", Some(1))
        .expect("SimpleGraph.addVertex should be pack-modeled");
    assert_eq!(
        add_vertex.receiver_mutability,
        ReceiverMutabilityFact::MutatesReceiver
    );

    let set_edge_weight = facts
        .external_api_effect_for_receiver_with_arity(
            Some("DefaultDirectedWeightedGraph"),
            "setEdgeWeight",
            Some(2),
        )
        .expect("DefaultDirectedWeightedGraph.setEdgeWeight should be pack-modeled");
    assert_eq!(
        set_edge_weight.receiver_mutability,
        ReceiverMutabilityFact::MutatesReceiver
    );
    assert_eq!(set_edge_weight.return_type, None);

    let export_graph = facts
        .external_api_effect_for_receiver(Some("DOTExporter"), "exportGraph")
        .expect("DOTExporter.exportGraph should be pack-modeled");
    assert_eq!(export_graph.role, ExternalApiRole::SideEffect);
    assert_eq!(
        export_graph.receiver_mutability,
        ReceiverMutabilityFact::ReadOnly
    );
    assert_eq!(export_graph.return_type, None);
}

#[test]
fn semantic_analysis_uses_jgrapht_pack_for_gatk_shaped_graph_code() {
    let semantic = analyze(
        r#"
class JGraphTRuntimeUse {
    void run(Graph graph, Class edgeClass, Object v1, Object v2, Object p1, Object p2, Object p3, Object p4, Object p5, File out) {
        SimpleGraph simpleGraph = new SimpleGraph(edgeClass);
        simpleGraph.addVertex(v1);
        Object simpleEdge = simpleGraph.addEdge(v1, v2);
        ConnectivityInspector inspector = new ConnectivityInspector(simpleGraph);
        java.util.List components = inspector.connectedSets();
        CycleDetector detector = new CycleDetector(graph);
        boolean cyclic = detector.detectCycles();
        DefaultDirectedWeightedGraph weightedGraph = new DefaultDirectedWeightedGraph(edgeClass);
        weightedGraph.addVertex(v1);
        Object weightedEdge = weightedGraph.addEdge(v1, v2);
        weightedGraph.setEdgeWeight(weightedEdge, 2.0);
        java.util.Set vertices = weightedGraph.vertexSet();
        IntegerComponentNameProvider names = new IntegerComponentNameProvider();
        DOTExporter exporter = new DOTExporter(p1, p2, p3, p4, p5);
        exporter.exportGraph(weightedGraph, out);
    }
}
"#,
    );

    assert!(
        semantic.diagnostics.iter().all(|diagnostic| diagnostic.code
            != "external_pack_missing_constructor"
            && diagnostic.code != "external_pack_missing_api"),
        "modeled JGraphT constructors and APIs should not report pack gaps"
    );

    let constructor = semantic
        .facts
        .external_constructor_for(
            "ConnectivityInspector",
            &[JavaType::Named {
                name: "SimpleGraph".to_string(),
                args: Vec::new(),
            }],
        )
        .expect("semantic analysis should use the JGraphT pack for ConnectivityInspector/1");
    assert_eq!(constructor.provenance, FactProvenance::ExternalLibraryPack);

    let connected_sets = semantic
        .facts
        .external_api_effect_for_receiver_with_arity(
            Some("ConnectivityInspector"),
            "connectedSets",
            Some(0),
        )
        .expect("semantic analysis should model ConnectivityInspector.connectedSets");
    assert_eq!(
        connected_sets.provenance,
        FactProvenance::ExternalLibraryPack
    );

    let detect_cycles = semantic
        .facts
        .external_api_effect_for_receiver_with_arity(Some("CycleDetector"), "detectCycles", Some(0))
        .expect("semantic analysis should model CycleDetector.detectCycles");
    assert_eq!(
        detect_cycles.provenance,
        FactProvenance::ExternalLibraryPack
    );

    let set_edge_weight = semantic
        .facts
        .external_api_effect_for_receiver_with_arity(
            Some("DefaultDirectedWeightedGraph"),
            "setEdgeWeight",
            Some(2),
        )
        .expect("semantic analysis should model DefaultDirectedWeightedGraph.setEdgeWeight");
    assert_eq!(
        set_edge_weight.receiver_mutability,
        ReceiverMutabilityFact::MutatesReceiver
    );
}

#[test]
fn fastutil_pack_declares_primitive_collection_boundary() {
    let facts = fastutil_pack_facts();

    let pack = facts
        .external_library_pack_for_type("Int2ObjectOpenHashMap")
        .expect("Int2ObjectOpenHashMap should be owned by the fastutil pack");
    assert_eq!(pack.pack_name, "it.unimi.dsi.fastutil");
    assert_eq!(pack.mode, ExternalLibraryMode::ExternalFactModel);
    assert_eq!(pack.coverage, ExternalApiCoverage::Semantic);
    assert_eq!(pack.provenance, FactProvenance::ExternalLibraryPack);

    for (type_name, args) in [
        ("Int2ObjectArrayMap", Vec::new()),
        (
            "Int2ObjectArrayMap",
            vec![JavaType::Primitive(PrimitiveType::Int)],
        ),
        ("Int2ObjectOpenHashMap", Vec::new()),
        (
            "Int2ObjectOpenHashMap",
            vec![JavaType::Primitive(PrimitiveType::Int)],
        ),
        ("IntArrayList", Vec::new()),
        (
            "IntArrayList",
            vec![JavaType::Primitive(PrimitiveType::Int)],
        ),
        (
            "Int2IntArrayMap",
            vec![
                JavaType::Array(Box::new(JavaType::Primitive(PrimitiveType::Int))),
                JavaType::Array(Box::new(JavaType::Primitive(PrimitiveType::Int))),
            ],
        ),
        ("Object2IntOpenHashMap", Vec::new()),
        (
            "Object2IntOpenHashMap",
            vec![JavaType::Primitive(PrimitiveType::Int)],
        ),
    ] {
        let constructor = facts
            .external_constructor_for(type_name, &args)
            .unwrap_or_else(|| panic!("{type_name}/{} should be pack-owned", args.len()));
        assert_eq!(constructor.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(
            new_java2rust::stdlib::external_constructor_provenance(type_name, &args),
            FactProvenance::ConservativeFallback,
            "{type_name}/{} should not be a generic stdlib fallback",
            args.len()
        );
    }

    let object_put = facts
        .external_api_effect_for_receiver_with_arity(Some("Int2ObjectOpenHashMap"), "put", Some(2))
        .expect("Int2ObjectOpenHashMap.put should be pack-modeled");
    assert_eq!(object_put.provenance, FactProvenance::ExternalLibraryPack);
    assert_eq!(
        object_put.receiver_mutability,
        ReceiverMutabilityFact::MutatesReceiver
    );
    assert_eq!(
        object_put.return_type,
        Some(JavaType::Named {
            name: "Object".to_string(),
            args: Vec::new(),
        })
    );
    assert_eq!(object_put.return_nullability, NullabilityState::MaybeNull);

    let int_get = facts
        .external_api_effect_for_receiver_with_arity(Some("IntArrayList"), "getInt", Some(1))
        .expect("IntArrayList.getInt should be pack-modeled");
    assert_eq!(
        int_get.return_type,
        Some(JavaType::Primitive(PrimitiveType::Int))
    );
    assert_eq!(
        int_get.return_nullability,
        NullabilityState::DefinitelyNonNull
    );

    let default_return_value = facts
        .external_api_effect_for_receiver_with_arity(
            Some("Object2IntOpenHashMap"),
            "defaultReturnValue",
            Some(1),
        )
        .expect("Object2IntOpenHashMap.defaultReturnValue(int) should be pack-modeled");
    assert_eq!(
        default_return_value.receiver_mutability,
        ReceiverMutabilityFact::MutatesReceiver
    );
    assert_eq!(default_return_value.return_type, None);
}

#[test]
fn semantic_analysis_uses_fastutil_pack_for_gatk_shaped_primitive_maps() {
    let semantic = analyze(
        r#"
class FastutilRuntimeUse {
    void run(int expectedSize, int[] keys, int[] values, Object value) {
        Int2ObjectOpenHashMap objectMap = new Int2ObjectOpenHashMap(expectedSize);
        objectMap.put(7, value);
        Object stored = objectMap.get(7);
        boolean hasKey = objectMap.containsKey(7);
        IntArrayList ints = new IntArrayList(expectedSize);
        ints.add(7);
        int first = ints.getInt(0);
        int count = ints.size();
        Int2IntArrayMap intMap = new Int2IntArrayMap(keys, values);
        int previous = intMap.put(1, 2);
        int current = intMap.get(1);
        Object2IntOpenHashMap objectCounts = new Object2IntOpenHashMap(expectedSize);
        objectCounts.defaultReturnValue(-1);
        objectCounts.put(value, 3);
        int countForValue = objectCounts.getInt(value);
    }
}
"#,
    );

    assert!(
        semantic.diagnostics.iter().all(|diagnostic| diagnostic.code
            != "external_pack_missing_constructor"
            && diagnostic.code != "external_pack_missing_api"),
        "modeled fastutil constructors and APIs should not report pack gaps"
    );

    let constructor = semantic
        .facts
        .external_constructor_for(
            "Int2ObjectOpenHashMap",
            &[JavaType::Primitive(PrimitiveType::Int)],
        )
        .expect("semantic analysis should use the fastutil pack for Int2ObjectOpenHashMap/1");
    assert_eq!(constructor.provenance, FactProvenance::ExternalLibraryPack);

    let put = semantic
        .facts
        .external_api_effect_for_receiver_with_arity(Some("Int2ObjectOpenHashMap"), "put", Some(2))
        .expect("semantic analysis should model Int2ObjectOpenHashMap.put");
    assert_eq!(put.provenance, FactProvenance::ExternalLibraryPack);
    assert_eq!(
        put.receiver_mutability,
        ReceiverMutabilityFact::MutatesReceiver
    );

    let get_int = semantic
        .facts
        .external_api_effect_for_receiver_with_arity(
            Some("Object2IntOpenHashMap"),
            "getInt",
            Some(1),
        )
        .expect("semantic analysis should model Object2IntOpenHashMap.getInt");
    assert_eq!(
        get_int.return_type,
        Some(JavaType::Primitive(PrimitiveType::Int))
    );
}

#[test]
fn disq_write_pack_declares_temp_parts_option_boundary() {
    let facts = disq_write_pack_facts();

    let pack = facts
        .external_library_pack_for_type("TempPartsDirectoryWriteOption")
        .expect("TempPartsDirectoryWriteOption should be owned by the DISQ pack");
    assert_eq!(pack.pack_name, "org.disq_bio.disq");
    assert_eq!(pack.mode, ExternalLibraryMode::ExternalFactModel);
    assert_eq!(pack.coverage, ExternalApiCoverage::Semantic);
    assert_eq!(pack.provenance, FactProvenance::ExternalLibraryPack);

    let constructor = facts
        .external_constructor_for("TempPartsDirectoryWriteOption", &[JavaType::String])
        .expect("TempPartsDirectoryWriteOption/1 should be pack-owned");
    assert_eq!(constructor.provenance, FactProvenance::ExternalLibraryPack);

    for type_name in [
        "HtsjdkReadsRdd",
        "HtsjdkReadsTraversalParameters",
        "HtsjdkVariantsRdd",
    ] {
        let args = vec![JavaType::Unknown("Object".to_string()); 2];
        let constructor = facts
            .external_constructor_for(type_name, &args)
            .unwrap_or_else(|| panic!("{type_name}/2 should be pack-owned"));
        assert_eq!(constructor.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(
            new_java2rust::stdlib::external_constructor_provenance(type_name, &args),
            FactProvenance::ConservativeFallback,
            "{type_name}/2 constructor precision should come from the DISQ pack, not stdlib fallback"
        );
    }

    for (type_name, field_name, expected_type) in [
        (
            "ReadsFormatWriteOption",
            "BAM",
            JavaType::Named {
                name: "ReadsFormatWriteOption".to_string(),
                args: Vec::new(),
            },
        ),
        (
            "FileCardinalityWriteOption",
            "MULTIPLE",
            JavaType::Named {
                name: "FileCardinalityWriteOption".to_string(),
                args: Vec::new(),
            },
        ),
        (
            "FileCardinalityWriteOption",
            "SINGLE",
            JavaType::Named {
                name: "FileCardinalityWriteOption".to_string(),
                args: Vec::new(),
            },
        ),
    ] {
        let field = facts
            .external_field_for(type_name, field_name)
            .unwrap_or_else(|| panic!("{type_name}.{field_name} should be pack-owned"));
        assert_eq!(field.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(field.field_type, Some(expected_type));
        let dispatched = new_java2rust::external_packs::external_field_for(type_name, field_name)
            .unwrap_or_else(|| panic!("{type_name}.{field_name} should dispatch to DISQ pack"));
        assert_eq!(dispatched.provenance, FactProvenance::ExternalLibraryPack);
    }
}

#[test]
fn semantic_analysis_uses_disq_write_pack_for_temp_parts_option() {
    let semantic = analyze(
        r#"
class DisqWriteOptionUse {
    Object option(
        String path,
        JavaSparkContext context,
        JavaRDD reads,
        JavaRDD variants,
        SAMFileHeader header,
        IntervalList intervals
    ) {
        ReadsFormatWriteOption format = ReadsFormatWriteOption.BAM;
        FileCardinalityWriteOption multiple = FileCardinalityWriteOption.MULTIPLE;
        FileCardinalityWriteOption single = FileCardinalityWriteOption.SINGLE;
        HtsjdkReadsRdd readRdd = new HtsjdkReadsRdd(context, reads);
        HtsjdkVariantsRdd variantRdd = new HtsjdkVariantsRdd(context, variants);
        HtsjdkReadsTraversalParameters traversal =
            new HtsjdkReadsTraversalParameters(header, intervals);
        return new TempPartsDirectoryWriteOption(path);
    }
}
"#,
    );

    assert!(
        semantic.diagnostics.iter().all(|diagnostic| diagnostic.code
            != "external_pack_missing_constructor"
            && diagnostic.code != "external_pack_missing_api"),
        "modeled DISQ write-option constructors should not report pack gaps"
    );

    let constructor = semantic
        .facts
        .external_constructor_for("TempPartsDirectoryWriteOption", &[JavaType::String])
        .expect("TempPartsDirectoryWriteOption/1 should be explained by the pack");
    assert_eq!(constructor.provenance, FactProvenance::ExternalLibraryPack);
    for type_name in [
        "HtsjdkReadsRdd",
        "HtsjdkVariantsRdd",
        "HtsjdkReadsTraversalParameters",
    ] {
        let constructor = semantic
            .facts
            .external_constructor_for(
                type_name,
                &[
                    JavaType::Unknown("_".to_string()),
                    JavaType::Unknown("_".to_string()),
                ],
            )
            .unwrap_or_else(|| panic!("{type_name}/2 should be explained by the DISQ pack"));
        assert_eq!(constructor.provenance, FactProvenance::ExternalLibraryPack);
    }
}

#[test]
fn intel_gkl_pack_declares_native_boundary() {
    let facts = intel_gkl_pack_facts();

    let pack = facts
        .external_library_pack_for_type("IntelPDHMM")
        .expect("IntelPDHMM should be owned by the Intel GKL pack");
    assert_eq!(pack.pack_name, "com.intel.gkl");
    assert_eq!(pack.mode, ExternalLibraryMode::ExternalFactModel);
    assert_eq!(pack.coverage, ExternalApiCoverage::Semantic);
    assert_eq!(pack.provenance, FactProvenance::ExternalLibraryPack);

    for type_name in [
        "IntelDeflaterFactory",
        "IntelInflaterFactory",
        "IntelPDHMM",
        "IntelPairHmm",
        "IntelPairHmmOMP",
    ] {
        let constructor = facts
            .external_constructor_for(type_name, &[])
            .unwrap_or_else(|| panic!("{type_name}/0 should be pack-owned"));
        assert_eq!(constructor.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(
            new_java2rust::stdlib::external_constructor_provenance(type_name, &[]),
            FactProvenance::ConservativeFallback,
            "{type_name}/0 constructor precision should come from the Intel GKL pack, not stdlib fallback"
        );
    }

    for (type_name, field_name, expected_type) in [
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
        let field = facts
            .external_field_for(type_name, field_name)
            .unwrap_or_else(|| panic!("{type_name}.{field_name} should be pack-owned"));
        assert_eq!(field.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(field.field_type, Some(expected_type.clone()));
        let dispatched = new_java2rust::external_packs::external_field_for(type_name, field_name)
            .unwrap_or_else(|| panic!("{type_name}.{field_name} should be globally dispatched"));
        assert_eq!(dispatched.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(dispatched.field_type, Some(expected_type));
        assert_eq!(
            new_java2rust::stdlib::external_field_provenance(type_name, field_name),
            FactProvenance::ConservativeFallback,
            "{type_name}.{field_name} precision should come from the Intel GKL pack, not stdlib fallback"
        );
    }
}

#[test]
fn semantic_analysis_uses_intel_gkl_pack_for_native_wrappers_and_public_fields() {
    let semantic = analyze(
        r#"
class IntelGklNativeUse {
    Object configure(
        PDHMMNativeArguments pdhmmArgs,
        PairHMMNativeArguments pairArgs,
        ReadDataHolder read,
        HaplotypeDataHolder haplotype,
        BatchCompletionCallbackFunctionArgument callback,
        SWNativeAlignerResult result
    ) {
        IntelDeflaterFactory deflater = new IntelDeflaterFactory();
        IntelInflaterFactory inflater = new IntelInflaterFactory();
        IntelPDHMM pdhmm = new IntelPDHMM();
        IntelPairHmm pairHmm = new IntelPairHmm();
        IntelPairHmmOMP omp = new IntelPairHmmOMP();
        AVXLevel avx = pdhmmArgs.avxLevel;
        OpenMPSetting openMp = pdhmmArgs.openMPSetting;
        int pdhmmThreads = pdhmmArgs.maxNumberOfThreads;
        int pairThreads = pairArgs.maxNumberOfThreads;
        boolean doublePrecision = pairArgs.useDoublePrecision;
        byte[] bases = read.readBases;
        byte[] quals = read.readQuals;
        byte[] insertion = read.insertionGOP;
        byte[] deletion = read.deletionGOP;
        byte[] gc = read.overallGCP;
        byte[] hapBases = haplotype.haplotypeBases;
        byte[] hapPdBases = haplotype.haplotypePDBases;
        int batch = callback.batchCount + callback.totalBatchCount;
        int offset = result.alignment_offset;
        String cigar = result.cigar;
        return cigar;
    }
}
"#,
    );

    assert!(
        semantic.diagnostics.iter().all(|diagnostic| diagnostic.code
            != "external_pack_missing_constructor"
            && diagnostic.code != "external_pack_missing_field"
            && diagnostic.code != "external_pack_missing_api"),
        "modeled Intel GKL native wrappers should not report pack gaps: {:?}",
        semantic.diagnostics
    );

    for type_name in [
        "IntelDeflaterFactory",
        "IntelInflaterFactory",
        "IntelPDHMM",
        "IntelPairHmm",
        "IntelPairHmmOMP",
    ] {
        let constructor = semantic
            .facts
            .external_constructor_for(type_name, &[])
            .unwrap_or_else(|| panic!("{type_name}/0 should be explained by the Intel GKL pack"));
        assert_eq!(constructor.provenance, FactProvenance::ExternalLibraryPack);
    }

    for (type_name, field_name, expected_type) in [
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
        let field = semantic
            .facts
            .external_field_for(type_name, field_name)
            .unwrap_or_else(|| panic!("{type_name}.{field_name} should be explained by the pack"));
        assert_eq!(field.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(field.field_type, Some(expected_type));
    }
}

#[test]
fn guava_immutable_collect_pack_declares_immutable_map_builder_boundary() {
    let facts = guava_immutable_collect_pack_facts();

    let pack = facts
        .external_library_pack_for_type("ImmutableMap.Builder")
        .expect("ImmutableMap.Builder should be owned by the Guava pack");
    assert_eq!(pack.pack_name, "com.google.common.collect");
    assert_eq!(pack.mode, ExternalLibraryMode::ExternalFactModel);
    assert_eq!(pack.coverage, ExternalApiCoverage::Semantic);
    assert_eq!(pack.provenance, FactProvenance::ExternalLibraryPack);

    let abstract_iterator = facts
        .external_library_pack_for_type("com.google.common.collect.AbstractIterator")
        .expect("Guava AbstractIterator should be owned by the collect pack");
    assert_eq!(abstract_iterator.pack_name, "com.google.common.collect");
    assert!(
        facts.has_object_representation_facts(&JavaType::Named {
            name: "com.google.common.collect.AbstractIterator".to_string(),
            args: Vec::new(),
        }),
        "anonymous external iterator implementations should have explicit Guava object-representation facts"
    );
    assert!(facts.has_object_representation_facts(&JavaType::Named {
        name: "AbstractIterator".to_string(),
        args: Vec::new(),
    }));

    let constructor = facts
        .external_constructor_for("Builder", &[])
        .expect("Builder/0 should be pack-owned as an ImmutableMap.Builder compatibility alias");
    assert_eq!(constructor.provenance, FactProvenance::ExternalLibraryPack);
    assert!(
        facts.external_library_pack_for_type("Builder").is_none(),
        "constructor compatibility for simple Builder must not make every Builder receiver Guava-owned"
    );

    let put = facts
        .external_api_effect_for_receiver(Some("ImmutableMap.Builder"), "put")
        .expect("ImmutableMap.Builder.put should be pack-owned");
    assert_eq!(put.role, ExternalApiRole::FluentMutator);
    assert_eq!(
        put.receiver_mutability,
        ReceiverMutabilityFact::MutatesReceiver
    );
    assert_eq!(
        put.return_type,
        Some(JavaType::Named {
            name: "Builder".to_string(),
            args: Vec::new(),
        })
    );

    let build = facts
        .external_api_effect_for_receiver(Some("ImmutableMap.Builder"), "build")
        .expect("ImmutableMap.Builder.build should be pack-owned");
    assert_eq!(build.role, ExternalApiRole::Getter);
    assert_eq!(build.receiver_mutability, ReceiverMutabilityFact::ReadOnly);
    assert_eq!(
        build.return_type,
        Some(JavaType::Named {
            name: "ImmutableMap".to_string(),
            args: Vec::new(),
        })
    );
    assert_eq!(
        build.return_nullability,
        NullabilityState::DefinitelyNonNull
    );
}

#[test]
fn semantic_analysis_uses_guava_pack_for_immutable_map_builder_workflow() {
    let semantic = analyze(
        r#"
class GuavaImmutableMapBuilderUse {
    Object build(String key, Object value) {
        ImmutableMap.Builder<String, Object> builder = new ImmutableMap.Builder<>();
        builder.put(key, value);
        return builder.build();
    }
}
"#,
    );

    assert!(
        semantic.diagnostics.iter().all(|diagnostic| diagnostic.code
            != "external_pack_missing_constructor"
            && diagnostic.code != "external_pack_missing_api"),
        "modeled Guava ImmutableMap.Builder constructors and methods should not report pack gaps"
    );

    let constructor = semantic
        .facts
        .external_constructor_for("Builder", &[])
        .expect("Builder/0 should be explained by the Guava pack");
    assert_eq!(constructor.provenance, FactProvenance::ExternalLibraryPack);

    if let Some(build) = semantic
        .facts
        .external_api_effect_for_receiver(Some("Builder"), "build")
    {
        assert_ne!(
            build.provenance,
            FactProvenance::ExternalLibraryPack,
            "parser-collapsed Builder receiver should not globally claim Guava pack method ownership"
        );
    }
}

#[test]
fn guava_concurrent_pack_declares_thread_factory_builder_boundary() {
    let facts = guava_concurrent_pack_facts();

    let pack = facts
        .external_library_pack_for_type("ThreadFactoryBuilder")
        .expect("ThreadFactoryBuilder should be owned by the Guava concurrent pack");
    assert_eq!(pack.pack_name, "com.google.common.util.concurrent");
    assert_eq!(pack.mode, ExternalLibraryMode::ExternalFactModel);
    assert_eq!(pack.coverage, ExternalApiCoverage::Semantic);
    assert_eq!(pack.provenance, FactProvenance::ExternalLibraryPack);

    let constructor = facts
        .external_constructor_for("ThreadFactoryBuilder", &[])
        .expect("ThreadFactoryBuilder/0 should be pack-owned");
    assert_eq!(constructor.provenance, FactProvenance::ExternalLibraryPack);
    assert_eq!(
        new_java2rust::stdlib::external_constructor_provenance("ThreadFactoryBuilder", &[]),
        FactProvenance::ConservativeFallback,
        "ThreadFactoryBuilder constructor precision should come from the Guava concurrent pack, not stdlib fallback"
    );

    for method_name in ["setNameFormat", "setDaemon"] {
        let effect = facts
            .external_api_effect_for_receiver(Some("ThreadFactoryBuilder"), method_name)
            .unwrap_or_else(|| panic!("ThreadFactoryBuilder.{method_name} should be pack-owned"));
        assert_eq!(effect.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(effect.role, ExternalApiRole::FluentMutator);
        assert_eq!(
            effect.return_type,
            Some(JavaType::Named {
                name: "ThreadFactoryBuilder".to_string(),
                args: Vec::new(),
            })
        );
    }

    let build = facts
        .external_api_effect_for_receiver(Some("ThreadFactoryBuilder"), "build")
        .expect("ThreadFactoryBuilder.build should be pack-owned");
    assert_eq!(build.provenance, FactProvenance::ExternalLibraryPack);
    assert_eq!(build.role, ExternalApiRole::Getter);
    assert_eq!(
        build.return_type,
        Some(JavaType::Named {
            name: "ThreadFactory".to_string(),
            args: Vec::new(),
        })
    );
}

#[test]
fn broad_http_pack_declares_retry_settings_boundary() {
    let facts = broad_http_pack_facts();

    let pack = facts
        .external_library_pack_for_type("RetrySettings")
        .expect("RetrySettings should be owned by the Broad HTTP pack");
    assert_eq!(pack.pack_name, "org.broadinstitute.http.nio");
    assert_eq!(pack.mode, ExternalLibraryMode::ExternalFactModel);
    assert_eq!(pack.coverage, ExternalApiCoverage::Semantic);
    assert_eq!(pack.provenance, FactProvenance::ExternalLibraryPack);

    let retry_constructor = facts
        .external_constructor_for(
            "RetrySettings",
            &[
                JavaType::Primitive(new_java2rust::java_ir::PrimitiveType::Int),
                JavaType::Named {
                    name: "Object".to_string(),
                    args: Vec::new(),
                },
                JavaType::Named {
                    name: "Object".to_string(),
                    args: Vec::new(),
                },
                JavaType::Named {
                    name: "Object".to_string(),
                    args: Vec::new(),
                },
                JavaType::Named {
                    name: "?lambda".to_string(),
                    args: Vec::new(),
                },
            ],
        )
        .expect("RetrySettings/5 should be pack-owned");
    assert_eq!(
        retry_constructor.provenance,
        FactProvenance::ExternalLibraryPack
    );

    let settings_constructor = facts
        .external_constructor_for(
            "HttpFileSystemProviderSettings",
            &[
                JavaType::Named {
                    name: "Object".to_string(),
                    args: Vec::new(),
                },
                JavaType::Named {
                    name: "Object".to_string(),
                    args: Vec::new(),
                },
                JavaType::Named {
                    name: "Object".to_string(),
                    args: Vec::new(),
                },
            ],
        )
        .expect("HttpFileSystemProviderSettings/3 should be pack-owned");
    assert_eq!(
        settings_constructor.provenance,
        FactProvenance::ExternalLibraryPack
    );
    assert_eq!(
        new_java2rust::stdlib::external_constructor_provenance(
            "HttpFileSystemProviderSettings",
            &[
                JavaType::Named {
                    name: "Object".to_string(),
                    args: Vec::new(),
                },
                JavaType::Named {
                    name: "Object".to_string(),
                    args: Vec::new(),
                },
                JavaType::Named {
                    name: "Object".to_string(),
                    args: Vec::new(),
                },
            ],
        ),
        FactProvenance::ConservativeFallback,
        "Broad HTTP constructor precision should come from the pack, not stdlib fallback"
    );

    let redirect = facts
        .external_api_effect_for_receiver(Some("HttpFileSystemProviderSettings"), "redirect")
        .expect("HttpFileSystemProviderSettings.redirect should be pack-owned");
    assert_eq!(redirect.role, ExternalApiRole::Getter);
    assert_eq!(
        redirect.receiver_mutability,
        ReceiverMutabilityFact::ReadOnly
    );

    let default_settings = facts
        .external_field_for("HttpFileSystemProviderSettings", "DEFAULT_SETTINGS")
        .expect("DEFAULT_SETTINGS should be pack-owned");
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
    assert_eq!(
        new_java2rust::stdlib::external_field_provenance(
            "HttpFileSystemProviderSettings",
            "DEFAULT_SETTINGS"
        ),
        FactProvenance::ConservativeFallback,
        "HttpFileSystemProviderSettings.DEFAULT_SETTINGS precision should come from the Broad HTTP pack, not stdlib"
    );

    for type_name in ["HttpFileSystemProvider", "HttpsFileSystemProvider"] {
        let field = facts
            .external_field_for(type_name, "SCHEME")
            .unwrap_or_else(|| panic!("{type_name}.SCHEME should be pack-owned"));
        assert_eq!(field.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(field.field_type, Some(JavaType::String));
        let dispatched = new_java2rust::external_packs::external_field_for(type_name, "SCHEME")
            .unwrap_or_else(|| panic!("{type_name}.SCHEME should be globally pack-dispatched"));
        assert_eq!(dispatched.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(
            new_java2rust::stdlib::external_field_provenance(type_name, "SCHEME"),
            FactProvenance::ConservativeFallback,
            "{type_name}.SCHEME precision should come from the Broad HTTP pack, not stdlib"
        );
    }

    for field_name in [
        "DEFAULT_RETRYABLE_HTTP_CODES",
        "DEFAULT_RETRYABLE_EXCEPTIONS",
        "DEFALT_RETRYABLE_MESSAGES",
    ] {
        let field = facts
            .external_field_for("RetryHandler", field_name)
            .unwrap_or_else(|| panic!("RetryHandler.{field_name} should be pack-owned"));
        assert_eq!(field.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(
            field.field_type,
            Some(JavaType::Named {
                name: "Object".to_string(),
                args: Vec::new(),
            })
        );
        let dispatched =
            new_java2rust::external_packs::external_field_for("RetryHandler", field_name)
                .unwrap_or_else(|| {
                    panic!("RetryHandler.{field_name} should be globally pack-dispatched")
                });
        assert_eq!(dispatched.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(
            new_java2rust::stdlib::external_field_provenance("RetryHandler", field_name),
            FactProvenance::ConservativeFallback,
            "RetryHandler.{field_name} precision should come from the Broad HTTP pack, not stdlib"
        );
    }
}

#[test]
fn semantic_analysis_uses_broad_http_pack_for_retry_settings_workflow() {
    let semantic = analyze(
        r#"
class BroadHttpRetrySettingsUse {
    Object configure(int maxRetries, Object codes, Object exceptions, Object messages) {
        RetrySettings retrySettings =
            new RetrySettings(maxRetries, codes, exceptions, messages, e -> false);
        return new HttpFileSystemProviderSettings(
            retrySettings,
            HttpFileSystemProviderSettings.DEFAULT_SETTINGS.redirect(),
            retrySettings);
    }
}
"#,
    );

    assert!(
        semantic.diagnostics.iter().all(|diagnostic| diagnostic.code
            != "external_pack_missing_constructor"
            && diagnostic.code != "external_pack_missing_api"
            && diagnostic.code != "external_pack_missing_field"),
        "modeled Broad HTTP settings constructors, field, and redirect should not report pack gaps"
    );

    let retry_constructor = semantic
        .facts
        .external_constructor_facts
        .iter()
        .find(|fact| fact.type_name == "RetrySettings" && fact.arg_types.len() == 5)
        .expect("RetrySettings/5 should be explained by the Broad HTTP pack");
    assert_eq!(
        retry_constructor.provenance,
        FactProvenance::ExternalLibraryPack
    );
}

#[test]
fn picard_vcf_pack_declares_truth_and_call_states_boundary() {
    let facts = picard_vcf_pack_facts();

    let pack = facts
        .external_library_pack_for_type("TruthAndCallStates")
        .expect("TruthAndCallStates should be owned by the Picard VCF pack");
    assert_eq!(pack.pack_name, "picard.vcf");
    assert_eq!(pack.mode, ExternalLibraryMode::ExternalFactModel);
    assert_eq!(pack.coverage, ExternalApiCoverage::Semantic);
    assert_eq!(pack.provenance, FactProvenance::ExternalLibraryPack);

    let constructor = facts
        .external_constructor_for(
            "TruthAndCallStates",
            &[
                JavaType::Named {
                    name: "TruthState".to_string(),
                    args: Vec::new(),
                },
                JavaType::Named {
                    name: "CallState".to_string(),
                    args: Vec::new(),
                },
            ],
        )
        .expect("TruthAndCallStates/2 should be pack-owned");
    assert_eq!(constructor.provenance, FactProvenance::ExternalLibraryPack);

    for (type_name, arg_len) in [
        ("GenotypeConcordanceCounts", 0),
        ("GenotypeConcordanceSummaryMetrics", 5),
    ] {
        let args = vec![JavaType::Unknown("Object".to_string()); arg_len];
        let constructor = facts
            .external_constructor_for(type_name, &args)
            .unwrap_or_else(|| panic!("{type_name}/{arg_len} should be pack-owned"));
        assert_eq!(constructor.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(
            new_java2rust::stdlib::external_constructor_provenance(type_name, &args),
            FactProvenance::ConservativeFallback,
            "{type_name}/{arg_len} precision should come from the Picard VCF pack, not stdlib"
        );
    }

    for (field_name, expected_type) in [
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
        let field = facts
            .external_field_for("GenotypeConcordanceStates", field_name)
            .unwrap_or_else(|| {
                panic!("GenotypeConcordanceStates.{field_name} should be pack-owned")
            });
        assert_eq!(field.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(field.field_type, Some(expected_type));
        assert_eq!(
            new_java2rust::stdlib::external_field_provenance(
                "GenotypeConcordanceStates",
                field_name
            ),
            FactProvenance::ConservativeFallback,
            "GenotypeConcordanceStates.{field_name} precision should come from the Picard VCF pack, not stdlib"
        );
    }

    for field_name in ["truthState", "callState"] {
        let field = facts
            .external_field_for("TruthAndCallStates", field_name)
            .unwrap_or_else(|| panic!("TruthAndCallStates.{field_name} should be pack-owned"));
        assert_eq!(field.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(
            field.field_type,
            Some(JavaType::Named {
                name: if field_name == "truthState" {
                    "TruthState"
                } else {
                    "CallState"
                }
                .to_string(),
                args: Vec::new(),
            })
        );
        assert_eq!(
            new_java2rust::stdlib::external_field_provenance("TruthAndCallStates", field_name),
            FactProvenance::ConservativeFallback,
            "TruthAndCallStates.{field_name} precision should come from the Picard VCF pack, not stdlib"
        );
    }

    for (type_name, field_name, expected_type) in [
        ("CallState", "HOM_REF", "CallState"),
        ("CallState", "HET_REF_VAR1", "CallState"),
        ("TruthState", "HOM_REF", "TruthState"),
        ("TruthState", "NO_CALL", "TruthState"),
    ] {
        let field = facts
            .external_field_for(type_name, field_name)
            .unwrap_or_else(|| panic!("{type_name}.{field_name} should be pack-owned"));
        assert_eq!(field.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(
            field.field_type,
            Some(JavaType::Named {
                name: expected_type.to_string(),
                args: Vec::new(),
            })
        );
        let dispatched = new_java2rust::external_packs::external_field_for(type_name, field_name)
            .unwrap_or_else(|| {
                panic!("{type_name}.{field_name} should be globally pack-dispatched")
            });
        assert_eq!(dispatched.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(
            new_java2rust::stdlib::external_field_provenance(type_name, field_name),
            FactProvenance::ConservativeFallback,
            "{type_name}.{field_name} precision should come from the Picard VCF pack, not stdlib"
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
        let field = facts
            .external_field_for(type_name, field_name)
            .unwrap_or_else(|| panic!("{type_name}.{field_name} should be pack-owned"));
        assert_eq!(field.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(field.field_type, Some(JavaType::String));
        let dispatched = new_java2rust::external_packs::external_field_for(type_name, field_name)
            .unwrap_or_else(|| {
                panic!("{type_name}.{field_name} should be globally pack-dispatched")
            });
        assert_eq!(dispatched.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(
            new_java2rust::stdlib::external_field_provenance(type_name, field_name),
            FactProvenance::ConservativeFallback,
            "{type_name}.{field_name} precision should come from the Picard VCF pack, not stdlib"
        );
    }
}

#[test]
fn semantic_analysis_uses_picard_vcf_pack_for_truth_and_call_states() {
    let semantic = analyze(
        r#"
class PicardVcfTruthAndCallStatesUse {
    Object classify(TruthState truthState, CallState callState) {
        TruthAndCallStates states = new TruthAndCallStates(truthState, callState);
        return states.truthState == null ? states.callState : states.truthState;
    }
}
"#,
    );

    assert!(
        semantic.diagnostics.iter().all(|diagnostic| diagnostic.code
            != "external_pack_missing_constructor"
            && diagnostic.code != "external_pack_missing_field"),
        "modeled Picard VCF constructor and fields should not report pack gaps"
    );

    let constructor = semantic
        .facts
        .external_constructor_for(
            "TruthAndCallStates",
            &[
                JavaType::Named {
                    name: "TruthState".to_string(),
                    args: Vec::new(),
                },
                JavaType::Named {
                    name: "CallState".to_string(),
                    args: Vec::new(),
                },
            ],
        )
        .expect("TruthAndCallStates/2 should be explained by the Picard VCF pack");
    assert_eq!(constructor.provenance, FactProvenance::ExternalLibraryPack);
}

#[test]
fn htsjdk_sam_header_pack_declares_selected_header_boundary() {
    let facts = htsjdk_sam_header_pack_facts();

    let pack = facts
        .external_library_pack_for_type("htsjdk.samtools.SAMSequenceDictionary")
        .expect("SAM header package should be owned by the external pack");
    assert_eq!(pack.pack_name, "htsjdk.samtools.header");
    assert_eq!(pack.mode, ExternalLibraryMode::ExternalFactModel);
    assert_eq!(pack.coverage, ExternalApiCoverage::Semantic);
    assert_eq!(pack.provenance, FactProvenance::ExternalLibraryPack);

    let dictionary = facts
        .external_constructor_for("SAMSequenceDictionary", &[])
        .expect("SAMSequenceDictionary constructor should be pack-owned");
    assert_eq!(dictionary.provenance, FactProvenance::ExternalLibraryPack);

    let record = facts
        .external_constructor_for(
            "SAMSequenceRecord",
            &[
                JavaType::String,
                JavaType::Primitive(new_java2rust::java_ir::PrimitiveType::Int),
            ],
        )
        .expect("SAMSequenceRecord constructor should be pack-owned by arity");
    assert_eq!(record.provenance, FactProvenance::ExternalLibraryPack);

    for (type_name, args) in [
        ("SAMSequenceDictionary", Vec::new()),
        (
            "SAMSequenceDictionary",
            vec![JavaType::Named {
                name: "List".to_string(),
                args: vec![JavaType::Named {
                    name: "SAMSequenceRecord".to_string(),
                    args: Vec::new(),
                }],
            }],
        ),
        (
            "SAMSequenceRecord",
            vec![JavaType::String, JavaType::Primitive(PrimitiveType::Int)],
        ),
        ("SAMFileHeader", Vec::new()),
        (
            "SAMFileHeader",
            vec![JavaType::Named {
                name: "SAMSequenceDictionary".to_string(),
                args: Vec::new(),
            }],
        ),
        ("SAMReadGroupRecord", vec![JavaType::String]),
        ("SAMProgramRecord", vec![JavaType::String]),
        ("SAMTextHeaderCodec", Vec::new()),
        ("SAMFormatException", vec![JavaType::String]),
    ] {
        let constructor = facts
            .external_constructor_for(type_name, &args)
            .unwrap_or_else(|| panic!("{type_name}/{} should be pack-owned", args.len()));
        assert_eq!(constructor.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(
            new_java2rust::stdlib::external_constructor_provenance(type_name, &args),
            FactProvenance::ConservativeFallback,
            "{type_name}/{:?} should be owned by the SAM header external pack, not stdlib",
            args.len()
        );
    }
}

#[test]
fn htsjdk_sam_header_pack_models_dictionary_header_and_read_group_effects() {
    let facts = htsjdk_sam_header_pack_facts();

    let sequence = facts
        .external_api_effect_for_receiver(Some("SAMSequenceDictionary"), "getSequence")
        .expect("dictionary indexed getter should be modeled by the pack");
    assert_eq!(sequence.role, ExternalApiRole::IndexedGetter);
    assert_eq!(
        sequence.return_nullability,
        NullabilityState::DefinitelyNonNull
    );
    assert_eq!(sequence.provenance, FactProvenance::ExternalLibraryPack);

    let same_dictionary = facts
        .external_api_effect_for_receiver(Some("SAMSequenceDictionary"), "isSameDictionary")
        .expect("dictionary equivalence predicate should be modeled by the pack");
    assert_eq!(same_dictionary.role, ExternalApiRole::BooleanGetter);
    assert_eq!(
        same_dictionary.return_type,
        Some(JavaType::Primitive(
            new_java2rust::java_ir::PrimitiveType::Boolean
        ))
    );

    let assert_same_dictionary = facts
        .external_api_effect_for_receiver(Some("SAMSequenceDictionary"), "assertSameDictionary")
        .expect("dictionary assertion check should be modeled by the pack");
    assert_eq!(assert_same_dictionary.role, ExternalApiRole::SideEffect);
    assert_eq!(
        assert_same_dictionary.thrown,
        vec![JavaType::Named {
            name: "AssertionError".to_string(),
            args: Vec::new(),
        }]
    );

    let set_dictionary = facts
        .external_api_effect_for_receiver(Some("SAMFileHeader"), "setSequenceDictionary")
        .expect("header dictionary setter should be modeled by the pack");
    assert_eq!(set_dictionary.role, ExternalApiRole::Mutator);
    assert_eq!(
        set_dictionary.receiver_mutability,
        ReceiverMutabilityFact::MutatesReceiver
    );

    let encode = facts
        .external_api_effect_for_receiver_with_arity(Some("SAMTextHeaderCodec"), "encode", Some(3))
        .expect("SAMTextHeaderCodec.encode(writer,header,keepVersion) should be modeled");
    assert_eq!(encode.role, ExternalApiRole::SideEffect);
    assert_eq!(encode.receiver_mutability, ReceiverMutabilityFact::ReadOnly);
    assert_eq!(encode.provenance, FactProvenance::ExternalLibraryPack);

    let sample = facts
        .external_api_effect_for_receiver(Some("SAMReadGroupRecord"), "getSample")
        .expect("read-group sample getter should be modeled by the pack");
    assert_eq!(sample.role, ExternalApiRole::Getter);
    assert_eq!(sample.return_nullability, NullabilityState::MaybeNull);

    let md5 = facts
        .external_api_effect_for_receiver(Some("SAMSequenceRecord"), "getMd5")
        .expect("sequence record MD5 getter should be modeled by the pack");
    assert_eq!(md5.role, ExternalApiRole::Getter);
    assert_eq!(md5.return_nullability, NullabilityState::MaybeNull);
    assert_eq!(md5.return_type, Some(JavaType::String));

    let header_index = facts
        .external_api_effect_for_receiver(Some("SAMFileHeader"), "getSequenceIndex")
        .expect("header sequence index getter should be modeled by the pack");
    assert_eq!(
        header_index.return_type,
        Some(JavaType::Primitive(
            new_java2rust::java_ir::PrimitiveType::Int
        ))
    );

    let platform_unit = facts
        .external_api_effect_for_receiver(Some("SAMReadGroupRecord"), "getPlatformUnit")
        .expect("read-group platform unit getter should be modeled by the pack");
    assert_eq!(platform_unit.return_type, Some(JavaType::String));
    assert_eq!(
        platform_unit.return_nullability,
        NullabilityState::MaybeNull
    );

    let sequencing_center = facts
        .external_api_effect_for_receiver(Some("SAMReadGroupRecord"), "getSequencingCenter")
        .expect("read-group sequencing center getter should be modeled by the pack");
    assert_eq!(sequencing_center.return_type, Some(JavaType::String));
    assert_eq!(
        sequencing_center.return_nullability,
        NullabilityState::MaybeNull
    );

    let read_group_attribute = facts
        .external_api_effect_for_receiver(Some("SAMReadGroupRecord"), "getAttribute")
        .expect("read-group attribute getter should be modeled by the pack");
    assert_eq!(
        read_group_attribute.return_type,
        Some(JavaType::Named {
            name: "Object".to_string(),
            args: Vec::new(),
        })
    );
    assert_eq!(
        read_group_attribute.return_nullability,
        NullabilityState::MaybeNull
    );

    let read_group_id = facts
        .external_api_effect_for_receiver(Some("SAMReadGroupRecord"), "getReadGroupId")
        .expect("read-group ID getter should be modeled by the pack");
    assert_eq!(read_group_id.return_type, Some(JavaType::String));
    assert_eq!(
        read_group_id.return_nullability,
        NullabilityState::DefinitelyNonNull
    );

    let header_sequence = facts
        .external_api_effect_for_receiver(Some("SAMFileHeader"), "getSequence")
        .expect("header sequence lookup should be modeled by the pack");
    assert_eq!(
        header_sequence.return_type,
        Some(JavaType::Named {
            name: "SAMSequenceRecord".to_string(),
            args: Vec::new(),
        })
    );
    assert_eq!(
        header_sequence.return_nullability,
        NullabilityState::MaybeNull
    );

    let read_group = facts
        .external_api_effect_for_receiver(Some("SAMFileHeader"), "getReadGroup")
        .expect("header read-group lookup should be modeled by the pack");
    assert_eq!(
        read_group.return_type,
        Some(JavaType::Named {
            name: "SAMReadGroupRecord".to_string(),
            args: Vec::new(),
        })
    );
    assert_eq!(read_group.return_nullability, NullabilityState::MaybeNull);

    let clone = facts
        .external_api_effect_for_receiver(Some("SAMFileHeader"), "clone")
        .expect("header clone should be modeled by the pack");
    assert_eq!(
        clone.return_type,
        Some(JavaType::Named {
            name: "SAMFileHeader".to_string(),
            args: Vec::new(),
        })
    );
    assert_eq!(
        clone.return_nullability,
        NullabilityState::DefinitelyNonNull
    );

    let add_sequence = facts
        .external_api_effect_for_receiver(Some("SAMFileHeader"), "addSequence")
        .expect("header add-sequence mutator should be modeled by the pack");
    assert_eq!(add_sequence.role, ExternalApiRole::Mutator);

    let dictionary_add_sequence = facts
        .external_api_effect_for_receiver(Some("SAMSequenceDictionary"), "addSequence")
        .expect("dictionary add-sequence mutator should be modeled by the pack");
    assert_eq!(dictionary_add_sequence.role, ExternalApiRole::Mutator);

    let program_record = facts
        .external_api_effect_for_receiver(Some("SAMFileHeader"), "getProgramRecord")
        .expect("header program-record lookup should be modeled by the pack");
    assert_eq!(
        program_record.return_type,
        Some(JavaType::Named {
            name: "SAMProgramRecord".to_string(),
            args: Vec::new(),
        })
    );
    assert_eq!(
        program_record.return_nullability,
        NullabilityState::MaybeNull
    );

    let sam_string = facts
        .external_api_effect_for_receiver(Some("SAMFileHeader"), "getSAMString")
        .expect("header SAM string getter should be modeled by the pack");
    assert_eq!(sam_string.return_type, Some(JavaType::String));
    assert_eq!(
        sam_string.return_nullability,
        NullabilityState::DefinitelyNonNull
    );

    let program_id = facts
        .external_api_effect_for_receiver(Some("SAMProgramRecord"), "getId")
        .expect("program-record ID getter should be modeled by the pack");
    assert_eq!(program_id.return_type, Some(JavaType::String));
    assert_eq!(
        program_id.return_nullability,
        NullabilityState::DefinitelyNonNull
    );

    let program_name = facts
        .external_api_effect_for_receiver(Some("SAMProgramRecord"), "getProgramName")
        .expect("program-record name getter should be modeled by the pack");
    assert_eq!(program_name.return_type, Some(JavaType::String));
    assert_eq!(program_name.return_nullability, NullabilityState::MaybeNull);

    let program_group_id = facts
        .external_api_effect_for_receiver(Some("SAMProgramRecord"), "getProgramGroupId")
        .expect("program-record group ID getter should be modeled by the pack");
    assert_eq!(program_group_id.return_type, Some(JavaType::String));
    assert_eq!(
        program_group_id.return_nullability,
        NullabilityState::MaybeNull
    );

    for method_name in [
        "setCommandLine",
        "setPreviousProgramGroupId",
        "setProgramName",
        "setProgramVersion",
    ] {
        let effect = facts
            .external_api_effect_for_receiver(Some("SAMProgramRecord"), method_name)
            .unwrap_or_else(|| {
                panic!("SAMProgramRecord.{method_name} should be modeled by the pack")
            });
        assert_eq!(effect.role, ExternalApiRole::Mutator);
        assert_eq!(
            effect.receiver_mutability,
            ReceiverMutabilityFact::MutatesReceiver
        );
    }

    let assembly = facts
        .external_api_effect_for_receiver(Some("SAMSequenceRecord"), "getAssembly")
        .expect("sequence assembly getter should be modeled by the pack");
    assert_eq!(assembly.return_type, Some(JavaType::String));
    assert_eq!(assembly.return_nullability, NullabilityState::MaybeNull);

    let truncated_name = facts
        .external_api_effect_for_receiver(Some("SAMSequenceRecord"), "truncateSequenceName")
        .expect("sequence-name truncation helper should be modeled by the pack");
    assert_eq!(truncated_name.return_type, Some(JavaType::String));
    assert_eq!(
        truncated_name.return_nullability,
        NullabilityState::DefinitelyNonNull
    );

    let unknown_length = facts
        .external_field_for("SAMSequenceRecord", "UNKNOWN_SEQUENCE_LENGTH")
        .expect("sequence length constant should be pack-owned");
    assert_eq!(
        unknown_length.provenance,
        FactProvenance::ExternalLibraryPack
    );
    assert_eq!(
        unknown_length.field_type,
        Some(JavaType::Primitive(PrimitiveType::Int))
    );
    assert_eq!(
        new_java2rust::stdlib::external_field_provenance(
            "SAMSequenceRecord",
            "UNKNOWN_SEQUENCE_LENGTH"
        ),
        FactProvenance::ConservativeFallback,
        "SAMSequenceRecord.UNKNOWN_SEQUENCE_LENGTH precision should come from the SAM header pack, not stdlib"
    );

    for field_name in ["READ_GROUP_ID_TAG", "READ_GROUP_SAMPLE_TAG"] {
        let field = facts
            .external_field_for("SAMReadGroupRecord", field_name)
            .unwrap_or_else(|| panic!("SAMReadGroupRecord.{field_name} should be pack-owned"));
        assert_eq!(field.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(field.field_type, Some(JavaType::String));
        let dispatched =
            new_java2rust::external_packs::external_field_for("SAMReadGroupRecord", field_name)
                .unwrap_or_else(|| {
                    panic!("SAMReadGroupRecord.{field_name} should be globally pack-dispatched")
                });
        assert_eq!(dispatched.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(
            new_java2rust::stdlib::external_field_provenance("SAMReadGroupRecord", field_name),
            FactProvenance::ConservativeFallback,
            "SAMReadGroupRecord.{field_name} precision should come from the SAM header pack, not stdlib"
        );
    }

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
        let field = facts
            .external_field_for("SAMFileHeader", field_name)
            .unwrap_or_else(|| panic!("SAMFileHeader.{field_name} should be pack-owned"));
        assert_eq!(field.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(field.field_type, Some(expected_ty));
        assert_eq!(
            new_java2rust::stdlib::external_field_provenance("SAMFileHeader", field_name),
            FactProvenance::ConservativeFallback,
            "SAMFileHeader.{field_name} precision should come from the SAM header pack, not stdlib"
        );
    }

    for (type_name, field_name, expected_type_name) in [
        ("SortOrder", "coordinate", "SortOrder"),
        ("SortOrder", "unsorted", "SortOrder"),
        ("GroupOrder", "query", "GroupOrder"),
    ] {
        let field = facts
            .external_field_for(type_name, field_name)
            .unwrap_or_else(|| panic!("{type_name}.{field_name} should be pack-owned"));
        assert_eq!(field.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(
            field.field_type,
            Some(JavaType::Named {
                name: expected_type_name.to_string(),
                args: Vec::new(),
            })
        );

        let dispatched = new_java2rust::external_packs::external_field_for(type_name, field_name)
            .unwrap_or_else(|| {
                panic!("{type_name}.{field_name} should be available through global pack dispatch")
            });
        assert_eq!(dispatched.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(
            new_java2rust::stdlib::external_field_provenance(type_name, field_name),
            FactProvenance::ConservativeFallback,
            "{type_name}.{field_name} precision should come from the SAM header pack, not stdlib"
        );
    }
}

#[test]
fn semantic_analysis_uses_sam_header_pack_for_dictionary_workflows() {
    let semantic = analyze(
        r#"
import htsjdk.samtools.SAMFileHeader;
import htsjdk.samtools.SAMProgramRecord;
import htsjdk.samtools.SAMReadGroupRecord;
import htsjdk.samtools.SAMSequenceDictionary;
import htsjdk.samtools.SAMSequenceRecord;
import java.util.List;

class SamHeaderPackUse {
    SAMFileHeader header(List<SAMSequenceRecord> records) {
        SAMSequenceDictionary dictionary = new SAMSequenceDictionary(records);
        dictionary.addSequence(new SAMSequenceRecord("chr1", 248956422));
        SAMFileHeader header = new SAMFileHeader();
        header.setSequenceDictionary(dictionary);
        header.addSequence(new SAMSequenceRecord("chrM", 16569));
        header.addReadGroup(new SAMReadGroupRecord("sample"));
        return header;
    }

    Object dictionary(SAMFileHeader header) {
        return header.getSequenceDictionary();
    }

    Object headerSequence(SAMFileHeader header) {
        return header.getSequence("chr1");
    }

    Object contig(SAMSequenceDictionary dictionary) {
        return dictionary.getSequence(0);
    }

    int index(SAMFileHeader header, SAMSequenceDictionary dictionary) {
        return header.getSequenceIndex("chr1") + dictionary.getReferenceLength();
    }

    Object md5(SAMSequenceRecord record) {
        return record.getMd5();
    }

    Object assembly(SAMSequenceRecord record) {
        return record.getAssembly();
    }

    SAMSequenceRecord resize(SAMSequenceRecord record) {
        return record.setSequenceLength(100);
    }

    String truncatedName(SAMSequenceRecord record) {
        return record.truncateSequenceName("chr1 extra");
    }

    Object readGroup(SAMFileHeader header) {
        return header.getReadGroup("sample");
    }

    Object programRecord(SAMFileHeader header) {
        return header.getProgramRecord("program");
    }

    Object programRecordMeta(SAMProgramRecord program) {
        return program.getProgramGroupId() + program.getProgramName();
    }

    void updateProgramRecord(SAMProgramRecord program) {
        program.setCommandLine("cmd");
        program.setPreviousProgramGroupId("previous");
        program.setProgramName("name");
        program.setProgramVersion("1");
    }

    String samString(SAMFileHeader header) {
        return header.getSAMString();
    }

    Object comments(SAMFileHeader header) {
        return header.getComments();
    }

    void replaceComments(SAMFileHeader header, List<String> comments) {
        header.setComments(comments);
    }

    void replaceHeaderRecords(SAMFileHeader header, List<SAMReadGroupRecord> readGroups, List<SAMProgramRecord> programs) {
        header.setReadGroups(readGroups);
        header.setProgramRecords(programs);
        header.setAttribute("VN", "1.6");
    }

    Object copy(SAMFileHeader header) {
        return header.clone();
    }

    Object sample(SAMReadGroupRecord readGroup) {
        return readGroup.getSample();
    }

    Object platform(SAMReadGroupRecord readGroup) {
        return readGroup.getPlatformUnit() + readGroup.getFlowOrder() + readGroup.getSequencingCenter();
    }

    Object readGroupMeta(SAMReadGroupRecord readGroup) {
        return readGroup.getReadGroupId() + readGroup.getAttribute("PLATFORM");
    }

    void updateReadGroup(SAMReadGroupRecord readGroup) {
        readGroup.setPlatformUnit("unit");
        readGroup.setFlowOrder("TACG");
        readGroup.setSequencingCenter("center");
        readGroup.setAttribute("CN", "center");
    }
}
"#,
    );

    let set_dictionary = semantic
        .facts
        .external_api_effect_for_receiver(Some("SAMFileHeader"), "setSequenceDictionary")
        .unwrap();
    assert_eq!(
        set_dictionary.provenance,
        FactProvenance::ExternalLibraryPack
    );
    assert_eq!(set_dictionary.role, ExternalApiRole::Mutator);

    let get_dictionary = semantic
        .facts
        .external_api_effect_for_receiver(Some("SAMFileHeader"), "getSequenceDictionary")
        .unwrap();
    assert_eq!(
        get_dictionary.return_nullability,
        NullabilityState::DefinitelyNonNull
    );
    assert_eq!(
        get_dictionary.provenance,
        FactProvenance::ExternalLibraryPack
    );

    let get_sample = semantic
        .facts
        .external_api_effect_for_receiver(Some("SAMReadGroupRecord"), "getSample")
        .unwrap();
    assert_eq!(get_sample.return_nullability, NullabilityState::MaybeNull);
    assert_eq!(get_sample.provenance, FactProvenance::ExternalLibraryPack);

    let md5 = semantic
        .facts
        .external_api_effect_for_receiver(Some("SAMSequenceRecord"), "getMd5")
        .expect("sequence record MD5 getter should be modeled by the pack");
    assert_eq!(md5.return_type, Some(JavaType::String));
    assert_eq!(md5.provenance, FactProvenance::ExternalLibraryPack);

    let flow_order = semantic
        .facts
        .external_api_effect_for_receiver(Some("SAMReadGroupRecord"), "getFlowOrder")
        .expect("read-group flow-order getter should be modeled by the pack");
    assert_eq!(flow_order.return_type, Some(JavaType::String));
    assert_eq!(flow_order.return_nullability, NullabilityState::MaybeNull);

    let reference_length = semantic
        .facts
        .external_api_effect_for_receiver(Some("SAMSequenceDictionary"), "getReferenceLength")
        .expect("dictionary reference length getter should be modeled by the pack");
    assert_eq!(
        reference_length.return_type,
        Some(JavaType::Primitive(
            new_java2rust::java_ir::PrimitiveType::Int
        ))
    );

    let header_sequence = semantic
        .facts
        .external_api_effect_for_receiver(Some("SAMFileHeader"), "getSequence")
        .expect("header sequence lookup should be modeled by the pack");
    assert_eq!(
        header_sequence.return_type,
        Some(JavaType::Named {
            name: "SAMSequenceRecord".to_string(),
            args: Vec::new(),
        })
    );
    assert_eq!(
        header_sequence.return_nullability,
        NullabilityState::MaybeNull
    );

    let read_group = semantic
        .facts
        .external_api_effect_for_receiver(Some("SAMFileHeader"), "getReadGroup")
        .expect("header read-group lookup should be modeled by the pack");
    assert_eq!(read_group.return_nullability, NullabilityState::MaybeNull);
    assert_eq!(read_group.provenance, FactProvenance::ExternalLibraryPack);

    let clone = semantic
        .facts
        .external_api_effect_for_receiver(Some("SAMFileHeader"), "clone")
        .expect("header clone should be modeled by the pack");
    assert_eq!(
        clone.return_type,
        Some(JavaType::Named {
            name: "SAMFileHeader".to_string(),
            args: Vec::new(),
        })
    );

    let assembly = semantic
        .facts
        .external_api_effect_for_receiver(Some("SAMSequenceRecord"), "getAssembly")
        .expect("sequence assembly getter should be modeled by the pack");
    assert_eq!(assembly.return_type, Some(JavaType::String));
    assert_eq!(assembly.return_nullability, NullabilityState::MaybeNull);

    let set_sequence_length = semantic
        .facts
        .external_api_effect_for_receiver(Some("SAMSequenceRecord"), "setSequenceLength")
        .expect("sequence-length setter should be modeled by the pack");
    assert_eq!(set_sequence_length.role, ExternalApiRole::FluentMutator);
    assert_eq!(
        set_sequence_length.return_type,
        Some(JavaType::Named {
            name: "SAMSequenceRecord".to_string(),
            args: Vec::new(),
        })
    );

    let sequencing_center = semantic
        .facts
        .external_api_effect_for_receiver(Some("SAMReadGroupRecord"), "getSequencingCenter")
        .expect("read-group sequencing center should be modeled by the pack");
    assert_eq!(sequencing_center.return_type, Some(JavaType::String));
    assert_eq!(
        sequencing_center.return_nullability,
        NullabilityState::MaybeNull
    );

    let read_group_attribute = semantic
        .facts
        .external_api_effect_for_receiver(Some("SAMReadGroupRecord"), "getAttribute")
        .expect("read-group attribute getter should be modeled by the pack");
    assert_eq!(
        read_group_attribute.return_nullability,
        NullabilityState::MaybeNull
    );

    let read_group_id = semantic
        .facts
        .external_api_effect_for_receiver(Some("SAMReadGroupRecord"), "getReadGroupId")
        .expect("read-group ID getter should be modeled by the pack");
    assert_eq!(read_group_id.return_type, Some(JavaType::String));

    let add_sequence = semantic
        .facts
        .external_api_effect_for_receiver(Some("SAMFileHeader"), "addSequence")
        .expect("header add-sequence mutator should be modeled by the pack");
    assert_eq!(add_sequence.role, ExternalApiRole::Mutator);

    let dictionary_add_sequence = semantic
        .facts
        .external_api_effect_for_receiver(Some("SAMSequenceDictionary"), "addSequence")
        .expect("dictionary add-sequence mutator should be modeled by the pack");
    assert_eq!(dictionary_add_sequence.role, ExternalApiRole::Mutator);

    let program_record = semantic
        .facts
        .external_api_effect_for_receiver(Some("SAMFileHeader"), "getProgramRecord")
        .expect("header program-record lookup should be modeled by the pack");
    assert_eq!(
        program_record.return_nullability,
        NullabilityState::MaybeNull
    );

    let program_group_id = semantic
        .facts
        .external_api_effect_for_receiver(Some("SAMProgramRecord"), "getProgramGroupId")
        .expect("program-record group ID getter should be modeled by the pack");
    assert_eq!(program_group_id.return_type, Some(JavaType::String));
    assert_eq!(
        program_group_id.return_nullability,
        NullabilityState::MaybeNull
    );

    for method_name in [
        "setCommandLine",
        "setPreviousProgramGroupId",
        "setProgramName",
        "setProgramVersion",
    ] {
        let mutator = semantic
            .facts
            .external_api_effect_for_receiver(Some("SAMProgramRecord"), method_name)
            .expect("SAM program-record mutator should be modeled by the pack");
        assert_eq!(mutator.role, ExternalApiRole::Mutator);
        assert_eq!(
            mutator.receiver_mutability,
            ReceiverMutabilityFact::MutatesReceiver
        );
    }

    let sam_string = semantic
        .facts
        .external_api_effect_for_receiver(Some("SAMFileHeader"), "getSAMString")
        .expect("header SAM string getter should be modeled by the pack");
    assert_eq!(sam_string.return_type, Some(JavaType::String));

    let set_comments = semantic
        .facts
        .external_api_effect_for_receiver(Some("SAMFileHeader"), "setComments")
        .expect("header comment replacement should be modeled by the pack");
    assert_eq!(set_comments.role, ExternalApiRole::Mutator);
    assert_eq!(
        set_comments.receiver_mutability,
        ReceiverMutabilityFact::MutatesReceiver
    );

    for method_name in ["setReadGroups", "setProgramRecords", "setAttribute"] {
        let mutator = semantic
            .facts
            .external_api_effect_for_receiver(Some("SAMFileHeader"), method_name)
            .expect("SAM header mutator should be modeled by the pack");
        assert_eq!(mutator.role, ExternalApiRole::Mutator);
        assert_eq!(
            mutator.receiver_mutability,
            ReceiverMutabilityFact::MutatesReceiver
        );
    }

    for method_name in [
        "setPlatformUnit",
        "setFlowOrder",
        "setSequencingCenter",
        "setAttribute",
    ] {
        let setter = semantic
            .facts
            .external_api_effect_for_receiver(Some("SAMReadGroupRecord"), method_name)
            .expect("SAM read-group setter should be modeled by the pack");
        assert_eq!(setter.role, ExternalApiRole::Setter);
        assert_eq!(
            setter.receiver_mutability,
            ReceiverMutabilityFact::MutatesReceiver
        );
    }

    let comments = semantic
        .facts
        .external_api_effect_for_receiver(Some("SAMFileHeader"), "getComments")
        .expect("header comments getter should be modeled by the pack");
    assert_eq!(
        comments.return_type,
        Some(JavaType::Named {
            name: "List".to_string(),
            args: vec![JavaType::String],
        })
    );
    assert_eq!(
        comments.return_nullability,
        NullabilityState::DefinitelyNonNull
    );

    let truncated_name = semantic
        .facts
        .external_api_effect_for_receiver(Some("SAMSequenceRecord"), "truncateSequenceName")
        .expect("sequence-name truncation helper should be modeled by the pack");
    assert_eq!(truncated_name.return_type, Some(JavaType::String));

    let header = semantic
        .facts
        .external_constructor_for("SAMFileHeader", &[])
        .unwrap();
    assert_eq!(header.provenance, FactProvenance::ExternalLibraryPack);

    assert!(
        !semantic
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "external_pack_missing_api"),
        "newly modeled SAM header calls should not produce missing-pack diagnostics: {:?}",
        semantic.diagnostics
    );
}

#[test]
fn htsjdk_sam_record_pack_declares_record_boundary() {
    let facts = htsjdk_sam_record_pack_facts();

    let pack = facts
        .external_library_pack_for_type("SAMRecord")
        .expect("SAMRecord should be owned by the SAM record pack");
    assert_eq!(pack.pack_name, "htsjdk.samtools.record");
    assert_eq!(pack.mode, ExternalLibraryMode::ExternalFactModel);
    assert_eq!(pack.coverage, ExternalApiCoverage::Semantic);
    assert_eq!(pack.provenance, FactProvenance::ExternalLibraryPack);

    let args = vec![JavaType::Named {
        name: "SAMFileHeader".to_string(),
        args: Vec::new(),
    }];
    let constructor = facts
        .external_constructor_for("SAMRecord", &args)
        .expect("SAMRecord/1 should be pack-owned");
    assert_eq!(constructor.provenance, FactProvenance::ExternalLibraryPack);
    assert_eq!(
        new_java2rust::stdlib::external_constructor_provenance("SAMRecord", &args),
        FactProvenance::ConservativeFallback,
        "SAMRecord/1 should not be a generic stdlib fallback"
    );
    let filter_pack = facts
        .external_library_pack_for_type("SamRecordFilter")
        .expect("SamRecordFilter should be owned by the SAM record pack");
    assert_eq!(filter_pack.pack_name, "htsjdk.samtools.record");
    assert!(
        facts.has_object_representation_facts(&JavaType::Named {
            name: "SamRecordFilter".to_string(),
            args: Vec::new(),
        }),
        "SamRecordFilter should be an explicit erased external boundary for anonymous implementations"
    );

    let int_arg = JavaType::Primitive(PrimitiveType::Int);
    let cigar_operator = JavaType::Named {
        name: "CigarOperator".to_string(),
        args: Vec::new(),
    };
    let cigar_element = JavaType::Named {
        name: "CigarElement".to_string(),
        args: Vec::new(),
    };
    for (type_name, args) in [
        (
            "CigarElement",
            vec![int_arg.clone(), cigar_operator.clone()],
        ),
        ("Cigar", Vec::new()),
        (
            "Cigar",
            vec![JavaType::Named {
                name: "List".to_string(),
                args: vec![cigar_element],
            }],
        ),
        (
            "FilteringSamIterator",
            vec![
                JavaType::Named {
                    name: "Iterator".to_string(),
                    args: vec![JavaType::Named {
                        name: "SAMRecord".to_string(),
                        args: Vec::new(),
                    }],
                },
                JavaType::Named {
                    name: "Object".to_string(),
                    args: Vec::new(),
                },
            ],
        ),
    ] {
        let constructor = facts
            .external_constructor_for(type_name, &args)
            .unwrap_or_else(|| panic!("{type_name}/{} should be pack-owned", args.len()));
        assert_eq!(constructor.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(
            new_java2rust::stdlib::external_constructor_provenance(type_name, &args),
            FactProvenance::ConservativeFallback,
            "{type_name}/{} should not be a generic stdlib fallback",
            args.len()
        );
    }

    for type_name in [
        "DefaultSAMRecordFactory",
        "SAMRecordCoordinateComparator",
        "SAMRecordQueryNameComparator",
    ] {
        let constructor = facts
            .external_constructor_for(type_name, &[])
            .unwrap_or_else(|| panic!("{type_name}/0 should be pack-owned"));
        assert_eq!(constructor.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(
            new_java2rust::stdlib::external_constructor_provenance(type_name, &[]),
            FactProvenance::ConservativeFallback,
            "{type_name}/0 should not be a generic stdlib fallback"
        );
    }

    let contig = facts
        .external_api_effect_for_receiver(Some("SAMRecord"), "getContig")
        .expect("SAMRecord contig getter should be modeled by the pack");
    assert_eq!(contig.return_type, Some(JavaType::String));
    assert_eq!(contig.return_nullability, NullabilityState::MaybeNull);

    let start = facts
        .external_api_effect_for_receiver(Some("SAMRecord"), "getStart")
        .expect("SAMRecord start getter should be modeled by the pack");
    assert_eq!(
        start.return_type,
        Some(JavaType::Primitive(PrimitiveType::Int))
    );
    assert_eq!(
        start.return_nullability,
        NullabilityState::DefinitelyNonNull
    );

    let header = facts
        .external_api_effect_for_receiver(Some("SAMRecord"), "getHeader")
        .expect("SAMRecord header getter should be modeled by the pack");
    assert_eq!(
        header.return_type,
        Some(JavaType::Named {
            name: "SAMFileHeader".to_string(),
            args: Vec::new(),
        })
    );
    assert_eq!(header.return_nullability, NullabilityState::MaybeNull);

    let compare = facts
        .external_api_effect_for_receiver(Some("SAMRecordCoordinateComparator"), "compare")
        .expect("SAM record coordinate comparator should be modeled by the pack");
    assert_eq!(
        compare.return_type,
        Some(JavaType::Primitive(PrimitiveType::Int))
    );
    assert_eq!(
        compare.return_nullability,
        NullabilityState::DefinitelyNonNull
    );

    let cigar_add = facts
        .external_api_effect_for_receiver(Some("Cigar"), "add")
        .expect("Cigar.add should be modeled by the SAM record pack");
    assert_eq!(cigar_add.role, ExternalApiRole::Mutator);
    assert_eq!(
        cigar_add.receiver_mutability,
        ReceiverMutabilityFact::MutatesReceiver
    );

    let cigar_element_ty = JavaType::Named {
        name: "CigarElement".to_string(),
        args: Vec::new(),
    };
    for (method_name, expected_nullability) in [
        ("getCigarElement", NullabilityState::DefinitelyNonNull),
        ("getFirstCigarElement", NullabilityState::MaybeNull),
        ("getLastCigarElement", NullabilityState::MaybeNull),
    ] {
        let effect = facts
            .external_api_effect_for_receiver(Some("Cigar"), method_name)
            .unwrap_or_else(|| {
                panic!("Cigar.{method_name} should be modeled by the SAM record pack")
            });
        assert_eq!(effect.role, ExternalApiRole::Getter);
        assert_eq!(effect.receiver_mutability, ReceiverMutabilityFact::ReadOnly);
        assert_eq!(effect.return_type, Some(cigar_element_ty.clone()));
        assert_eq!(effect.return_nullability, expected_nullability);
    }

    let cigar_elements = facts
        .external_api_effect_for_receiver(Some("Cigar"), "getCigarElements")
        .expect("Cigar.getCigarElements should be modeled by the SAM record pack");
    assert_eq!(
        cigar_elements.return_type,
        Some(JavaType::Named {
            name: "List".to_string(),
            args: vec![cigar_element_ty.clone()],
        })
    );

    for method_name in [
        "getReadLength",
        "getReferenceLength",
        "getPaddedReferenceLength",
    ] {
        let effect = facts
            .external_api_effect_for_receiver(Some("Cigar"), method_name)
            .unwrap_or_else(|| {
                panic!("Cigar.{method_name} should be modeled by the SAM record pack")
            });
        assert_eq!(
            effect.return_type,
            Some(JavaType::Primitive(PrimitiveType::Int))
        );
    }

    let cigar_empty = facts
        .external_api_effect_for_receiver(Some("Cigar"), "isEmpty")
        .expect("Cigar.isEmpty should be modeled by the SAM record pack");
    assert_eq!(cigar_empty.role, ExternalApiRole::BooleanGetter);

    let cigar_is_valid = facts
        .external_api_effect_for_receiver(Some("Cigar"), "isValid")
        .expect("Cigar.isValid should be modeled by the SAM record pack");
    assert_eq!(cigar_is_valid.role, ExternalApiRole::BooleanGetter);
    assert_eq!(
        cigar_is_valid.return_type,
        Some(JavaType::Primitive(PrimitiveType::Boolean))
    );

    let cigar_count = facts
        .external_api_effect_for_receiver(Some("Cigar"), "numCigarElements")
        .expect("Cigar.numCigarElements should be modeled by the SAM record pack");
    assert_eq!(
        cigar_count.return_type,
        Some(JavaType::Primitive(PrimitiveType::Int))
    );

    let cigar_operator = facts
        .external_api_effect_for_receiver(Some("CigarElement"), "getOperator")
        .expect("CigarElement.getOperator should be modeled by the SAM record pack");
    assert_eq!(
        cigar_operator.return_type,
        Some(JavaType::Named {
            name: "CigarOperator".to_string(),
            args: Vec::new(),
        })
    );

    for method_name in [
        "consumesReadBases",
        "consumesReferenceBases",
        "isClipping",
        "isAlignment",
        "isIndel",
    ] {
        let effect = facts
            .external_api_effect_for_receiver(Some("CigarOperator"), method_name)
            .unwrap_or_else(|| {
                panic!("CigarOperator.{method_name} should be modeled by the SAM record pack")
            });
        assert_eq!(effect.role, ExternalApiRole::BooleanGetter);
        assert_eq!(
            effect.return_type,
            Some(JavaType::Primitive(PrimitiveType::Boolean))
        );
    }

    let character_to_enum = facts
        .external_api_effect_for_receiver(Some("CigarOperator"), "characterToEnum")
        .expect("CigarOperator.characterToEnum should be modeled by the SAM record pack");
    assert_eq!(
        character_to_enum.return_type,
        Some(JavaType::Named {
            name: "CigarOperator".to_string(),
            args: Vec::new(),
        })
    );

    for method_name in [
        "getReadLength",
        "getAlignmentStart",
        "getAlignmentEnd",
        "getMappingQuality",
        "getFlags",
        "getMateAlignmentStart",
        "getInferredInsertSize",
        "getCigarLength",
        "getMateAlignmentEnd",
        "getMateReferenceIndex",
        "getUnclippedStart",
        "getUnclippedEnd",
    ] {
        let effect = facts
            .external_api_effect_for_receiver(Some("SAMRecord"), method_name)
            .unwrap_or_else(|| {
                panic!("SAMRecord.{method_name} should be modeled by the SAM record pack")
            });
        assert_eq!(
            effect.return_type,
            Some(JavaType::Primitive(PrimitiveType::Int))
        );
    }

    let record_cigar = facts
        .external_api_effect_for_receiver(Some("SAMRecord"), "getCigar")
        .expect("SAMRecord.getCigar should be modeled by the SAM record pack");
    assert_eq!(
        record_cigar.return_type,
        Some(JavaType::Named {
            name: "Cigar".to_string(),
            args: Vec::new(),
        })
    );

    for method_name in [
        "getReadUnmappedFlag",
        "getReadNegativeStrandFlag",
        "getReadPairedFlag",
        "getMateUnmappedFlag",
        "getSecondOfPairFlag",
        "getFirstOfPairFlag",
        "getProperPairFlag",
        "getDuplicateReadFlag",
        "isSecondaryAlignment",
        "isSecondaryOrSupplementary",
        "getSupplementaryAlignmentFlag",
        "getMateNegativeStrandFlag",
        "getReadFailsVendorQualityCheckFlag",
        "hasAttribute",
    ] {
        let effect = facts
            .external_api_effect_for_receiver(Some("SAMRecord"), method_name)
            .unwrap_or_else(|| {
                panic!("SAMRecord.{method_name} should be modeled by the SAM record pack")
            });
        assert_eq!(effect.role, ExternalApiRole::BooleanGetter);
    }

    for method_name in ["getReferenceName", "getMateReferenceName"] {
        let effect = facts
            .external_api_effect_for_receiver(Some("SAMRecord"), method_name)
            .unwrap_or_else(|| {
                panic!("SAMRecord.{method_name} should be modeled by the SAM record pack")
            });
        assert_eq!(effect.return_type, Some(JavaType::String));
        assert_eq!(effect.return_nullability, NullabilityState::MaybeNull);
    }

    let cigar_string = facts
        .external_api_effect_for_receiver(Some("SAMRecord"), "getCigarString")
        .expect("SAMRecord.getCigarString should be modeled by the SAM record pack");
    assert_eq!(cigar_string.return_type, Some(JavaType::String));
    assert_eq!(
        cigar_string.return_nullability,
        NullabilityState::DefinitelyNonNull
    );

    let sam_string = facts
        .external_api_effect_for_receiver(Some("SAMRecord"), "getSAMString")
        .expect("SAMRecord.getSAMString should be modeled by the SAM record pack");
    assert_eq!(sam_string.return_type, Some(JavaType::String));
    assert_eq!(
        sam_string.return_nullability,
        NullabilityState::DefinitelyNonNull
    );

    let attribute = facts
        .external_api_effect_for_receiver(Some("SAMRecord"), "getAttribute")
        .expect("SAMRecord.getAttribute should be modeled by the SAM record pack");
    assert_eq!(
        attribute.return_type,
        Some(JavaType::Named {
            name: "Object".to_string(),
            args: Vec::new(),
        })
    );
    assert_eq!(attribute.return_nullability, NullabilityState::MaybeNull);

    let integer_attribute = facts
        .external_api_effect_for_receiver(Some("SAMRecord"), "getIntegerAttribute")
        .expect("SAMRecord.getIntegerAttribute should be modeled by the SAM record pack");
    assert_eq!(
        integer_attribute.return_type,
        Some(JavaType::Named {
            name: "Integer".to_string(),
            args: Vec::new(),
        })
    );
    assert_eq!(
        integer_attribute.return_nullability,
        NullabilityState::MaybeNull
    );

    let byte_array_attribute = facts
        .external_api_effect_for_receiver(Some("SAMRecord"), "getByteArrayAttribute")
        .expect("SAMRecord.getByteArrayAttribute should be modeled by the SAM record pack");
    assert_eq!(
        byte_array_attribute.return_type,
        Some(JavaType::Array(Box::new(JavaType::Primitive(
            PrimitiveType::Byte
        ))))
    );
    assert_eq!(
        byte_array_attribute.return_nullability,
        NullabilityState::MaybeNull
    );

    let signed_byte_array_attribute = facts
        .external_api_effect_for_receiver(Some("SAMRecord"), "getSignedByteArrayAttribute")
        .expect("SAMRecord.getSignedByteArrayAttribute should be modeled by the SAM record pack");
    assert_eq!(
        signed_byte_array_attribute.return_type,
        Some(JavaType::Array(Box::new(JavaType::Primitive(
            PrimitiveType::Byte
        ))))
    );
    assert_eq!(
        signed_byte_array_attribute.return_nullability,
        NullabilityState::MaybeNull
    );

    let string_attribute = facts
        .external_api_effect_for_receiver(Some("SAMRecord"), "getStringAttribute")
        .expect("SAMRecord.getStringAttribute should be modeled by the SAM record pack");
    assert_eq!(string_attribute.return_type, Some(JavaType::String));
    assert_eq!(
        string_attribute.return_nullability,
        NullabilityState::MaybeNull
    );

    let transient_attribute = facts
        .external_api_effect_for_receiver(Some("SAMRecord"), "getTransientAttribute")
        .expect("SAMRecord.getTransientAttribute should be modeled by the SAM record pack");
    assert_eq!(
        transient_attribute.return_type,
        Some(JavaType::Named {
            name: "Object".to_string(),
            args: Vec::new(),
        })
    );
    assert_eq!(
        transient_attribute.return_nullability,
        NullabilityState::MaybeNull
    );

    let read_group = facts
        .external_api_effect_for_receiver(Some("SAMRecord"), "getReadGroup")
        .expect("SAMRecord.getReadGroup should be modeled by the SAM record pack");
    assert_eq!(
        read_group.return_type,
        Some(JavaType::Named {
            name: "SAMReadGroupRecord".to_string(),
            args: Vec::new(),
        })
    );
    assert_eq!(read_group.return_nullability, NullabilityState::MaybeNull);

    let attributes = facts
        .external_api_effect_for_receiver(Some("SAMRecord"), "getAttributes")
        .expect("SAMRecord.getAttributes should be modeled by the SAM record pack");
    assert_eq!(
        attributes.return_type,
        Some(JavaType::Named {
            name: "List".to_string(),
            args: vec![JavaType::Named {
                name: "SAMTagAndValue".to_string(),
                args: Vec::new(),
            }],
        })
    );

    let alignment_blocks = facts
        .external_api_effect_for_receiver(Some("SAMRecord"), "getAlignmentBlocks")
        .expect("SAMRecord.getAlignmentBlocks should be modeled by the SAM record pack");
    assert_eq!(
        alignment_blocks.return_type,
        Some(JavaType::Named {
            name: "List".to_string(),
            args: vec![JavaType::Named {
                name: "AlignmentBlock".to_string(),
                args: Vec::new(),
            }],
        })
    );

    for method_name in ["clone", "deepCopy"] {
        let copy = facts
            .external_api_effect_for_receiver(Some("SAMRecord"), method_name)
            .unwrap_or_else(|| {
                panic!("SAMRecord.{method_name} should be modeled by the SAM record pack")
            });
        assert_eq!(
            copy.return_type,
            Some(JavaType::Named {
                name: "SAMRecord".to_string(),
                args: Vec::new(),
            })
        );
        assert_eq!(copy.return_nullability, NullabilityState::DefinitelyNonNull);
    }

    for method_name in [
        "setAttribute",
        "setBaseQualities",
        "setReadBases",
        "setCigar",
        "setReadName",
        "setReferenceName",
        "setMappingQuality",
        "setMateUnmappedFlag",
        "setProperPairFlag",
        "setSecondaryAlignment",
        "setFirstOfPairFlag",
        "setSecondOfPairFlag",
        "setCigarString",
        "setMateAlignmentStart",
        "setMateNegativeStrandFlag",
        "setReferenceIndex",
        "setMateReferenceIndex",
        "setMateReferenceName",
        "setReadUnmappedFlag",
        "setAlignmentStart",
        "setFlags",
        "setHeaderStrict",
        "clearAttributes",
        "removeTransientAttribute",
        "setReadNegativeStrandFlag",
        "setReadPairedFlag",
        "setSupplementaryAlignmentFlag",
        "setReadFailsVendorQualityCheckFlag",
        "setDuplicateReadFlag",
        "setTransientAttribute",
        "setInferredInsertSize",
        "reverseComplement",
    ] {
        let effect = facts
            .external_api_effect_for_receiver(Some("SAMRecord"), method_name)
            .unwrap_or_else(|| {
                panic!("SAMRecord.{method_name} should be modeled by the SAM record pack")
            });
        assert_eq!(effect.role, ExternalApiRole::Mutator);
        assert_eq!(
            effect.receiver_mutability,
            ReceiverMutabilityFact::MutatesReceiver
        );
    }

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
        let field = facts
            .external_field_for("SAMRecord", field_name)
            .unwrap_or_else(|| panic!("SAMRecord.{field_name} should be pack-owned"));
        assert_eq!(field.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(field.field_type, Some(expected_ty));
        assert_eq!(
            new_java2rust::stdlib::external_field_provenance("SAMRecord", field_name),
            FactProvenance::ConservativeFallback,
            "SAMRecord.{field_name} precision should come from the SAM record pack, not stdlib"
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
        let field = facts
            .external_field_for(type_name, field_name)
            .unwrap_or_else(|| panic!("{type_name}.{field_name} should be pack-owned"));
        assert_eq!(field.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(field.field_type, Some(expected_ty.clone()));

        let dispatched = new_java2rust::external_packs::external_field_for(type_name, field_name)
            .unwrap_or_else(|| panic!("{type_name}.{field_name} should be globally dispatched"));
        assert_eq!(dispatched.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(dispatched.field_type, Some(expected_ty));
        assert_eq!(
            new_java2rust::stdlib::external_field_provenance(type_name, field_name),
            FactProvenance::ConservativeFallback,
            "{type_name}.{field_name} precision should come from the SAM record pack, not stdlib"
        );
    }

    for field_name in ["MATCH_OR_MISMATCH", "INSERTION", "SOFT_CLIP"] {
        let cigar_operator = facts
            .external_field_for("CigarOperator", field_name)
            .unwrap_or_else(|| panic!("CigarOperator.{field_name} should be pack-owned"));
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

        let dispatched = new_java2rust::external_packs::external_field_for(
            "cigar_operator",
            field_name,
        )
        .unwrap_or_else(|| {
            panic!("CigarOperator.{field_name} should be available through global pack dispatch")
        });
        assert_eq!(dispatched.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(
            dispatched.field_type,
            Some(JavaType::Named {
                name: "CigarOperator".to_string(),
                args: Vec::new(),
            })
        );
        assert_eq!(
            new_java2rust::stdlib::external_field_provenance("CigarOperator", field_name),
            FactProvenance::ConservativeFallback,
            "CigarOperator.{field_name} precision should come from the SAM record pack, not stdlib"
        );
    }
}

#[test]
fn semantic_analysis_uses_sam_record_pack_for_read_record_helpers() {
    let semantic = analyze(
        r#"
import htsjdk.samtools.SAMFileHeader;
import htsjdk.samtools.SAMRecord;
import htsjdk.samtools.Cigar;
import htsjdk.samtools.CigarElement;
import htsjdk.samtools.CigarOperator;
import htsjdk.samtools.AlignmentBlock;
import htsjdk.samtools.SAMReadGroupRecord;
import htsjdk.samtools.SAMRecord.SAMTagAndValue;
import java.util.List;

class SamRecordUse {
    int span(SAMFileHeader header, List<CigarElement> elements) {
        CigarElement element = new CigarElement(10, CigarOperator.MATCH_OR_MISMATCH);
        Cigar empty = new Cigar();
        Cigar copied = new Cigar(elements);
        empty.add(element);
        CigarElement indexed = copied.getCigarElement(0);
        CigarElement first = copied.getFirstCigarElement();
        CigarElement last = copied.getLastCigarElement();
        CigarOperator op = indexed.getOperator();
        List<CigarElement> cigarElements = copied.getCigarElements();
        int count = copied.numCigarElements() + copied.getReadLength() + copied.getReferenceLength();
        boolean valid = copied.isValid();
        boolean emptyCigar = copied.isEmpty();
        boolean operatorFacts = op.consumesReadBases()
            || op.consumesReferenceBases()
            || op.isClipping()
            || op.isAlignment()
            || op.isIndel();
        CigarOperator converted = CigarOperator.characterToEnum('M');
        SAMRecord record = new SAMRecord(header);
        record.setAttribute("XX", "value");
        record.setBaseQualities(new byte[0]);
        record.setReadBases(new byte[0]);
        record.setCigar(copied);
        record.setReadName("read");
        record.setReferenceName("chr1");
        record.setMappingQuality(60);
        record.setMateUnmappedFlag(false);
        record.setProperPairFlag(true);
        record.setSecondaryAlignment(false);
        record.setFirstOfPairFlag(true);
        record.setSecondOfPairFlag(false);
        record.setCigarString("10M");
        record.setMateAlignmentStart(12);
        record.setMateNegativeStrandFlag(false);
        record.setReferenceIndex(0);
        record.setMateReferenceIndex(0);
        record.setMateReferenceName("chr1");
        record.setReadNegativeStrandFlag(false);
        record.setReadPairedFlag(true);
        record.setSupplementaryAlignmentFlag(false);
        record.setReadFailsVendorQualityCheckFlag(false);
        record.setDuplicateReadFlag(false);
        record.setTransientAttribute("ta", "value");
        record.setInferredInsertSize(42);
        record.setReadUnmappedFlag(false);
        record.setAlignmentStart(10);
        record.setFlags(record.getFlags());
        record.setHeaderStrict(header);
        Object shallowCopy = record.clone();
        SAMRecord deepCopy = record.deepCopy();
        List<SAMTagAndValue> attributes = record.getAttributes();
        List<AlignmentBlock> blocks = record.getAlignmentBlocks();
        String cigarString = record.getCigarString();
        String samString = record.getSAMString();
        byte[] signedBytes = record.getSignedByteArrayAttribute("sb");
        String stringAttribute = record.getStringAttribute("sa");
        Object transientAttribute = record.getTransientAttribute("ta");
        record.clearAttributes();
        record.removeTransientAttribute("ta");
        record.reverseComplement();
        SAMRecordCoordinateComparator coordinate = new SAMRecordCoordinateComparator();
        SAMRecordQueryNameComparator query = new SAMRecordQueryNameComparator();
        DefaultSAMRecordFactory factory = new DefaultSAMRecordFactory();
        int coordinateOrder = coordinate.compare(record, record);
        int queryOrder = query.compare(record, record);
        String readName = record.getReadName();
        String contig = record.getContig();
        String referenceName = record.getReferenceName();
        String mateReferenceName = record.getMateReferenceName();
        Cigar recordCigar = record.getCigar();
        Object attribute = record.getAttribute("XX");
        Integer intAttribute = record.getIntegerAttribute("NM");
        byte[] byteAttribute = record.getByteArrayAttribute("BQ");
        SAMReadGroupRecord readGroup = record.getReadGroup();
        boolean flags = record.getReadUnmappedFlag()
            || record.getReadNegativeStrandFlag()
            || record.getReadPairedFlag()
            || record.getMateUnmappedFlag()
            || record.getSecondOfPairFlag()
            || record.getFirstOfPairFlag()
            || record.getProperPairFlag()
            || record.getDuplicateReadFlag()
            || record.isSecondaryAlignment()
            || record.isSecondaryOrSupplementary()
            || record.getSupplementaryAlignmentFlag()
            || record.getMateNegativeStrandFlag()
            || record.getReadFailsVendorQualityCheckFlag()
            || record.hasAttribute("XX");
        int readStats = record.getReadLength()
            + record.getAlignmentStart()
            + record.getAlignmentEnd()
            + record.getMappingQuality()
            + record.getFlags()
            + record.getMateAlignmentStart()
            + record.getInferredInsertSize()
            + record.getCigarLength()
            + record.getMateAlignmentEnd()
            + record.getMateReferenceIndex()
            + record.getUnclippedStart()
            + record.getUnclippedEnd();
        SAMFileHeader copiedHeader = record.getHeader();
        return count + readStats + record.getEnd() - record.getStart();
    }
}
"#,
    );

    assert!(
        semantic.diagnostics.iter().all(|diagnostic| diagnostic.code
            != "external_pack_missing_constructor"
            && diagnostic.code != "external_pack_missing_api"),
        "modeled SAMRecord constructor and getters should not report pack gaps"
    );

    let constructor = semantic
        .facts
        .external_constructor_for(
            "SAMRecord",
            &[JavaType::Named {
                name: "SAMFileHeader".to_string(),
                args: Vec::new(),
            }],
        )
        .expect("semantic analysis should use the SAM record pack for SAMRecord/1");
    assert_eq!(constructor.provenance, FactProvenance::ExternalLibraryPack);

    let cigar_element = semantic
        .facts
        .external_constructor_for(
            "CigarElement",
            &[
                JavaType::Primitive(PrimitiveType::Int),
                JavaType::Named {
                    name: "CigarOperator".to_string(),
                    args: Vec::new(),
                },
            ],
        )
        .expect("semantic analysis should use the SAM record pack for CigarElement/2");
    assert_eq!(
        cigar_element.provenance,
        FactProvenance::ExternalLibraryPack
    );

    for arg_count in [0, 1] {
        let fact = semantic
            .facts
            .external_constructor_facts
            .iter()
            .find(|fact| fact.type_name == "Cigar" && fact.arg_types.len() == arg_count)
            .unwrap_or_else(|| panic!("semantic analysis should model Cigar/{arg_count}"));
        assert_eq!(fact.provenance, FactProvenance::ExternalLibraryPack);
    }

    let start = semantic
        .facts
        .external_api_effect_for_receiver(Some("SAMRecord"), "getStart")
        .expect("semantic analysis should model SAMRecord.getStart");
    assert_eq!(start.provenance, FactProvenance::ExternalLibraryPack);
    assert_eq!(
        start.return_type,
        Some(JavaType::Primitive(PrimitiveType::Int))
    );

    let compare = semantic
        .facts
        .external_api_effect_for_receiver(Some("SAMRecordCoordinateComparator"), "compare")
        .expect("semantic analysis should model SAMRecordCoordinateComparator.compare");
    assert_eq!(compare.provenance, FactProvenance::ExternalLibraryPack);

    for method_name in [
        "add",
        "getCigarElement",
        "getFirstCigarElement",
        "getLastCigarElement",
        "isValid",
        "numCigarElements",
        "getCigarElements",
        "getReadLength",
        "getReferenceLength",
        "isEmpty",
    ] {
        let effect = semantic
            .facts
            .external_api_effect_for_receiver(Some("Cigar"), method_name)
            .unwrap_or_else(|| panic!("semantic analysis should model Cigar.{method_name}"));
        assert_eq!(effect.provenance, FactProvenance::ExternalLibraryPack);
    }

    let operator = semantic
        .facts
        .external_api_effect_for_receiver(Some("CigarElement"), "getOperator")
        .expect("semantic analysis should model CigarElement.getOperator");
    assert_eq!(operator.provenance, FactProvenance::ExternalLibraryPack);

    for method_name in [
        "consumesReadBases",
        "consumesReferenceBases",
        "isClipping",
        "isAlignment",
        "isIndel",
        "characterToEnum",
    ] {
        let effect = semantic
            .facts
            .external_api_effect_for_receiver(Some("CigarOperator"), method_name)
            .unwrap_or_else(|| {
                panic!("semantic analysis should model CigarOperator.{method_name}")
            });
        assert_eq!(effect.provenance, FactProvenance::ExternalLibraryPack);
    }

    for method_name in [
        "getCigar",
        "getReadLength",
        "getAlignmentStart",
        "getAlignmentEnd",
        "getMappingQuality",
        "getFlags",
        "getMateAlignmentStart",
        "getInferredInsertSize",
        "getCigarLength",
        "getMateAlignmentEnd",
        "getMateReferenceIndex",
        "getUnclippedStart",
        "getUnclippedEnd",
        "getReadUnmappedFlag",
        "getReadNegativeStrandFlag",
        "getReadPairedFlag",
        "getMateUnmappedFlag",
        "getSecondOfPairFlag",
        "getFirstOfPairFlag",
        "getProperPairFlag",
        "getDuplicateReadFlag",
        "isSecondaryAlignment",
        "isSecondaryOrSupplementary",
        "getSupplementaryAlignmentFlag",
        "getMateNegativeStrandFlag",
        "getReadFailsVendorQualityCheckFlag",
        "getReferenceName",
        "getMateReferenceName",
        "getCigarString",
        "getSAMString",
        "getAttribute",
        "getIntegerAttribute",
        "getByteArrayAttribute",
        "getSignedByteArrayAttribute",
        "getStringAttribute",
        "getTransientAttribute",
        "getReadGroup",
        "getAttributes",
        "getAlignmentBlocks",
        "clone",
        "deepCopy",
        "hasAttribute",
        "setAttribute",
        "setBaseQualities",
        "setReadBases",
        "setCigar",
        "setReadName",
        "setReferenceName",
        "setMappingQuality",
        "setMateUnmappedFlag",
        "setProperPairFlag",
        "setSecondaryAlignment",
        "setFirstOfPairFlag",
        "setSecondOfPairFlag",
        "setCigarString",
        "setMateAlignmentStart",
        "setMateNegativeStrandFlag",
        "setReferenceIndex",
        "setMateReferenceIndex",
        "setMateReferenceName",
        "setReadUnmappedFlag",
        "setAlignmentStart",
        "setFlags",
        "setHeaderStrict",
        "clearAttributes",
        "removeTransientAttribute",
        "setReadNegativeStrandFlag",
        "setReadPairedFlag",
        "setSupplementaryAlignmentFlag",
        "setReadFailsVendorQualityCheckFlag",
        "setDuplicateReadFlag",
        "setTransientAttribute",
        "setInferredInsertSize",
        "reverseComplement",
    ] {
        let effect = semantic
            .facts
            .external_api_effect_for_receiver(Some("SAMRecord"), method_name)
            .unwrap_or_else(|| panic!("semantic analysis should model SAMRecord.{method_name}"));
        assert_eq!(effect.provenance, FactProvenance::ExternalLibraryPack);
    }
}

#[test]
fn htsjdk_sam_reference_pack_declares_reference_boundary() {
    let facts = htsjdk_sam_reference_pack_facts();

    let pack = facts
        .external_library_pack_for_type("ReferenceSequence")
        .expect("ReferenceSequence should be owned by the SAM reference pack");
    assert_eq!(pack.pack_name, "htsjdk.samtools.reference");
    assert_eq!(pack.mode, ExternalLibraryMode::ExternalFactModel);
    assert_eq!(pack.coverage, ExternalApiCoverage::Semantic);
    assert_eq!(pack.provenance, FactProvenance::ExternalLibraryPack);
    assert!(
        facts
            .external_library_pack_for_type("ReferenceSequenceFileWalker")
            .is_some_and(|pack| pack.pack_name == "htsjdk.samtools.reference"),
        "ReferenceSequenceFileWalker should be owned by the SAM reference pack"
    );

    for (type_name, args) in [
        (
            "FastaSequenceFile",
            vec![
                JavaType::Named {
                    name: "Path".to_string(),
                    args: Vec::new(),
                },
                JavaType::Primitive(PrimitiveType::Boolean),
            ],
        ),
        (
            "IndexedFastaSequenceFile",
            vec![JavaType::Named {
                name: "Path".to_string(),
                args: Vec::new(),
            }],
        ),
        ("FastaReferenceWriterBuilder", Vec::new()),
        (
            "ReferenceSequence",
            vec![
                JavaType::String,
                JavaType::Primitive(PrimitiveType::Int),
                JavaType::Array(Box::new(JavaType::Primitive(PrimitiveType::Byte))),
            ],
        ),
        (
            "ReferenceSequenceFileWalker",
            vec![JavaType::Named {
                name: "File".to_string(),
                args: Vec::new(),
            }],
        ),
    ] {
        let constructor = facts
            .external_constructor_for(type_name, &args)
            .unwrap_or_else(|| panic!("{type_name}/{} should be pack-owned", args.len()));
        assert_eq!(constructor.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(
            new_java2rust::stdlib::external_constructor_provenance(type_name, &args),
            FactProvenance::ConservativeFallback,
            "{type_name}/{} should not be a generic stdlib fallback",
            args.len()
        );
    }

    let name = facts
        .external_api_effect_for_receiver(Some("ReferenceSequence"), "getName")
        .expect("ReferenceSequence.getName should be modeled by the pack");
    assert_eq!(name.return_type, Some(JavaType::String));
    assert_eq!(name.return_nullability, NullabilityState::DefinitelyNonNull);

    let bases = facts
        .external_api_effect_for_receiver(Some("ReferenceSequence"), "getBases")
        .expect("ReferenceSequence.getBases should be modeled by the pack");
    assert_eq!(
        bases.return_type,
        Some(JavaType::Array(Box::new(JavaType::Primitive(
            PrimitiveType::Byte
        ))))
    );
    assert_eq!(
        bases.return_nullability,
        NullabilityState::DefinitelyNonNull
    );

    let next = facts
        .external_api_effect_for_receiver(Some("FastaSequenceFile"), "nextSequence")
        .expect("FastaSequenceFile.nextSequence should be modeled by the pack");
    assert_eq!(
        next.receiver_mutability,
        ReceiverMutabilityFact::MutatesReceiver
    );
    assert_eq!(next.return_nullability, NullabilityState::MaybeNull);
    assert_eq!(
        next.return_type,
        Some(JavaType::Named {
            name: "ReferenceSequence".to_string(),
            args: Vec::new(),
        })
    );
}

#[test]
fn semantic_analysis_uses_sam_reference_pack_for_reference_helpers() {
    let semantic = analyze(
        r#"
import htsjdk.samtools.reference.FastaReferenceWriterBuilder;
import htsjdk.samtools.reference.FastaSequenceFile;
import htsjdk.samtools.reference.IndexedFastaSequenceFile;
import htsjdk.samtools.reference.ReferenceSequence;
import htsjdk.samtools.reference.ReferenceSequenceFileWalker;
import htsjdk.samtools.SAMSequenceDictionary;
import java.io.File;
import java.nio.file.Path;

class ReferenceUse {
    void run(Path referencePath, File referenceFile, byte[] bases) {
        FastaSequenceFile fasta = new FastaSequenceFile(referencePath, false);
        ReferenceSequence next = fasta.nextSequence();
        ReferenceSequence explicit = new ReferenceSequence("chr1", 0, bases);
        String name = explicit.getName();
        int index = explicit.getContigIndex();
        byte[] explicitBases = explicit.getBases();
        IndexedFastaSequenceFile indexed = new IndexedFastaSequenceFile(referencePath);
        ReferenceSequenceFileWalker walker = new ReferenceSequenceFileWalker(referenceFile);
        SAMSequenceDictionary dictionary = walker.getSequenceDictionary();
        ReferenceSequence walked = walker.get(0);
        byte[] walkedBases = walked.getBases();
        FastaReferenceWriterBuilder builder = new FastaReferenceWriterBuilder();
        fasta.close();
    }
}
"#,
    );

    assert!(
        semantic.diagnostics.iter().all(|diagnostic| diagnostic.code
            != "external_pack_missing_constructor"
            && diagnostic.code != "external_pack_missing_api"),
        "modeled SAM reference constructors and getters should not report pack gaps"
    );

    let constructor_args = [
        JavaType::String,
        JavaType::Primitive(PrimitiveType::Int),
        JavaType::Array(Box::new(JavaType::Primitive(PrimitiveType::Byte))),
    ];
    let constructor = semantic
        .facts
        .external_constructor_for("ReferenceSequence", &constructor_args)
        .expect("semantic analysis should use the SAM reference pack for ReferenceSequence/3");
    assert_eq!(constructor.provenance, FactProvenance::ExternalLibraryPack);
    let walker_args = [JavaType::Named {
        name: "File".to_string(),
        args: Vec::new(),
    }];
    let walker = semantic
        .facts
        .external_constructor_for("ReferenceSequenceFileWalker", &walker_args)
        .expect(
            "semantic analysis should use the SAM reference pack for ReferenceSequenceFileWalker/1",
        );
    assert_eq!(walker.provenance, FactProvenance::ExternalLibraryPack);

    let bases = semantic
        .facts
        .external_api_effect_for_receiver(Some("ReferenceSequence"), "getBases")
        .expect("semantic analysis should model ReferenceSequence.getBases");
    assert_eq!(bases.provenance, FactProvenance::ExternalLibraryPack);
    assert_eq!(
        bases.return_type,
        Some(JavaType::Array(Box::new(JavaType::Primitive(
            PrimitiveType::Byte
        ))))
    );

    let next = semantic
        .facts
        .external_api_effect_for_receiver(Some("FastaSequenceFile"), "nextSequence")
        .expect("semantic analysis should model FastaSequenceFile.nextSequence");
    assert_eq!(next.provenance, FactProvenance::ExternalLibraryPack);
    assert_eq!(
        next.receiver_mutability,
        ReceiverMutabilityFact::MutatesReceiver
    );
    let dictionary = semantic
        .facts
        .external_api_effect_for_receiver(
            Some("ReferenceSequenceFileWalker"),
            "getSequenceDictionary",
        )
        .expect("semantic analysis should model ReferenceSequenceFileWalker.getSequenceDictionary");
    assert_eq!(dictionary.provenance, FactProvenance::ExternalLibraryPack);
    assert_eq!(
        dictionary.return_type,
        Some(JavaType::Named {
            name: "SAMSequenceDictionary".to_string(),
            args: Vec::new(),
        })
    );
    assert_eq!(dictionary.return_nullability, NullabilityState::MaybeNull);

    let get = semantic
        .facts
        .external_api_effect_for_receiver_with_arity(
            Some("ReferenceSequenceFileWalker"),
            "get",
            Some(1),
        )
        .expect("semantic analysis should model ReferenceSequenceFileWalker.get/1");
    assert_eq!(get.provenance, FactProvenance::ExternalLibraryPack);
    assert_eq!(
        get.return_type,
        Some(JavaType::Named {
            name: "ReferenceSequence".to_string(),
            args: Vec::new(),
        })
    );
    assert_eq!(get.return_nullability, NullabilityState::DefinitelyNonNull);
}

#[test]
fn htsjdk_tribble_feature_pack_declares_feature_boundary() {
    let facts = htsjdk_tribble_feature_pack_facts();

    let pack = facts
        .external_library_pack_for_type("htsjdk.tribble.Feature")
        .expect("Tribble Feature should be owned by the external pack");
    assert_eq!(pack.pack_name, "htsjdk.tribble.feature");
    assert_eq!(pack.mode, ExternalLibraryMode::ExternalFactModel);
    assert_eq!(pack.coverage, ExternalApiCoverage::Semantic);
    assert_eq!(pack.provenance, FactProvenance::ExternalLibraryPack);

    let reader_pack = facts
        .external_library_pack_for_type("FeatureReader")
        .expect("Tribble FeatureReader should be owned by the feature external pack");
    assert_eq!(reader_pack.pack_name, "htsjdk.tribble.feature");
    assert_eq!(reader_pack.mode, ExternalLibraryMode::ExternalFactModel);
    assert!(
        facts.has_object_representation_facts(&JavaType::Named {
            name: "FeatureReader".to_string(),
            args: Vec::new(),
        }),
        "anonymous FeatureReader implementations should have explicit erased-object facts"
    );
    assert!(facts.has_object_representation_facts(&JavaType::Named {
        name: "htsjdk.tribble.FeatureReader".to_string(),
        args: Vec::new(),
    }));

    let feature = facts
        .external_constructor_for(
            "SimpleFeature",
            &[
                JavaType::String,
                JavaType::Primitive(new_java2rust::java_ir::PrimitiveType::Int),
                JavaType::Primitive(new_java2rust::java_ir::PrimitiveType::Int),
            ],
        )
        .expect("SimpleFeature constructor should be pack-owned by arity");
    assert_eq!(feature.provenance, FactProvenance::ExternalLibraryPack);

    let args = vec![
        JavaType::String,
        JavaType::Primitive(PrimitiveType::Int),
        JavaType::Primitive(PrimitiveType::Int),
    ];
    assert_eq!(
        new_java2rust::stdlib::external_constructor_provenance("SimpleFeature", &args),
        FactProvenance::ConservativeFallback,
        "SimpleFeature/3 constructor precision should come from the tribble feature pack, not stdlib"
    );

    for (type_name, args) in [
        (
            "FeatureCodecHeader",
            vec![
                JavaType::Named {
                    name: "Object".to_string(),
                    args: Vec::new(),
                },
                JavaType::Primitive(PrimitiveType::Long),
            ],
        ),
        ("CodecLineParsingException", vec![JavaType::String]),
        (
            "Gff3BaseData",
            vec![
                JavaType::String,
                JavaType::String,
                JavaType::String,
                JavaType::Primitive(PrimitiveType::Int),
                JavaType::Primitive(PrimitiveType::Int),
                JavaType::String,
                JavaType::String,
                JavaType::String,
                JavaType::Named {
                    name: "Map".to_string(),
                    args: vec![JavaType::String, JavaType::String],
                },
            ],
        ),
    ] {
        let constructor = facts
            .external_constructor_for(type_name, &args)
            .unwrap_or_else(|| panic!("{type_name}/{} should be pack-owned", args.len()));
        assert_eq!(constructor.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(
            new_java2rust::stdlib::external_constructor_provenance(type_name, &args),
            FactProvenance::ConservativeFallback,
            "{type_name}/{} constructor precision should come from the tribble feature pack, not stdlib",
            args.len()
        );
    }

    let no_header_end = facts
        .external_field_for("FeatureCodecHeader", "NO_HEADER_END")
        .expect("FeatureCodecHeader.NO_HEADER_END should be pack-owned");
    assert_eq!(
        no_header_end.provenance,
        FactProvenance::ExternalLibraryPack
    );
    assert_eq!(
        no_header_end.field_type,
        Some(JavaType::Primitive(PrimitiveType::Long))
    );
    let dispatched =
        new_java2rust::external_packs::external_field_for("FeatureCodecHeader", "NO_HEADER_END")
            .expect("FeatureCodecHeader.NO_HEADER_END should be globally pack-dispatched");
    assert_eq!(dispatched.provenance, FactProvenance::ExternalLibraryPack);
    assert_eq!(
        new_java2rust::stdlib::external_field_provenance("FeatureCodecHeader", "NO_HEADER_END"),
        FactProvenance::ConservativeFallback,
        "FeatureCodecHeader.NO_HEADER_END precision should come from the tribble feature pack, not stdlib"
    );
}

#[test]
fn htsjdk_tribble_feature_pack_models_coordinate_getters() {
    let facts = htsjdk_tribble_feature_pack_facts();

    let contig = facts
        .external_api_effect_for_receiver(Some("Feature"), "getContig")
        .expect("Feature contig getter should be modeled by the pack");
    assert_eq!(contig.role, ExternalApiRole::Getter);
    assert_eq!(
        contig.return_nullability,
        NullabilityState::DefinitelyNonNull
    );
    assert_eq!(contig.provenance, FactProvenance::ExternalLibraryPack);

    let start = facts
        .external_api_effect_for_receiver(Some("SimpleFeature"), "getStart")
        .expect("SimpleFeature start getter should be modeled by the pack");
    assert_eq!(start.role, ExternalApiRole::Getter);
    assert_eq!(start.receiver_mutability, ReceiverMutabilityFact::ReadOnly);
    assert_eq!(
        start.return_nullability,
        NullabilityState::DefinitelyNonNull
    );

    let name = facts
        .external_api_effect_for_receiver(Some("NamedFeature"), "getName")
        .expect("NamedFeature name getter should be modeled by the pack");
    assert_eq!(name.return_nullability, NullabilityState::MaybeNull);

    let score = facts
        .external_api_effect_for_receiver(Some("BEDFeature"), "getScore")
        .expect("BEDFeature score getter should be modeled by the pack");
    assert_eq!(
        score.return_type,
        Some(JavaType::Primitive(
            new_java2rust::java_ir::PrimitiveType::Float
        ))
    );

    let header = facts
        .external_api_effect_for_receiver(Some("FeatureReader"), "getHeader")
        .expect("FeatureReader.getHeader should be modeled by the pack");
    assert_eq!(header.role, ExternalApiRole::Getter);
    assert_eq!(header.return_nullability, NullabilityState::MaybeNull);
    assert_eq!(
        header.return_type,
        Some(JavaType::Named {
            name: "VCFHeader".to_string(),
            args: Vec::new(),
        })
    );

    let query = facts
        .external_api_effect_for_receiver(Some("FeatureReader"), "query")
        .expect("FeatureReader.query should be modeled by the pack");
    assert_eq!(query.role, ExternalApiRole::Getter);
    assert_eq!(
        query.return_type,
        Some(JavaType::Named {
            name: "CloseableIterator".to_string(),
            args: vec![JavaType::Named {
                name: "Object".to_string(),
                args: Vec::new(),
            }],
        })
    );

    let sequence_names = facts
        .external_api_effect_for_receiver(Some("FeatureReader"), "getSequenceNames")
        .expect("FeatureReader.getSequenceNames should be modeled by the pack");
    assert_eq!(
        sequence_names.return_type,
        Some(JavaType::Named {
            name: "List".to_string(),
            args: vec![JavaType::String],
        })
    );

    let queryable = facts
        .external_api_effect_for_receiver(Some("FeatureReader"), "isQueryable")
        .expect("FeatureReader.isQueryable should be modeled by the pack");
    assert_eq!(queryable.role, ExternalApiRole::BooleanGetter);

    let close = facts
        .external_api_effect_for_receiver(Some("FeatureReader"), "close")
        .expect("FeatureReader.close should be modeled by the pack");
    assert_eq!(close.role, ExternalApiRole::SideEffect);
    assert_eq!(
        close.receiver_mutability,
        ReceiverMutabilityFact::MutatesReceiver
    );
}

#[test]
fn semantic_analysis_uses_tribble_feature_pack_for_coordinate_workflows() {
    let semantic = analyze(
        r#"
import htsjdk.tribble.Feature;
import htsjdk.tribble.SimpleFeature;
import htsjdk.tribble.bed.BEDFeature;

class TribbleFeaturePackUse {
    Feature make() {
        return new SimpleFeature("chr1", 1, 10);
    }

    String contig(Feature feature) {
        return feature.getContig();
    }

    int width(SimpleFeature feature) {
        return feature.getEnd() - feature.getStart();
    }

    boolean sameContig(Feature left, Feature right) {
        return left.contigsMatch(right);
    }

    float score(BEDFeature feature) {
        return feature.getScore();
    }
}
"#,
    );

    let contig = semantic
        .facts
        .external_api_effect_for_receiver(Some("Feature"), "getContig")
        .unwrap();
    assert_eq!(contig.provenance, FactProvenance::ExternalLibraryPack);
    assert_eq!(
        contig.return_nullability,
        NullabilityState::DefinitelyNonNull
    );

    let start = semantic
        .facts
        .external_api_effect_for_receiver(Some("SimpleFeature"), "getStart")
        .unwrap();
    assert_eq!(start.provenance, FactProvenance::ExternalLibraryPack);

    let contigs_match = semantic
        .facts
        .external_api_effect_for_receiver(Some("Feature"), "contigsMatch")
        .expect("Feature contig comparison predicate should be modeled by the pack");
    assert_eq!(contigs_match.role, ExternalApiRole::BooleanGetter);
    assert_eq!(
        contigs_match.return_nullability,
        NullabilityState::DefinitelyNonNull
    );

    let score = semantic
        .facts
        .external_api_effect_for_receiver(Some("BEDFeature"), "getScore")
        .expect("BEDFeature score getter should be modeled by the pack");
    assert_eq!(
        score.return_type,
        Some(JavaType::Primitive(
            new_java2rust::java_ir::PrimitiveType::Float
        ))
    );

    let feature = semantic
        .facts
        .external_constructor_for(
            "SimpleFeature",
            &[
                JavaType::String,
                JavaType::Primitive(new_java2rust::java_ir::PrimitiveType::Int),
                JavaType::Primitive(new_java2rust::java_ir::PrimitiveType::Int),
            ],
        )
        .unwrap();
    assert_eq!(feature.provenance, FactProvenance::ExternalLibraryPack);
}

#[test]
fn htsjdk_sam_reader_pack_declares_factory_option_nested_enum() {
    let facts = htsjdk_sam_reader_pack_facts();

    let pack = facts
        .external_library_pack_for_type("htsjdk.samtools.SamReaderFactory")
        .expect("SamReaderFactory should be owned by the SAM reader pack");
    assert_eq!(pack.pack_name, "htsjdk.samtools.reader");
    assert_eq!(pack.mode, ExternalLibraryMode::ExternalFactModel);
    assert_eq!(pack.coverage, ExternalApiCoverage::Semantic);
    assert_eq!(pack.provenance, FactProvenance::ExternalLibraryPack);

    assert!(
        facts.external_library_pack_for_type("Option").is_none(),
        "the SAM reader pack should not claim the ambiguous bare Option type"
    );

    let field = facts
        .external_field_for("SamReaderFactory", "Option")
        .expect("SamReaderFactory.Option should be pack-owned");
    assert_eq!(field.provenance, FactProvenance::ExternalLibraryPack);
    assert_eq!(
        field.field_type,
        Some(JavaType::Named {
            name: "Option".to_string(),
            args: Vec::new(),
        })
    );
    assert_eq!(
        new_java2rust::stdlib::external_field_provenance("SamReaderFactory", "Option"),
        FactProvenance::ConservativeFallback,
        "SamReaderFactory.Option precision should come from the SAM reader pack, not stdlib"
    );
    for field_name in ["CACHE_FILE_BASED_INDEXES", "INCLUDE_SOURCE_IN_RECORDS"] {
        let field = facts
            .external_field_for("SamReaderFactory.Option", field_name)
            .unwrap_or_else(|| panic!("SamReaderFactory.Option.{field_name} should be pack-owned"));
        assert_eq!(field.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(
            field.field_type,
            Some(JavaType::Named {
                name: "Option".to_string(),
                args: Vec::new(),
            })
        );
        let dispatched = new_java2rust::external_packs::external_field_for(
            "SamReaderFactory.Option",
            field_name,
        )
        .unwrap_or_else(|| {
            panic!("SamReaderFactory.Option.{field_name} should be globally pack-dispatched")
        });
        assert_eq!(dispatched.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(
            new_java2rust::stdlib::external_field_provenance("SamReaderFactory.Option", field_name),
            FactProvenance::ConservativeFallback,
            "SamReaderFactory.Option.{field_name} precision should come from the SAM reader pack, not stdlib"
        );
        assert_eq!(
            new_java2rust::stdlib::external_field_provenance("Option", field_name),
            FactProvenance::BuiltinExternalModel,
            "bare Option.{field_name} remains generic because the SAM reader pack does not own ambiguous bare Option"
        );
    }

    let validation_stringency_pack = facts
        .external_library_pack_for_type("ValidationStringency")
        .expect("ValidationStringency should be owned by the SAM reader pack");
    assert_eq!(
        validation_stringency_pack.pack_name,
        "htsjdk.samtools.reader"
    );
    for field_name in ["LENIENT", "SILENT", "STRICT"] {
        let field = facts
            .external_field_for("ValidationStringency", field_name)
            .unwrap_or_else(|| panic!("ValidationStringency.{field_name} should be pack-owned"));
        assert_eq!(field.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(
            field.field_type,
            Some(JavaType::Named {
                name: "ValidationStringency".to_string(),
                args: Vec::new(),
            })
        );
        let dispatched =
            new_java2rust::external_packs::external_field_for("ValidationStringency", field_name)
                .unwrap_or_else(|| {
                    panic!("ValidationStringency.{field_name} should be globally pack-dispatched")
                });
        assert_eq!(dispatched.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(
            new_java2rust::stdlib::external_field_provenance("ValidationStringency", field_name),
            FactProvenance::ConservativeFallback,
            "ValidationStringency.{field_name} precision should come from the SAM reader pack, not stdlib"
        );
    }

    let sort_order = JavaType::Named {
        name: "SortOrder".to_string(),
        args: Vec::new(),
    };
    let header_collection = JavaType::Named {
        name: "Collection".to_string(),
        args: vec![JavaType::Named {
            name: "SAMFileHeader".to_string(),
            args: Vec::new(),
        }],
    };
    let reader_collection = JavaType::Named {
        name: "Collection".to_string(),
        args: vec![JavaType::Named {
            name: "SamReader".to_string(),
            args: Vec::new(),
        }],
    };
    let header_merger = JavaType::Named {
        name: "SamFileHeaderMerger".to_string(),
        args: Vec::new(),
    };

    for (type_name, args) in [
        (
            "SamFileHeaderMerger",
            vec![
                sort_order,
                header_collection,
                JavaType::Primitive(PrimitiveType::Boolean),
            ],
        ),
        (
            "MergingSamRecordIterator",
            vec![
                header_merger,
                reader_collection,
                JavaType::Primitive(PrimitiveType::Boolean),
            ],
        ),
    ] {
        let constructor = facts
            .external_constructor_for(type_name, &args)
            .unwrap_or_else(|| panic!("{type_name}/{} should be pack-owned", args.len()));
        assert_eq!(constructor.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(
            new_java2rust::stdlib::external_constructor_provenance(type_name, &args),
            FactProvenance::ConservativeFallback,
            "{type_name}/{} precision should come from the SAM reader pack, not stdlib",
            args.len()
        );
    }

    for (receiver, method_name, expected_type) in [
        ("SamReaderFactory", "makeDefault", "SamReaderFactory"),
        ("SamReaderFactory", "open", "SamReader"),
        ("SamReaderFactory", "referenceSequence", "SamReaderFactory"),
        ("SamFileHeaderMerger", "getMergedHeader", "SAMFileHeader"),
    ] {
        let effect = facts
            .external_api_effect_for_receiver(Some(receiver), method_name)
            .unwrap_or_else(|| panic!("{receiver}.{method_name} should be modeled"));
        assert_eq!(effect.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(
            effect.return_type,
            Some(JavaType::Named {
                name: expected_type.to_string(),
                args: Vec::new(),
            })
        );
        assert_eq!(
            effect.return_nullability,
            NullabilityState::DefinitelyNonNull
        );
        if method_name == "referenceSequence" {
            assert_eq!(effect.role, ExternalApiRole::FluentMutator);
            assert_eq!(
                effect.receiver_mutability,
                ReceiverMutabilityFact::MutatesReceiver
            );
        }
    }
}

#[test]
fn htsjdk_sam_writer_pack_declares_file_writer_factory_boundary() {
    let facts = htsjdk_sam_writer_pack_facts();

    let pack = facts
        .external_library_pack_for_type("htsjdk.samtools.SAMFileWriterFactory")
        .expect("SAMFileWriterFactory should be owned by the SAM writer pack");
    assert_eq!(pack.pack_name, "htsjdk.samtools.writer");
    assert_eq!(pack.mode, ExternalLibraryMode::ExternalFactModel);
    assert_eq!(pack.coverage, ExternalApiCoverage::Semantic);
    assert_eq!(pack.provenance, FactProvenance::ExternalLibraryPack);

    let constructor = facts
        .external_constructor_for("SAMFileWriterFactory", &[])
        .expect("SAMFileWriterFactory/0 constructor should be pack-owned");
    assert_eq!(constructor.provenance, FactProvenance::ExternalLibraryPack);
    assert_eq!(
        new_java2rust::stdlib::external_constructor_provenance("SAMFileWriterFactory", &[]),
        FactProvenance::ConservativeFallback,
        "SAMFileWriterFactory/0 precision should come from the SAM writer pack, not stdlib"
    );

    let make_writer = facts
        .external_api_effect_for_receiver(Some("SAMFileWriterFactory"), "makeWriter")
        .expect("SAMFileWriterFactory.makeWriter should be modeled");
    assert_eq!(make_writer.provenance, FactProvenance::ExternalLibraryPack);
    assert_eq!(make_writer.role, ExternalApiRole::Getter);
    assert_eq!(
        make_writer.return_type,
        Some(JavaType::Named {
            name: "SAMFileWriter".to_string(),
            args: Vec::new(),
        })
    );
    assert_eq!(
        make_writer.return_nullability,
        NullabilityState::DefinitelyNonNull
    );

    for method_name in ["setCreateIndex", "setCreateMd5File"] {
        let effect = facts
            .external_api_effect_for_receiver(Some("SAMFileWriterFactory"), method_name)
            .unwrap_or_else(|| panic!("SAMFileWriterFactory.{method_name} should be modeled"));
        assert_eq!(effect.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(effect.role, ExternalApiRole::FluentMutator);
        assert_eq!(
            effect.receiver_mutability,
            ReceiverMutabilityFact::MutatesReceiver
        );
        assert_eq!(
            effect.return_type,
            Some(JavaType::Named {
                name: "SAMFileWriterFactory".to_string(),
                args: Vec::new(),
            })
        );
    }
}

#[test]
fn semantic_analysis_uses_sam_reader_pack_for_factory_methods() {
    let semantic = analyze(
        r#"
import htsjdk.samtools.SAMFileHeader;
import htsjdk.samtools.SamFileHeaderMerger;
import htsjdk.samtools.SamReader;
import htsjdk.samtools.SamReaderFactory;

class SamReaderFactoryUse {
    SamReader open(Object path, Object reference) {
        SamReaderFactory factory = SamReaderFactory.makeDefault();
        return factory.referenceSequence(reference).open(path);
    }

    SAMFileHeader header(SamFileHeaderMerger merger) {
        return merger.getMergedHeader();
    }
}
"#,
    );

    assert!(
        semantic
            .diagnostics
            .iter()
            .all(|diagnostic| diagnostic.code != "external_pack_missing_api"),
        "modeled SAM reader factory helpers should not report pack gaps: {:?}",
        semantic.diagnostics
    );

    for (receiver, method_name) in [
        ("SamReaderFactory", "makeDefault"),
        ("SamReaderFactory", "referenceSequence"),
        ("SamReaderFactory", "open"),
        ("SamFileHeaderMerger", "getMergedHeader"),
    ] {
        let effect = semantic
            .facts
            .external_api_effect_for_receiver(Some(receiver), method_name)
            .unwrap_or_else(|| panic!("semantic analysis should model {receiver}.{method_name}"));
        assert_eq!(effect.provenance, FactProvenance::ExternalLibraryPack);
    }
}

#[test]
fn semantic_analysis_uses_sam_writer_pack_for_factory_method() {
    let semantic = analyze(
        r#"
import htsjdk.samtools.SAMFileHeader;
import htsjdk.samtools.SAMFileWriter;
import htsjdk.samtools.SAMFileWriterFactory;

class SamWriterFactoryUse {
    SAMFileWriter writer(SAMFileWriterFactory factory, SAMFileHeader header, Object output) {
        factory.setCreateIndex(true);
        factory.setCreateMd5File(true);
        return factory.makeWriter(header, true, output, null);
    }
}
"#,
    );

    assert!(
        semantic
            .diagnostics
            .iter()
            .all(|diagnostic| diagnostic.code != "external_pack_missing_api"),
        "modeled SAM writer factory helper should not report pack gaps: {:?}",
        semantic.diagnostics
    );

    let effect = semantic
        .facts
        .external_api_effect_for_receiver(Some("SAMFileWriterFactory"), "makeWriter")
        .expect("semantic analysis should model SAMFileWriterFactory.makeWriter");
    assert_eq!(effect.provenance, FactProvenance::ExternalLibraryPack);

    for method_name in ["setCreateIndex", "setCreateMd5File"] {
        let effect = semantic
            .facts
            .external_api_effect_for_receiver(Some("SAMFileWriterFactory"), method_name)
            .unwrap_or_else(|| {
                panic!("semantic analysis should model SAMFileWriterFactory.{method_name}")
            });
        assert_eq!(effect.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(effect.role, ExternalApiRole::FluentMutator);
    }
}

#[test]
fn htsjdk_sam_bam_pack_declares_codec_and_indexer_boundaries() {
    let facts = htsjdk_sam_bam_pack_facts();

    let pack = facts
        .external_library_pack_for_type("htsjdk.samtools.BAMRecordCodec")
        .expect("BAMRecordCodec should be owned by the SAM BAM pack");
    assert_eq!(pack.pack_name, "htsjdk.samtools.bam");
    assert_eq!(pack.mode, ExternalLibraryMode::ExternalFactModel);
    assert_eq!(pack.coverage, ExternalApiCoverage::Semantic);
    assert_eq!(pack.provenance, FactProvenance::ExternalLibraryPack);

    for (type_name, args) in [
        (
            "BAMRecordCodec",
            vec![JavaType::Named {
                name: "SAMFileHeader".to_string(),
                args: Vec::new(),
            }],
        ),
        (
            "BAMIndexer",
            vec![
                JavaType::Named {
                    name: "File".to_string(),
                    args: Vec::new(),
                },
                JavaType::Named {
                    name: "SAMFileHeader".to_string(),
                    args: Vec::new(),
                },
            ],
        ),
        (
            "SBIIndexWriter",
            vec![
                JavaType::Named {
                    name: "OutputStream".to_string(),
                    args: Vec::new(),
                },
                JavaType::Primitive(PrimitiveType::Int),
            ],
        ),
    ] {
        let constructor = facts
            .external_constructor_for(type_name, &args)
            .unwrap_or_else(|| panic!("{type_name}/{} should be pack-owned", args.len()));
        assert_eq!(constructor.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(
            new_java2rust::stdlib::external_constructor_provenance(type_name, &args),
            FactProvenance::ConservativeFallback,
            "{type_name}/{} precision should come from the SAM BAM pack, not stdlib",
            args.len()
        );
    }

    let default_granularity = facts
        .external_field_for("SBIIndexWriter", "DEFAULT_GRANULARITY")
        .expect("SBIIndexWriter.DEFAULT_GRANULARITY should be pack-owned");
    assert_eq!(
        default_granularity.provenance,
        FactProvenance::ExternalLibraryPack
    );
    assert_eq!(
        default_granularity.field_type,
        Some(JavaType::Primitive(PrimitiveType::Long))
    );
    let dispatched_default_granularity = new_java2rust::external_packs::external_field_for(
        "s_b_i_index_writer",
        "DEFAULT_GRANULARITY",
    )
    .expect("SBIIndexWriter.DEFAULT_GRANULARITY should be available through global pack dispatch");
    assert_eq!(
        dispatched_default_granularity.provenance,
        FactProvenance::ExternalLibraryPack
    );
    assert_eq!(
        dispatched_default_granularity.field_type,
        Some(JavaType::Primitive(PrimitiveType::Long))
    );
    assert_eq!(
        new_java2rust::stdlib::external_field_provenance("SBIIndexWriter", "DEFAULT_GRANULARITY"),
        FactProvenance::ConservativeFallback,
        "SBIIndexWriter.DEFAULT_GRANULARITY precision should come from the SAM BAM pack, not stdlib"
    );

    let fixed_block_size = facts
        .external_field_for("BAMFileConstants", "FIXED_BLOCK_SIZE")
        .expect("BAMFileConstants.FIXED_BLOCK_SIZE should be pack-owned");
    assert_eq!(
        fixed_block_size.provenance,
        FactProvenance::ExternalLibraryPack
    );
    assert_eq!(
        fixed_block_size.field_type,
        Some(JavaType::Primitive(PrimitiveType::Int))
    );
    let dispatched_fixed_block_size = new_java2rust::external_packs::external_field_for(
        "BAMFileConstants",
        "FIXED_BLOCK_SIZE",
    )
    .expect("BAMFileConstants.FIXED_BLOCK_SIZE should be available through global pack dispatch");
    assert_eq!(
        dispatched_fixed_block_size.provenance,
        FactProvenance::ExternalLibraryPack
    );
    assert_eq!(
        dispatched_fixed_block_size.field_type,
        Some(JavaType::Primitive(PrimitiveType::Int))
    );
    assert_eq!(
        new_java2rust::stdlib::external_field_provenance("BAMFileConstants", "FIXED_BLOCK_SIZE"),
        FactProvenance::ConservativeFallback,
        "BAMFileConstants.FIXED_BLOCK_SIZE precision should come from the SAM BAM pack, not stdlib"
    );
}

#[test]
fn htsjdk_sam_cram_pack_declares_container_and_reference_boundaries() {
    let facts = htsjdk_sam_cram_pack_facts();

    let pack = facts
        .external_library_pack_for_type("htsjdk.samtools.cram.build.CramContainerIterator")
        .expect("CramContainerIterator should be owned by the SAM CRAM pack");
    assert_eq!(pack.pack_name, "htsjdk.samtools.cram");
    assert_eq!(pack.mode, ExternalLibraryMode::ExternalFactModel);
    assert_eq!(pack.coverage, ExternalApiCoverage::Semantic);
    assert_eq!(pack.provenance, FactProvenance::ExternalLibraryPack);

    for (type_name, args) in [
        (
            "CramContainerIterator",
            vec![JavaType::Named {
                name: "InputStream".to_string(),
                args: Vec::new(),
            }],
        ),
        (
            "Container",
            vec![
                JavaType::Named {
                    name: "CramHeader".to_string(),
                    args: Vec::new(),
                },
                JavaType::Named {
                    name: "CompressionHeader".to_string(),
                    args: Vec::new(),
                },
                JavaType::Named {
                    name: "List".to_string(),
                    args: vec![JavaType::Named {
                        name: "Slice".to_string(),
                        args: Vec::new(),
                    }],
                },
            ],
        ),
        (
            "CRAMReferenceRegion",
            vec![
                JavaType::Named {
                    name: "ReferenceSource".to_string(),
                    args: Vec::new(),
                },
                JavaType::Named {
                    name: "SAMSequenceDictionary".to_string(),
                    args: Vec::new(),
                },
            ],
        ),
    ] {
        let constructor = facts
            .external_constructor_for(type_name, &args)
            .unwrap_or_else(|| panic!("{type_name}/{} should be pack-owned", args.len()));
        assert_eq!(constructor.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(
            new_java2rust::stdlib::external_constructor_provenance(type_name, &args),
            FactProvenance::ConservativeFallback,
            "{type_name}/{} precision should come from the SAM CRAM pack, not stdlib",
            args.len()
        );
    }

    let data_series_pack = facts
        .external_library_pack_for_type("DataSeriesType")
        .expect("DataSeriesType should be owned by the SAM CRAM pack");
    assert_eq!(data_series_pack.pack_name, "htsjdk.samtools.cram");
    let byte_array = facts
        .external_field_for("DataSeriesType", "BYTE_ARRAY")
        .expect("DataSeriesType.BYTE_ARRAY should be pack-owned");
    assert_eq!(byte_array.provenance, FactProvenance::ExternalLibraryPack);
    assert_eq!(
        byte_array.field_type,
        Some(JavaType::Named {
            name: "DataSeriesType".to_string(),
            args: Vec::new(),
        })
    );
    let dispatched_byte_array =
        new_java2rust::external_packs::external_field_for("DataSeriesType", "BYTE_ARRAY")
            .expect("DataSeriesType.BYTE_ARRAY should be globally pack-dispatched");
    assert_eq!(
        dispatched_byte_array.provenance,
        FactProvenance::ExternalLibraryPack
    );
    assert_eq!(
        new_java2rust::stdlib::external_field_provenance("DataSeriesType", "BYTE_ARRAY"),
        FactProvenance::ConservativeFallback,
        "DataSeriesType.BYTE_ARRAY precision should come from the SAM CRAM pack, not stdlib"
    );
}

#[test]
fn htsjdk_sam_metrics_pack_declares_metrics_file_boundary() {
    let facts = htsjdk_sam_metrics_pack_facts();

    let pack = facts
        .external_library_pack_for_type("htsjdk.samtools.metrics.MetricsFile")
        .expect("MetricsFile should be owned by the SAM metrics pack");
    assert_eq!(pack.pack_name, "htsjdk.samtools.metrics");
    assert_eq!(pack.mode, ExternalLibraryMode::ExternalFactModel);
    assert_eq!(pack.coverage, ExternalApiCoverage::Semantic);
    assert_eq!(pack.provenance, FactProvenance::ExternalLibraryPack);

    let constructor = facts
        .external_constructor_for("MetricsFile", &[])
        .expect("MetricsFile/0 constructor should be pack-owned");
    assert_eq!(constructor.provenance, FactProvenance::ExternalLibraryPack);
    assert_eq!(
        new_java2rust::stdlib::external_constructor_provenance("MetricsFile", &[]),
        FactProvenance::ConservativeFallback,
        "MetricsFile/0 precision should come from the SAM metrics pack, not stdlib"
    );
}

#[test]
fn htsjdk_nested_helper_pack_declares_component_methods_boundary() {
    let facts = htsjdk_nested_helper_pack_facts();

    let pack = facts
        .external_library_pack_for_type("ComponentMethods")
        .expect("ComponentMethods should be owned by the HTSJDK nested-helper pack");
    assert_eq!(pack.pack_name, "htsjdk.nested_helper");
    assert_eq!(pack.mode, ExternalLibraryMode::ExternalFactModel);
    assert_eq!(pack.coverage, ExternalApiCoverage::Semantic);
    assert_eq!(pack.provenance, FactProvenance::ExternalLibraryPack);

    let constructor = facts
        .external_constructor_for("ComponentMethods", &[])
        .expect("ComponentMethods/0 constructor should be pack-owned");
    assert_eq!(constructor.provenance, FactProvenance::ExternalLibraryPack);
    assert_eq!(
        new_java2rust::stdlib::external_constructor_provenance("ComponentMethods", &[]),
        FactProvenance::ConservativeFallback,
        "ComponentMethods/0 precision should come from the nested-helper pack, not stdlib"
    );
}

#[test]
fn apache_commons_collections_pack_declares_iterator_and_map_constructor_boundary() {
    let facts = apache_commons_collections_pack_facts();

    let pack = facts
        .external_library_pack_for_type("org.apache.commons.collections4.map.DefaultedMap")
        .expect("DefaultedMap should be owned by the Apache Commons Collections pack");
    assert_eq!(pack.pack_name, "org.apache.commons.collections4");
    assert_eq!(pack.mode, ExternalLibraryMode::ExternalFactModel);
    assert_eq!(pack.coverage, ExternalApiCoverage::Semantic);
    assert_eq!(pack.provenance, FactProvenance::ExternalLibraryPack);

    for (type_name, arg_len) in [
        ("DefaultedMap", 1),
        ("FilterIterator", 2),
        ("SingletonIterator", 1),
        ("MultiMap", 0),
    ] {
        let args = vec![JavaType::Unknown("Object".to_string()); arg_len];
        let constructor = facts
            .external_constructor_for(type_name, &args)
            .unwrap_or_else(|| panic!("{type_name}/{arg_len} should be pack-owned"));
        assert_eq!(constructor.provenance, FactProvenance::ExternalLibraryPack);
        let dispatched = new_java2rust::external_packs::external_constructor_for(type_name, &args)
            .unwrap_or_else(|| panic!("{type_name}/{arg_len} should be globally pack-dispatched"));
        assert_eq!(dispatched.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(
            new_java2rust::stdlib::external_constructor_provenance(type_name, &args),
            FactProvenance::ConservativeFallback,
            "{type_name}/{arg_len} precision should come from the Apache Commons Collections pack, not stdlib"
        );
    }

    let append = facts
        .external_api_effect_for_receiver(Some("MultiMap"), "append")
        .expect("MultiMap.append should be modeled by the Commons Collections pack");
    assert_eq!(append.provenance, FactProvenance::ExternalLibraryPack);
    assert_eq!(append.role, ExternalApiRole::Mutator);
    assert_eq!(
        append.receiver_mutability,
        ReceiverMutabilityFact::MutatesReceiver
    );

    let get = facts
        .external_api_effect_for_receiver(Some("MultiMap"), "get")
        .expect("MultiMap.get should be modeled by the Commons Collections pack");
    assert_eq!(get.provenance, FactProvenance::ExternalLibraryPack);
    assert_eq!(get.role, ExternalApiRole::IndexedGetter);
    assert_eq!(get.return_nullability, NullabilityState::MaybeNull);
}

#[test]
fn semantic_analysis_uses_commons_collections_pack_for_multimap_helpers() {
    let semantic = analyze(
        r#"
import org.apache.commons.collections4.MultiMap;

class CommonsCollectionsUse {
    Object select(MultiMap map, Object key, Object value) {
        map.append(key, value);
        return map.get(key);
    }
}
"#,
    );

    assert!(
        semantic
            .diagnostics
            .iter()
            .all(|diagnostic| diagnostic.code != "external_pack_missing_api"),
        "modeled MultiMap helpers should not report pack gaps: {:?}",
        semantic.diagnostics
    );

    for method_name in ["append", "get"] {
        let effect = semantic
            .facts
            .external_api_effect_for_receiver(Some("MultiMap"), method_name)
            .unwrap_or_else(|| panic!("semantic analysis should model MultiMap.{method_name}"));
        assert_eq!(effect.provenance, FactProvenance::ExternalLibraryPack);
    }
}

#[test]
fn apache_commons_compress_and_lang_packs_declare_helper_constructor_boundaries() {
    let compress_facts = apache_commons_compress_pack_facts();
    let compress_pack = compress_facts
        .external_library_pack_for_type("org.apache.commons.compress.archivers.tar.TarArchiveEntry")
        .expect("TarArchiveEntry should be owned by the Apache Commons Compress pack");
    assert_eq!(compress_pack.pack_name, "org.apache.commons.compress");
    assert_eq!(compress_pack.mode, ExternalLibraryMode::ExternalFactModel);
    assert_eq!(compress_pack.coverage, ExternalApiCoverage::Semantic);
    assert_eq!(
        compress_pack.provenance,
        FactProvenance::ExternalLibraryPack
    );

    for (type_name, arg_len) in [
        ("GzipCompressorInputStream", 1),
        ("GzipCompressorOutputStream", 1),
        ("NullInputStream", 0),
        ("NullInputStream", 1),
        ("NullOutputStream", 0),
        ("NullOutputStream", 1),
        ("TarArchiveInputStream", 1),
        ("TarArchiveOutputStream", 1),
        ("TarArchiveEntry", 1),
        ("TarArchiveEntry", 2),
    ] {
        let args = vec![JavaType::Unknown("Object".to_string()); arg_len];
        let constructor = compress_facts
            .external_constructor_for(type_name, &args)
            .unwrap_or_else(|| panic!("{type_name}/{arg_len} should be pack-owned"));
        assert_eq!(constructor.provenance, FactProvenance::ExternalLibraryPack);
        let dispatched = new_java2rust::external_packs::external_constructor_for(type_name, &args)
            .unwrap_or_else(|| panic!("{type_name}/{arg_len} should be globally pack-dispatched"));
        assert_eq!(dispatched.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(
            new_java2rust::stdlib::external_constructor_provenance(type_name, &args),
            FactProvenance::ConservativeFallback,
            "{type_name}/{arg_len} precision should come from the Apache Commons Compress pack, not stdlib"
        );
    }
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
        (
            "NullWriter",
            "INSTANCE",
            JavaType::Named {
                name: "Writer".to_string(),
                args: Vec::new(),
            },
        ),
    ] {
        let field = compress_facts
            .external_field_for(type_name, field_name)
            .unwrap_or_else(|| panic!("{type_name}.{field_name} should be pack-owned"));
        assert_eq!(field.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(field.field_type, Some(expected_ty));
        let dispatched = new_java2rust::external_packs::external_field_for(type_name, field_name)
            .unwrap_or_else(|| {
                panic!("{type_name}.{field_name} should be globally pack-dispatched")
            });
        assert_eq!(dispatched.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(
            new_java2rust::stdlib::external_field_provenance(type_name, field_name),
            FactProvenance::ConservativeFallback,
            "{type_name}.{field_name} precision should come from the Apache Commons Compress pack, not stdlib"
        );
    }

    let lang_facts = apache_commons_lang_pack_facts();
    let lang_pack = lang_facts
        .external_library_pack_for_type("org.apache.commons.lang3.builder.HashCodeBuilder")
        .expect("HashCodeBuilder should be owned by the Apache Commons Lang pack");
    assert_eq!(lang_pack.pack_name, "org.apache.commons.lang3");
    assert_eq!(lang_pack.mode, ExternalLibraryMode::ExternalFactModel);
    assert_eq!(lang_pack.coverage, ExternalApiCoverage::Semantic);
    assert_eq!(lang_pack.provenance, FactProvenance::ExternalLibraryPack);
    assert_eq!(
        lang_facts.exception_kind_for("NotImplementedException"),
        Some(ExceptionKind::Unchecked)
    );

    for (type_name, arg_len) in [
        ("HashCodeBuilder", 0),
        ("MutableInt", 0),
        ("MutableInt", 1),
        ("MutableLong", 0),
        ("MutableLong", 1),
        ("MutableDouble", 0),
        ("MutableDouble", 1),
        ("NotImplementedException", 1),
        ("NotImplementedException", 2),
        ("Pair", 2),
        ("ImmutablePair", 2),
        ("MutablePair", 2),
        ("Triple", 3),
        ("ImmutableTriple", 3),
        ("MutableTriple", 3),
    ] {
        let args = vec![JavaType::Unknown("Object".to_string()); arg_len];
        let constructor = lang_facts
            .external_constructor_for(type_name, &args)
            .unwrap_or_else(|| panic!("{type_name}/{arg_len} should be pack-owned"));
        assert_eq!(constructor.provenance, FactProvenance::ExternalLibraryPack);
        let dispatched = new_java2rust::external_packs::external_constructor_for(type_name, &args)
            .unwrap_or_else(|| panic!("{type_name}/{arg_len} should be globally pack-dispatched"));
        assert_eq!(dispatched.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(
            new_java2rust::stdlib::external_constructor_provenance(type_name, &args),
            FactProvenance::ConservativeFallback,
            "{type_name}/{arg_len} precision should come from the Apache Commons Lang pack, not stdlib"
        );
    }

    for (type_name, method_name, arg_len) in [
        ("Pair", "of", 2),
        ("ImmutablePair", "of", 2),
        ("Pair", "getLeft", 0),
        ("Pair", "getRight", 0),
        ("MutablePair", "getLeft", 0),
        ("MutablePair", "getRight", 0),
        ("Triple", "getLeft", 0),
        ("Triple", "getMiddle", 0),
        ("Triple", "getRight", 0),
        ("MutableTriple", "getLeft", 0),
        ("MutableTriple", "getMiddle", 0),
        ("MutableTriple", "getRight", 0),
    ] {
        let effect = lang_facts
            .external_api_effect_for_receiver_with_arity(
                Some(type_name),
                method_name,
                Some(arg_len),
            )
            .unwrap_or_else(|| panic!("{type_name}.{method_name}/{arg_len} should be pack-owned"));
        assert_eq!(effect.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(effect.role, ExternalApiRole::Getter);
        assert_eq!(effect.receiver_mutability, ReceiverMutabilityFact::ReadOnly);
        assert_eq!(
            new_java2rust::stdlib::external_api_method_policy(
                Some(type_name),
                method_name,
                arg_len
            )
            .provenance,
            FactProvenance::ConservativeFallback,
            "{type_name}.{method_name}/{arg_len} precision should come from the Apache Commons Lang pack, not stdlib"
        );
    }

    let mutable_int_value = lang_facts
        .external_api_effect_for_receiver(Some("MutableInt"), "getValue")
        .expect("MutableInt.getValue should be pack-owned");
    assert_eq!(
        mutable_int_value.return_type,
        Some(JavaType::Primitive(PrimitiveType::Int))
    );
    assert_eq!(mutable_int_value.role, ExternalApiRole::Getter);

    let mutable_int_double = lang_facts
        .external_api_effect_for_receiver(Some("MutableInt"), "doubleValue")
        .expect("MutableInt.doubleValue should be pack-owned");
    assert_eq!(
        mutable_int_double.return_type,
        Some(JavaType::Primitive(PrimitiveType::Double))
    );
    assert_eq!(mutable_int_double.role, ExternalApiRole::Getter);

    for method_name in ["increment", "decrement"] {
        let effect = lang_facts
            .external_api_effect_for_receiver(Some("MutableInt"), method_name)
            .unwrap_or_else(|| panic!("MutableInt.{method_name} should be pack-owned"));
        assert_eq!(effect.role, ExternalApiRole::Mutator);
        assert_eq!(
            effect.receiver_mutability,
            ReceiverMutabilityFact::MutatesReceiver
        );
    }

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
        let field = lang_facts
            .external_field_for(type_name, field_name)
            .unwrap_or_else(|| panic!("{type_name}.{field_name} should be pack-owned"));
        assert_eq!(field.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(field.field_type, Some(expected_ty));
        let dispatched = new_java2rust::external_packs::external_field_for(type_name, field_name)
            .unwrap_or_else(|| {
                panic!("{type_name}.{field_name} should be globally pack-dispatched")
            });
        assert_eq!(dispatched.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(
            new_java2rust::stdlib::external_field_provenance(type_name, field_name),
            FactProvenance::ConservativeFallback,
            "{type_name}.{field_name} precision should come from the Apache Commons Lang pack, not stdlib"
        );
    }
}

#[test]
fn semantic_analysis_uses_commons_lang_pack_for_mutable_int() {
    let semantic = analyze(
        r#"
import org.apache.commons.lang3.mutable.MutableInt;

class CommonsLangMutableCounterUse {
    double score(int start) {
        MutableInt counter = new MutableInt(start);
        counter.increment();
        counter.decrement();
        return counter.doubleValue() + counter.getValue();
    }
}
"#,
    );

    assert!(
        semantic.diagnostics.iter().all(|diagnostic| diagnostic.code
            != "external_pack_missing_constructor"
            && diagnostic.code != "external_pack_missing_api"),
        "modeled MutableInt constructor and methods should not report pack gaps: {:?}",
        semantic.diagnostics
    );

    for method_name in ["doubleValue", "getValue", "increment", "decrement"] {
        let effect = semantic
            .facts
            .external_api_effect_for_receiver(Some("MutableInt"), method_name)
            .unwrap_or_else(|| panic!("semantic analysis should model MutableInt.{method_name}"));
        assert_eq!(effect.provenance, FactProvenance::ExternalLibraryPack);
    }
}

#[test]
fn apache_commons_codec_pack_declares_digest_algorithm_constants() {
    let facts = apache_commons_codec_pack_facts();

    let pack = facts
        .external_library_pack_for_type("org.apache.commons.codec.digest.MessageDigestAlgorithms")
        .expect("MessageDigestAlgorithms should be owned by the Apache Commons Codec pack");
    assert_eq!(pack.pack_name, "org.apache.commons.codec");
    assert_eq!(pack.mode, ExternalLibraryMode::ExternalFactModel);
    assert_eq!(pack.coverage, ExternalApiCoverage::Semantic);
    assert_eq!(pack.provenance, FactProvenance::ExternalLibraryPack);

    let field = facts
        .external_field_for("MessageDigestAlgorithms", "SHA_256")
        .expect("MessageDigestAlgorithms.SHA_256 should be pack-owned");
    assert_eq!(field.provenance, FactProvenance::ExternalLibraryPack);
    assert_eq!(field.field_type, Some(JavaType::String));
    let dispatched =
        new_java2rust::external_packs::external_field_for("MessageDigestAlgorithms", "SHA_256")
            .expect("MessageDigestAlgorithms.SHA_256 should be globally pack-dispatched");
    assert_eq!(dispatched.provenance, FactProvenance::ExternalLibraryPack);
    assert_eq!(
        new_java2rust::stdlib::external_field_provenance("MessageDigestAlgorithms", "SHA_256"),
        FactProvenance::ConservativeFallback,
        "MessageDigestAlgorithms.SHA_256 precision should come from the Apache Commons Codec pack, not stdlib"
    );
}

#[test]
fn apache_commons_math_pack_declares_remaining_math_constructor_boundaries() {
    let facts = apache_commons_math_pack_facts();

    let pack = facts
        .external_library_pack_for_type("org.apache.commons.math3.linear.Array2DRowRealMatrix")
        .expect("Array2DRowRealMatrix should be owned by the Apache Commons Math pack");
    assert_eq!(pack.pack_name, "org.apache.commons.math3");
    assert_eq!(pack.mode, ExternalLibraryMode::ExternalFactModel);
    assert_eq!(pack.coverage, ExternalApiCoverage::Semantic);
    assert_eq!(pack.provenance, FactProvenance::ExternalLibraryPack);

    for (type_name, arg_len) in [
        ("Array2DRowRealMatrix", 1),
        ("Array2DRowRealMatrix", 2),
        ("ArrayRealVector", 0),
        ("ArrayRealVector", 1),
        ("ArrayRealVector", 2),
        ("DefaultRealMatrixChangingVisitor", 0),
        ("Mean", 0),
        ("BinomialDistribution", 2),
        ("BinomialDistribution", 3),
        ("BetaDistribution", 3),
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
        ("TDistribution", 1),
        ("TDistribution", 2),
    ] {
        let args = vec![JavaType::Unknown("Object".to_string()); arg_len];
        let constructor = facts
            .external_constructor_for(type_name, &args)
            .unwrap_or_else(|| panic!("{type_name}/{arg_len} should be pack-owned"));
        assert_eq!(constructor.provenance, FactProvenance::ExternalLibraryPack);
        let dispatched = new_java2rust::external_packs::external_constructor_for(type_name, &args)
            .unwrap_or_else(|| panic!("{type_name}/{arg_len} should be globally pack-dispatched"));
        assert_eq!(dispatched.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(
            new_java2rust::stdlib::external_constructor_provenance(type_name, &args),
            FactProvenance::ConservativeFallback,
            "{type_name}/{arg_len} precision should come from the Apache Commons Math pack, not stdlib"
        );
    }

    let optim_facts = apache_commons_math_optim_pack_facts();
    let brent_args = vec![JavaType::Unknown("Object".to_string()); 2];
    let brent = optim_facts
        .external_constructor_for("BrentOptimizer", &brent_args)
        .expect("BrentOptimizer/2 should be owned by the Apache Commons Math optim pack");
    assert_eq!(brent.provenance, FactProvenance::ExternalLibraryPack);
    assert_eq!(
        new_java2rust::stdlib::external_constructor_provenance("BrentOptimizer", &brent_args),
        FactProvenance::ConservativeFallback,
        "BrentOptimizer/2 precision should come from the optim pack, not stdlib"
    );
}

#[test]
fn jama_and_hipparchus_packs_declare_matrix_and_empirical_distribution_boundaries() {
    let jama_facts = jama_pack_facts();
    let jama_pack = jama_facts
        .external_library_pack_for_type("Jama.Matrix")
        .expect("Jama.Matrix should be owned by the Jama pack");
    assert_eq!(jama_pack.pack_name, "Jama");
    assert_eq!(jama_pack.mode, ExternalLibraryMode::ExternalFactModel);
    assert_eq!(jama_pack.coverage, ExternalApiCoverage::Semantic);
    assert_eq!(jama_pack.provenance, FactProvenance::ExternalLibraryPack);

    for arg_len in [1, 2] {
        let args = vec![JavaType::Unknown("Object".to_string()); arg_len];
        let constructor = jama_facts
            .external_constructor_for("Matrix", &args)
            .unwrap_or_else(|| panic!("Matrix/{arg_len} should be pack-owned"));
        assert_eq!(constructor.provenance, FactProvenance::ExternalLibraryPack);
        let dispatched = new_java2rust::external_packs::external_constructor_for("Matrix", &args)
            .unwrap_or_else(|| panic!("Matrix/{arg_len} should be globally pack-dispatched"));
        assert_eq!(dispatched.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(
            new_java2rust::stdlib::external_constructor_provenance("Matrix", &args),
            FactProvenance::ConservativeFallback,
            "Matrix/{arg_len} precision should come from the Jama pack, not stdlib"
        );
    }

    for (method_name, expected_type) in [
        (
            "numRows",
            JavaType::Primitive(new_java2rust::java_ir::PrimitiveType::Int),
        ),
        (
            "numCols",
            JavaType::Primitive(new_java2rust::java_ir::PrimitiveType::Int),
        ),
        (
            "toArray",
            JavaType::Array(Box::new(JavaType::Array(Box::new(JavaType::Primitive(
                new_java2rust::java_ir::PrimitiveType::Double,
            ))))),
        ),
    ] {
        let effect = jama_facts
            .external_api_effect_for_receiver(Some("Matrix"), method_name)
            .unwrap_or_else(|| panic!("Matrix.{method_name} should be pack-owned"));
        assert_eq!(effect.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(effect.role, ExternalApiRole::Getter);
        assert_eq!(effect.receiver_mutability, ReceiverMutabilityFact::ReadOnly);
        assert_eq!(
            effect.return_nullability,
            NullabilityState::DefinitelyNonNull
        );
        assert_eq!(effect.return_type, Some(expected_type));
    }

    let hipparchus_facts = hipparchus_distribution_pack_facts();
    let hipparchus_pack = hipparchus_facts
        .external_library_pack_for_type("org.hipparchus.random.EmpiricalDistribution")
        .expect("EmpiricalDistribution should be owned by the Hipparchus distribution pack");
    assert_eq!(hipparchus_pack.pack_name, "org.hipparchus.random");
    assert_eq!(hipparchus_pack.mode, ExternalLibraryMode::ExternalFactModel);
    assert_eq!(hipparchus_pack.coverage, ExternalApiCoverage::Semantic);
    assert_eq!(
        hipparchus_pack.provenance,
        FactProvenance::ExternalLibraryPack
    );
    let constructor = hipparchus_facts
        .external_constructor_for("EmpiricalDistribution", &[])
        .expect("EmpiricalDistribution/0 should be pack-owned");
    assert_eq!(constructor.provenance, FactProvenance::ExternalLibraryPack);
    let dispatched =
        new_java2rust::external_packs::external_constructor_for("EmpiricalDistribution", &[])
            .expect("EmpiricalDistribution/0 should be globally pack-dispatched");
    assert_eq!(dispatched.provenance, FactProvenance::ExternalLibraryPack);
    assert_eq!(
        new_java2rust::stdlib::external_constructor_provenance("EmpiricalDistribution", &[]),
        FactProvenance::ConservativeFallback,
        "EmpiricalDistribution/0 precision should come from the Hipparchus pack, not stdlib"
    );
}

#[test]
fn semantic_analysis_uses_jama_pack_for_matrix_accessors() {
    let semantic = analyze(
        r#"
import Jama.Matrix;

class JamaMatrixUse {
    int cells(Matrix matrix) {
        return matrix.numRows() * matrix.numCols();
    }

    double[][] copy(Matrix matrix) {
        return matrix.toArray();
    }

    Matrix create(double[][] values) {
        return new Matrix(values);
    }
}
"#,
    );

    assert!(
        semantic
            .diagnostics
            .iter()
            .all(|diagnostic| diagnostic.code != "external_pack_missing_api"),
        "modeled Jama matrix helpers should not report pack gaps: {:?}",
        semantic.diagnostics
    );

    let rows = semantic
        .facts
        .external_api_effect_for_receiver(Some("Matrix"), "numRows")
        .expect("semantic analysis should use the Jama pack for Matrix.numRows");
    assert_eq!(
        rows.return_type,
        Some(JavaType::Primitive(
            new_java2rust::java_ir::PrimitiveType::Int
        ))
    );

    let cols = semantic
        .facts
        .external_api_effect_for_receiver(Some("Matrix"), "numCols")
        .expect("semantic analysis should use the Jama pack for Matrix.numCols");
    assert_eq!(
        cols.return_type,
        Some(JavaType::Primitive(
            new_java2rust::java_ir::PrimitiveType::Int
        ))
    );

    let to_array = semantic
        .facts
        .external_api_effect_for_receiver(Some("Matrix"), "toArray")
        .expect("semantic analysis should use the Jama pack for Matrix.toArray");
    assert_eq!(
        to_array.return_type,
        Some(JavaType::Array(Box::new(JavaType::Array(Box::new(
            JavaType::Primitive(new_java2rust::java_ir::PrimitiveType::Double)
        )))))
    );

    let constructor = semantic
        .facts
        .external_constructor_for("Matrix", &[JavaType::Unknown("values".to_string())])
        .expect("semantic analysis should use the Jama pack for Matrix/1");
    assert_eq!(constructor.provenance, FactProvenance::ExternalLibraryPack);
}

#[test]
fn apache_http_pack_declares_request_and_connection_manager_boundaries() {
    let facts = apache_http_pack_facts();

    let pack = facts
        .external_library_pack_for_type("org.apache.http.client.methods.HttpGet")
        .expect("HttpGet should be owned by the Apache HTTP pack");
    assert_eq!(pack.pack_name, "org.apache.http");
    assert_eq!(pack.mode, ExternalLibraryMode::ExternalFactModel);
    assert_eq!(pack.coverage, ExternalApiCoverage::Semantic);
    assert_eq!(pack.provenance, FactProvenance::ExternalLibraryPack);

    for (type_name, arg_len) in [("HttpGet", 1), ("PoolingHttpClientConnectionManager", 0)] {
        let args = vec![JavaType::Unknown("Object".to_string()); arg_len];
        let constructor = facts
            .external_constructor_for(type_name, &args)
            .unwrap_or_else(|| panic!("{type_name}/{arg_len} should be pack-owned"));
        assert_eq!(constructor.provenance, FactProvenance::ExternalLibraryPack);
        let dispatched = new_java2rust::external_packs::external_constructor_for(type_name, &args)
            .unwrap_or_else(|| panic!("{type_name}/{arg_len} should be globally pack-dispatched"));
        assert_eq!(dispatched.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(
            new_java2rust::stdlib::external_constructor_provenance(type_name, &args),
            FactProvenance::ConservativeFallback,
            "{type_name}/{arg_len} precision should come from the Apache HTTP pack, not stdlib"
        );
    }

    let add_header = facts
        .external_api_effect_for_receiver(Some("HttpGet"), "addHeader")
        .expect("HttpGet.addHeader should be pack-owned");
    assert_eq!(add_header.provenance, FactProvenance::ExternalLibraryPack);
    assert_eq!(add_header.role, ExternalApiRole::Mutator);
    assert_eq!(
        add_header.receiver_mutability,
        ReceiverMutabilityFact::MutatesReceiver
    );
    let manifest = builtin_external_pack_manifests()
        .into_iter()
        .find(|manifest| manifest.pack_name == "org.apache.http")
        .expect("Apache HTTP manifest should exist");
    assert_eq!(
        manifest.coverage_for_api("HttpGet", "addHeader"),
        ExternalApiCoverage::Semantic,
        "HttpGet.addHeader should have explicit manifest coverage"
    );
}

#[test]
fn apache_commons_math_stats_pack_declares_percentile_boundary() {
    let facts = apache_commons_math_stats_pack_facts();

    let pack = facts
        .external_library_pack_for_type("org.apache.commons.math3.stat.descriptive.rank.Percentile")
        .expect("Percentile should be owned by the Apache Commons Math stats pack");
    assert_eq!(
        pack.pack_name,
        "org.apache.commons.math3.stat.descriptive.rank"
    );
    assert_eq!(pack.mode, ExternalLibraryMode::ExternalFactModel);
    assert_eq!(pack.coverage, ExternalApiCoverage::Semantic);
    assert_eq!(pack.provenance, FactProvenance::ExternalLibraryPack);

    for arg_types in [Vec::new(), vec![JavaType::Primitive(PrimitiveType::Double)]] {
        let constructor = facts
            .external_constructor_for("Percentile", &arg_types)
            .unwrap_or_else(|| {
                panic!(
                    "Percentile constructor with {} argument(s) should be pack-owned",
                    arg_types.len()
                )
            });
        assert_eq!(constructor.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(
            new_java2rust::stdlib::external_constructor_provenance("Percentile", &arg_types),
            FactProvenance::ConservativeFallback,
            "Percentile constructor precision should come from the stats pack, not stdlib"
        );
    }

    let field = facts
        .external_field_for("Percentile", "EstimationType")
        .expect("Percentile.EstimationType should be pack-owned");
    assert_eq!(field.provenance, FactProvenance::ExternalLibraryPack);
    assert_eq!(
        field.field_type,
        Some(JavaType::Named {
            name: "EstimationType".to_string(),
            args: Vec::new(),
        })
    );
    assert_eq!(
        new_java2rust::stdlib::external_field_provenance("Percentile", "EstimationType"),
        FactProvenance::ConservativeFallback,
        "Percentile.EstimationType precision should come from the stats pack, not stdlib"
    );

    let r1 = facts
        .external_field_for("EstimationType", "R_1")
        .expect("EstimationType.R_1 should be pack-owned");
    assert_eq!(r1.provenance, FactProvenance::ExternalLibraryPack);
    assert_eq!(
        r1.field_type,
        Some(JavaType::Named {
            name: "EstimationType".to_string(),
            args: Vec::new(),
        })
    );
    let dispatched = new_java2rust::external_packs::external_field_for("EstimationType", "R_1")
        .expect("EstimationType.R_1 should be globally pack-dispatched");
    assert_eq!(dispatched.provenance, FactProvenance::ExternalLibraryPack);
    assert_eq!(
        new_java2rust::stdlib::external_field_provenance("EstimationType", "R_1"),
        FactProvenance::ConservativeFallback,
        "EstimationType.R_1 precision should come from the stats pack, not stdlib"
    );
}

#[test]
fn google_bigquery_pack_declares_nested_enum_qualifiers() {
    let facts = google_bigquery_pack_facts();

    let pack = facts
        .external_library_pack_for_type("com.google.cloud.bigquery.FieldValue")
        .expect("FieldValue should be owned by the BigQuery pack");
    assert_eq!(pack.pack_name, "com.google.cloud.bigquery");
    assert_eq!(pack.mode, ExternalLibraryMode::ExternalFactModel);
    assert_eq!(pack.coverage, ExternalApiCoverage::Semantic);
    assert_eq!(pack.provenance, FactProvenance::ExternalLibraryPack);

    assert!(
        facts.external_library_pack_for_type("Attribute").is_none(),
        "the BigQuery pack should not claim the ambiguous bare Attribute type"
    );
    assert!(
        facts.external_library_pack_for_type("Priority").is_none(),
        "the BigQuery pack should not claim the ambiguous bare Priority type"
    );

    for (type_name, field_name, expected_type) in [
        ("FieldValue", "Attribute", "Attribute"),
        ("QueryJobConfiguration", "Priority", "Priority"),
    ] {
        let field = facts
            .external_field_for(type_name, field_name)
            .unwrap_or_else(|| panic!("{type_name}.{field_name} should be pack-owned"));
        assert_eq!(field.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(
            field.field_type,
            Some(JavaType::Named {
                name: expected_type.to_string(),
                args: Vec::new(),
            })
        );
        assert_eq!(
            new_java2rust::stdlib::external_field_provenance(type_name, field_name),
            FactProvenance::ConservativeFallback,
            "{type_name}.{field_name} precision should come from the BigQuery pack, not stdlib"
        );
    }

    for (type_name, field_name, expected_type) in [
        ("FieldValue.Attribute", "PRIMITIVE", "Attribute"),
        ("QueryJobConfiguration.Priority", "BATCH", "Priority"),
        ("QueryJobConfiguration.Priority", "INTERACTIVE", "Priority"),
    ] {
        let field = facts
            .external_field_for(type_name, field_name)
            .unwrap_or_else(|| panic!("{type_name}.{field_name} should be pack-owned"));
        assert_eq!(field.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(
            field.field_type,
            Some(JavaType::Named {
                name: expected_type.to_string(),
                args: Vec::new(),
            })
        );
        let dispatched = new_java2rust::external_packs::external_field_for(type_name, field_name)
            .unwrap_or_else(|| {
                panic!("{type_name}.{field_name} should be globally pack-dispatched")
            });
        assert_eq!(dispatched.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(
            new_java2rust::stdlib::external_field_provenance(type_name, field_name),
            FactProvenance::ConservativeFallback,
            "{type_name}.{field_name} precision should come from the BigQuery pack, not stdlib"
        );
    }

    assert_eq!(
        new_java2rust::stdlib::external_field_provenance("Attribute", "PRIMITIVE"),
        FactProvenance::BuiltinExternalModel,
        "bare Attribute.PRIMITIVE remains generic because the BigQuery pack does not own ambiguous bare Attribute"
    );
    assert_eq!(
        new_java2rust::stdlib::external_field_provenance("Priority", "BATCH"),
        FactProvenance::BuiltinExternalModel,
        "bare Priority.BATCH remains generic because the BigQuery pack does not own ambiguous bare Priority"
    );
}

#[test]
fn google_bigquery_storage_pack_declares_data_format_boundary() {
    let facts = google_bigquery_storage_pack_facts();

    let pack = facts
        .external_library_pack_for_type("com.google.cloud.bigquery.storage.v1.DataFormat")
        .expect("DataFormat should be owned by the BigQuery Storage pack");
    assert_eq!(pack.pack_name, "com.google.cloud.bigquery.storage.v1");
    assert_eq!(pack.mode, ExternalLibraryMode::ExternalFactModel);
    assert_eq!(pack.coverage, ExternalApiCoverage::Semantic);
    assert_eq!(pack.provenance, FactProvenance::ExternalLibraryPack);

    for field_name in ["AVRO", "ARROW"] {
        let field = facts
            .external_field_for("DataFormat", field_name)
            .unwrap_or_else(|| panic!("DataFormat.{field_name} should be pack-owned"));
        assert_eq!(field.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(
            field.field_type,
            Some(JavaType::Named {
                name: "DataFormat".to_string(),
                args: Vec::new(),
            })
        );
        let dispatched =
            new_java2rust::external_packs::external_field_for("DataFormat", field_name)
                .unwrap_or_else(|| {
                    panic!("DataFormat.{field_name} should be globally pack-dispatched")
                });
        assert_eq!(dispatched.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(
            new_java2rust::stdlib::external_field_provenance("DataFormat", field_name),
            FactProvenance::ConservativeFallback,
            "DataFormat.{field_name} precision should come from the BigQuery Storage pack, not stdlib"
        );
    }
}

#[test]
fn jackson_databind_pack_declares_feature_enum_boundaries() {
    let facts = jackson_databind_pack_facts();

    let pack = facts
        .external_library_pack_for_type("com.fasterxml.jackson.databind.MapperFeature")
        .expect("MapperFeature should be owned by the Jackson databind pack");
    assert_eq!(pack.pack_name, "com.fasterxml.jackson.databind");
    assert_eq!(pack.mode, ExternalLibraryMode::ExternalFactModel);
    assert_eq!(pack.coverage, ExternalApiCoverage::Semantic);
    assert_eq!(pack.provenance, FactProvenance::ExternalLibraryPack);

    for (type_name, field_name) in [
        ("MapperFeature", "ACCEPT_CASE_INSENSITIVE_PROPERTIES"),
        ("DeserializationFeature", "UNWRAP_ROOT_VALUE"),
    ] {
        let field = facts
            .external_field_for(type_name, field_name)
            .unwrap_or_else(|| panic!("{type_name}.{field_name} should be pack-owned"));
        assert_eq!(field.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(
            field.field_type,
            Some(JavaType::Named {
                name: type_name.to_string(),
                args: Vec::new(),
            })
        );
        let dispatched = new_java2rust::external_packs::external_field_for(type_name, field_name)
            .unwrap_or_else(|| {
                panic!("{type_name}.{field_name} should be globally pack-dispatched")
            });
        assert_eq!(dispatched.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(
            new_java2rust::stdlib::external_field_provenance(type_name, field_name),
            FactProvenance::ConservativeFallback,
            "{type_name}.{field_name} precision should come from the Jackson databind pack, not stdlib"
        );
    }
}

#[test]
fn google_storage_pack_declares_sign_url_option_qualifier() {
    let facts = google_storage_pack_facts();

    let pack = facts
        .external_library_pack_for_type("com.google.cloud.storage.Storage")
        .expect("Storage should be owned by the Google Storage pack");
    assert_eq!(pack.pack_name, "com.google.cloud.storage");
    assert_eq!(pack.mode, ExternalLibraryMode::ExternalFactModel);
    assert_eq!(pack.coverage, ExternalApiCoverage::Semantic);
    assert_eq!(pack.provenance, FactProvenance::ExternalLibraryPack);

    let field = facts
        .external_field_for("Storage", "SignUrlOption")
        .expect("Storage.SignUrlOption should be pack-owned");
    assert_eq!(field.provenance, FactProvenance::ExternalLibraryPack);
    assert_eq!(
        field.field_type,
        Some(JavaType::Named {
            name: "SignUrlOption".to_string(),
            args: Vec::new(),
        })
    );
    assert_eq!(
        new_java2rust::stdlib::external_field_provenance("Storage", "SignUrlOption"),
        FactProvenance::ConservativeFallback,
        "Storage.SignUrlOption precision should come from the Google Storage pack, not stdlib"
    );
}

#[test]
fn google_cloud_nio_pack_declares_static_configuration_fields() {
    let facts = google_cloud_nio_pack_facts();

    let pack = facts
        .external_library_pack_for_type(
            "com.google.cloud.storage.contrib.nio.CloudStorageFileSystem",
        )
        .expect("CloudStorageFileSystem should be owned by the Google Cloud NIO pack");
    assert_eq!(pack.pack_name, "com.google.cloud.storage.contrib.nio");
    assert_eq!(pack.mode, ExternalLibraryMode::ExternalFactModel);
    assert_eq!(pack.coverage, ExternalApiCoverage::Semantic);
    assert_eq!(pack.provenance, FactProvenance::ExternalLibraryPack);

    for (type_name, field_name, expected_type) in [
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
        let field = facts
            .external_field_for(type_name, field_name)
            .unwrap_or_else(|| panic!("{type_name}.{field_name} should be pack-owned"));
        assert_eq!(field.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(field.field_type, Some(expected_type));
        let dispatched = new_java2rust::external_packs::external_field_for(type_name, field_name)
            .unwrap_or_else(|| {
                panic!("{type_name}.{field_name} should be globally pack-dispatched")
            });
        assert_eq!(dispatched.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(
            new_java2rust::stdlib::external_field_provenance(type_name, field_name),
            FactProvenance::ConservativeFallback,
            "{type_name}.{field_name} precision should come from the Google Cloud NIO pack, not stdlib"
        );
    }
}

#[test]
fn genomicsdb_pack_declares_constructors_and_nested_builder_qualifiers() {
    let facts = genomicsdb_pack_facts();

    let pack = facts
        .external_library_pack_for_type("GenomicsDBFeatureReader")
        .expect("GenomicsDBFeatureReader should be owned by the GenomicsDB pack");
    assert_eq!(pack.pack_name, "com.intel.genomicsdb");
    assert_eq!(pack.mode, ExternalLibraryMode::ExternalFactModel);
    assert_eq!(pack.coverage, ExternalApiCoverage::Semantic);
    assert_eq!(pack.provenance, FactProvenance::ExternalLibraryPack);

    for (type_name, arg_len) in [
        ("GenomicsDBFeatureReader", 3),
        ("GenomicsDBImporter", 1),
        ("ImportConfig", 8),
    ] {
        let arg_types = vec![JavaType::Unknown("_".to_string()); arg_len];
        let constructor = facts
            .external_constructor_for(type_name, &arg_types)
            .unwrap_or_else(|| panic!("{type_name}/{arg_len} should be pack-owned"));
        assert_eq!(constructor.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(
            new_java2rust::stdlib::external_constructor_provenance(type_name, &arg_types),
            FactProvenance::ConservativeFallback,
            "{type_name}/{arg_len} precision should come from the GenomicsDB pack, not stdlib"
        );
    }

    for (type_name, field_name, expected_type) in [
        ("Coordinates", "GenomicsDBColumn", "GenomicsDBColumn"),
        ("Coordinates", "ContigPosition", "ContigPosition"),
        (
            "GenomicsDBExportConfiguration",
            "ExportConfiguration",
            "ExportConfiguration",
        ),
        (
            "GenomicsDBImportConfiguration",
            "ImportConfiguration",
            "ImportConfiguration",
        ),
        ("GenomicsDBImportConfiguration", "Partition", "Partition"),
        ("GenomicsDBVidMapProto", "VidMappingPB", "VidMappingPB"),
        (
            "GenomicsDBVidMapProto",
            "FieldLengthDescriptorComponentPB",
            "FieldLengthDescriptorComponentPB",
        ),
        ("ReadSession", "TableReadOptions", "TableReadOptions"),
    ] {
        let field = facts
            .external_field_for(type_name, field_name)
            .unwrap_or_else(|| panic!("{type_name}.{field_name} should be pack-owned"));
        assert_eq!(field.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(
            field.field_type,
            Some(JavaType::Named {
                name: expected_type.to_string(),
                args: Vec::new(),
            })
        );
        assert_eq!(
            new_java2rust::stdlib::external_field_provenance(type_name, field_name),
            FactProvenance::ConservativeFallback,
            "{type_name}.{field_name} precision should come from the GenomicsDB pack, not stdlib"
        );
    }

    for (receiver, method_name, expected_type) in [
        ("GenomicsDBColumn", "newBuilder", "GenomicsDBColumn.Builder"),
        ("ContigPosition", "newBuilder", "ContigPosition.Builder"),
        (
            "ExportConfiguration",
            "newBuilder",
            "ExportConfiguration.Builder",
        ),
        (
            "ImportConfiguration",
            "newBuilder",
            "ImportConfiguration.Builder",
        ),
        ("Partition", "newBuilder", "Partition.Builder"),
        ("VidMappingPB", "newBuilder", "VidMappingPB.Builder"),
        ("VidMappingPB", "toBuilder", "VidMappingPB.Builder"),
        (
            "FieldLengthDescriptorComponentPB",
            "newBuilder",
            "FieldLengthDescriptorComponentPB.Builder",
        ),
    ] {
        let effect = facts
            .external_api_effect_for_receiver(Some(receiver), method_name)
            .unwrap_or_else(|| panic!("{receiver}.{method_name} should be pack-owned"));
        assert_eq!(effect.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(effect.role, ExternalApiRole::Getter);
        assert_eq!(
            effect.return_type,
            Some(JavaType::Named {
                name: expected_type.to_string(),
                args: Vec::new(),
            })
        );
        assert_eq!(
            effect.return_nullability,
            NullabilityState::DefinitelyNonNull
        );
    }

    let protobuf_mapping = facts
        .external_api_effect_for_receiver(Some("GenomicsDBImporter"), "getProtobufVidMapping")
        .expect("GenomicsDBImporter.getProtobufVidMapping should be pack-owned");
    assert_eq!(
        protobuf_mapping.provenance,
        FactProvenance::ExternalLibraryPack
    );
    assert_eq!(protobuf_mapping.role, ExternalApiRole::Getter);
    assert_eq!(
        protobuf_mapping.return_type,
        Some(JavaType::Named {
            name: "VidMappingPB".to_string(),
            args: Vec::new(),
        })
    );
    assert_eq!(
        protobuf_mapping.return_nullability,
        NullabilityState::MaybeNull
    );

    let contigs = facts
        .external_api_effect_for_receiver(Some("VidMappingPB"), "getContigsList")
        .expect("VidMappingPB.getContigsList should be pack-owned");
    assert_eq!(
        contigs.return_type,
        Some(JavaType::Named {
            name: "List".to_string(),
            args: vec![JavaType::Named {
                name: "GenomicsDBVidMapProto.Chromosome".to_string(),
                args: Vec::new(),
            }],
        })
    );

    assert!(
        facts
            .external_library_pack_for_type("GenomicsDBVidMapProto.Chromosome")
            .is_some(),
        "GenomicsDBVidMapProto.Chromosome should be owned by the GenomicsDB pack"
    );

    let chromosome_name = facts
        .external_api_effect_for_receiver(Some("GenomicsDBVidMapProto.Chromosome"), "getName")
        .expect("GenomicsDBVidMapProto.Chromosome.getName should be pack-owned");
    assert_eq!(chromosome_name.return_type, Some(JavaType::String));
    assert_eq!(
        chromosome_name.return_nullability,
        NullabilityState::DefinitelyNonNull
    );

    let chromosome_offset = facts
        .external_api_effect_for_receiver(
            Some("GenomicsDBVidMapProto.Chromosome"),
            "getTiledbColumnOffset",
        )
        .expect("GenomicsDBVidMapProto.Chromosome.getTiledbColumnOffset should be pack-owned");
    assert_eq!(
        chromosome_offset.return_type,
        Some(JavaType::Primitive(
            new_java2rust::java_ir::PrimitiveType::Long
        ))
    );

    let chromosome_length = facts
        .external_api_effect_for_receiver(Some("GenomicsDBVidMapProto.Chromosome"), "getLength")
        .expect("GenomicsDBVidMapProto.Chromosome.getLength should be pack-owned");
    assert_eq!(
        chromosome_length.return_type,
        Some(JavaType::Primitive(
            new_java2rust::java_ir::PrimitiveType::Int
        ))
    );

    let field = facts
        .external_api_effect_for_receiver_with_arity(Some("VidMappingPB"), "getFields", Some(1))
        .expect("VidMappingPB.getFields(int) should be pack-owned");
    assert_eq!(field.role, ExternalApiRole::IndexedGetter);
    assert_eq!(
        field.return_type,
        Some(JavaType::Named {
            name: "FieldInfo".to_string(),
            args: Vec::new(),
        })
    );

    let field_count = facts
        .external_api_effect_for_receiver(Some("VidMappingPB"), "getFieldsCount")
        .expect("VidMappingPB.getFieldsCount should be pack-owned");
    assert_eq!(
        field_count.return_type,
        Some(JavaType::Primitive(
            new_java2rust::java_ir::PrimitiveType::Int
        ))
    );

    let field_name = facts
        .external_api_effect_for_receiver(Some("FieldInfo"), "getName")
        .expect("FieldInfo.getName should be pack-owned");
    assert_eq!(field_name.return_type, Some(JavaType::String));

    let is_array = facts
        .external_api_effect_for_receiver_with_arity(
            Some("GenomicsDBUtils"),
            "isGenomicsDBArray",
            Some(2),
        )
        .expect("GenomicsDBUtils.isGenomicsDBArray should be pack-owned");
    assert_eq!(
        is_array.return_type,
        Some(JavaType::Primitive(
            new_java2rust::java_ir::PrimitiveType::Boolean
        ))
    );

    let read_file = facts
        .external_api_effect_for_receiver_with_arity(
            Some("org.genomicsdb.GenomicsDBUtils"),
            "readEntireFile",
            Some(1),
        )
        .expect("GenomicsDBUtils.readEntireFile should be pack-owned");
    assert_eq!(read_file.return_type, Some(JavaType::String));

    let gcs_connector = facts
        .external_api_effect_for_receiver_with_arity(
            Some("GenomicsDBUtils"),
            "useGcsHdfsConnector",
            Some(1),
        )
        .expect("GenomicsDBUtils.useGcsHdfsConnector should be pack-owned");
    assert_eq!(gcs_connector.return_type, None);
    assert_eq!(gcs_connector.role, ExternalApiRole::SideEffect);

    for (receiver, method_name, parameter_index, parameter_type) in [
        (
            "org.genomicsdb.GenomicsDBUtils",
            "isGenomicsDBArray",
            0,
            "String",
        ),
        ("GenomicsDBUtils", "isGenomicsDBArray", 1, "String"),
        ("GenomicsDBUtils", "readEntireFile", 0, "String"),
        ("GenomicsDBUtils", "useGcsHdfsConnector", 0, "boolean"),
    ] {
        let argument = facts
            .external_api_argument_effect_for_receiver(Some(receiver), method_name, parameter_index)
            .unwrap_or_else(|| {
                panic!("{receiver}.{method_name} parameter {parameter_index} should be pack-owned")
            });
        assert_eq!(
            argument.parameter_type,
            Some(new_java2rust::parser::parse_type(parameter_type))
        );
        assert_eq!(
            argument.argument_mutability,
            ReceiverMutabilityFact::ReadOnly
        );
        assert_eq!(
            argument.alias_policy,
            new_java2rust::facts::ExternalArgumentAliasPolicy::Borrowed
        );
    }

    for receiver in [
        "FieldInfo",
        "field_info",
        "com.intel.genomicsdb.model.GenomicsDBVidMapProto.FieldInfo",
    ] {
        let indexed_field_name =
            external_api_effect_for_receiver_with_arity(Some(receiver), "getName", Some(0))
                .unwrap_or_else(|| panic!("{receiver}.getName should be found by builtin index"));
        assert_eq!(indexed_field_name.return_type, Some(JavaType::String));
        assert_eq!(
            indexed_field_name.provenance,
            FactProvenance::ExternalLibraryPack
        );
    }
    let indexed_field =
        external_api_effect_for_receiver_with_arity(Some("VidMappingPB"), "getFields", Some(1))
            .expect("VidMappingPB.getFields(int) should be found by builtin index");
    assert_eq!(
        indexed_field.return_type,
        Some(JavaType::Named {
            name: "FieldInfo".to_string(),
            args: Vec::new(),
        })
    );

    for (receiver, method_name, role) in [
        (
            "GenomicsDBImporter",
            "updateProtobufVidMapping",
            ExternalApiRole::Mutator,
        ),
        (
            "GenomicsDBImporter",
            "coalesceContigsIntoNumPartitions",
            ExternalApiRole::Getter,
        ),
        (
            "GenomicsDBImporter",
            "executeImport",
            ExternalApiRole::SideEffect,
        ),
        (
            "ImportConfig",
            "setFunctionToCallOnBatchCompletion",
            ExternalApiRole::Mutator,
        ),
        (
            "ImportConfig",
            "setOutputCallsetmapJsonFile",
            ExternalApiRole::Mutator,
        ),
        (
            "ImportConfig",
            "setOutputVcfHeaderFile",
            ExternalApiRole::Mutator,
        ),
        (
            "ImportConfig",
            "setOutputVidmapJsonFile",
            ExternalApiRole::Mutator,
        ),
        (
            "ImportConfig",
            "setUseSamplesInOrder",
            ExternalApiRole::Mutator,
        ),
    ] {
        let effect = facts
            .external_api_effect_for_receiver(Some(receiver), method_name)
            .unwrap_or_else(|| panic!("{receiver}.{method_name} should be pack-owned"));
        assert_eq!(effect.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(effect.role, role);
    }
}

#[test]
fn semantic_analysis_uses_genomicsdb_pack_for_importer_and_protobuf_boundaries() {
    let semantic = analyze(
        r#"
import com.intel.genomicsdb.GenomicsDBImporter;
import com.intel.genomicsdb.importer.model.ImportConfig;
import com.intel.genomicsdb.model.Coordinates;
import com.intel.genomicsdb.model.GenomicsDBExportConfiguration;
import com.intel.genomicsdb.model.GenomicsDBImportConfiguration;
import com.intel.genomicsdb.model.GenomicsDBVidMapProto;

class GenomicsDbPackUse {
    Object builders() {
        Object column = Coordinates.GenomicsDBColumn.newBuilder();
        Object position = Coordinates.ContigPosition.newBuilder();
        Object exportConfig = GenomicsDBExportConfiguration.ExportConfiguration.newBuilder();
        Object importConfig = GenomicsDBImportConfiguration.ImportConfiguration.newBuilder();
        Object partition = GenomicsDBImportConfiguration.Partition.newBuilder();
        Object vidMap = GenomicsDBVidMapProto.VidMappingPB.newBuilder();
        Object fieldLength = GenomicsDBVidMapProto.FieldLengthDescriptorComponentPB.newBuilder();
        return column.toString() + position + exportConfig + importConfig + partition + vidMap + fieldLength;
    }

    Object importer(GenomicsDBImporter importer, ImportConfig config, GenomicsDBVidMapProto.VidMappingPB vidMap) {
        Object builder = vidMap.toBuilder();
        Object contigs = vidMap.getContigsList();
        String fieldName = "";
        for (int i = 0; i < vidMap.getFieldsCount(); ++i) {
            fieldName = vidMap.getFields(i).getName();
        }
        importer.updateProtobufVidMapping(vidMap);
        importer.coalesceContigsIntoNumPartitions(4);
        importer.executeImport(2);
        config.setOutputCallsetmapJsonFile("callset.json");
        config.setOutputVidmapJsonFile("vid.json");
        config.setOutputVcfHeaderFile("header.vcf");
        config.setUseSamplesInOrder(true);
        config.setFunctionToCallOnBatchCompletion(null);
        return importer.getProtobufVidMapping().toBuilder().toString() + builder + contigs + fieldName;
    }
}
"#,
    );

    let collected_genomicsdb_methods = semantic
        .facts
        .external_api_effect_facts
        .iter()
        .filter(|effect| {
            effect.receiver_type.as_deref().is_some_and(|receiver| {
                receiver.contains("VidMapping") || receiver.contains("GenomicsDB")
            })
        })
        .map(|effect| {
            format!(
                "{}.{}",
                effect.receiver_type.as_deref().unwrap_or("*"),
                effect.method_name
            )
        })
        .collect::<Vec<_>>();

    for (receiver, method_name) in [
        ("GenomicsDBColumn", "newBuilder"),
        ("ContigPosition", "newBuilder"),
        ("ExportConfiguration", "newBuilder"),
        ("ImportConfiguration", "newBuilder"),
        ("Partition", "newBuilder"),
        ("VidMappingPB", "newBuilder"),
        ("VidMappingPB", "toBuilder"),
        ("VidMappingPB", "getContigsList"),
        ("VidMappingPB", "getFields"),
        ("VidMappingPB", "getFieldsCount"),
        ("FieldInfo", "getName"),
        ("FieldLengthDescriptorComponentPB", "newBuilder"),
        ("GenomicsDBImporter", "getProtobufVidMapping"),
        ("GenomicsDBImporter", "updateProtobufVidMapping"),
        ("GenomicsDBImporter", "coalesceContigsIntoNumPartitions"),
        ("GenomicsDBImporter", "executeImport"),
        ("ImportConfig", "setFunctionToCallOnBatchCompletion"),
        ("ImportConfig", "setOutputCallsetmapJsonFile"),
        ("ImportConfig", "setOutputVcfHeaderFile"),
        ("ImportConfig", "setOutputVidmapJsonFile"),
        ("ImportConfig", "setUseSamplesInOrder"),
    ] {
        let effect = semantic
            .facts
            .external_api_effect_for_receiver(Some(receiver), method_name)
            .unwrap_or_else(|| {
                panic!(
                    "{receiver}.{method_name} should be modeled; collected: {collected_genomicsdb_methods:?}; diagnostics: {:?}",
                    semantic.diagnostics
                )
            });
        assert_eq!(effect.provenance, FactProvenance::ExternalLibraryPack);
    }

    assert!(
        !semantic
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "external_pack_missing_api"),
        "modeled GenomicsDB pack calls should not produce missing-pack diagnostics: {:?}",
        semantic.diagnostics
    );
}

#[test]
fn htsjdk_tribble_index_pack_declares_index_factory_nested_enums() {
    let facts = htsjdk_tribble_index_pack_facts();

    let pack = facts
        .external_library_pack_for_type("htsjdk.tribble.index.IndexFactory")
        .expect("IndexFactory should be owned by the tribble index pack");
    assert_eq!(pack.pack_name, "htsjdk.tribble.index");
    assert_eq!(pack.mode, ExternalLibraryMode::ExternalFactModel);
    assert_eq!(pack.coverage, ExternalApiCoverage::Semantic);
    assert_eq!(pack.provenance, FactProvenance::ExternalLibraryPack);
    let tabix_pack = facts
        .external_library_pack_for_type("htsjdk.tribble.index.tabix.TabixFormat")
        .expect("TabixFormat should be owned by the tribble index pack");
    assert_eq!(tabix_pack.pack_name, "htsjdk.tribble.index");
    assert_eq!(tabix_pack.provenance, FactProvenance::ExternalLibraryPack);

    for (type_name, field_name, expected_type) in [
        ("IndexFactory", "IndexType", "IndexType"),
        (
            "IndexFactory",
            "IndexBalanceApproach",
            "IndexBalanceApproach",
        ),
    ] {
        let field = facts
            .external_field_for(type_name, field_name)
            .unwrap_or_else(|| panic!("{type_name}.{field_name} should be pack-owned"));
        assert_eq!(field.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(
            field.field_type,
            Some(JavaType::Named {
                name: expected_type.to_string(),
                args: Vec::new(),
            })
        );
        assert_eq!(
            new_java2rust::stdlib::external_field_provenance(type_name, field_name),
            FactProvenance::ConservativeFallback,
            "{type_name}.{field_name} precision should come from the tribble index pack, not stdlib"
        );
    }

    for (type_name, field_name, expected_type) in [
        ("IndexType", "TABIX", "IndexType"),
        (
            "IndexBalanceApproach",
            "FOR_SEEK_TIME",
            "IndexBalanceApproach",
        ),
    ] {
        let field = facts
            .external_field_for(type_name, field_name)
            .unwrap_or_else(|| panic!("{type_name}.{field_name} should be pack-owned"));
        assert_eq!(field.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(
            field.field_type,
            Some(JavaType::Named {
                name: expected_type.to_string(),
                args: Vec::new(),
            })
        );
        let dispatched = new_java2rust::external_packs::external_field_for(type_name, field_name)
            .unwrap_or_else(|| {
                panic!("{type_name}.{field_name} should be globally pack-dispatched")
            });
        assert_eq!(dispatched.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(
            new_java2rust::stdlib::external_field_provenance(type_name, field_name),
            FactProvenance::ConservativeFallback,
            "{type_name}.{field_name} precision should come from the tribble index pack, not stdlib"
        );
    }
    for type_name in ["TabixFormat", "htsjdk.tribble.index.tabix.TabixFormat"] {
        for field_name in ["GENERIC_FLAGS", "ZERO_BASED"] {
            let field = facts
                .external_field_for(type_name, field_name)
                .unwrap_or_else(|| panic!("{type_name}.{field_name} should be pack-owned"));
            assert_eq!(field.provenance, FactProvenance::ExternalLibraryPack);
            assert_eq!(
                field.field_type,
                Some(JavaType::Primitive(PrimitiveType::Int))
            );
            let dispatched =
                new_java2rust::external_packs::external_field_for(type_name, field_name)
                    .unwrap_or_else(|| {
                        panic!("{type_name}.{field_name} should be globally pack-dispatched")
                    });
            assert_eq!(dispatched.provenance, FactProvenance::ExternalLibraryPack);
            assert_eq!(
                new_java2rust::stdlib::external_field_provenance(type_name, field_name),
                FactProvenance::ConservativeFallback,
                "{type_name}.{field_name} precision should come from the tribble index pack, not stdlib"
            );
        }
    }
}

#[test]
fn htsjdk_samtools_util_pack_declares_interval_and_iterator_boundary() {
    let facts = htsjdk_samtools_util_pack_facts();

    let pack = facts
        .external_library_pack_for_type("htsjdk.samtools.util.Interval")
        .expect("samtools.util Interval should be owned by the external pack");
    assert_eq!(pack.pack_name, "htsjdk.samtools.util");
    assert_eq!(pack.mode, ExternalLibraryMode::ExternalFactModel);
    assert_eq!(pack.coverage, ExternalApiCoverage::Semantic);
    assert_eq!(pack.provenance, FactProvenance::ExternalLibraryPack);

    for (type_name, field_name, expected_type) in [
        (
            "BlockCompressedInputStream",
            "FileTermination",
            "FileTermination",
        ),
        ("Log", "LogLevel", "LogLevel"),
        ("SamPairUtil", "PairOrientation", "PairOrientation"),
    ] {
        let field = facts
            .external_field_for(type_name, field_name)
            .unwrap_or_else(|| panic!("{type_name}.{field_name} should be pack-owned"));
        assert_eq!(field.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(
            field.field_type,
            Some(JavaType::Named {
                name: expected_type.to_string(),
                args: Vec::new(),
            })
        );
        assert_eq!(
            new_java2rust::stdlib::external_field_provenance(type_name, field_name),
            FactProvenance::ConservativeFallback,
            "{type_name}.{field_name} precision should come from the samtools-util pack, not stdlib"
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
        let field = facts
            .external_field_for(type_name, field_name)
            .unwrap_or_else(|| panic!("{type_name}.{field_name} should be pack-owned"));
        assert_eq!(field.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(field.field_type, Some(expected_ty));
        let dispatched = new_java2rust::external_packs::external_field_for(type_name, field_name)
            .unwrap_or_else(|| {
                panic!("{type_name}.{field_name} should be globally pack-dispatched")
            });
        assert_eq!(dispatched.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(
            new_java2rust::stdlib::external_field_provenance(type_name, field_name),
            FactProvenance::ConservativeFallback,
            "{type_name}.{field_name} precision should come from the samtools-util pack, not stdlib"
        );
    }

    for (type_name, field_name, expected_type) in [
        ("PairOrientation", "FR", "PairOrientation"),
        ("PairOrientation", "RF", "PairOrientation"),
        ("PairOrientation", "TANDEM", "PairOrientation"),
        ("FileTermination", "DEFECTIVE", "FileTermination"),
        ("FileTermination", "HAS_TERMINATOR_BLOCK", "FileTermination"),
        ("LogLevel", "ERROR", "LogLevel"),
        ("LogLevel", "WARNING", "LogLevel"),
        ("LogLevel", "INFO", "LogLevel"),
        ("LogLevel", "DEBUG", "LogLevel"),
    ] {
        let field = facts
            .external_field_for(type_name, field_name)
            .unwrap_or_else(|| panic!("{type_name}.{field_name} should be pack-owned"));
        assert_eq!(field.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(
            field.field_type,
            Some(JavaType::Named {
                name: expected_type.to_string(),
                args: Vec::new(),
            })
        );
        let dispatched = new_java2rust::external_packs::external_field_for(type_name, field_name)
            .unwrap_or_else(|| {
                panic!("{type_name}.{field_name} should be globally pack-dispatched")
            });
        assert_eq!(dispatched.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(
            new_java2rust::stdlib::external_field_provenance(type_name, field_name),
            FactProvenance::ConservativeFallback,
            "{type_name}.{field_name} precision should come from the samtools-util pack, not stdlib"
        );
    }

    for arg_types in [
        vec![JavaType::Named {
            name: "InputStream".to_string(),
            args: Vec::new(),
        }],
        vec![
            JavaType::Named {
                name: "InputStream".to_string(),
                args: Vec::new(),
            },
            JavaType::Primitive(PrimitiveType::Boolean),
        ],
    ] {
        let constructor = facts
            .external_constructor_for("BlockCompressedInputStream", &arg_types)
            .unwrap_or_else(|| {
                panic!(
                    "BlockCompressedInputStream/{} constructor should be pack-owned",
                    arg_types.len()
                )
            });
        assert_eq!(constructor.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(
            new_java2rust::stdlib::external_constructor_provenance(
                "BlockCompressedInputStream",
                &arg_types
            ),
            FactProvenance::ConservativeFallback,
            "BlockCompressedInputStream/{} precision should come from the samtools-util pack, not stdlib",
            arg_types.len()
        );
    }

    for arg_types in [
        vec![
            JavaType::Named {
                name: "File".to_string(),
                args: Vec::new(),
            },
            JavaType::Primitive(PrimitiveType::Int),
        ],
        vec![
            JavaType::Named {
                name: "OutputStream".to_string(),
                args: Vec::new(),
            },
            JavaType::Named {
                name: "File".to_string(),
                args: Vec::new(),
            },
            JavaType::Primitive(PrimitiveType::Int),
        ],
    ] {
        let constructor = facts
            .external_constructor_for("BlockCompressedOutputStream", &arg_types)
            .unwrap_or_else(|| {
                panic!(
                    "BlockCompressedOutputStream/{} constructor should be pack-owned",
                    arg_types.len()
                )
            });
        assert_eq!(constructor.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(
            new_java2rust::stdlib::external_constructor_provenance(
                "BlockCompressedOutputStream",
                &arg_types
            ),
            FactProvenance::ConservativeFallback,
            "BlockCompressedOutputStream/{} precision should come from the samtools-util pack, not stdlib",
            arg_types.len()
        );
    }

    let interval = facts
        .external_constructor_for(
            "Interval",
            &[
                JavaType::String,
                JavaType::Primitive(new_java2rust::java_ir::PrimitiveType::Int),
                JavaType::Primitive(new_java2rust::java_ir::PrimitiveType::Int),
            ],
        )
        .expect("Interval constructor should be pack-owned by arity");
    assert_eq!(interval.provenance, FactProvenance::ExternalLibraryPack);

    let iterator = facts
        .external_constructor_for(
            "PeekableIterator",
            &[JavaType::Named {
                name: "Iterator".to_string(),
                args: vec![JavaType::Named {
                    name: "Interval".to_string(),
                    args: Vec::new(),
                }],
            }],
        )
        .expect("PeekableIterator constructor should be pack-owned by arity");
    assert_eq!(iterator.provenance, FactProvenance::ExternalLibraryPack);

    let delegating_iterator = facts
        .external_constructor_for(
            "DelegatingIterator",
            &[JavaType::Named {
                name: "Iterator".to_string(),
                args: vec![JavaType::Named {
                    name: "Interval".to_string(),
                    args: Vec::new(),
                }],
            }],
        )
        .expect("DelegatingIterator constructor should be pack-owned by arity");
    assert_eq!(
        delegating_iterator.provenance,
        FactProvenance::ExternalLibraryPack
    );
    assert!(
        facts.has_object_representation_facts(&JavaType::Named {
            name: "DelegatingIterator".to_string(),
            args: Vec::new(),
        }),
        "DelegatingIterator should be an explicit erased external boundary for anonymous implementations"
    );

    let interval_list = facts
        .external_constructor_for(
            "IntervalList",
            &[JavaType::Named {
                name: "SAMFileHeader".to_string(),
                args: Vec::new(),
            }],
        )
        .expect("IntervalList header/dictionary constructor should be pack-owned by arity");
    assert_eq!(
        interval_list.provenance,
        FactProvenance::ExternalLibraryPack
    );

    let interval = JavaType::Named {
        name: "Interval".to_string(),
        args: Vec::new(),
    };
    let comparator = JavaType::Named {
        name: "Comparator".to_string(),
        args: vec![interval.clone()],
    };
    let iterator = JavaType::Named {
        name: "Iterator".to_string(),
        args: vec![interval.clone()],
    };
    let string_arg = JavaType::String;
    let int_arg = JavaType::Primitive(PrimitiveType::Int);

    for (type_name, args) in [
        ("Interval", vec![string_arg.clone()]),
        ("Interval", vec![string_arg.clone(), int_arg.clone()]),
        (
            "Interval",
            vec![string_arg.clone(), int_arg.clone(), int_arg.clone()],
        ),
        (
            "Interval",
            vec![
                string_arg.clone(),
                int_arg.clone(),
                int_arg.clone(),
                JavaType::Primitive(PrimitiveType::Boolean),
                string_arg.clone(),
            ],
        ),
        ("IntervalTree", Vec::new()),
        (
            "IntervalMergerIterator",
            vec![
                iterator.clone(),
                comparator.clone(),
                JavaType::Primitive(PrimitiveType::Boolean),
                JavaType::Primitive(PrimitiveType::Boolean),
            ],
        ),
        ("Histogram", Vec::new()),
        ("Histogram", vec![string_arg.clone(), string_arg.clone()]),
        ("PeekableIterator", vec![iterator.clone()]),
        ("DelegatingIterator", vec![iterator.clone()]),
        ("MergingIterator", vec![comparator, iterator]),
        (
            "DefaultingMap",
            vec![
                JavaType::Named {
                    name: "Supplier".to_string(),
                    args: vec![JavaType::Named {
                        name: "Object".to_string(),
                        args: Vec::new(),
                    }],
                },
                JavaType::Primitive(PrimitiveType::Boolean),
            ],
        ),
        (
            "Lazy",
            vec![JavaType::Named {
                name: "Supplier".to_string(),
                args: vec![JavaType::Named {
                    name: "Object".to_string(),
                    args: Vec::new(),
                }],
            }],
        ),
        ("RuntimeIOException", vec![string_arg.clone()]),
        (
            "RuntimeIOException",
            vec![
                string_arg,
                JavaType::Named {
                    name: "Throwable".to_string(),
                    args: Vec::new(),
                },
            ],
        ),
        (
            "SeekableBufferedStream",
            vec![JavaType::Named {
                name: "SeekableStream".to_string(),
                args: Vec::new(),
            }],
        ),
        (
            "SeekablePathStream",
            vec![JavaType::Named {
                name: "Path".to_string(),
                args: Vec::new(),
            }],
        ),
        ("BinaryCodec", Vec::new()),
        (
            "BinaryCodec",
            vec![JavaType::Named {
                name: "OutputStream".to_string(),
                args: Vec::new(),
            }],
        ),
        (
            "BufferedLineReader",
            vec![JavaType::Named {
                name: "InputStream".to_string(),
                args: Vec::new(),
            }],
        ),
        (
            "AsciiLineReaderIterator",
            vec![JavaType::Named {
                name: "AsciiLineReader".to_string(),
                args: Vec::new(),
            }],
        ),
        (
            "LineIteratorImpl",
            vec![JavaType::Named {
                name: "LineReader".to_string(),
                args: Vec::new(),
            }],
        ),
        (
            "SynchronousLineReader",
            vec![JavaType::Named {
                name: "InputStream".to_string(),
                args: Vec::new(),
            }],
        ),
    ] {
        let constructor = facts
            .external_constructor_for(type_name, &args)
            .unwrap_or_else(|| panic!("{type_name}/{:?} should be pack-owned", args.len()));
        assert_eq!(constructor.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(
            new_java2rust::stdlib::external_constructor_provenance(type_name, &args),
            FactProvenance::ConservativeFallback,
            "{type_name}/{:?} should be owned by the samtools-util external pack, not stdlib",
            args.len()
        );
    }
}

#[test]
fn htsjdk_samtools_util_pack_models_locatable_interval_and_iterator_effects() {
    let facts = htsjdk_samtools_util_pack_facts();

    let start = facts
        .external_api_effect_for_receiver(Some("Locatable"), "getStart")
        .expect("Locatable start getter should be modeled by the pack");
    assert_eq!(start.role, ExternalApiRole::Getter);
    assert_eq!(
        start.return_nullability,
        NullabilityState::DefinitelyNonNull
    );
    assert_eq!(start.provenance, FactProvenance::ExternalLibraryPack);

    let name = facts
        .external_api_effect_for_receiver(Some("Interval"), "getName")
        .expect("Interval name getter should be modeled by the pack");
    assert_eq!(name.return_nullability, NullabilityState::MaybeNull);

    let intersects = facts
        .external_api_effect_for_receiver(Some("Interval"), "intersects")
        .expect("Interval intersection predicate should be modeled by the pack");
    assert_eq!(intersects.role, ExternalApiRole::BooleanGetter);

    let interval_size = facts
        .external_api_effect_for_receiver(Some("Interval"), "size")
        .expect("Interval length getter should be modeled by the pack");
    assert_eq!(interval_size.role, ExternalApiRole::Getter);
    assert_eq!(
        interval_size.return_nullability,
        NullabilityState::DefinitelyNonNull
    );

    let interval_list_from_file = facts
        .external_api_effect_for_receiver(Some("IntervalList"), "fromFile")
        .expect("IntervalList file factory should be modeled by the pack");
    assert_eq!(interval_list_from_file.role, ExternalApiRole::Getter);
    assert_eq!(
        interval_list_from_file.return_type,
        Some(JavaType::Named {
            name: "IntervalList".to_string(),
            args: Vec::new(),
        })
    );

    let interval_tree_put = facts
        .external_api_effect_for_receiver(Some("IntervalTree"), "put")
        .expect("IntervalTree insertion should be modeled by the pack");
    assert_eq!(interval_tree_put.role, ExternalApiRole::Mutator);
    assert_eq!(
        interval_tree_put.receiver_mutability,
        ReceiverMutabilityFact::MutatesReceiver
    );

    let peek = facts
        .external_api_effect_for_receiver(Some("PeekableIterator"), "peek")
        .expect("PeekableIterator peek should be modeled by the pack");
    assert_eq!(peek.role, ExternalApiRole::Getter);
    assert_eq!(peek.return_nullability, NullabilityState::DefinitelyNonNull);

    let delegating_close = facts
        .external_api_effect_for_receiver(Some("DelegatingIterator"), "close")
        .expect("DelegatingIterator close should be modeled by the pack");
    assert_eq!(delegating_close.role, ExternalApiRole::SideEffect);

    let interval_merger_has_next = facts
        .external_api_effect_for_receiver(Some("IntervalMergerIterator"), "hasNext")
        .expect("IntervalMergerIterator cursor predicate should be modeled by the pack");
    assert_eq!(
        interval_merger_has_next.role,
        ExternalApiRole::BooleanGetter
    );
    assert_eq!(
        interval_merger_has_next.return_nullability,
        NullabilityState::DefinitelyNonNull
    );

    let interval_merger_next = facts
        .external_api_effect_for_receiver(Some("IntervalMergerIterator"), "next")
        .expect("IntervalMergerIterator next should be modeled by the pack");
    assert_eq!(interval_merger_next.role, ExternalApiRole::Getter);
    assert_eq!(
        interval_merger_next.return_nullability,
        NullabilityState::DefinitelyNonNull
    );

    let close = facts
        .external_api_effect_for_receiver(Some("CloseableIterator"), "close")
        .expect("CloseableIterator close should be modeled by the pack");
    assert_eq!(close.role, ExternalApiRole::SideEffect);

    let remove = facts
        .external_api_effect_for_receiver(Some("CloseableIterator"), "remove")
        .expect(
            "CloseableIterator remove should be modeled as a side-effecting iterator operation",
        );
    assert_eq!(remove.role, ExternalApiRole::SideEffect);

    for method_name in ["flush", "close"] {
        let effect = facts
            .external_api_effect_for_receiver(Some("BlockCompressedOutputStream"), method_name)
            .unwrap_or_else(|| {
                panic!("BlockCompressedOutputStream.{method_name} should be modeled by the pack")
            });
        assert_eq!(effect.role, ExternalApiRole::SideEffect);
        assert_eq!(
            effect.receiver_mutability,
            ReceiverMutabilityFact::MutatesReceiver
        );
    }

    for (method_name, arity) in [("writeBytes", 1), ("writeInt", 1), ("writeString", 3)] {
        let effect = facts
            .external_api_effect_for_receiver_with_arity(
                Some("BinaryCodec"),
                method_name,
                Some(arity),
            )
            .unwrap_or_else(|| panic!("BinaryCodec.{method_name}/{arity} should be modeled"));
        assert_eq!(effect.role, ExternalApiRole::Mutator);
        assert_eq!(
            effect.receiver_mutability,
            ReceiverMutabilityFact::MutatesReceiver
        );
        assert_eq!(effect.provenance, FactProvenance::ExternalLibraryPack);
    }

    let value_label = facts
        .external_api_effect_for_receiver(Some("Histogram"), "getValueLabel")
        .expect("Histogram value-label getter should be modeled by the pack");
    assert_eq!(value_label.return_type, Some(JavaType::String));
    assert_eq!(
        value_label.return_nullability,
        NullabilityState::DefinitelyNonNull
    );

    let get = facts
        .external_api_effect_for_receiver(Some("Histogram"), "get")
        .expect("Histogram indexed lookup should be modeled by the pack");
    assert_eq!(get.role, ExternalApiRole::IndexedGetter);
    assert_eq!(get.return_nullability, NullabilityState::MaybeNull);

    let phred_to_fastq = facts
        .external_api_effect_for_receiver(Some("SAMUtils"), "phredToFastq")
        .expect("SAMUtils.phredToFastq should be modeled by the pack");
    assert_eq!(phred_to_fastq.role, ExternalApiRole::Getter);
    assert_eq!(phred_to_fastq.return_type, Some(JavaType::String));
    assert_eq!(
        phred_to_fastq.return_nullability,
        NullabilityState::DefinitelyNonNull
    );

    let cigar_maps_no_bases = facts
        .external_api_effect_for_receiver(Some("SAMUtils"), "cigarMapsNoBasesToRef")
        .expect("SAMUtils.cigarMapsNoBasesToRef should be modeled by the pack");
    assert_eq!(cigar_maps_no_bases.role, ExternalApiRole::BooleanGetter);
    assert_eq!(
        cigar_maps_no_bases.return_type,
        Some(JavaType::Primitive(PrimitiveType::Boolean))
    );

    for method_name in ["combineMapqs", "compareMapqs", "getMateAlignmentEnd"] {
        let mapq = facts
            .external_api_effect_for_receiver(Some("SAMUtils"), method_name)
            .unwrap_or_else(|| panic!("SAMUtils.{method_name} should be modeled by the pack"));
        assert_eq!(
            mapq.return_type,
            Some(JavaType::Primitive(PrimitiveType::Int))
        );
    }

    for method_name in ["hasMateCigar", "recordMapsEntirelyBeyondEndOfReference"] {
        let predicate = facts
            .external_api_effect_for_receiver(Some("SAMUtils"), method_name)
            .unwrap_or_else(|| panic!("SAMUtils.{method_name} should be modeled by the pack"));
        assert_eq!(predicate.role, ExternalApiRole::BooleanGetter);
        assert_eq!(
            predicate.return_type,
            Some(JavaType::Primitive(PrimitiveType::Boolean))
        );
    }

    let mate_cigar = facts
        .external_api_effect_for_receiver(Some("SAMUtils"), "getMateCigar")
        .expect("SAMUtils.getMateCigar should be modeled by the pack");
    assert_eq!(
        mate_cigar.return_type,
        Some(JavaType::Named {
            name: "Cigar".to_string(),
            args: Vec::new(),
        })
    );
    assert_eq!(mate_cigar.return_nullability, NullabilityState::MaybeNull);

    for method_name in [
        "setMateInfo",
        "setMateInformationOnSupplementalAlignment",
        "setProperPairFlags",
    ] {
        let effect = facts
            .external_api_effect_for_receiver(Some("SamPairUtil"), method_name)
            .unwrap_or_else(|| panic!("SamPairUtil.{method_name} should be modeled by the pack"));
        assert_eq!(effect.role, ExternalApiRole::SideEffect);
    }

    let chain_program = facts
        .external_api_effect_for_receiver(Some("SAMUtils"), "chainSAMProgramRecord")
        .expect("SAMUtils.chainSAMProgramRecord should be modeled by the pack");
    assert_eq!(
        chain_program.return_type,
        Some(JavaType::Named {
            name: "SAMProgramRecord".to_string(),
            args: Vec::new(),
        })
    );
    assert_eq!(
        chain_program.return_nullability,
        NullabilityState::MaybeNull
    );

    let make_read_unmapped = facts
        .external_api_effect_for_receiver(Some("SAMUtils"), "makeReadUnmapped")
        .expect("SAMUtils.makeReadUnmapped should be modeled by the pack");
    assert_eq!(make_read_unmapped.role, ExternalApiRole::SideEffect);

    let fastq_to_phred = facts
        .external_api_effect_for_receiver(Some("SAMUtils"), "fastqToPhred")
        .expect("SAMUtils.fastqToPhred should be modeled by the pack");
    assert_eq!(fastq_to_phred.role, ExternalApiRole::Getter);
    assert_eq!(fastq_to_phred.return_type, None);
    assert_eq!(
        fastq_to_phred.return_nullability,
        NullabilityState::DefinitelyNonNull
    );

    let is_no_call = facts
        .external_api_effect_for_receiver(Some("SequenceUtil"), "isNoCall")
        .expect("SequenceUtil.isNoCall should be modeled by the pack");
    assert_eq!(is_no_call.role, ExternalApiRole::BooleanGetter);
    assert_eq!(
        is_no_call.return_type,
        Some(JavaType::Primitive(PrimitiveType::Boolean))
    );

    let reverse_complement = facts
        .external_api_effect_for_receiver(Some("SequenceUtil"), "reverseComplement")
        .expect("SequenceUtil.reverseComplement should be modeled by the pack");
    assert_eq!(reverse_complement.role, ExternalApiRole::Getter);
    assert_eq!(
        reverse_complement.return_type,
        Some(JavaType::Named {
            name: "Object".to_string(),
            args: Vec::new(),
        })
    );

    let nm_tag = facts
        .external_api_effect_for_receiver(Some("SequenceUtil"), "calculateSamNmTag")
        .expect("SequenceUtil.calculateSamNmTag should be modeled by the pack");
    assert_eq!(
        nm_tag.return_type,
        Some(JavaType::Primitive(PrimitiveType::Int))
    );

    let mismatch_qualities = facts
        .external_api_effect_for_receiver(Some("SequenceUtil"), "sumQualitiesOfMismatches")
        .expect("SequenceUtil.sumQualitiesOfMismatches should be modeled by the pack");
    assert_eq!(
        mismatch_qualities.return_type,
        Some(JavaType::Primitive(PrimitiveType::Int))
    );

    let count = facts
        .external_api_effect_for_receiver(Some("Histogram"), "getCount")
        .expect("Histogram count getter should be modeled by the pack");
    assert_eq!(
        count.return_type,
        Some(JavaType::Primitive(
            new_java2rust::java_ir::PrimitiveType::Double
        ))
    );

    for method_name in [
        "getSumOfValues",
        "getMin",
        "getMax",
        "getMean",
        "getStandardDeviation",
        "getMedian",
        "getMedianAbsoluteDeviation",
    ] {
        let statistic = facts
            .external_api_effect_for_receiver(Some("Histogram"), method_name)
            .expect("Histogram statistic getter should be modeled by the pack");
        assert_eq!(
            statistic.return_type,
            Some(JavaType::Primitive(
                new_java2rust::java_ir::PrimitiveType::Double
            ))
        );
    }

    let key_set = facts
        .external_api_effect_for_receiver(Some("Histogram"), "keySet")
        .expect("Histogram key-set getter should be modeled by the pack");
    assert_eq!(
        key_set.return_type,
        Some(JavaType::Named {
            name: "Set".to_string(),
            args: vec![JavaType::Named {
                name: "Object".to_string(),
                args: Vec::new(),
            }],
        })
    );

    let values = facts
        .external_api_effect_for_receiver(Some("Histogram"), "values")
        .expect("Histogram values getter should be modeled by the pack");
    assert_eq!(
        values.return_type,
        Some(JavaType::Named {
            name: "Collection".to_string(),
            args: vec![JavaType::Named {
                name: "Object".to_string(),
                args: Vec::new(),
            }],
        })
    );

    let increment = facts
        .external_api_effect_for_receiver(Some("Histogram"), "increment")
        .expect("Histogram increment should be modeled by the pack");
    assert_eq!(increment.role, ExternalApiRole::Mutator);
    assert_eq!(
        increment.receiver_mutability,
        ReceiverMutabilityFact::MutatesReceiver
    );
}

#[test]
fn semantic_analysis_uses_htsjdk_header_and_binary_codec_packs_for_header_writes() {
    let semantic = analyze(
        r#"
import htsjdk.samtools.SAMFileHeader;
import htsjdk.samtools.SAMTextHeaderCodec;
import htsjdk.samtools.util.BinaryCodec;
import java.io.Writer;

class HtsjdkHeaderWriteUse {
    void write(
            BinaryCodec codec,
            SAMTextHeaderCodec headerCodec,
            Writer writer,
            SAMFileHeader header,
            byte[] magic,
            String text) {
        headerCodec.encode(writer, header, true);
        codec.writeBytes(magic);
        codec.writeString(text, true, false);
        codec.writeInt(header.getSequenceDictionary().size());
    }
}
"#,
    );

    assert!(
        semantic
            .diagnostics
            .iter()
            .all(|diagnostic| diagnostic.code != "external_pack_missing_api"),
        "modeled HTSJDK header/binary codec helpers should not report pack gaps: {:?}",
        semantic.diagnostics
    );

    let encode = semantic
        .facts
        .external_api_effect_for_receiver_with_arity(Some("SAMTextHeaderCodec"), "encode", Some(3))
        .expect("semantic analysis should use the SAM header pack for SAMTextHeaderCodec.encode/3");
    assert_eq!(encode.role, ExternalApiRole::SideEffect);
    assert_eq!(encode.provenance, FactProvenance::ExternalLibraryPack);

    for (method_name, arity) in [("writeBytes", 1), ("writeString", 3), ("writeInt", 1)] {
        let effect = semantic
            .facts
            .external_api_effect_for_receiver_with_arity(
                Some("BinaryCodec"),
                method_name,
                Some(arity),
            )
            .unwrap_or_else(|| {
                panic!(
                    "semantic analysis should use the samtools-util pack for BinaryCodec.{method_name}/{arity}"
                )
            });
        assert_eq!(effect.role, ExternalApiRole::Mutator);
        assert_eq!(effect.provenance, FactProvenance::ExternalLibraryPack);
    }
}

#[test]
fn semantic_analysis_uses_samtools_util_pack_for_interval_workflows() {
    let semantic = analyze(
        r#"
import htsjdk.samtools.util.Histogram;
import htsjdk.samtools.util.Interval;
import htsjdk.samtools.util.IntervalTree;
import htsjdk.samtools.util.Locatable;
import htsjdk.samtools.util.PeekableIterator;
import htsjdk.samtools.util.BlockCompressedOutputStream;
import htsjdk.samtools.SAMUtils;
import htsjdk.samtools.SamPairUtil;
import htsjdk.samtools.SAMFileHeader;
import htsjdk.samtools.SAMProgramRecord;
import htsjdk.samtools.SAMRecord;
import htsjdk.samtools.Cigar;
import htsjdk.samtools.util.SequenceUtil;
import java.io.File;
import java.io.OutputStream;
import java.util.Iterator;

class SamtoolsUtilPackUse {
    Interval interval(String contig, int start, int end) {
        return new Interval(contig, start, end);
    }

    int width(Locatable value) {
        return value.getLengthOnReference() + value.getEnd() - value.getStart();
    }

    boolean contains(Locatable left, Locatable right) {
        return left.contains(right);
    }

    boolean locatableOverlap(Locatable left, Locatable right) {
        return left.overlaps(right);
    }

    boolean sameContig(Locatable left, Locatable right) {
        return left.contigsMatch(right);
    }

    boolean overlaps(Interval left, Interval right) {
        return left.intersects(right);
    }

    Object peek(Iterator<Interval> intervals) {
        return new PeekableIterator<Interval>(intervals).peek();
    }

    Object firstOverlap(IntervalTree<String> tree) {
        return tree.minOverlapper(1, 10);
    }

    String valueLabel(Histogram<Integer> histogram) {
        return histogram.getValueLabel();
    }

    Object bucket(Histogram<Integer> histogram) {
        return histogram.get(1);
    }

    Object keys(Histogram<Integer> histogram) {
        return histogram.keySet();
    }

    Object bins(Histogram<Integer> histogram) {
        return histogram.values();
    }

    String quality(byte q) {
        return SAMUtils.phredToFastq(q);
    }

    boolean noReferenceBases(Cigar cigar) {
        return SAMUtils.cigarMapsNoBasesToRef(cigar);
    }

    int mappingQuality(int left, int right) {
        return SAMUtils.combineMapqs(left, right) + SAMUtils.compareMapqs(left, right);
    }

    int mateEnd(SAMRecord record) {
        return SAMUtils.getMateAlignmentEnd(record);
    }

    boolean mateCigar(SAMRecord record) {
        return SAMUtils.hasMateCigar(record);
    }

    boolean beyondReference(SAMRecord record) {
        return SAMUtils.recordMapsEntirelyBeyondEndOfReference(record, null);
    }

    Object mateCigarObject(SAMRecord record) {
        return SAMUtils.getMateCigar(record);
    }

    void pair(SAMRecord left, SAMRecord right) {
        SamPairUtil.setMateInfo(left, right, null);
        SamPairUtil.setMateInformationOnSupplementalAlignment(left, right);
        SamPairUtil.setProperPairFlags(left, right);
    }

    Object chain(SAMFileHeader header, SAMProgramRecord program) {
        return SAMUtils.chainSAMProgramRecord(header, program);
    }

    void unmap(SAMRecord record) {
        SAMUtils.makeReadUnmapped(record);
    }

    Object phred(String qualities) {
        return SAMUtils.fastqToPhred(qualities);
    }

    boolean noCall(byte base) {
        return SequenceUtil.isNoCall(base);
    }

    Object reverse(String bases) {
        return SequenceUtil.reverseComplement(bases);
    }

    int nm(SAMRecord record) {
        return SequenceUtil.calculateSamNmTag(record, null);
    }

    int mismatchQualities(SAMRecord record) {
        return SequenceUtil.sumQualitiesOfMismatches(record, null);
    }

    double count(Histogram<Integer> histogram) {
        histogram.increment(1);
        histogram.prefillBins(0, 10);
        histogram.trimByWidth(100);
        return histogram.getCount();
    }

    double stats(Histogram<Integer> histogram) {
        return histogram.getSumOfValues() + histogram.getMin() + histogram.getMax()
            + histogram.getMean() + histogram.getStandardDeviation()
            + histogram.getMedian() + histogram.getMedianAbsoluteDeviation();
    }

    void writeBlocks(File outputFile, OutputStream output) {
        BlockCompressedOutputStream fileWriter = new BlockCompressedOutputStream(outputFile, 5);
        BlockCompressedOutputStream streamWriter =
            new BlockCompressedOutputStream(output, outputFile, 5);
        fileWriter.flush();
        streamWriter.close();
    }
}
"#,
    );

    let start = semantic
        .facts
        .external_api_effect_for_receiver(Some("Locatable"), "getStart")
        .unwrap();
    assert_eq!(start.provenance, FactProvenance::ExternalLibraryPack);
    assert_eq!(
        start.return_nullability,
        NullabilityState::DefinitelyNonNull
    );

    let contains = semantic
        .facts
        .external_api_effect_for_receiver(Some("Locatable"), "contains")
        .expect("Locatable contains predicate should be modeled by the pack");
    assert_eq!(contains.role, ExternalApiRole::BooleanGetter);

    let locatable_overlaps = semantic
        .facts
        .external_api_effect_for_receiver(Some("Locatable"), "overlaps")
        .expect("Locatable overlap predicate should be modeled by the pack");
    assert_eq!(locatable_overlaps.role, ExternalApiRole::BooleanGetter);
    assert_eq!(
        locatable_overlaps.return_nullability,
        NullabilityState::DefinitelyNonNull
    );

    let contigs_match = semantic
        .facts
        .external_api_effect_for_receiver(Some("Locatable"), "contigsMatch")
        .expect("Locatable contig comparison predicate should be modeled by the pack");
    assert_eq!(contigs_match.role, ExternalApiRole::BooleanGetter);
    assert_eq!(
        contigs_match.return_nullability,
        NullabilityState::DefinitelyNonNull
    );

    let length_on_reference = semantic
        .facts
        .external_api_effect_for_receiver(Some("Locatable"), "getLengthOnReference")
        .expect("Locatable reference-length getter should be modeled by the pack");
    assert_eq!(
        length_on_reference.return_type,
        Some(JavaType::Primitive(
            new_java2rust::java_ir::PrimitiveType::Int
        ))
    );

    let intersects = semantic
        .facts
        .external_api_effect_for_receiver(Some("Interval"), "intersects")
        .unwrap();
    assert_eq!(intersects.role, ExternalApiRole::BooleanGetter);
    assert_eq!(intersects.provenance, FactProvenance::ExternalLibraryPack);

    let min_overlapper = semantic
        .facts
        .external_api_effect_for_receiver(Some("IntervalTree"), "minOverlapper")
        .expect("IntervalTree minimum-overlap lookup should be modeled by the pack");
    assert_eq!(min_overlapper.role, ExternalApiRole::Getter);
    assert_eq!(
        min_overlapper.return_nullability,
        NullabilityState::MaybeNull
    );

    let interval = semantic
        .facts
        .external_constructor_for(
            "Interval",
            &[
                JavaType::String,
                JavaType::Primitive(new_java2rust::java_ir::PrimitiveType::Int),
                JavaType::Primitive(new_java2rust::java_ir::PrimitiveType::Int),
            ],
        )
        .unwrap();
    assert_eq!(interval.provenance, FactProvenance::ExternalLibraryPack);

    let value_label = semantic
        .facts
        .external_api_effect_for_receiver(Some("Histogram"), "getValueLabel")
        .expect("Histogram value-label getter should be modeled by the pack");
    assert_eq!(value_label.return_type, Some(JavaType::String));
    assert_eq!(value_label.provenance, FactProvenance::ExternalLibraryPack);

    let get = semantic
        .facts
        .external_api_effect_for_receiver(Some("Histogram"), "get")
        .expect("Histogram indexed lookup should be modeled by the pack");
    assert_eq!(get.role, ExternalApiRole::IndexedGetter);
    assert_eq!(get.return_nullability, NullabilityState::MaybeNull);

    let increment = semantic
        .facts
        .external_api_effect_for_receiver(Some("Histogram"), "increment")
        .expect("Histogram increment should be modeled by the pack");
    assert_eq!(increment.role, ExternalApiRole::Mutator);

    let phred_to_fastq = semantic
        .facts
        .external_api_effect_for_receiver(Some("SAMUtils"), "phredToFastq")
        .expect("SAMUtils.phredToFastq should be modeled by the pack");
    assert_eq!(phred_to_fastq.return_type, Some(JavaType::String));
    assert_eq!(
        phred_to_fastq.provenance,
        FactProvenance::ExternalLibraryPack
    );

    let cigar_maps_no_bases = semantic
        .facts
        .external_api_effect_for_receiver(Some("SAMUtils"), "cigarMapsNoBasesToRef")
        .expect("SAMUtils.cigarMapsNoBasesToRef should be modeled by the pack");
    assert_eq!(cigar_maps_no_bases.role, ExternalApiRole::BooleanGetter);

    for method_name in ["combineMapqs", "compareMapqs", "getMateAlignmentEnd"] {
        let effect = semantic
            .facts
            .external_api_effect_for_receiver(Some("SAMUtils"), method_name)
            .unwrap_or_else(|| panic!("SAMUtils.{method_name} should be modeled by the pack"));
        assert_eq!(
            effect.return_type,
            Some(JavaType::Primitive(
                new_java2rust::java_ir::PrimitiveType::Int
            ))
        );
    }

    for method_name in ["hasMateCigar", "recordMapsEntirelyBeyondEndOfReference"] {
        let predicate = semantic
            .facts
            .external_api_effect_for_receiver(Some("SAMUtils"), method_name)
            .unwrap_or_else(|| panic!("SAMUtils.{method_name} should be modeled by the pack"));
        assert_eq!(predicate.role, ExternalApiRole::BooleanGetter);
    }

    let mate_cigar = semantic
        .facts
        .external_api_effect_for_receiver(Some("SAMUtils"), "getMateCigar")
        .expect("SAMUtils.getMateCigar should be modeled by the pack");
    assert_eq!(
        mate_cigar.return_type,
        Some(JavaType::Named {
            name: "Cigar".to_string(),
            args: Vec::new(),
        })
    );
    assert_eq!(mate_cigar.return_nullability, NullabilityState::MaybeNull);

    for method_name in [
        "setMateInfo",
        "setMateInformationOnSupplementalAlignment",
        "setProperPairFlags",
    ] {
        let effect = semantic
            .facts
            .external_api_effect_for_receiver(Some("SamPairUtil"), method_name)
            .unwrap_or_else(|| panic!("SamPairUtil.{method_name} should be modeled by the pack"));
        assert_eq!(effect.role, ExternalApiRole::SideEffect);
    }

    let chain_program = semantic
        .facts
        .external_api_effect_for_receiver(Some("SAMUtils"), "chainSAMProgramRecord")
        .expect("SAMUtils.chainSAMProgramRecord should be modeled by the pack");
    assert_eq!(
        chain_program.provenance,
        FactProvenance::ExternalLibraryPack
    );

    let make_read_unmapped = semantic
        .facts
        .external_api_effect_for_receiver(Some("SAMUtils"), "makeReadUnmapped")
        .expect("SAMUtils.makeReadUnmapped should be modeled by the pack");
    assert_eq!(make_read_unmapped.role, ExternalApiRole::SideEffect);

    let fastq_to_phred = semantic
        .facts
        .external_api_effect_for_receiver(Some("SAMUtils"), "fastqToPhred")
        .expect("SAMUtils.fastqToPhred should be modeled by the pack");
    assert_eq!(fastq_to_phred.return_type, None);
    assert_eq!(
        fastq_to_phred.provenance,
        FactProvenance::ExternalLibraryPack
    );

    let is_no_call = semantic
        .facts
        .external_api_effect_for_receiver(Some("SequenceUtil"), "isNoCall")
        .expect("SequenceUtil.isNoCall should be modeled by the pack");
    assert_eq!(is_no_call.role, ExternalApiRole::BooleanGetter);
    assert_eq!(is_no_call.provenance, FactProvenance::ExternalLibraryPack);

    let reverse_complement = semantic
        .facts
        .external_api_effect_for_receiver(Some("SequenceUtil"), "reverseComplement")
        .expect("SequenceUtil.reverseComplement should be modeled by the pack");
    assert_eq!(
        reverse_complement.provenance,
        FactProvenance::ExternalLibraryPack
    );

    let nm_tag = semantic
        .facts
        .external_api_effect_for_receiver(Some("SequenceUtil"), "calculateSamNmTag")
        .expect("SequenceUtil.calculateSamNmTag should be modeled by the pack");
    assert_eq!(
        nm_tag.return_type,
        Some(JavaType::Primitive(
            new_java2rust::java_ir::PrimitiveType::Int
        ))
    );

    let mismatch_qualities = semantic
        .facts
        .external_api_effect_for_receiver(Some("SequenceUtil"), "sumQualitiesOfMismatches")
        .expect("SequenceUtil.sumQualitiesOfMismatches should be modeled by the pack");
    assert_eq!(
        mismatch_qualities.return_type,
        Some(JavaType::Primitive(
            new_java2rust::java_ir::PrimitiveType::Int
        ))
    );

    let prefill_bins = semantic
        .facts
        .external_api_effect_for_receiver(Some("Histogram"), "prefillBins")
        .expect("Histogram prefill mutator should be modeled by the pack");
    assert_eq!(prefill_bins.role, ExternalApiRole::Mutator);

    let trim_by_width = semantic
        .facts
        .external_api_effect_for_receiver(Some("Histogram"), "trimByWidth")
        .expect("Histogram width trim mutator should be modeled by the pack");
    assert_eq!(trim_by_width.role, ExternalApiRole::Mutator);

    let count = semantic
        .facts
        .external_api_effect_for_receiver(Some("Histogram"), "getCount")
        .expect("Histogram count getter should be modeled by the pack");
    assert_eq!(
        count.return_type,
        Some(JavaType::Primitive(
            new_java2rust::java_ir::PrimitiveType::Double
        ))
    );

    for method_name in [
        "getSumOfValues",
        "getMin",
        "getMax",
        "getMean",
        "getStandardDeviation",
        "getMedian",
        "getMedianAbsoluteDeviation",
    ] {
        let statistic = semantic
            .facts
            .external_api_effect_for_receiver(Some("Histogram"), method_name)
            .expect("Histogram statistic getter should be modeled by the pack");
        assert_eq!(
            statistic.return_type,
            Some(JavaType::Primitive(
                new_java2rust::java_ir::PrimitiveType::Double
            ))
        );
    }

    let key_set = semantic
        .facts
        .external_api_effect_for_receiver(Some("Histogram"), "keySet")
        .expect("Histogram key-set getter should be modeled by the pack");
    assert_eq!(key_set.role, ExternalApiRole::Getter);

    let values = semantic
        .facts
        .external_api_effect_for_receiver(Some("Histogram"), "values")
        .expect("Histogram values getter should be modeled by the pack");
    assert_eq!(values.role, ExternalApiRole::Getter);
    assert_eq!(
        values.return_nullability,
        NullabilityState::DefinitelyNonNull
    );

    let output_args = [
        JavaType::Named {
            name: "File".to_string(),
            args: Vec::new(),
        },
        JavaType::Primitive(PrimitiveType::Int),
    ];
    let output_constructor = semantic
        .facts
        .external_constructor_for("BlockCompressedOutputStream", &output_args)
        .expect("BlockCompressedOutputStream/2 should be modeled by the samtools-util pack");
    assert_eq!(
        output_constructor.provenance,
        FactProvenance::ExternalLibraryPack
    );

    let flush = semantic
        .facts
        .external_api_effect_for_receiver(Some("BlockCompressedOutputStream"), "flush")
        .expect("BlockCompressedOutputStream.flush should be modeled by the pack");
    assert_eq!(flush.role, ExternalApiRole::SideEffect);
    assert_eq!(flush.provenance, FactProvenance::ExternalLibraryPack);

    let close = semantic
        .facts
        .external_api_effect_for_receiver(Some("BlockCompressedOutputStream"), "close")
        .expect("BlockCompressedOutputStream.close should be modeled by the pack");
    assert_eq!(close.role, ExternalApiRole::SideEffect);
    assert_eq!(close.provenance, FactProvenance::ExternalLibraryPack);

    assert!(
        !semantic
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "external_pack_missing_api"),
        "newly modeled samtools util calls should not produce missing-pack diagnostics: {:?}",
        semantic.diagnostics
    );
}

#[test]
fn user_override_beats_variantcontext_pack_fact() {
    let mut facts = htsjdk_variantcontext_pack_facts();
    let override_facts = parse_external_facts(
        "external-api getAttribute VariantContext getter read-only definitely-non-null",
    );
    facts.extend_from(override_facts.facts);

    let attribute = facts
        .external_api_effect_for_receiver(Some("VariantContext"), "getAttribute")
        .unwrap();
    assert_eq!(
        attribute.return_nullability,
        NullabilityState::DefinitelyNonNull
    );
    assert_eq!(attribute.provenance, FactProvenance::UserOverride);
}

#[test]
fn semantic_analysis_uses_user_override_for_variantcontext_pack_flow() {
    let source = r#"
import htsjdk.variant.variantcontext.VariantContext;

class VariantContextOverrideUse {
    Object attribute(VariantContext variant) {
        return variant.getAttribute("DP");
    }
}
"#;
    let parsed = parser::parse_java_source(source);
    let resolved = resolve::resolve_program(&parsed.program, parsed.diagnostics);
    let mut facts = htsjdk_variantcontext_pack_facts();
    let override_facts = parse_external_facts(
        "external-api getAttribute VariantContext getter read-only definitely-non-null args=1 return=Object",
    );
    assert!(
        override_facts.diagnostics.is_empty(),
        "{:?}",
        override_facts.diagnostics
    );
    facts.extend_from(override_facts.facts);

    let semantic = semantic_analysis::analyze_program(&resolved.program, &resolved.facts, &facts);
    let attribute = semantic
        .facts
        .external_api_effect_for_receiver_with_arity(
            Some("VariantContext"),
            "getAttribute",
            Some(1),
        )
        .expect("user override should still expose a VariantContext getAttribute fact");
    assert_eq!(attribute.provenance, FactProvenance::UserOverride);
    assert_eq!(
        attribute.return_nullability,
        NullabilityState::DefinitelyNonNull
    );
    assert_eq!(
        attribute.return_type,
        Some(JavaType::Named {
            name: "Object".to_string(),
            args: Vec::new(),
        })
    );
    assert!(
        !semantic
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "external_pack_missing_api"
                && diagnostic.message.contains("VariantContext.getAttribute")),
        "user-overridden pack APIs should remain covered by the external boundary: {:?}",
        semantic.diagnostics
    );
}

#[test]
fn inherited_object_methods_on_pack_types_are_builtin_not_pack_specific() {
    let semantic = analyze(
        r#"
import htsjdk.variant.variantcontext.Allele;

class ObjectMethodOnPackTypeUse {
    boolean same(Allele left, Allele right) {
        return left.equals(right);
    }
}
"#,
    );

    let equals = semantic
        .facts
        .external_api_effect_for_receiver_with_arity(Some("Allele"), "equals", Some(1))
        .expect("Object.equals should be modeled as a builtin inherited method");
    assert_eq!(equals.role, ExternalApiRole::Getter);
    assert_eq!(
        equals.return_nullability,
        NullabilityState::DefinitelyNonNull
    );
    assert_eq!(equals.provenance, FactProvenance::BuiltinExternalModel);
    assert!(
        !semantic
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "external_pack_missing_api"
                && diagnostic.message.contains("Allele.equals")),
        "inherited Object methods should not produce pack-missing diagnostics: {:?}",
        semantic.diagnostics
    );
}

#[test]
fn htsjdk_allele_equals_overload_is_arity_specific_pack_fact() {
    let semantic = analyze(
        r#"
import htsjdk.variant.variantcontext.Allele;

class AlleleEqualsOverloadUse {
    boolean sameIgnoringReferenceState(Allele left, Allele right) {
        return left.equals(right, true);
    }
}
"#,
    );

    let equals = semantic
        .facts
        .external_api_effect_for_receiver_with_arity(Some("Allele"), "equals", Some(2))
        .expect("HTSJDK two-argument Allele.equals overload should be modeled by the pack");
    assert_eq!(equals.role, ExternalApiRole::BooleanGetter);
    assert_eq!(
        equals.return_nullability,
        NullabilityState::DefinitelyNonNull
    );
    assert_eq!(equals.provenance, FactProvenance::ExternalLibraryPack);
    assert!(
        !semantic
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "external_pack_missing_api"
                && diagnostic.message.contains("Allele.equals")),
        "modeled Allele.equals overload should not produce pack-missing diagnostics: {:?}",
        semantic.diagnostics
    );
}

#[test]
fn sam_header_nested_enum_name_is_pack_owned_non_null_getter() {
    let semantic = analyze(
        r#"
import htsjdk.samtools.SAMFileHeader;

class NestedEnumNameUse {
    String sortOrderName() {
        return SAMFileHeader.SortOrder.coordinate.name();
    }
}
"#,
    );

    let name = semantic
        .facts
        .external_api_effect_for_receiver_with_arity(Some("SortOrder"), "name", Some(0))
        .expect("SAM header nested enum name() should be modeled by the SAM header pack");
    assert_eq!(name.role, ExternalApiRole::Getter);
    assert_eq!(name.return_nullability, NullabilityState::DefinitelyNonNull);
    assert_eq!(name.provenance, FactProvenance::ExternalLibraryPack);
    assert!(
        !semantic
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "external_pack_missing_api"
                && diagnostic.message.contains("SortOrder.name")),
        "SAM header nested enum name() should not produce pack-missing diagnostics: {:?}",
        semantic.diagnostics
    );
}

#[test]
fn user_override_beats_crate_mapping_and_builtin_external_facts() {
    let mut hdf5_facts = hdf5_pack_facts();
    hdf5_facts
        .external_api_effect_facts
        .push(new_java2rust::facts::ExternalApiEffectFact {
            method_name: "exists".to_string(),
            receiver_type: Some("HDF5File".to_string()),
            arg_count: None,
            role: ExternalApiRole::Getter,
            receiver_mutability: ReceiverMutabilityFact::ReadOnly,
            return_type: None,
            return_nullability: NullabilityState::MaybeNull,
            thrown: Vec::new(),
            provenance: FactProvenance::UserOverride,
        });
    let exists = hdf5_facts
        .external_api_effect_for_receiver(Some("HDF5File"), "exists")
        .unwrap();
    assert_eq!(exists.provenance, FactProvenance::UserOverride);
    assert_eq!(exists.role, ExternalApiRole::Getter);
    assert_eq!(exists.return_nullability, NullabilityState::MaybeNull);

    let mut builtin_facts = FactSet::default();
    builtin_facts
        .external_api_effect_facts
        .push(new_java2rust::facts::ExternalApiEffectFact {
            method_name: "size".to_string(),
            receiver_type: Some("List".to_string()),
            arg_count: None,
            role: ExternalApiRole::Getter,
            receiver_mutability: ReceiverMutabilityFact::ReadOnly,
            return_type: None,
            return_nullability: NullabilityState::DefinitelyNonNull,
            thrown: Vec::new(),
            provenance: FactProvenance::BuiltinExternalModel,
        });
    builtin_facts
        .external_api_effect_facts
        .push(new_java2rust::facts::ExternalApiEffectFact {
            method_name: "size".to_string(),
            receiver_type: Some("List".to_string()),
            arg_count: None,
            role: ExternalApiRole::Unknown,
            receiver_mutability: ReceiverMutabilityFact::Unknown,
            return_type: None,
            return_nullability: NullabilityState::Unknown,
            thrown: Vec::new(),
            provenance: FactProvenance::UserOverride,
        });
    let size = builtin_facts
        .external_api_effect_for_receiver(Some("List"), "size")
        .unwrap();
    assert_eq!(size.provenance, FactProvenance::UserOverride);
    assert_eq!(size.role, ExternalApiRole::Unknown);
}

#[test]
fn semantic_analysis_uses_variantcontext_pack_before_generic_guessing() {
    let semantic = analyze(
        r#"
import htsjdk.variant.variantcontext.Allele;
import htsjdk.variant.variantcontext.Genotype;
import htsjdk.variant.variantcontext.GenotypeBuilder;
import htsjdk.variant.variantcontext.GenotypeLikelihoods;
import htsjdk.variant.variantcontext.GenotypeType;
import htsjdk.variant.variantcontext.GenotypesContext;
import htsjdk.variant.variantcontext.VariantContext;
import htsjdk.variant.variantcontext.VariantContextBuilder;
import htsjdk.variant.variantcontext.VariantContextComparator;
import htsjdk.samtools.SAMSequenceDictionary;

class VariantPackUse {
    VariantContextComparator comparator(SAMSequenceDictionary dictionary) {
        return new VariantContextComparator(dictionary);
    }

    VariantContext build(String contig, int start, Allele ref, Allele alt) {
        return new VariantContextBuilder()
            .chr(contig)
            .start(start)
            .stop(start)
            .alleles(java.util.Arrays.asList(ref, alt))
            .make();
    }

    Object attr(VariantContext vc) {
        return vc.getAttribute("DP");
    }

    Object sample(VariantContext vc) {
        return vc.getGenotype("SAMPLE");
    }

    int metrics(VariantContext vc, Genotype genotype) {
        return vc.getPloidy()
            + vc.getNAlleles()
            + vc.getNSamples()
            + vc.getCalledChrCount()
            + vc.getHetCount()
            + vc.getHomVarCount()
            + vc.getMaxPloidy()
            + vc.getNoCallCount()
            + (int) vc.getLog10PError()
            + vc.getAttributeAsInt("DP", 0)
            + (int) vc.getAttributeAsDouble("AF", 0.0)
            + (vc.getAttributeAsBoolean("SOMATIC", false) ? 1 : 0)
            + (int) vc.getPhredScaledQual()
            + vc.getLengthOnReference()
            + genotype.getPloidy()
            + genotype.getDP();
    }

    Object allele(VariantContext vc, Genotype genotype, Allele allele) {
        if (vc.isSymbolic()
            || vc.isVariant()
            || vc.isNotFiltered()
            || vc.isMixed()
            || vc.isMNP()
            || vc.isSimpleInsertion()
            || vc.isSimpleDeletion()
            || vc.isComplexIndel()
            || vc.isStructuralIndel()
            || vc.isSymbolicOrSV()
            || vc.isMonomorphicInSamples()
            || vc.isPolymorphicInSamples()
            || vc.hasSymbolicAlleles()
            || vc.overlaps(vc)
            || vc.hasID()
            || vc.hasAllele(allele)
            || vc.hasAlternateAllele(allele)
            || vc.contains(vc)
            || vc.withinDistanceOf(vc, 10)
            || vc.hasExtendedAttribute("END")
            || vc.isFiltered()
            || vc.contigsMatch(vc)
            || allele.isSymbolic()
            || allele.isCalled()
            || allele.isBreakpoint()
            || allele.isSingleBreakend()
            || allele.isNonRefAllele()
            || Allele.oneIsPrefixOfOther(allele, allele)
            || genotype.hasExtendedAttribute("AD")
            || genotype.hasDP()
            || genotype.hasAnyAttribute("AD")
            || genotype.isFiltered()
            || genotype.isHet()
            || genotype.isHetNonRef()
            || genotype.isMixed()
            || genotype.isNonInformative()
            || genotype.sameGenotype(genotype)
            || genotype.isHomRef()) {
            return vc.getAlternateAllele(0);
        }
        return genotype.getAllele(0);
    }

    String display(VariantContext vc, Genotype genotype, Allele allele) {
        return vc.getDisplayString()
            + vc.toStringWithoutGenotypes()
            + vc.toStringDecodeGenotypes()
            + allele.getDisplayString()
            + genotype.getGenotypeString();
    }

    Object sampleSubset(VariantContext vc) {
        return vc.subContextFromSamples(java.util.Collections.emptySet());
    }

    Object samples(VariantContext vc) {
        return vc.getSampleNames();
    }

    Object orderedGenotypes(VariantContext vc) {
        return vc.getGenotypesOrderedByName();
    }

    Object stringList(VariantContext vc) {
        return vc.getAttributeAsStringList("ANN");
    }

    Object objectList(VariantContext vc) {
        return vc.getAttributeAsList("ANN");
    }

    Object doubleList(VariantContext vc) {
        return vc.getAttributeAsDoubleList("AF", 0.0);
    }

    Object intList(VariantContext vc) {
        return vc.getAttributeAsIntList("AD", 0);
    }

    Object indelLengths(VariantContext vc) {
        return vc.getIndelLengths();
    }

    Object alternateLikelihoodIndices(VariantContext vc, Allele allele) {
        return vc.getGLIndicesOfAlternateAllele(allele);
    }

    Object alleleIndices(VariantContext vc, java.util.Collection<Allele> alleles) {
        return vc.getAlleleIndices(alleles);
    }

    Object commonInfo(VariantContext vc) {
        return vc.getCommonInfo();
    }

    Object commonInfoStringList(CommonInfo info) {
        return info.getAttributeAsStringList("ANN");
    }

    Object commonInfoObjectList(CommonInfo info) {
        return info.getAttributeAsList("ANN");
    }

    Object structuralType(VariantContext vc) {
        return vc.getStructuralVariantType();
    }

    boolean selectedTypes(VariantContext.Type type) {
        return type == VariantContext.Type.SNP
            || type == VariantContext.Type.MNP
            || type == VariantContext.Type.SYMBOLIC;
    }

    VariantContext withoutAttribute(VariantContextBuilder builder) {
        return builder.rmAttribute("END").make();
    }

    VariantContext withoutAttributes(VariantContextBuilder builder) {
        return builder.rmAttributes(java.util.Arrays.asList("END")).make();
    }

    VariantContext withBulkAttributes(VariantContextBuilder builder) {
        return builder.putAttributes(java.util.Collections.emptyMap()).make();
    }

    VariantContext unfilteredVariant(VariantContextBuilder builder) {
        return builder.unfiltered().make();
    }

    VariantContext passingVariant(VariantContextBuilder builder) {
        return builder.passFilters().make();
    }

    VariantContext withoutGenotypes(VariantContextBuilder builder) {
        return builder.noGenotypes().make();
    }

    VariantContext computedEnd(VariantContextBuilder builder, Allele ref, Allele alt) {
        return builder.computeEndFromAlleles(java.util.Arrays.asList(ref, alt), 10).make();
    }

    VariantContext relocated(VariantContextBuilder builder) {
        return builder.loc("chr1", 1, 10).source("tool").noID().make();
    }

    Object optional(VariantContext vc, Genotype genotype) {
        return genotype.getExtendedAttribute("AD");
    }

    Object likelihoods(Genotype genotype) {
        return genotype.getLikelihoods();
    }

    Object genotypeFilters(Genotype genotype) {
        return genotype.getFilters();
    }

    Object likelihoodsFromArray(double[] values) {
        return GenotypeLikelihoods.fromLog10Likelihoods(values);
    }

    Object likelihoodsFromPls(int[] values) {
        return GenotypeLikelihoods.fromPLs(values);
    }

    int likelihoodIndex(int first, int second) {
        return GenotypeLikelihoods.calculatePLindex(first, second);
    }

    int likelihoodCount(int alleles, int ploidy) {
        return GenotypeLikelihoods.numLikelihoods(alleles, ploidy);
    }

    Object likelihoodAllelePair(int index) {
        return GenotypeLikelihoods.getAllelePair(index);
    }

    Object likelihoodArrays(GenotypeLikelihoods likelihoods) {
        return likelihoods.getAsPLs();
    }

    Object likelihoodVector(GenotypeLikelihoods likelihoods) {
        return likelihoods.getAsVector();
    }

    double likelihoodQuality(GenotypeLikelihoods likelihoods) {
        return likelihoods.getGQLog10FromLikelihoods();
    }

    void likelihoodForEach(GenotypeLikelihoods likelihoods) {
        likelihoods.forEach(null);
    }

    Object types() {
        return GenotypeType.values();
    }

    boolean addGenotype(GenotypesContext genotypes, Genotype genotype) {
        return genotypes.add(genotype);
    }

    boolean replaceGenotype(GenotypesContext genotypes, Genotype genotype) {
        return genotypes.replace(genotype);
    }

    boolean addGenotypes(GenotypesContext genotypes, GenotypesContext more) {
        return genotypes.addAll(more);
    }

    boolean containsSample(GenotypesContext genotypes) {
        return genotypes.containsSample("SAMPLE");
    }

    int countedAlleles(Genotype genotype, Allele allele) {
        return genotype.countAllele(allele);
    }

    Object genotypeCopies(GenotypesContext genotypes) {
        GenotypesContext copy = genotypes.copy();
        return copy.getSampleNames();
    }

    Object orderedSampleIteration(GenotypesContext genotypes) {
        return genotypes.iterateInSampleNameOrder();
    }

    Object genotypeSpliterator(GenotypesContext genotypes) {
        return genotypes.spliterator();
    }

    Object emptyGenotypes() {
        return GenotypesContext.create();
    }

    int stop(VariantContextBuilder builder) {
        return builder.stop(10).getStop();
    }

    Object builderState(VariantContextBuilder builder) {
        return builder.filter("q10").filters(java.util.Collections.emptySet()).getFilters()
            + builder.getAttributes()
            + builder.getGenotypes();
    }

    Genotype genotypeQuality(GenotypeBuilder builder) {
        return builder.log10PError(-1.0).phased(true).make();
    }

    Genotype genotypeShallowCopy(GenotypeBuilder builder) {
        return builder.makeWithShallowCopy();
    }

    boolean predicates(VariantContext vc, Genotype genotype, Allele allele) {
        return vc.hasGenotypes()
            || vc.hasGenotype("SAMPLE")
            || vc.hasLog10PError()
            || vc.filtersWereApplied()
            || genotype.hasLikelihoods()
            || genotype.isHomVar()
            || genotype.isPhased()
            || allele.basesMatch(allele);
    }

    int alleleIndex(VariantContext vc, Allele allele) {
        return vc.getAlleleIndex(allele);
    }

    Object extendedAllele(Allele allele) {
        return allele.extend("A".getBytes());
    }

    GenotypeBuilder clearGenotypeQuality(GenotypeBuilder builder) {
        return builder.noGQ().noDP().noAttributes().filters(java.util.Collections.emptySet());
    }
}
"#,
    );

    let comparator = semantic
        .facts
        .external_constructor_for(
            "VariantContextComparator",
            &[JavaType::Named {
                name: "SAMSequenceDictionary".to_string(),
                args: Vec::new(),
            }],
        )
        .expect(
            "semantic analysis should use the variantcontext pack for VariantContextComparator/1",
        );
    assert_eq!(comparator.provenance, FactProvenance::ExternalLibraryPack);

    let chr = semantic
        .facts
        .external_api_effect_for_receiver(Some("VariantContextBuilder"), "chr")
        .unwrap();
    assert_eq!(chr.role, ExternalApiRole::FluentMutator);
    assert_eq!(chr.provenance, FactProvenance::ExternalLibraryPack);

    let attribute = semantic
        .facts
        .external_api_effect_for_receiver(Some("VariantContext"), "getAttribute")
        .unwrap();
    assert_eq!(attribute.return_nullability, NullabilityState::MaybeNull);
    assert_eq!(attribute.provenance, FactProvenance::ExternalLibraryPack);

    let genotype = semantic
        .facts
        .external_api_effect_for_receiver(Some("VariantContext"), "getGenotype")
        .expect("GATK-shaped sample genotype lookup should be modeled by the pack");
    assert_eq!(
        genotype.return_type,
        Some(JavaType::Named {
            name: "Genotype".to_string(),
            args: Vec::new(),
        })
    );
    assert_eq!(genotype.return_nullability, NullabilityState::MaybeNull);

    let attribute_as_int = semantic
        .facts
        .external_api_effect_for_receiver(Some("VariantContext"), "getAttributeAsInt")
        .expect("integer attribute getter should be modeled by the pack");
    assert_eq!(
        attribute_as_int.return_type,
        Some(JavaType::Primitive(
            new_java2rust::java_ir::PrimitiveType::Int
        ))
    );

    let attribute_as_boolean = semantic
        .facts
        .external_api_effect_for_receiver(Some("VariantContext"), "getAttributeAsBoolean")
        .expect("boolean attribute getter should be modeled by the pack");
    assert_eq!(attribute_as_boolean.role, ExternalApiRole::BooleanGetter);
    assert_eq!(
        attribute_as_boolean.return_type,
        Some(JavaType::Primitive(
            new_java2rust::java_ir::PrimitiveType::Boolean
        ))
    );

    let alternate_allele = semantic
        .facts
        .external_api_effect_for_receiver(Some("VariantContext"), "getAlternateAllele")
        .expect("indexed alternate allele getter should be modeled by the pack");
    assert_eq!(alternate_allele.role, ExternalApiRole::IndexedGetter);

    let rm_attribute = semantic
        .facts
        .external_api_effect_for_receiver(Some("VariantContextBuilder"), "rmAttribute")
        .expect("builder attribute removal should be modeled by the pack");
    assert_eq!(rm_attribute.role, ExternalApiRole::FluentMutator);
    assert_eq!(
        rm_attribute.receiver_mutability,
        ReceiverMutabilityFact::MutatesReceiver
    );

    let depth = semantic
        .facts
        .external_api_effect_for_receiver(Some("Genotype"), "getDP")
        .expect("genotype depth getter should be modeled by the pack");
    assert_eq!(
        depth.return_type,
        Some(JavaType::Primitive(
            new_java2rust::java_ir::PrimitiveType::Int
        ))
    );

    let genotype_allele = semantic
        .facts
        .external_api_effect_for_receiver(Some("Genotype"), "getAllele")
        .expect("genotype indexed allele getter should be modeled by the pack");
    assert_eq!(genotype_allele.role, ExternalApiRole::IndexedGetter);

    let allele_count = semantic
        .facts
        .external_api_effect_for_receiver(Some("VariantContext"), "getNAlleles")
        .expect("variant allele-count getter should be modeled by the pack");
    assert_eq!(
        allele_count.return_type,
        Some(JavaType::Primitive(
            new_java2rust::java_ir::PrimitiveType::Int
        ))
    );

    for method_name in [
        "getNSamples",
        "getCalledChrCount",
        "getHetCount",
        "getHomVarCount",
        "getNoCallCount",
    ] {
        let count = semantic
            .facts
            .external_api_effect_for_receiver(Some("VariantContext"), method_name)
            .expect("variant aggregate count getter should be modeled by the pack");
        assert_eq!(
            count.return_type,
            Some(JavaType::Primitive(
                new_java2rust::java_ir::PrimitiveType::Int
            ))
        );
    }

    let allele_symbolic = semantic
        .facts
        .external_api_effect_for_receiver(Some("Allele"), "isSymbolic")
        .expect("allele symbolic predicate should be modeled by the pack");
    assert_eq!(allele_symbolic.role, ExternalApiRole::BooleanGetter);

    for method_name in [
        "isCalled",
        "isBreakpoint",
        "isSingleBreakend",
        "isNonRefAllele",
        "oneIsPrefixOfOther",
    ] {
        let allele_predicate = semantic
            .facts
            .external_api_effect_for_receiver(Some("Allele"), method_name)
            .expect("allele predicate should be modeled by the pack");
        assert_eq!(allele_predicate.role, ExternalApiRole::BooleanGetter);
        assert_eq!(
            allele_predicate.return_nullability,
            NullabilityState::DefinitelyNonNull
        );
    }

    let contigs_match = semantic
        .facts
        .external_api_effect_for_receiver(Some("VariantContext"), "contigsMatch")
        .expect("variant contig comparison predicate should be modeled by the pack");
    assert_eq!(contigs_match.role, ExternalApiRole::BooleanGetter);

    for method_name in [
        "isNotFiltered",
        "isMixed",
        "isMNP",
        "isSimpleInsertion",
        "isSimpleDeletion",
        "isComplexIndel",
        "isStructuralIndel",
        "isSymbolicOrSV",
        "isMonomorphicInSamples",
        "isPolymorphicInSamples",
        "hasSymbolicAlleles",
        "overlaps",
        "hasLog10PError",
        "hasID",
        "hasAllele",
        "hasAlternateAllele",
        "contains",
        "withinDistanceOf",
    ] {
        let predicate = semantic
            .facts
            .external_api_effect_for_receiver(Some("VariantContext"), method_name)
            .expect("variant predicate should be modeled by the pack");
        assert_eq!(predicate.role, ExternalApiRole::BooleanGetter);
        assert_eq!(
            predicate.return_nullability,
            NullabilityState::DefinitelyNonNull
        );
    }

    let sample_names = semantic
        .facts
        .external_api_effect_for_receiver(Some("VariantContext"), "getSampleNames")
        .expect("variant sample name getter should be modeled by the pack");
    assert_eq!(
        sample_names.return_type,
        Some(JavaType::Named {
            name: "Set".to_string(),
            args: vec![JavaType::String],
        })
    );

    let ordered_genotypes = semantic
        .facts
        .external_api_effect_for_receiver(Some("VariantContext"), "getGenotypesOrderedByName")
        .expect("variant ordered-genotype getter should be modeled by the pack");
    assert_eq!(
        ordered_genotypes.return_type,
        Some(JavaType::Named {
            name: "Iterable".to_string(),
            args: vec![JavaType::Named {
                name: "Genotype".to_string(),
                args: Vec::new(),
            }],
        })
    );
    assert_eq!(
        ordered_genotypes.return_nullability,
        NullabilityState::DefinitelyNonNull
    );

    let sample_subset = semantic
        .facts
        .external_api_effect_for_receiver(Some("VariantContext"), "subContextFromSamples")
        .expect("variant sample-subset helper should be modeled by the pack");
    assert_eq!(
        sample_subset.return_type,
        Some(JavaType::Named {
            name: "VariantContext".to_string(),
            args: Vec::new(),
        })
    );
    assert_eq!(
        sample_subset.return_nullability,
        NullabilityState::DefinitelyNonNull
    );

    let qual = semantic
        .facts
        .external_api_effect_for_receiver(Some("VariantContext"), "getPhredScaledQual")
        .expect("variant phred-scaled quality getter should be modeled by the pack");
    assert_eq!(
        qual.return_type,
        Some(JavaType::Primitive(
            new_java2rust::java_ir::PrimitiveType::Double
        ))
    );

    let log_error = semantic
        .facts
        .external_api_effect_for_receiver(Some("VariantContext"), "getLog10PError")
        .expect("variant log10PError getter should be modeled by the pack");
    assert_eq!(
        log_error.return_type,
        Some(JavaType::Primitive(
            new_java2rust::java_ir::PrimitiveType::Double
        ))
    );

    let structural_type = semantic
        .facts
        .external_api_effect_for_receiver(Some("VariantContext"), "getStructuralVariantType")
        .expect("variant structural type getter should be modeled by the pack");
    assert_eq!(
        structural_type.return_type,
        Some(JavaType::Named {
            name: "StructuralVariantType".to_string(),
            args: Vec::new(),
        })
    );
    assert_eq!(
        structural_type.return_nullability,
        NullabilityState::MaybeNull
    );

    let has_dp = semantic
        .facts
        .external_api_effect_for_receiver(Some("Genotype"), "hasDP")
        .expect("genotype depth predicate should be modeled by the pack");
    assert_eq!(has_dp.role, ExternalApiRole::BooleanGetter);

    let likelihoods = semantic
        .facts
        .external_api_effect_for_receiver(Some("Genotype"), "getLikelihoods")
        .expect("genotype likelihood getter should be modeled by the pack");
    assert_eq!(
        likelihoods.return_type,
        Some(JavaType::Named {
            name: "GenotypeLikelihoods".to_string(),
            args: Vec::new(),
        })
    );
    assert_eq!(likelihoods.return_nullability, NullabilityState::MaybeNull);

    let likelihoods_factory = semantic
        .facts
        .external_api_effect_for_receiver(Some("GenotypeLikelihoods"), "fromLog10Likelihoods")
        .expect("likelihood factory should be modeled by the pack");
    assert_eq!(
        likelihoods_factory.return_type,
        Some(JavaType::Named {
            name: "GenotypeLikelihoods".to_string(),
            args: Vec::new(),
        })
    );
    assert_eq!(
        likelihoods_factory.return_nullability,
        NullabilityState::DefinitelyNonNull
    );

    let likelihoods_from_pls = semantic
        .facts
        .external_api_effect_for_receiver(Some("GenotypeLikelihoods"), "fromPLs")
        .expect("PL likelihood factory should be modeled by the pack");
    assert_eq!(
        likelihoods_from_pls.return_type,
        Some(JavaType::Named {
            name: "GenotypeLikelihoods".to_string(),
            args: Vec::new(),
        })
    );

    let likelihood_index = semantic
        .facts
        .external_api_effect_for_receiver(Some("GenotypeLikelihoods"), "calculatePLindex")
        .expect("likelihood PL index helper should be modeled by the pack");
    assert_eq!(
        likelihood_index.return_type,
        Some(JavaType::Primitive(
            new_java2rust::java_ir::PrimitiveType::Int
        ))
    );

    let likelihood_count = semantic
        .facts
        .external_api_effect_for_receiver(Some("GenotypeLikelihoods"), "numLikelihoods")
        .expect("likelihood count helper should be modeled by the pack");
    assert_eq!(
        likelihood_count.return_type,
        Some(JavaType::Primitive(
            new_java2rust::java_ir::PrimitiveType::Int
        ))
    );

    let likelihood_allele_pair = semantic
        .facts
        .external_api_effect_for_receiver(Some("GenotypeLikelihoods"), "getAllelePair")
        .expect("likelihood allele-pair helper should be modeled by the pack");
    assert_eq!(
        likelihood_allele_pair.return_type,
        Some(JavaType::Named {
            name: "GenotypeLikelihoodsAllelePair".to_string(),
            args: Vec::new(),
        })
    );

    let likelihood_pls = semantic
        .facts
        .external_api_effect_for_receiver(Some("GenotypeLikelihoods"), "getAsPLs")
        .expect("likelihood PL array getter should be modeled by the pack");
    assert_eq!(
        likelihood_pls.return_type,
        Some(JavaType::Array(Box::new(JavaType::Primitive(
            new_java2rust::java_ir::PrimitiveType::Int,
        ))))
    );

    let likelihood_vector = semantic
        .facts
        .external_api_effect_for_receiver(Some("GenotypeLikelihoods"), "getAsVector")
        .expect("likelihood vector getter should be modeled by the pack");
    assert_eq!(
        likelihood_vector.return_type,
        Some(JavaType::Array(Box::new(JavaType::Primitive(
            new_java2rust::java_ir::PrimitiveType::Double,
        ))))
    );

    let likelihood_quality = semantic
        .facts
        .external_api_effect_for_receiver(Some("GenotypeLikelihoods"), "getGQLog10FromLikelihoods")
        .expect("likelihood GQ helper should be modeled by the pack");
    assert_eq!(
        likelihood_quality.return_type,
        Some(JavaType::Primitive(
            new_java2rust::java_ir::PrimitiveType::Double
        ))
    );

    let likelihood_for_each = semantic
        .facts
        .external_api_effect_for_receiver(Some("GenotypeLikelihoods"), "forEach")
        .expect("likelihood forEach callback boundary should be modeled by the pack");
    assert_eq!(likelihood_for_each.role, ExternalApiRole::SideEffect);
    assert_eq!(
        likelihood_for_each.receiver_mutability,
        ReceiverMutabilityFact::ReadOnly
    );

    let type_values = semantic
        .facts
        .external_api_effect_for_receiver(Some("GenotypeType"), "values")
        .expect("genotype type values should be modeled by the pack");
    assert_eq!(
        type_values.return_type,
        Some(JavaType::Array(Box::new(JavaType::Named {
            name: "GenotypeType".to_string(),
            args: Vec::new(),
        })))
    );

    let string_list = semantic
        .facts
        .external_api_effect_for_receiver(Some("VariantContext"), "getAttributeAsStringList")
        .expect("variant string-list attribute getter should be modeled by the pack");
    assert_eq!(
        string_list.return_type,
        Some(JavaType::Named {
            name: "List".to_string(),
            args: vec![JavaType::String],
        })
    );
    assert_eq!(
        string_list.return_nullability,
        NullabilityState::DefinitelyNonNull
    );

    let object_list = semantic
        .facts
        .external_api_effect_for_receiver(Some("VariantContext"), "getAttributeAsList")
        .expect("variant object-list attribute getter should be modeled by the pack");
    assert_eq!(
        object_list.return_type,
        Some(JavaType::Named {
            name: "List".to_string(),
            args: vec![JavaType::Named {
                name: "Object".to_string(),
                args: Vec::new(),
            }],
        })
    );

    let double_list = semantic
        .facts
        .external_api_effect_for_receiver(Some("VariantContext"), "getAttributeAsDoubleList")
        .expect("variant double-list attribute getter should be modeled by the pack");
    assert_eq!(
        double_list.return_type,
        Some(JavaType::Named {
            name: "List".to_string(),
            args: vec![JavaType::Named {
                name: "Double".to_string(),
                args: Vec::new(),
            }],
        })
    );

    let int_list = semantic
        .facts
        .external_api_effect_for_receiver(Some("VariantContext"), "getAttributeAsIntList")
        .expect("variant int-list attribute getter should be modeled by the pack");
    assert_eq!(
        int_list.return_type,
        Some(JavaType::Named {
            name: "List".to_string(),
            args: vec![JavaType::Named {
                name: "Integer".to_string(),
                args: Vec::new(),
            }],
        })
    );

    let indel_lengths = semantic
        .facts
        .external_api_effect_for_receiver(Some("VariantContext"), "getIndelLengths")
        .expect("variant indel-length getter should be modeled by the pack");
    assert_eq!(
        indel_lengths.return_type,
        Some(JavaType::Named {
            name: "List".to_string(),
            args: vec![JavaType::Named {
                name: "Integer".to_string(),
                args: Vec::new(),
            }],
        })
    );
    assert_eq!(
        indel_lengths.return_nullability,
        NullabilityState::MaybeNull
    );

    let likelihood_indices = semantic
        .facts
        .external_api_effect_for_receiver(Some("VariantContext"), "getGLIndicesOfAlternateAllele")
        .expect("variant alternate-likelihood index helper should be modeled by the pack");
    assert_eq!(
        likelihood_indices.return_type,
        Some(JavaType::Array(Box::new(JavaType::Primitive(
            new_java2rust::java_ir::PrimitiveType::Int,
        ))))
    );

    let allele_indices = semantic
        .facts
        .external_api_effect_for_receiver(Some("VariantContext"), "getAlleleIndices")
        .expect("variant allele-index list helper should be modeled by the pack");
    assert_eq!(
        allele_indices.return_type,
        Some(JavaType::Named {
            name: "List".to_string(),
            args: vec![JavaType::Named {
                name: "Integer".to_string(),
                args: Vec::new(),
            }],
        })
    );
    assert_eq!(
        allele_indices.return_nullability,
        NullabilityState::DefinitelyNonNull
    );

    let without_genotypes = semantic
        .facts
        .external_api_effect_for_receiver(Some("VariantContext"), "toStringWithoutGenotypes")
        .expect("variant display helper should be modeled by the pack");
    assert_eq!(without_genotypes.return_type, Some(JavaType::String));

    let decode_genotypes = semantic
        .facts
        .external_api_effect_for_receiver(Some("VariantContext"), "toStringDecodeGenotypes")
        .expect("variant decoded-genotype display helper should be modeled by the pack");
    assert_eq!(decode_genotypes.return_type, Some(JavaType::String));

    let common_info = semantic
        .facts
        .external_api_effect_for_receiver(Some("VariantContext"), "getCommonInfo")
        .expect("variant common-info getter should be modeled by the pack");
    assert_eq!(
        common_info.return_type,
        Some(JavaType::Named {
            name: "CommonInfo".to_string(),
            args: Vec::new(),
        })
    );

    let common_info_string_list = semantic
        .facts
        .external_api_effect_for_receiver(Some("CommonInfo"), "getAttributeAsStringList")
        .expect("CommonInfo string-list attribute getter should be modeled by the pack");
    assert_eq!(
        common_info_string_list.return_type,
        Some(JavaType::Named {
            name: "List".to_string(),
            args: vec![JavaType::String],
        })
    );

    let common_info_object_list = semantic
        .facts
        .external_api_effect_for_receiver(Some("CommonInfo"), "getAttributeAsList")
        .expect("CommonInfo object-list attribute getter should be modeled by the pack");
    assert_eq!(
        common_info_object_list.return_type,
        Some(JavaType::Named {
            name: "List".to_string(),
            args: vec![JavaType::Named {
                name: "Object".to_string(),
                args: Vec::new(),
            }],
        })
    );

    let max_ploidy = semantic
        .facts
        .external_api_effect_for_receiver(Some("VariantContext"), "getMaxPloidy")
        .expect("variant max-ploidy getter should be modeled by the pack");
    assert_eq!(
        max_ploidy.return_type,
        Some(JavaType::Primitive(
            new_java2rust::java_ir::PrimitiveType::Int
        ))
    );

    let length_on_reference = semantic
        .facts
        .external_api_effect_for_receiver(Some("VariantContext"), "getLengthOnReference")
        .expect("variant reference-length getter should be modeled by the pack");
    assert_eq!(
        length_on_reference.return_type,
        Some(JavaType::Primitive(
            new_java2rust::java_ir::PrimitiveType::Int
        ))
    );

    let has_genotypes = semantic
        .facts
        .external_api_effect_for_receiver(Some("VariantContext"), "hasGenotypes")
        .expect("variant genotype presence predicate should be modeled by the pack");
    assert_eq!(has_genotypes.role, ExternalApiRole::BooleanGetter);

    let has_genotype = semantic
        .facts
        .external_api_effect_for_receiver(Some("VariantContext"), "hasGenotype")
        .expect("variant sample genotype predicate should be modeled by the pack");
    assert_eq!(has_genotype.role, ExternalApiRole::BooleanGetter);

    let filters_were_applied = semantic
        .facts
        .external_api_effect_for_receiver(Some("VariantContext"), "filtersWereApplied")
        .expect("variant filter-state predicate should be modeled by the pack");
    assert_eq!(filters_were_applied.role, ExternalApiRole::BooleanGetter);

    let allele_index = semantic
        .facts
        .external_api_effect_for_receiver(Some("VariantContext"), "getAlleleIndex")
        .expect("variant allele index getter should be modeled by the pack");
    assert_eq!(allele_index.role, ExternalApiRole::IndexedGetter);

    let hom_var = semantic
        .facts
        .external_api_effect_for_receiver(Some("Genotype"), "isHomVar")
        .expect("genotype hom-var predicate should be modeled by the pack");
    assert_eq!(hom_var.role, ExternalApiRole::BooleanGetter);

    let het = semantic
        .facts
        .external_api_effect_for_receiver(Some("Genotype"), "isHet")
        .expect("genotype heterozygosity predicate should be modeled by the pack");
    assert_eq!(het.role, ExternalApiRole::BooleanGetter);

    let het_non_ref = semantic
        .facts
        .external_api_effect_for_receiver(Some("Genotype"), "isHetNonRef")
        .expect("genotype het-non-ref predicate should be modeled by the pack");
    assert_eq!(het_non_ref.role, ExternalApiRole::BooleanGetter);

    let mixed_genotype = semantic
        .facts
        .external_api_effect_for_receiver(Some("Genotype"), "isMixed")
        .expect("genotype mixed predicate should be modeled by the pack");
    assert_eq!(mixed_genotype.role, ExternalApiRole::BooleanGetter);

    let phased = semantic
        .facts
        .external_api_effect_for_receiver(Some("Genotype"), "isPhased")
        .expect("genotype phase predicate should be modeled by the pack");
    assert_eq!(phased.role, ExternalApiRole::BooleanGetter);

    let non_informative = semantic
        .facts
        .external_api_effect_for_receiver(Some("Genotype"), "isNonInformative")
        .expect("genotype non-informative predicate should be modeled by the pack");
    assert_eq!(non_informative.role, ExternalApiRole::BooleanGetter);

    let same_genotype = semantic
        .facts
        .external_api_effect_for_receiver(Some("Genotype"), "sameGenotype")
        .expect("genotype equivalence predicate should be modeled by the pack");
    assert_eq!(same_genotype.role, ExternalApiRole::BooleanGetter);
    assert_eq!(
        same_genotype.return_nullability,
        NullabilityState::DefinitelyNonNull
    );

    let has_likelihoods = semantic
        .facts
        .external_api_effect_for_receiver(Some("Genotype"), "hasLikelihoods")
        .expect("genotype likelihood predicate should be modeled by the pack");
    assert_eq!(has_likelihoods.role, ExternalApiRole::BooleanGetter);

    let has_any_attribute = semantic
        .facts
        .external_api_effect_for_receiver(Some("Genotype"), "hasAnyAttribute")
        .expect("genotype any-attribute predicate should be modeled by the pack");
    assert_eq!(has_any_attribute.role, ExternalApiRole::BooleanGetter);

    let genotype_filters = semantic
        .facts
        .external_api_effect_for_receiver(Some("Genotype"), "getFilters")
        .expect("genotype filter getter should be modeled by the pack");
    assert_eq!(
        genotype_filters.return_type,
        Some(JavaType::Named {
            name: "Set".to_string(),
            args: vec![JavaType::String],
        })
    );

    let count_allele = semantic
        .facts
        .external_api_effect_for_receiver(Some("Genotype"), "countAllele")
        .expect("genotype allele-count helper should be modeled by the pack");
    assert_eq!(
        count_allele.return_type,
        Some(JavaType::Primitive(
            new_java2rust::java_ir::PrimitiveType::Int
        ))
    );

    let make_with_shallow_copy = semantic
        .facts
        .external_api_effect_for_receiver(Some("GenotypeBuilder"), "makeWithShallowCopy")
        .expect("genotype builder shallow-copy make should be modeled by the pack");
    assert_eq!(
        make_with_shallow_copy.return_type,
        Some(JavaType::Named {
            name: "Genotype".to_string(),
            args: Vec::new(),
        })
    );
    assert_eq!(
        make_with_shallow_copy.return_nullability,
        NullabilityState::DefinitelyNonNull
    );

    let bases_match = semantic
        .facts
        .external_api_effect_for_receiver(Some("Allele"), "basesMatch")
        .expect("allele base comparison should be modeled by the pack");
    assert_eq!(bases_match.role, ExternalApiRole::BooleanGetter);

    let extend = semantic
        .facts
        .external_api_effect_for_receiver(Some("Allele"), "extend")
        .expect("allele extension should be modeled by the pack");
    assert_eq!(
        extend.return_type,
        Some(JavaType::Named {
            name: "Allele".to_string(),
            args: Vec::new(),
        })
    );

    let rm_attributes = semantic
        .facts
        .external_api_effect_for_receiver(Some("VariantContextBuilder"), "rmAttributes")
        .expect("builder bulk attribute removal should be modeled by the pack");
    assert_eq!(rm_attributes.role, ExternalApiRole::FluentMutator);

    let put_attributes = semantic
        .facts
        .external_api_effect_for_receiver(Some("VariantContextBuilder"), "putAttributes")
        .expect("builder bulk attribute insertion should be modeled by the pack");
    assert_eq!(put_attributes.role, ExternalApiRole::FluentMutator);
    assert_eq!(
        put_attributes.receiver_mutability,
        ReceiverMutabilityFact::MutatesReceiver
    );

    let builder_attributes = semantic
        .facts
        .external_api_effect_for_receiver(Some("VariantContextBuilder"), "getAttributes")
        .expect("builder attribute getter should be modeled by the pack");
    assert_eq!(
        builder_attributes.return_type,
        Some(JavaType::Named {
            name: "Map".to_string(),
            args: vec![
                JavaType::String,
                JavaType::Named {
                    name: "Object".to_string(),
                    args: Vec::new(),
                },
            ],
        })
    );
    assert_eq!(
        builder_attributes.return_nullability,
        NullabilityState::MaybeNull
    );

    let unfiltered = semantic
        .facts
        .external_api_effect_for_receiver(Some("VariantContextBuilder"), "unfiltered")
        .expect("builder unfiltered mutator should be modeled by the pack");
    assert_eq!(unfiltered.role, ExternalApiRole::FluentMutator);

    for method_name in [
        "passFilters",
        "noGenotypes",
        "computeEndFromAlleles",
        "loc",
        "source",
        "noID",
        "phased",
    ] {
        let receiver = if method_name == "phased" {
            "GenotypeBuilder"
        } else {
            "VariantContextBuilder"
        };
        let builder_mutator = semantic
            .facts
            .external_api_effect_for_receiver(Some(receiver), method_name)
            .expect("builder filter/end mutator should be modeled by the pack");
        assert_eq!(builder_mutator.role, ExternalApiRole::FluentMutator);
    }

    let add = semantic
        .facts
        .external_api_effect_for_receiver(Some("GenotypesContext"), "add")
        .expect("genotypes collection add should be modeled by the pack");
    assert_eq!(add.role, ExternalApiRole::Mutator);
    assert_eq!(
        add.receiver_mutability,
        ReceiverMutabilityFact::MutatesReceiver
    );

    let add_all = semantic
        .facts
        .external_api_effect_for_receiver(Some("GenotypesContext"), "addAll")
        .expect("genotypes collection bulk add should be modeled by the pack");
    assert_eq!(add_all.role, ExternalApiRole::Mutator);

    let contains_sample = semantic
        .facts
        .external_api_effect_for_receiver(Some("GenotypesContext"), "containsSample")
        .expect("genotypes sample predicate should be modeled by the pack");
    assert_eq!(contains_sample.role, ExternalApiRole::BooleanGetter);

    let replace = semantic
        .facts
        .external_api_effect_for_receiver(Some("GenotypesContext"), "replace")
        .expect("genotypes replacement mutator should be modeled by the pack");
    assert_eq!(replace.role, ExternalApiRole::Mutator);

    let create = semantic
        .facts
        .external_api_effect_for_receiver(Some("GenotypesContext"), "create")
        .expect("genotypes context factory should be modeled by the pack");
    assert_eq!(
        create.return_type,
        Some(JavaType::Named {
            name: "GenotypesContext".to_string(),
            args: Vec::new(),
        })
    );

    let copy = semantic
        .facts
        .external_api_effect_for_receiver(Some("GenotypesContext"), "copy")
        .expect("genotypes context copy should be modeled by the pack");
    assert_eq!(
        copy.return_type,
        Some(JavaType::Named {
            name: "GenotypesContext".to_string(),
            args: Vec::new(),
        })
    );

    let ordered_iteration = semantic
        .facts
        .external_api_effect_for_receiver(Some("GenotypesContext"), "iterateInSampleNameOrder")
        .expect("genotypes ordered iteration should be modeled by the pack");
    assert_eq!(ordered_iteration.role, ExternalApiRole::Getter);
    assert_eq!(
        ordered_iteration.return_type,
        Some(JavaType::Named {
            name: "Iterable".to_string(),
            args: vec![JavaType::Named {
                name: "Genotype".to_string(),
                args: Vec::new(),
            }],
        })
    );

    let spliterator = semantic
        .facts
        .external_api_effect_for_receiver(Some("GenotypesContext"), "spliterator")
        .expect("genotypes spliterator should be modeled by the pack");
    assert_eq!(spliterator.role, ExternalApiRole::Getter);
    assert_eq!(
        spliterator.return_type,
        Some(JavaType::Named {
            name: "Spliterator".to_string(),
            args: vec![JavaType::Named {
                name: "Genotype".to_string(),
                args: Vec::new(),
            }],
        })
    );

    let genotype_sample_names = semantic
        .facts
        .external_api_effect_for_receiver(Some("GenotypesContext"), "getSampleNames")
        .expect("genotypes sample-name getter should be modeled by the pack");
    assert_eq!(
        genotype_sample_names.return_type,
        Some(JavaType::Named {
            name: "Set".to_string(),
            args: vec![JavaType::String],
        })
    );

    let get_stop = semantic
        .facts
        .external_api_effect_for_receiver(Some("VariantContextBuilder"), "getStop")
        .expect("builder stop getter should be modeled by the pack");
    assert_eq!(
        get_stop.return_type,
        Some(JavaType::Primitive(
            new_java2rust::java_ir::PrimitiveType::Int
        ))
    );

    let builder_filter = semantic
        .facts
        .external_api_effect_for_receiver(Some("VariantContextBuilder"), "filter")
        .expect("builder filter mutator should be modeled by the pack");
    assert_eq!(builder_filter.role, ExternalApiRole::FluentMutator);

    let builder_filters = semantic
        .facts
        .external_api_effect_for_receiver(Some("VariantContextBuilder"), "getFilters")
        .expect("builder filter getter should be modeled by the pack");
    assert_eq!(
        builder_filters.return_type,
        Some(JavaType::Named {
            name: "Set".to_string(),
            args: vec![JavaType::String],
        })
    );

    let builder_genotypes = semantic
        .facts
        .external_api_effect_for_receiver(Some("VariantContextBuilder"), "getGenotypes")
        .expect("builder genotype getter should be modeled by the pack");
    assert_eq!(
        builder_genotypes.return_type,
        Some(JavaType::Named {
            name: "GenotypesContext".to_string(),
            args: Vec::new(),
        })
    );

    let genotype_log_error = semantic
        .facts
        .external_api_effect_for_receiver(Some("GenotypeBuilder"), "log10PError")
        .expect("genotype builder log10PError mutator should be modeled by the pack");
    assert_eq!(genotype_log_error.role, ExternalApiRole::FluentMutator);

    let no_gq = semantic
        .facts
        .external_api_effect_for_receiver(Some("GenotypeBuilder"), "noGQ")
        .expect("genotype builder noGQ mutator should be modeled by the pack");
    assert_eq!(no_gq.role, ExternalApiRole::FluentMutator);

    let no_dp = semantic
        .facts
        .external_api_effect_for_receiver(Some("GenotypeBuilder"), "noDP")
        .expect("genotype builder noDP mutator should be modeled by the pack");
    assert_eq!(no_dp.role, ExternalApiRole::FluentMutator);

    let no_attributes = semantic
        .facts
        .external_api_effect_for_receiver(Some("GenotypeBuilder"), "noAttributes")
        .expect("genotype builder attribute cleanup mutator should be modeled by the pack");
    assert_eq!(no_attributes.role, ExternalApiRole::FluentMutator);

    let genotype_filters = semantic
        .facts
        .external_api_effect_for_receiver(Some("GenotypeBuilder"), "filters")
        .expect("genotype builder filters mutator should be modeled by the pack");
    assert_eq!(genotype_filters.role, ExternalApiRole::FluentMutator);

    assert!(
        !semantic
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "external_pack_missing_api"),
        "newly modeled GATK-shaped variantcontext calls should not produce missing-pack diagnostics: {:?}",
        semantic.diagnostics
    );
}

#[test]
fn unsupported_variantcontext_pack_api_reports_structured_diagnostic() {
    let semantic = analyze(
        r#"
import htsjdk.variant.variantcontext.VariantContext;

class UnsupportedVariantPackUse {
    Object decode(VariantContext vc) {
        return vc.fullyDecode(null, false);
    }
}
"#,
    );

    assert!(
        semantic.diagnostics.iter().any(|diagnostic| diagnostic.code
            == "external_pack_unsupported_api"
            && diagnostic.message.contains("VariantContext.fullyDecode")),
        "unsupported pack APIs should produce a pack-owned diagnostic"
    );
    assert!(
        semantic
            .facts
            .external_api_effect_for_receiver(Some("VariantContext"), "fullyDecode")
            .is_none(),
        "unsupported pack APIs should not fall through to generic external guesses"
    );
}

#[test]
fn missing_variantcontext_pack_api_reports_structured_diagnostic() {
    let semantic = analyze(
        r#"
import htsjdk.variant.variantcontext.VariantContext;

class MissingVariantPackUse {
    Object decode(VariantContext vc) {
        return vc.unmodeledPackMethod("DP");
    }
}
"#,
    );

    assert!(
        semantic.diagnostics.iter().any(|diagnostic| diagnostic.code
            == "external_pack_missing_api"
            && diagnostic
                .message
                .contains("VariantContext.unmodeledPackMethod")
            && diagnostic.message.contains("htsjdk.variant.variantcontext")),
        "missing pack APIs should produce a pack-owned diagnostic"
    );
    assert!(
        semantic
            .facts
            .external_api_effect_for_receiver(Some("VariantContext"), "unmodeledPackMethod")
            .is_none(),
        "missing pack APIs should not fall through to generic external guesses"
    );
}

#[test]
fn object_methods_on_pack_owned_values_remain_builtin_facts() {
    let semantic = analyze(
        r#"
import htsjdk.variant.variantcontext.Allele;

class PackOwnedObjectMethodUse {
    boolean same(Allele left, Allele right) {
        return left.equals(right)
            && left.hashCode() != 0
            && left.toString().length() > 0
            && left.getClass() == right.getClass();
    }
}
"#,
    );

    assert!(
        semantic.diagnostics.is_empty(),
        "Object methods on pack-owned values should not be diagnosed as pack coverage gaps"
    );
    for (method_name, arg_count) in [
        ("equals", Some(1)),
        ("hashCode", Some(0)),
        ("toString", Some(0)),
        ("getClass", Some(0)),
    ] {
        let effect = semantic
            .facts
            .external_api_effect_for_receiver_with_arity(Some("Allele"), method_name, arg_count)
            .unwrap_or_else(|| {
                panic!("Object.{method_name} should still be modeled for pack-owned values")
            });
        assert_eq!(effect.provenance, FactProvenance::BuiltinExternalModel);
        assert_eq!(effect.role, ExternalApiRole::Getter);
        assert_eq!(effect.receiver_mutability, ReceiverMutabilityFact::ReadOnly);
        assert_eq!(
            effect.return_nullability,
            NullabilityState::DefinitelyNonNull
        );
    }
}

#[test]
fn missing_hdf5_pack_api_reports_structured_diagnostic() {
    let semantic = analyze(
        r#"
class MissingHdf5PackUse {
    Object read(File file) {
        HDF5File hdf5 = new HDF5File(file);
        return hdf5.unmodeledNativeBoundary("/metrics");
    }
}
"#,
    );

    assert!(
        semantic.diagnostics.iter().any(|diagnostic| diagnostic.code
            == "external_pack_missing_api"
            && diagnostic
                .message
                .contains("HDF5File.unmodeledNativeBoundary")
            && diagnostic.message.contains("hdf5")),
        "missing HDF5 facade APIs should produce a pack-owned diagnostic"
    );
    assert!(
        semantic
            .facts
            .external_api_effect_for_receiver(Some("HDF5File"), "unmodeledNativeBoundary")
            .is_none(),
        "missing HDF5 APIs should not become generic external guesses"
    );
}

#[test]
fn missing_hdf5_pack_constructor_reports_structured_diagnostic() {
    let semantic = analyze(
        r#"
class MissingHdf5ConstructorUse {
    Object open(File file) {
        return new HDF5File(file, "CREATE", true);
    }
}
"#,
    );

    assert!(
        semantic.diagnostics.iter().any(|diagnostic| diagnostic.code
            == "external_pack_missing_constructor"
            && diagnostic.message.contains("HDF5File constructor")
            && diagnostic.message.contains("hdf5")),
        "missing HDF5 facade constructors should produce a pack-owned diagnostic"
    );
    assert!(
        semantic
            .facts
            .external_constructor_for(
                "HDF5File",
                &[
                    JavaType::Named {
                        name: "File".to_string(),
                        args: Vec::new(),
                    },
                    JavaType::String,
                    JavaType::Primitive(new_java2rust::java_ir::PrimitiveType::Boolean),
                ],
            )
            .is_none(),
        "missing HDF5 constructors should not become generic external guesses"
    );
}

#[test]
fn missing_hdf5_pack_field_reports_structured_diagnostic() {
    let semantic = analyze(
        r#"
class MissingHdf5FieldUse {
    Object read(File file) {
        HDF5File hdf5 = new HDF5File(file);
        return hdf5.unmodeledField;
    }
}
"#,
    );

    assert!(
        semantic.diagnostics.iter().any(|diagnostic| diagnostic.code
            == "external_pack_missing_field"
            && diagnostic.message.contains("HDF5File.unmodeledField")
            && diagnostic.message.contains("hdf5")),
        "missing HDF5 facade fields should produce a pack-owned diagnostic"
    );
    assert!(
        semantic
            .facts
            .external_field_for("HDF5File", "unmodeledField")
            .is_none(),
        "missing HDF5 fields should not become generic external guesses"
    );
}

#[test]
fn missing_sqlite_pack_api_reports_structured_diagnostic() {
    let semantic = analyze(
        r#"
class MissingSqlitePackUse {
    Object connect(String path) {
        SQLiteConfig config = new SQLiteConfig();
        SQLiteConnection connection = config.createConnection(path);
        return connection.unmodeledStatementFactory();
    }
}
"#,
    );

    assert!(
        semantic.diagnostics.iter().any(|diagnostic| diagnostic.code
            == "external_pack_missing_api"
            && diagnostic
                .message
                .contains("SQLiteConnection.unmodeledStatementFactory")
            && diagnostic.message.contains("sqlite")),
        "missing SQLite facade APIs should produce a pack-owned diagnostic"
    );
    assert!(
        semantic
            .facts
            .external_api_effect_for_receiver(Some("SQLiteConnection"), "unmodeledStatementFactory")
            .is_none(),
        "missing SQLite APIs should not become generic external guesses"
    );
}

#[test]
fn missing_sqlite_pack_constructor_reports_structured_diagnostic() {
    let semantic = analyze(
        r#"
class MissingSqliteConstructorUse {
    Object configure(String path) {
        return new SQLiteConfig(path);
    }
}
"#,
    );

    assert!(
        semantic.diagnostics.iter().any(|diagnostic| diagnostic.code
            == "external_pack_missing_constructor"
            && diagnostic.message.contains("SQLiteConfig constructor")
            && diagnostic.message.contains("sqlite")),
        "missing SQLite facade constructors should produce a pack-owned diagnostic"
    );
    assert!(
        semantic
            .facts
            .external_constructor_for("SQLiteConfig", &[JavaType::String])
            .is_none(),
        "missing SQLite constructors should not become generic external guesses"
    );
}

#[test]
fn missing_sqlite_pack_field_reports_structured_diagnostic() {
    let semantic = analyze(
        r#"
class MissingSqliteFieldUse {
    Object configure() {
        SQLiteConfig config = new SQLiteConfig();
        return config.unmodeledOption;
    }
}
"#,
    );

    assert!(
        semantic.diagnostics.iter().any(|diagnostic| diagnostic.code
            == "external_pack_missing_field"
            && diagnostic.message.contains("SQLiteConfig.unmodeledOption")
            && diagnostic.message.contains("sqlite")),
        "missing SQLite facade fields should produce a pack-owned diagnostic"
    );
    assert!(
        semantic
            .facts
            .external_field_for("SQLiteConfig", "unmodeledOption")
            .is_none(),
        "missing SQLite fields should not become generic external guesses"
    );
}

#[test]
fn unowned_external_api_still_uses_conservative_guessing() {
    let semantic = analyze(
        r#"
class UnownedExternalUse {
    Object read(UnmodeledExternal value) {
        return value.unmodeledMethod("x");
    }
}
"#,
    );

    assert!(
        semantic.diagnostics.is_empty(),
        "unowned external APIs should not be diagnosed as pack coverage gaps"
    );
    let effect = semantic
        .facts
        .external_api_effect_for_receiver(Some("UnmodeledExternal"), "unmodeledMethod")
        .expect("unowned external APIs should still receive conservative facts");
    assert_eq!(effect.provenance, FactProvenance::ConservativeFallback);
}

#[test]
fn hdf5_pack_declares_hdf5_pure_rust_crate_mapping() {
    let facts = hdf5_pack_facts();

    let file_arg = JavaType::Named {
        name: "File".to_string(),
        args: Vec::new(),
    };
    assert_eq!(
        new_java2rust::stdlib::external_constructor_provenance("HDF5File", &[file_arg.clone()]),
        FactProvenance::ConservativeFallback,
        "HDF5 constructor precision should come from the crate-mapping pack, not stdlib fallback"
    );

    let pack = facts
        .external_library_pack_for_type("HDF5File")
        .expect("HDF5File should be owned by the HDF5 crate-mapping pack");
    assert_eq!(pack.pack_name, "hdf5");
    assert_eq!(pack.mode, ExternalLibraryMode::RustCrateMapping);
    assert_eq!(pack.coverage, ExternalApiCoverage::Mapped);
    assert_eq!(pack.rust_target.as_deref(), Some("hdf5_pure_rust"));
    assert_eq!(pack.provenance, FactProvenance::ExternalLibraryPack);

    let file = facts
        .external_constructor_for(
            "HDF5File",
            &[
                file_arg,
                JavaType::Named {
                    name: "OpenMode".to_string(),
                    args: Vec::new(),
                },
            ],
        )
        .expect("HDF5File constructor should be pack-owned by arity");
    assert_eq!(file.provenance, FactProvenance::ExternalLibraryPack);

    for (type_name, field_name) in [
        ("HDF5File", "OpenMode"),
        ("HDF5File.OpenMode", "READ_ONLY"),
        ("HDF5File.OpenMode", "CREATE"),
        ("OpenMode", "READ_ONLY"),
        ("OpenMode", "CREATE"),
    ] {
        let field = facts
            .external_field_for(type_name, field_name)
            .unwrap_or_else(|| panic!("{type_name}.{field_name} should be pack-owned"));
        assert_eq!(field.provenance, FactProvenance::ExternalLibraryPack);
        assert_eq!(
            field.field_type,
            Some(JavaType::Named {
                name: "OpenMode".to_string(),
                args: Vec::new(),
            })
        );
        assert_eq!(
            new_java2rust::stdlib::external_field_provenance(type_name, field_name),
            FactProvenance::ConservativeFallback,
            "{type_name}.{field_name} precision should come from the HDF5 pack, not stdlib"
        );
    }
}

#[test]
fn hdf5_pack_models_resource_and_dataset_effects() {
    let facts = hdf5_pack_facts();

    let exists = facts
        .external_api_effect_for_receiver(Some("HDF5File"), "exists")
        .expect("HDF5 existence predicate should be modeled by the pack");
    assert_eq!(exists.role, ExternalApiRole::BooleanGetter);
    assert_eq!(
        exists.return_type,
        Some(JavaType::Primitive(
            new_java2rust::java_ir::PrimitiveType::Boolean,
        ))
    );
    assert_eq!(
        exists.return_nullability,
        NullabilityState::DefinitelyNonNull
    );

    let read_doubles = facts
        .external_api_effect_for_receiver(Some("HDF5File"), "readDoubleArray")
        .expect("HDF5 double array read should be modeled by the pack");
    assert_eq!(
        read_doubles.return_type,
        Some(JavaType::Array(Box::new(JavaType::Primitive(
            new_java2rust::java_ir::PrimitiveType::Double,
        ))))
    );
    assert_eq!(
        read_doubles.thrown,
        vec![JavaType::Named {
            name: "HDF5Error".to_string(),
            args: Vec::new(),
        }]
    );
    assert_eq!(
        facts.exception_kind_for("HDF5Error"),
        Some(ExceptionKind::Checked)
    );

    let read_double = facts
        .external_api_effect_for_receiver(Some("HDF5File"), "readDouble")
        .expect("HDF5 scalar double read should be modeled by the pack");
    assert_eq!(
        read_double.return_type,
        Some(JavaType::Primitive(
            new_java2rust::java_ir::PrimitiveType::Double,
        ))
    );
    assert_eq!(
        read_double.thrown,
        vec![JavaType::Named {
            name: "HDF5Error".to_string(),
            args: Vec::new(),
        }]
    );

    let read_string_array = facts
        .external_api_effect_for_receiver(Some("HDF5File"), "readStringArray")
        .expect("HDF5 string-array read should be modeled by the pack");
    assert_eq!(
        read_string_array.return_type,
        Some(JavaType::Array(Box::new(JavaType::String)))
    );
    assert_eq!(
        read_string_array.thrown,
        vec![JavaType::Named {
            name: "HDF5Error".to_string(),
            args: Vec::new(),
        }]
    );

    let make_string_array = facts
        .external_api_effect_for_receiver(Some("HDF5File"), "makeStringArray")
        .expect("HDF5 string-array helper should be modeled by the pack");
    assert_eq!(
        make_string_array.return_type,
        Some(JavaType::Array(Box::new(JavaType::String)))
    );
    assert_eq!(
        make_string_array.return_nullability,
        NullabilityState::DefinitelyNonNull
    );

    let make_double = facts
        .external_api_effect_for_receiver(Some("HDF5File"), "makeDouble")
        .expect("HDF5 scalar double helper should be modeled by the pack");
    assert_eq!(
        make_double.return_type,
        Some(JavaType::Primitive(
            new_java2rust::java_ir::PrimitiveType::Double,
        ))
    );

    let make_double_array = facts
        .external_api_effect_for_receiver(Some("HDF5File"), "makeDoubleArray")
        .expect("HDF5 double-array helper should be modeled by the pack");
    assert_eq!(
        make_double_array.return_type,
        Some(JavaType::Array(Box::new(JavaType::Primitive(
            new_java2rust::java_ir::PrimitiveType::Double,
        ))))
    );

    let make_double_matrix = facts
        .external_api_effect_for_receiver(Some("HDF5File"), "makeDoubleMatrix")
        .expect("HDF5 double-matrix helper should be modeled by the pack");
    assert_eq!(
        make_double_matrix.return_type,
        Some(JavaType::Array(Box::new(JavaType::Array(Box::new(
            JavaType::Primitive(new_java2rust::java_ir::PrimitiveType::Double,),
        )))))
    );

    let is_present = facts
        .external_api_effect_for_receiver(Some("HDF5File"), "isPresent")
        .expect("HDF5 presence predicate should be modeled by the pack");
    assert_eq!(is_present.role, ExternalApiRole::BooleanGetter);
    assert_eq!(
        is_present.return_type,
        Some(JavaType::Primitive(
            new_java2rust::java_ir::PrimitiveType::Boolean,
        ))
    );

    let write = facts
        .external_api_effect_for_receiver(Some("HDF5File"), "writeDoubleArray")
        .expect("HDF5 dataset write should be modeled by the pack");
    assert_eq!(write.role, ExternalApiRole::Mutator);
    assert_eq!(
        write.receiver_mutability,
        ReceiverMutabilityFact::MutatesReceiver
    );

    let flush = facts
        .external_api_effect_for_receiver(Some("HDF5File"), "flush")
        .expect("HDF5 flush should be modeled by the pack");
    assert_eq!(flush.role, ExternalApiRole::Mutator);
    assert_eq!(
        flush.receiver_mutability,
        ReceiverMutabilityFact::MutatesReceiver
    );
    assert_eq!(
        flush.thrown,
        vec![JavaType::Named {
            name: "HDF5Error".to_string(),
            args: Vec::new(),
        }]
    );

    let close = facts
        .external_api_effect_for_receiver(Some("HDF5File"), "close")
        .expect("HDF5 close should be modeled by the pack");
    assert_eq!(close.role, ExternalApiRole::SideEffect);
}

#[test]
fn semantic_analysis_uses_hdf5_crate_mapping_pack() {
    let semantic = analyze(
        r#"
class Hdf5PackUse {
    void write(File file, double[] values) {
        HDF5File hdf5 = new HDF5File(file, HDF5File.OpenMode.CREATE);
        if (hdf5.exists("/metrics")) {
            hdf5.writeDoubleArray("/metrics", values);
        }
        hdf5.flush();
        hdf5.close();
    }

    double readScalar(File file) {
        HDF5File hdf5 = new HDF5File(file);
        return hdf5.readDouble("/metrics/value");
    }

    Object labels(File file) {
        HDF5File hdf5 = new HDF5File(file);
        if (hdf5.isPresent("/metrics/labels")) {
            return hdf5.makeStringArray("A", "B");
        }
        return hdf5.makeStringArray();
    }

    Object doubles(File file) {
        HDF5File hdf5 = new HDF5File(file);
        return hdf5.makeDoubleArray(1.0, hdf5.makeDouble(2.0));
    }

    Object matrix(File file) {
        HDF5File hdf5 = new HDF5File(file);
        return hdf5.makeDoubleMatrix(hdf5.makeDoubleArray(1.0));
    }

    Object readMatrix(File file) {
        HDF5File hdf5 = new HDF5File(file);
        return hdf5.readDoubleMatrix("/metrics/matrix");
    }

    Object readLabels(File file) {
        HDF5File hdf5 = new HDF5File(file);
        return hdf5.readStringArray("/metrics/labels");
    }

    void loadLibrary() {
        HDF5Library library = new HDF5Library();
        library.load();
    }
}
"#,
    );

    let constructor = semantic
        .facts
        .external_constructor_for(
            "HDF5File",
            &[
                JavaType::Named {
                    name: "File".to_string(),
                    args: Vec::new(),
                },
                JavaType::Named {
                    name: "OpenMode".to_string(),
                    args: Vec::new(),
                },
            ],
        )
        .unwrap();
    assert_eq!(constructor.provenance, FactProvenance::ExternalLibraryPack);

    let write = semantic
        .facts
        .external_api_effect_for_receiver(Some("HDF5File"), "writeDoubleArray")
        .unwrap();
    assert_eq!(write.provenance, FactProvenance::ExternalLibraryPack);
    assert_eq!(write.role, ExternalApiRole::Mutator);
    assert_eq!(
        write.thrown,
        vec![JavaType::Named {
            name: "HDF5Error".to_string(),
            args: Vec::new(),
        }]
    );
    let flush = semantic
        .facts
        .external_api_effect_for_receiver(Some("HDF5File"), "flush")
        .expect("HDF5 flush should be modeled by the pack");
    assert_eq!(flush.provenance, FactProvenance::ExternalLibraryPack);
    assert_eq!(flush.role, ExternalApiRole::Mutator);
    assert_eq!(
        flush.receiver_mutability,
        ReceiverMutabilityFact::MutatesReceiver
    );
    assert_eq!(flush.thrown, write.thrown);

    let read_double = semantic
        .facts
        .external_api_effect_for_receiver(Some("HDF5File"), "readDouble")
        .expect("HDF5 scalar double read should be modeled by the pack");
    assert_eq!(
        read_double.return_type,
        Some(JavaType::Primitive(
            new_java2rust::java_ir::PrimitiveType::Double,
        ))
    );
    assert_eq!(read_double.provenance, FactProvenance::ExternalLibraryPack);

    let make_string_array = semantic
        .facts
        .external_api_effect_for_receiver(Some("HDF5File"), "makeStringArray")
        .expect("HDF5 string array helper should be modeled by the pack");
    assert_eq!(
        make_string_array.return_type,
        Some(JavaType::Array(Box::new(JavaType::String)))
    );
    assert_eq!(
        make_string_array.return_nullability,
        NullabilityState::DefinitelyNonNull
    );

    let make_double = semantic
        .facts
        .external_api_effect_for_receiver(Some("HDF5File"), "makeDouble")
        .expect("HDF5 scalar double helper should be modeled by the pack");
    assert_eq!(
        make_double.return_type,
        Some(JavaType::Primitive(
            new_java2rust::java_ir::PrimitiveType::Double,
        ))
    );

    let make_double_array = semantic
        .facts
        .external_api_effect_for_receiver(Some("HDF5File"), "makeDoubleArray")
        .expect("HDF5 double-array helper should be modeled by the pack");
    assert_eq!(
        make_double_array.return_type,
        Some(JavaType::Array(Box::new(JavaType::Primitive(
            new_java2rust::java_ir::PrimitiveType::Double,
        ))))
    );

    let make_double_matrix = semantic
        .facts
        .external_api_effect_for_receiver(Some("HDF5File"), "makeDoubleMatrix")
        .expect("HDF5 double-matrix helper should be modeled by the pack");
    assert_eq!(
        make_double_matrix.return_type,
        Some(JavaType::Array(Box::new(JavaType::Array(Box::new(
            JavaType::Primitive(new_java2rust::java_ir::PrimitiveType::Double,),
        )))))
    );

    let read_double_matrix = semantic
        .facts
        .external_api_effect_for_receiver(Some("HDF5File"), "readDoubleMatrix")
        .expect("HDF5 double-matrix read should be modeled by the pack");
    assert_eq!(
        read_double_matrix.return_type,
        Some(JavaType::Array(Box::new(JavaType::Array(Box::new(
            JavaType::Primitive(new_java2rust::java_ir::PrimitiveType::Double,),
        )))))
    );
    assert_eq!(
        read_double_matrix.thrown,
        vec![JavaType::Named {
            name: "HDF5Error".to_string(),
            args: Vec::new(),
        }]
    );

    let read_string_array = semantic
        .facts
        .external_api_effect_for_receiver(Some("HDF5File"), "readStringArray")
        .expect("HDF5 string-array read should be modeled by the pack");
    assert_eq!(
        read_string_array.return_type,
        Some(JavaType::Array(Box::new(JavaType::String)))
    );
    assert_eq!(
        read_string_array.thrown,
        vec![JavaType::Named {
            name: "HDF5Error".to_string(),
            args: Vec::new(),
        }]
    );

    let load = semantic
        .facts
        .external_api_effect_for_receiver(Some("HDF5Library"), "load")
        .expect("HDF5 library load should be modeled by the pack");
    assert_eq!(load.role, ExternalApiRole::SideEffect);
    assert_eq!(
        load.thrown,
        vec![JavaType::Named {
            name: "HDF5Error".to_string(),
            args: Vec::new(),
        }]
    );

    let is_present = semantic
        .facts
        .external_api_effect_for_receiver(Some("HDF5File"), "isPresent")
        .expect("HDF5 presence predicate should be modeled by the pack");
    assert_eq!(is_present.role, ExternalApiRole::BooleanGetter);
    assert_eq!(is_present.provenance, FactProvenance::ExternalLibraryPack);

    let method_effect = semantic
        .facts
        .method_effect_for("Hdf5PackUse", "write")
        .expect("external HDF5 throws should propagate to the enclosing method effect");
    assert_eq!(
        method_effect.thrown,
        vec![JavaType::Named {
            name: "HDF5Error".to_string(),
            args: Vec::new(),
        }]
    );

    assert!(
        !semantic
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "external_pack_missing_api"),
        "newly modeled HDF5 facade calls should not produce missing-pack diagnostics: {:?}",
        semantic.diagnostics
    );
}

#[test]
fn sqlite_pack_declares_sqlite_crate_mapping() {
    let facts = sqlite_pack_facts();

    assert_eq!(
        new_java2rust::stdlib::external_constructor_provenance("SQLiteConfig", &[]),
        FactProvenance::ConservativeFallback,
        "SQLite constructor precision should come from the crate-mapping pack, not stdlib fallback"
    );

    let pack = facts
        .external_library_pack_for_type("org.sqlite.SQLiteConfig")
        .expect("SQLiteConfig should be owned by the SQLite crate-mapping pack");
    assert_eq!(pack.pack_name, "sqlite");
    assert_eq!(pack.mode, ExternalLibraryMode::RustCrateMapping);
    assert_eq!(pack.coverage, ExternalApiCoverage::Mapped);
    assert_eq!(pack.rust_target.as_deref(), Some("sqlite"));
    assert_eq!(
        facts.exception_kind_for("SQLException"),
        Some(ExceptionKind::Checked)
    );

    let config = facts
        .external_constructor_for("SQLiteConfig", &[])
        .expect("SQLiteConfig constructor should be pack-owned");
    assert_eq!(config.provenance, FactProvenance::ExternalLibraryPack);
}

#[test]
fn semantic_analysis_uses_sqlite_crate_mapping_pack() {
    let semantic = analyze(
        r#"
class SqlitePackUse {
    Object connect(String path) {
        SQLiteConfig config = new SQLiteConfig();
        config.setReadOnly(true);
        config.setBusyTimeout(1000);
        return config.createConnection(path);
    }

    Object properties() {
        SQLiteConfig config = new SQLiteConfig();
        return config.toProperties();
    }

    void transact(String path, String query) {
        SQLiteConfig config = new SQLiteConfig();
        SQLiteConnection connection = config.createConnection(path);
        SQLiteStatement statement = connection.prepareStatement(query);
        statement.bindString(1, "sample");
        statement.bindInt(2, 7);
        statement.bindDouble(3, 12.5);
        statement.execute();
        statement.clearParameters();
        statement.close();
        connection.setAutoCommit(false);
        connection.commit();
        connection.close();
    }

    Object firstName(String path, String query) {
        SQLiteConfig config = new SQLiteConfig();
        SQLiteConnection connection = config.createConnection(path);
        SQLiteStatement statement = connection.prepareStatement(query);
        SQLiteResultSet result = statement.executeQuery();
        if (result.next()) {
            result.getString("name");
            result.getLong(1);
            result.getBoolean("active");
            return result.getDouble("score");
        }
        result.close();
        return null;
    }
}
"#,
    );

    let constructor = semantic
        .facts
        .external_constructor_for("SQLiteConfig", &[])
        .unwrap();
    assert_eq!(constructor.provenance, FactProvenance::ExternalLibraryPack);

    let set_read_only = semantic
        .facts
        .external_api_effect_for_receiver(Some("SQLiteConfig"), "setReadOnly")
        .unwrap();
    assert_eq!(set_read_only.role, ExternalApiRole::Setter);
    assert_eq!(
        set_read_only.provenance,
        FactProvenance::ExternalLibraryPack
    );

    let connect = semantic
        .facts
        .external_api_effect_for_receiver(Some("SQLiteConfig"), "createConnection")
        .unwrap();
    assert_eq!(
        connect.return_type,
        Some(JavaType::Named {
            name: "SQLiteConnection".to_string(),
            args: Vec::new(),
        })
    );
    assert_eq!(
        connect.thrown,
        vec![JavaType::Named {
            name: "SQLException".to_string(),
            args: Vec::new(),
        }]
    );
    assert_eq!(
        connect.return_nullability,
        NullabilityState::DefinitelyNonNull
    );
    assert_eq!(connect.provenance, FactProvenance::ExternalLibraryPack);

    let properties = semantic
        .facts
        .external_api_effect_for_receiver(Some("SQLiteConfig"), "toProperties")
        .expect("SQLite properties conversion should be modeled by the pack");
    assert_eq!(
        properties.return_type,
        Some(JavaType::Named {
            name: "Properties".to_string(),
            args: Vec::new(),
        })
    );
    assert_eq!(
        properties.return_nullability,
        NullabilityState::DefinitelyNonNull
    );
    assert!(
        properties.thrown.is_empty(),
        "local config-to-properties conversion should not be modeled as a database boundary"
    );

    let prepare = semantic
        .facts
        .external_api_effect_for_receiver(Some("SQLiteConnection"), "prepareStatement")
        .expect("SQLite statement preparation should be modeled by the pack");
    assert_eq!(
        prepare.thrown,
        vec![JavaType::Named {
            name: "SQLException".to_string(),
            args: Vec::new(),
        }]
    );

    let execute = semantic
        .facts
        .external_api_effect_for_receiver(Some("SQLiteStatement"), "execute")
        .expect("SQLite statement execution should be modeled by the pack");
    assert_eq!(
        execute.thrown,
        vec![JavaType::Named {
            name: "SQLException".to_string(),
            args: Vec::new(),
        }]
    );
    let clear_parameters = semantic
        .facts
        .external_api_effect_for_receiver(Some("SQLiteStatement"), "clearParameters")
        .expect("SQLite statement parameter cleanup should be modeled by the pack");
    assert_eq!(clear_parameters.role, ExternalApiRole::Setter);
    assert_eq!(
        clear_parameters.receiver_mutability,
        ReceiverMutabilityFact::MutatesReceiver
    );
    assert_eq!(clear_parameters.thrown, execute.thrown);

    let statement_close = semantic
        .facts
        .external_api_effect_for_receiver(Some("SQLiteStatement"), "close")
        .expect("SQLite statement close should be modeled by the pack");
    assert_eq!(statement_close.role, ExternalApiRole::SideEffect);
    assert_eq!(statement_close.thrown, execute.thrown);

    let set_auto_commit = semantic
        .facts
        .external_api_effect_for_receiver(Some("SQLiteConnection"), "setAutoCommit")
        .expect("SQLite transaction configuration should be modeled by the pack");
    assert_eq!(set_auto_commit.role, ExternalApiRole::Setter);
    assert_eq!(
        set_auto_commit.receiver_mutability,
        ReceiverMutabilityFact::MutatesReceiver
    );
    assert_eq!(
        set_auto_commit.thrown,
        vec![JavaType::Named {
            name: "SQLException".to_string(),
            args: Vec::new(),
        }]
    );
    let auto_commit_arg = semantic
        .facts
        .external_api_argument_effect_for_receiver(Some("SQLiteConnection"), "setAutoCommit", 0)
        .expect("SQLite auto-commit flag should have an argument boundary fact");
    assert_eq!(
        auto_commit_arg.parameter_nullability,
        NullabilityState::DefinitelyNonNull
    );

    let bind_string = semantic
        .facts
        .external_api_effect_for_receiver(Some("SQLiteStatement"), "bindString")
        .expect("SQLite string binding should be modeled by the pack");
    assert_eq!(bind_string.role, ExternalApiRole::Setter);
    assert_eq!(
        bind_string.receiver_mutability,
        ReceiverMutabilityFact::MutatesReceiver
    );
    let bind_value = semantic
        .facts
        .external_api_argument_effect_for_receiver(Some("SQLiteStatement"), "bindString", 1)
        .expect("SQLite bind value should have an argument boundary fact");
    assert_eq!(
        bind_value.parameter_nullability,
        NullabilityState::DefinitelyNonNull
    );
    assert_eq!(
        bind_value.alias_policy,
        new_java2rust::facts::ExternalArgumentAliasPolicy::Borrowed
    );

    let query = semantic
        .facts
        .external_api_effect_for_receiver(Some("SQLiteStatement"), "executeQuery")
        .expect("SQLite result-set query should be modeled by the pack");
    assert_eq!(
        query.return_type,
        Some(JavaType::Named {
            name: "SQLiteResultSet".to_string(),
            args: Vec::new(),
        })
    );
    assert_eq!(
        query.return_nullability,
        NullabilityState::DefinitelyNonNull
    );

    let next = semantic
        .facts
        .external_api_effect_for_receiver(Some("SQLiteResultSet"), "next")
        .expect("SQLite result-set iteration should be modeled by the pack");
    assert_eq!(next.role, ExternalApiRole::BooleanGetter);
    assert_eq!(
        next.receiver_mutability,
        ReceiverMutabilityFact::MutatesReceiver
    );

    let get_string = semantic
        .facts
        .external_api_effect_for_receiver(Some("SQLiteResultSet"), "getString")
        .expect("SQLite nullable string result getter should be modeled by the pack");
    assert_eq!(get_string.return_type, Some(JavaType::String));
    assert_eq!(get_string.return_nullability, NullabilityState::MaybeNull);
    let get_long = semantic
        .facts
        .external_api_effect_for_receiver(Some("SQLiteResultSet"), "getLong")
        .expect("SQLite long result getter should be modeled by the pack");
    assert_eq!(
        get_long.return_type,
        Some(JavaType::Primitive(PrimitiveType::Long))
    );
    assert_eq!(
        get_long.return_nullability,
        NullabilityState::DefinitelyNonNull
    );
    assert_eq!(get_long.thrown, set_auto_commit.thrown);
    let get_long_index = semantic
        .facts
        .external_api_argument_effect_for_receiver(Some("SQLiteResultSet"), "getLong", 0)
        .expect("SQLite long result getter index should have an argument boundary fact");
    assert_eq!(
        get_long_index.parameter_nullability,
        NullabilityState::DefinitelyNonNull
    );
    let get_boolean = semantic
        .facts
        .external_api_effect_for_receiver(Some("SQLiteResultSet"), "getBoolean")
        .expect("SQLite boolean result getter should be modeled by the pack");
    assert_eq!(
        get_boolean.return_type,
        Some(JavaType::Primitive(PrimitiveType::Boolean))
    );
    assert_eq!(get_boolean.role, ExternalApiRole::BooleanGetter);
    let get_boolean_label = semantic
        .facts
        .external_api_argument_effect_for_receiver(Some("SQLiteResultSet"), "getBoolean", 0)
        .expect("SQLite boolean result getter label should have an argument boundary fact");
    assert_eq!(
        get_boolean_label.alias_policy,
        new_java2rust::facts::ExternalArgumentAliasPolicy::Borrowed
    );

    let method_effect = semantic
        .facts
        .method_effect_for("SqlitePackUse", "transact")
        .expect("external SQLite throws should propagate to the enclosing method effect");
    assert_eq!(
        method_effect.thrown,
        vec![JavaType::Named {
            name: "SQLException".to_string(),
            args: Vec::new(),
        }]
    );
}

#[test]
fn semantic_analysis_uses_java_sql_facade_pack_for_gatk_shaped_sqlite_workflow() {
    let semantic = analyze(
        r#"
import org.sqlite.SQLiteConfig;
import java.sql.*;

class JdbcSqlitePackUse {
    void dump(String path, String catalog, String schemaPattern, String tableName, String[] tableTypes) {
        SQLiteConfig config = new SQLiteConfig();
        config.setReadOnly(true);
        Connection connection = DriverManager.getConnection("jdbc:sqlite:" + path, config.toProperties());
        DatabaseMetaData database = connection.getMetaData();
        String product = database.getDatabaseProductName();
        ResultSet tables = database.getTables(catalog, schemaPattern, tableName, tableTypes);
        while (tables.next()) {
            tables.getString("TABLE_NAME");
        }
        tables.close();
        ResultSet columns = database.getColumns(catalog, schemaPattern, tableName, "GENE");
        while (columns.next()) {
            columns.getString("COLUMN_NAME");
        }
        columns.close();
        Statement statement = connection.createStatement();
        ResultSet results = statement.executeQuery("select gene, score from variants");
        ResultSetMetaData metadata = results.getMetaData();
        int columnCount = metadata.getColumnCount();
        for (int i = 1; i <= columnCount; i++) {
            metadata.getColumnName(i);
            metadata.getColumnType(i);
            metadata.getColumnTypeName(i);
            metadata.getColumnClassName(i);
            metadata.isNullable(i);
            metadata.isSigned(i);
            metadata.isAutoIncrement(i);
            metadata.getPrecision(i);
            metadata.getScale(i);
            metadata.getColumnDisplaySize(i);
        }
        PreparedStatement update = connection.prepareStatement("update variants set score = ?, depth = ?, active = ? where gene = ?");
        update.setDouble(1, 7.5);
        update.setString(2, path);
        update.setInt(3, columnCount);
        update.setLong(4, 123L);
        update.setBoolean(5, true);
        update.executeUpdate();
        update.close();
        while (results.next()) {
            results.getString(1);
            results.getString("gene");
            results.getInt("depth");
            results.getLong(2);
            results.getDouble("score");
            results.getFloat("quality");
            results.getShort(3);
            results.getByte("flags");
            results.getBoolean("active");
            results.getObject("payload");
            results.getBytes("blob");
            results.getDate("created_on");
            results.getTimestamp("updated_at");
            results.wasNull();
        }
        results.close();
        statement.close();
        connection.close();
    }
}
"#,
    );

    let java_sql = java_sql_pack_facts();
    assert!(
        java_sql
            .external_library_pack_for_type("java.sql.Connection")
            .is_some()
    );
    assert!(
        java_sql
            .external_library_pack_for_type("java.sql.ResultSetMetaData")
            .is_some()
    );
    assert!(
        java_sql
            .external_library_pack_for_type("java.sql.DatabaseMetaData")
            .is_some()
    );
    assert_eq!(
        java_sql.exception_kind_for("java.sql.SQLException"),
        Some(ExceptionKind::Checked)
    );

    let connection = semantic
        .facts
        .external_api_effect_for_receiver(Some("driver_manager"), "getConnection")
        .expect("DriverManager connection factory should be modeled by java.sql facade facts");
    assert_eq!(
        connection.return_type,
        Some(JavaType::Named {
            name: "Connection".to_string(),
            args: Vec::new(),
        })
    );
    assert_eq!(
        connection.thrown,
        vec![JavaType::Named {
            name: "SQLException".to_string(),
            args: Vec::new(),
        }]
    );

    let statement = semantic
        .facts
        .external_api_effect_for_receiver(Some("java.sql.Connection"), "createStatement")
        .expect("generic JDBC statements should be modeled by java.sql facade facts");
    assert_eq!(
        statement.return_type,
        Some(JavaType::Named {
            name: "Statement".to_string(),
            args: Vec::new(),
        })
    );

    let prepared_statement = semantic
        .facts
        .external_api_effect_for_receiver(Some("java.sql.Connection"), "prepareStatement")
        .expect("generic JDBC prepared statements should be modeled by java.sql facade facts");
    assert_eq!(
        prepared_statement.return_type,
        Some(JavaType::Named {
            name: "PreparedStatement".to_string(),
            args: Vec::new(),
        })
    );
    let database_metadata = semantic
        .facts
        .external_api_effect_for_receiver(Some("java.sql.Connection"), "getMetaData")
        .expect("generic JDBC database metadata should be modeled by java.sql facade facts");
    assert_eq!(
        database_metadata.return_type,
        Some(JavaType::Named {
            name: "DatabaseMetaData".to_string(),
            args: Vec::new(),
        })
    );

    let product_name = semantic
        .facts
        .external_api_effect_for_receiver(
            Some("java.sql.DatabaseMetaData"),
            "getDatabaseProductName",
        )
        .expect("database product metadata should be modeled by java.sql facade facts");
    assert_eq!(product_name.return_type, Some(JavaType::String));
    assert_eq!(
        product_name.return_nullability,
        NullabilityState::DefinitelyNonNull
    );

    let table_metadata = semantic
        .facts
        .external_api_effect_for_receiver(Some("java.sql.DatabaseMetaData"), "getTables")
        .expect("table metadata lookup should be modeled by java.sql facade facts");
    assert_eq!(
        table_metadata.return_type,
        Some(JavaType::Named {
            name: "ResultSet".to_string(),
            args: Vec::new(),
        })
    );
    let table_types_arg = semantic
        .facts
        .external_api_argument_effect_for_receiver(
            Some("java.sql.DatabaseMetaData"),
            "getTables",
            3,
        )
        .expect("table metadata type filters should have pack-owned argument facts");
    assert_eq!(
        table_types_arg.parameter_type,
        Some(JavaType::Array(Box::new(JavaType::String)))
    );
    let column_metadata = semantic
        .facts
        .external_api_effect_for_receiver(Some("java.sql.DatabaseMetaData"), "getColumns")
        .expect("column metadata lookup should be modeled by java.sql facade facts");
    assert_eq!(
        column_metadata.return_type,
        Some(JavaType::Named {
            name: "ResultSet".to_string(),
            args: Vec::new(),
        })
    );

    let bind_string = semantic
        .facts
        .external_api_effect_for_receiver(Some("java.sql.PreparedStatement"), "setString")
        .expect("prepared statement string binding should be modeled by java.sql facade facts");
    assert_eq!(bind_string.role, ExternalApiRole::SideEffect);
    assert_eq!(
        bind_string.receiver_mutability,
        ReceiverMutabilityFact::MutatesReceiver
    );

    let bind_int = semantic
        .facts
        .external_api_argument_effect_for_receiver(Some("java.sql.PreparedStatement"), "setInt", 1)
        .expect("prepared statement integer values should have pack-owned argument facts");
    assert_eq!(
        bind_int.parameter_type,
        Some(JavaType::Primitive(PrimitiveType::Int))
    );
    assert_eq!(
        bind_int.parameter_nullability,
        NullabilityState::DefinitelyNonNull
    );

    let bind_double = semantic
        .facts
        .external_api_argument_effect_for_receiver(
            Some("java.sql.PreparedStatement"),
            "setDouble",
            1,
        )
        .expect("prepared statement floating-point values should have pack-owned argument facts");
    assert_eq!(
        bind_double.parameter_type,
        Some(JavaType::Primitive(PrimitiveType::Double))
    );

    let bind_long = semantic
        .facts
        .external_api_argument_effect_for_receiver(Some("java.sql.PreparedStatement"), "setLong", 1)
        .expect("prepared statement long values should have pack-owned argument facts");
    assert_eq!(
        bind_long.parameter_type,
        Some(JavaType::Primitive(PrimitiveType::Long))
    );

    let bind_boolean = semantic
        .facts
        .external_api_argument_effect_for_receiver(
            Some("java.sql.PreparedStatement"),
            "setBoolean",
            1,
        )
        .expect("prepared statement boolean values should have pack-owned argument facts");
    assert_eq!(
        bind_boolean.parameter_type,
        Some(JavaType::Primitive(PrimitiveType::Boolean))
    );

    let update = semantic
        .facts
        .external_api_effect_for_receiver(Some("java.sql.PreparedStatement"), "executeUpdate")
        .expect("prepared statement updates should be modeled by java.sql facade facts");
    assert_eq!(
        update.return_type,
        Some(JavaType::Primitive(PrimitiveType::Int))
    );

    let metadata = semantic
        .facts
        .external_api_effect_for_receiver(Some("java.sql.ResultSet"), "getMetaData")
        .expect("result-set metadata should be modeled by java.sql facade facts");
    assert_eq!(
        metadata.return_type,
        Some(JavaType::Named {
            name: "ResultSetMetaData".to_string(),
            args: Vec::new(),
        })
    );
    assert_eq!(
        metadata.return_nullability,
        NullabilityState::DefinitelyNonNull
    );

    let column_name = semantic
        .facts
        .external_api_effect_for_receiver(Some("java.sql.ResultSetMetaData"), "getColumnName")
        .expect("metadata column name lookup should be modeled by java.sql facade facts");
    assert_eq!(column_name.role, ExternalApiRole::IndexedGetter);
    assert_eq!(column_name.return_type, Some(JavaType::String));
    let column_type = semantic
        .facts
        .external_api_effect_for_receiver(Some("java.sql.ResultSetMetaData"), "getColumnType")
        .expect("metadata column type lookup should be modeled by java.sql facade facts");
    assert_eq!(column_type.role, ExternalApiRole::IndexedGetter);
    assert_eq!(
        column_type.return_type,
        Some(JavaType::Primitive(PrimitiveType::Int))
    );
    let column_type_name = semantic
        .facts
        .external_api_effect_for_receiver(Some("java.sql.ResultSetMetaData"), "getColumnTypeName")
        .expect("metadata column type-name lookup should be modeled by java.sql facade facts");
    assert_eq!(column_type_name.return_type, Some(JavaType::String));
    let column_class_name = semantic
        .facts
        .external_api_effect_for_receiver(Some("java.sql.ResultSetMetaData"), "getColumnClassName")
        .expect("metadata column class-name lookup should be modeled by java.sql facade facts");
    assert_eq!(column_class_name.return_type, Some(JavaType::String));
    let column_class_index = semantic
        .facts
        .external_api_argument_effect_for_receiver(
            Some("java.sql.ResultSetMetaData"),
            "getColumnClassName",
            0,
        )
        .expect("metadata class-name lookup should have pack-owned column-index facts");
    assert_eq!(
        column_class_index.parameter_type,
        Some(JavaType::Primitive(PrimitiveType::Int))
    );
    let column_nullable = semantic
        .facts
        .external_api_effect_for_receiver(Some("java.sql.ResultSetMetaData"), "isNullable")
        .expect("metadata column nullability lookup should be modeled by java.sql facade facts");
    assert_eq!(column_nullable.role, ExternalApiRole::IndexedGetter);
    assert_eq!(
        column_nullable.return_type,
        Some(JavaType::Primitive(PrimitiveType::Int))
    );
    let column_signed = semantic
        .facts
        .external_api_effect_for_receiver(Some("java.sql.ResultSetMetaData"), "isSigned")
        .expect("metadata column signedness lookup should be modeled by java.sql facade facts");
    assert_eq!(column_signed.role, ExternalApiRole::BooleanGetter);
    assert_eq!(
        column_signed.return_type,
        Some(JavaType::Primitive(PrimitiveType::Boolean))
    );
    let column_auto_increment = semantic
        .facts
        .external_api_effect_for_receiver(Some("java.sql.ResultSetMetaData"), "isAutoIncrement")
        .expect("metadata column auto-increment lookup should be modeled by java.sql facade facts");
    assert_eq!(column_auto_increment.role, ExternalApiRole::BooleanGetter);
    let column_precision = semantic
        .facts
        .external_api_effect_for_receiver(Some("java.sql.ResultSetMetaData"), "getPrecision")
        .expect("metadata column precision lookup should be modeled by java.sql facade facts");
    assert_eq!(
        column_precision.return_type,
        Some(JavaType::Primitive(PrimitiveType::Int))
    );
    let column_scale_index = semantic
        .facts
        .external_api_argument_effect_for_receiver(
            Some("java.sql.ResultSetMetaData"),
            "getScale",
            0,
        )
        .expect("metadata column scale lookup should have pack-owned index facts");
    assert_eq!(
        column_scale_index.parameter_type,
        Some(JavaType::Primitive(PrimitiveType::Int))
    );

    let result_string = semantic
        .facts
        .external_api_effect_for_receiver(Some("java.sql.ResultSet"), "getString")
        .expect("JDBC nullable result strings should be modeled by java.sql facade facts");
    assert_eq!(
        result_string.return_nullability,
        NullabilityState::MaybeNull
    );
    let result_int = semantic
        .facts
        .external_api_effect_for_receiver(Some("java.sql.ResultSet"), "getInt")
        .expect("JDBC integer result getters should be modeled by java.sql facade facts");
    assert_eq!(
        result_int.return_type,
        Some(JavaType::Primitive(PrimitiveType::Int))
    );
    assert_eq!(
        result_int.return_nullability,
        NullabilityState::DefinitelyNonNull
    );
    let result_long = semantic
        .facts
        .external_api_effect_for_receiver(Some("java.sql.ResultSet"), "getLong")
        .expect("JDBC long result getters should be modeled by java.sql facade facts");
    assert_eq!(
        result_long.return_type,
        Some(JavaType::Primitive(PrimitiveType::Long))
    );
    let result_double = semantic
        .facts
        .external_api_effect_for_receiver(Some("java.sql.ResultSet"), "getDouble")
        .expect("JDBC double result getters should be modeled by java.sql facade facts");
    assert_eq!(
        result_double.return_type,
        Some(JavaType::Primitive(PrimitiveType::Double))
    );
    let result_float = semantic
        .facts
        .external_api_effect_for_receiver(Some("java.sql.ResultSet"), "getFloat")
        .expect("JDBC float result getters should be modeled by java.sql facade facts");
    assert_eq!(
        result_float.return_type,
        Some(JavaType::Primitive(PrimitiveType::Float))
    );
    let result_short = semantic
        .facts
        .external_api_effect_for_receiver(Some("java.sql.ResultSet"), "getShort")
        .expect("JDBC short result getters should be modeled by java.sql facade facts");
    assert_eq!(
        result_short.return_type,
        Some(JavaType::Primitive(PrimitiveType::Short))
    );
    let result_byte = semantic
        .facts
        .external_api_effect_for_receiver(Some("java.sql.ResultSet"), "getByte")
        .expect("JDBC byte result getters should be modeled by java.sql facade facts");
    assert_eq!(
        result_byte.return_type,
        Some(JavaType::Primitive(PrimitiveType::Byte))
    );
    let result_float_label = semantic
        .facts
        .external_api_argument_effect_for_receiver(Some("java.sql.ResultSet"), "getFloat", 0)
        .expect("JDBC float result getters should have pack-owned argument facts");
    assert_eq!(
        result_float_label.alias_policy,
        new_java2rust::facts::ExternalArgumentAliasPolicy::Borrowed
    );
    let result_boolean = semantic
        .facts
        .external_api_effect_for_receiver(Some("java.sql.ResultSet"), "getBoolean")
        .expect("JDBC boolean result getters should be modeled by java.sql facade facts");
    assert_eq!(result_boolean.role, ExternalApiRole::BooleanGetter);
    assert_eq!(
        result_boolean.return_type,
        Some(JavaType::Primitive(PrimitiveType::Boolean))
    );
    let result_boolean_label = semantic
        .facts
        .external_api_argument_effect_for_receiver(Some("java.sql.ResultSet"), "getBoolean", 0)
        .expect("JDBC boolean result getters should have pack-owned argument facts");
    assert_eq!(
        result_boolean_label.alias_policy,
        new_java2rust::facts::ExternalArgumentAliasPolicy::Borrowed
    );
    let result_object = semantic
        .facts
        .external_api_effect_for_receiver(Some("java.sql.ResultSet"), "getObject")
        .expect("JDBC object result getters should be modeled by java.sql facade facts");
    assert_eq!(
        result_object.return_type,
        Some(JavaType::Named {
            name: "Object".to_string(),
            args: Vec::new(),
        })
    );
    assert_eq!(
        result_object.return_nullability,
        NullabilityState::MaybeNull
    );
    let result_bytes = semantic
        .facts
        .external_api_effect_for_receiver(Some("java.sql.ResultSet"), "getBytes")
        .expect("JDBC byte-array result getters should be modeled by java.sql facade facts");
    assert_eq!(
        result_bytes.return_type,
        Some(JavaType::Array(Box::new(JavaType::Primitive(
            PrimitiveType::Byte
        ))))
    );
    assert_eq!(result_bytes.return_nullability, NullabilityState::MaybeNull);
    let result_date = semantic
        .facts
        .external_api_effect_for_receiver(Some("java.sql.ResultSet"), "getDate")
        .expect("JDBC date result getters should be modeled by java.sql facade facts");
    assert_eq!(
        result_date.return_type,
        Some(JavaType::Named {
            name: "Date".to_string(),
            args: Vec::new(),
        })
    );
    let result_timestamp = semantic
        .facts
        .external_api_effect_for_receiver(Some("java.sql.ResultSet"), "getTimestamp")
        .expect("JDBC timestamp result getters should be modeled by java.sql facade facts");
    assert_eq!(
        result_timestamp.return_type,
        Some(JavaType::Named {
            name: "Timestamp".to_string(),
            args: Vec::new(),
        })
    );
    assert!(
        java_sql
            .external_library_pack_for_type("java.sql.Timestamp")
            .is_some(),
        "java.sql temporal result values should stay inside the JDBC pack boundary"
    );
    let result_was_null = semantic
        .facts
        .external_api_effect_for_receiver(Some("java.sql.ResultSet"), "wasNull")
        .expect("JDBC primitive result null checks should be modeled by java.sql facade facts");
    assert_eq!(result_was_null.role, ExternalApiRole::BooleanGetter);
    assert_eq!(
        result_was_null.return_type,
        Some(JavaType::Primitive(PrimitiveType::Boolean))
    );
    assert_eq!(
        result_was_null.return_nullability,
        NullabilityState::DefinitelyNonNull
    );

    let method_effect = semantic
        .facts
        .method_effect_for("JdbcSqlitePackUse", "dump")
        .expect("JDBC checked exceptions should propagate to the enclosing method effect");
    assert_eq!(
        method_effect.thrown,
        vec![JavaType::Named {
            name: "SQLException".to_string(),
            args: Vec::new(),
        },]
    );

    assert!(
        !semantic
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "external_pack_missing_api"),
        "modeled JDBC workflow should not produce missing-pack diagnostics: {:?}",
        semantic.diagnostics
    );
}

#[test]
fn java_sql_heavyweight_result_conversions_are_explicit_unsupported_boundaries() {
    let semantic = analyze(
        r#"
import java.sql.*;

class HeavyJdbcResultUse {
    Object inspect(ResultSet results) {
        results.getBlob("payload");
        results.getClob("document");
        results.getArray("tags");
        return results.getSQLXML("xml");
    }
}
"#,
    );

    for method in ["getBlob", "getClob", "getArray", "getSQLXML"] {
        assert!(
            semantic.diagnostics.iter().any(|diagnostic| diagnostic.code
                == "external_pack_unsupported_api"
                && diagnostic
                    .message
                    .contains(&format!("java.sql.ResultSet.{method}"))
                && diagnostic.message.contains("java.sql")),
            "unsupported JDBC {method} should produce a java.sql pack-owned diagnostic: {:?}",
            semantic.diagnostics
        );
        assert!(
            !semantic.diagnostics.iter().any(|diagnostic| diagnostic.code
                == "external_pack_missing_api"
                && diagnostic
                    .message
                    .contains(&format!("java.sql.ResultSet.{method}"))),
            "unsupported JDBC {method} should not also be reported as a missing pack API: {:?}",
            semantic.diagnostics
        );
    }
}

#[test]
fn missing_java_sql_pack_members_report_structured_diagnostics() {
    let semantic = analyze(
        r#"
import java.sql.*;

class MissingJdbcPackUse {
    Object inspect(Connection connection, ResultSet results) {
        connection.unmodeledConnectionMethod();
        results.unmodeledResultSetMethod();
        Object opened = new Connection();
        return results.unmodeledColumn;
    }
}
"#,
    );

    assert!(
        semantic.diagnostics.iter().any(|diagnostic| diagnostic.code
            == "external_pack_missing_api"
            && diagnostic
                .message
                .contains("java.sql.Connection.unmodeledConnectionMethod")
            && diagnostic.message.contains("java.sql")),
        "missing JDBC connection APIs should produce a java.sql pack-owned diagnostic: {:?}",
        semantic.diagnostics
    );
    assert!(
        semantic.diagnostics.iter().any(|diagnostic| diagnostic.code
            == "external_pack_missing_api"
            && diagnostic
                .message
                .contains("java.sql.ResultSet.unmodeledResultSetMethod")
            && diagnostic.message.contains("java.sql")),
        "missing JDBC result-set APIs should produce a java.sql pack-owned diagnostic: {:?}",
        semantic.diagnostics
    );
    assert!(
        semantic.diagnostics.iter().any(|diagnostic| diagnostic.code
            == "external_pack_missing_field"
            && diagnostic
                .message
                .contains("java.sql.ResultSet.unmodeledColumn")
            && diagnostic.message.contains("java.sql")),
        "missing JDBC result-set fields should produce a java.sql pack-owned diagnostic: {:?}",
        semantic.diagnostics
    );
    assert!(
        semantic.diagnostics.iter().any(|diagnostic| diagnostic.code
            == "external_pack_missing_constructor"
            && diagnostic
                .message
                .contains("java.sql.Connection constructor")
            && diagnostic.message.contains("java.sql")),
        "missing JDBC facade constructors should produce a java.sql pack-owned diagnostic: {:?}",
        semantic.diagnostics
    );
    assert!(
        semantic
            .facts
            .external_api_effect_for_receiver(
                Some("java.sql.Connection"),
                "unmodeledConnectionMethod"
            )
            .is_none(),
        "missing JDBC APIs should not become generic external guesses"
    );
    assert!(
        semantic
            .facts
            .external_api_effect_for_receiver(
                Some("java.sql.ResultSet"),
                "unmodeledResultSetMethod"
            )
            .is_none(),
        "missing JDBC APIs should not become generic external guesses"
    );
    assert!(
        semantic
            .facts
            .external_field_for("java.sql.ResultSet", "unmodeledColumn")
            .is_none(),
        "missing JDBC fields should not become generic external guesses"
    );
    assert!(
        semantic
            .facts
            .external_constructor_for("java.sql.Connection", &[])
            .is_none(),
        "missing JDBC constructors should not become generic external guesses"
    );
}

#[test]
fn java_sql_pack_does_not_claim_project_connection_or_reflection_class_names() {
    let semantic = analyze(
        r#"
class Connection {
    Object getTarget() { return null; }
}

class ProjectConnectionUse {
    void inspect(Connection connection, Class toolClass) {
        connection.getTarget();
        toolClass.getSimpleName();
        toolClass.getName();
        toolClass.getCanonicalName();
        toolClass.isAssignableFrom(Connection.class);
        Class.forName("org.example.Tool");
    }
}
"#,
    );

    assert!(
        semantic
            .facts
            .external_api_effect_for_receiver(Some("Connection"), "getTarget")
            .is_none(),
        "unimported project-domain Connection should not be claimed by the java.sql pack"
    );
    let simple_name = semantic
        .facts
        .external_api_effect_for_receiver(Some("Class"), "getSimpleName")
        .expect("Class.getSimpleName should be modeled as a Java platform API");
    assert_eq!(simple_name.role, ExternalApiRole::Getter);
    assert_eq!(simple_name.return_type, Some(JavaType::String));
    assert_eq!(
        simple_name.return_nullability,
        NullabilityState::DefinitelyNonNull
    );
    assert_eq!(simple_name.provenance, FactProvenance::BuiltinExternalModel);

    let canonical_name = semantic
        .facts
        .external_api_effect_for_receiver(Some("Class"), "getCanonicalName")
        .expect("Class.getCanonicalName should be modeled as a Java platform API");
    assert_eq!(canonical_name.return_type, Some(JavaType::String));
    assert_eq!(
        canonical_name.return_nullability,
        NullabilityState::MaybeNull
    );

    let for_name = semantic
        .facts
        .external_api_effect_for_receiver_with_arity(Some("Class"), "forName", Some(1))
        .expect("Class.forName(String) should be modeled as a Java platform API");
    assert_eq!(
        for_name.return_type,
        Some(JavaType::Named {
            name: "Class".to_string(),
            args: Vec::new(),
        })
    );
    assert_eq!(
        for_name.thrown,
        vec![JavaType::Named {
            name: "ClassNotFoundException".to_string(),
            args: Vec::new(),
        }]
    );
    assert_eq!(for_name.provenance, FactProvenance::BuiltinExternalModel);

    let for_name_arg = semantic
        .facts
        .external_api_argument_effect_for_receiver(Some("Class"), "forName", 0)
        .expect("Class.forName argument should carry platform boundary facts");
    assert_eq!(for_name_arg.parameter_type, Some(JavaType::String));
    assert_eq!(
        for_name_arg.parameter_nullability,
        NullabilityState::DefinitelyNonNull
    );
    assert_eq!(
        for_name_arg.provenance,
        FactProvenance::BuiltinExternalModel
    );

    assert_eq!(
        semantic.facts.exception_kind_for("ClassNotFoundException"),
        Some(ExceptionKind::Checked)
    );
    assert!(
        !semantic
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("external pack java.sql")),
        "java.sql pack should not report missing APIs for project Connection/Class names: {:?}",
        semantic.diagnostics
    );
}

fn analyze(source: &str) -> semantic_analysis::SemanticFacts {
    let parsed = parser::parse_java_source(source);
    let resolved = resolve::resolve_program(&parsed.program, parsed.diagnostics);
    semantic_analysis::analyze_program(&resolved.program, &resolved.facts, &FactSet::default())
}
