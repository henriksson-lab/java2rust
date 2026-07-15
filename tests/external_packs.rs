use new_java2rust::external_facts::parse_external_facts;
use new_java2rust::external_packs::{
    builtin_external_library_packs, builtin_external_pack_manifests, hdf5_pack_facts,
    htsjdk_sam_header_pack_facts, htsjdk_samtools_util_pack_facts,
    htsjdk_tribble_feature_pack_facts, htsjdk_variantcontext_pack_facts, htsjdk_vcf_pack_facts,
    sqlite_pack_facts,
};
use new_java2rust::facts::{
    ExternalApiCoverage, ExternalApiRole, ExternalLibraryMode, FactProvenance, FactSet,
    NullabilityState, ReceiverMutabilityFact,
};
use new_java2rust::java_ir::JavaType;
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

    let hdf5 = manifests
        .iter()
        .find(|manifest| manifest.pack_name == "hdf5")
        .expect("hdf5 manifest should exist");
    assert!(hdf5.owns_type("HDF5File.OpenMode"));
    assert_eq!(hdf5.mode, ExternalLibraryMode::RustCrateMapping);
    assert_eq!(hdf5.default_coverage, ExternalApiCoverage::Mapped);
    assert_eq!(hdf5.rust_target, Some("hdf5_pure_rust"));

    let sqlite = manifests
        .iter()
        .find(|manifest| manifest.pack_name == "sqlite")
        .expect("sqlite manifest should exist");
    assert!(sqlite.owns_type("org.sqlite.SQLiteConnection"));
    assert_eq!(sqlite.rust_target, Some("sqlite"));
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
    }
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
fn htsjdk_variantcontext_pack_models_builder_and_getter_effects() {
    let facts = htsjdk_variantcontext_pack_facts();

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

    let sequence_dictionary = facts
        .external_api_effect_for_receiver(Some("VCFHeader"), "getSequenceDictionary")
        .expect("VCFHeader sequence dictionary getter should be modeled by the pack");
    assert_eq!(sequence_dictionary.role, ExternalApiRole::Getter);
    assert_eq!(
        sequence_dictionary.return_nullability,
        NullabilityState::DefinitelyNonNull
    );

    let add_line = facts
        .external_api_effect_for_receiver(Some("VCFHeader"), "addMetaDataLine")
        .expect("VCFHeader metadata mutation should be modeled by the pack");
    assert_eq!(add_line.role, ExternalApiRole::Mutator);
    assert_eq!(
        add_line.receiver_mutability,
        ReceiverMutabilityFact::MutatesReceiver
    );
}

#[test]
fn semantic_analysis_uses_vcf_pack_for_header_metadata_calls() {
    let semantic = analyze(
        r#"
import htsjdk.variant.vcf.VCFHeader;
import htsjdk.variant.vcf.VCFHeaderLine;
import htsjdk.variant.vcf.VCFHeaderLineType;
import htsjdk.variant.vcf.VCFInfoHeaderLine;

class VcfPackUse {
    VCFHeader header() {
        VCFHeader header = new VCFHeader();
        header.addMetaDataLine(new VCFHeaderLine("source", "tool"));
        header.addMetaDataLine(new VCFInfoHeaderLine("DP", 1, VCFHeaderLineType.Integer, "Depth"));
        return header;
    }

    Object samples(VCFHeader header) {
        return header.getGenotypeSamples();
    }

    Object dictionary(VCFHeader header) {
        return header.getSequenceDictionary();
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

    let add_line = semantic
        .facts
        .external_api_effect_for_receiver(Some("VCFHeader"), "addMetaDataLine")
        .unwrap();
    assert_eq!(add_line.role, ExternalApiRole::Mutator);
    assert_eq!(add_line.provenance, FactProvenance::ExternalLibraryPack);

    let header = semantic
        .facts
        .external_constructor_for("VCFHeader", &[])
        .unwrap();
    assert_eq!(header.provenance, FactProvenance::ExternalLibraryPack);
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

    let set_dictionary = facts
        .external_api_effect_for_receiver(Some("SAMFileHeader"), "setSequenceDictionary")
        .expect("header dictionary setter should be modeled by the pack");
    assert_eq!(set_dictionary.role, ExternalApiRole::Mutator);
    assert_eq!(
        set_dictionary.receiver_mutability,
        ReceiverMutabilityFact::MutatesReceiver
    );

    let sample = facts
        .external_api_effect_for_receiver(Some("SAMReadGroupRecord"), "getSample")
        .expect("read-group sample getter should be modeled by the pack");
    assert_eq!(sample.role, ExternalApiRole::Getter);
    assert_eq!(sample.return_nullability, NullabilityState::MaybeNull);

    let unknown_length = facts
        .external_field_for("SAMSequenceRecord", "UNKNOWN_SEQUENCE_LENGTH")
        .expect("sequence length constant should be pack-owned");
    assert_eq!(
        unknown_length.provenance,
        FactProvenance::ExternalLibraryPack
    );
}

#[test]
fn semantic_analysis_uses_sam_header_pack_for_dictionary_workflows() {
    let semantic = analyze(
        r#"
import htsjdk.samtools.SAMFileHeader;
import htsjdk.samtools.SAMReadGroupRecord;
import htsjdk.samtools.SAMSequenceDictionary;
import htsjdk.samtools.SAMSequenceRecord;
import java.util.List;

class SamHeaderPackUse {
    SAMFileHeader header(List<SAMSequenceRecord> records) {
        SAMSequenceDictionary dictionary = new SAMSequenceDictionary(records);
        SAMFileHeader header = new SAMFileHeader();
        header.setSequenceDictionary(dictionary);
        header.addReadGroup(new SAMReadGroupRecord("sample"));
        return header;
    }

    Object dictionary(SAMFileHeader header) {
        return header.getSequenceDictionary();
    }

    Object contig(SAMSequenceDictionary dictionary) {
        return dictionary.getSequence(0);
    }

    Object sample(SAMReadGroupRecord readGroup) {
        return readGroup.getSample();
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

    let header = semantic
        .facts
        .external_constructor_for("SAMFileHeader", &[])
        .unwrap();
    assert_eq!(header.provenance, FactProvenance::ExternalLibraryPack);
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
}

#[test]
fn semantic_analysis_uses_tribble_feature_pack_for_coordinate_workflows() {
    let semantic = analyze(
        r#"
import htsjdk.tribble.Feature;
import htsjdk.tribble.SimpleFeature;

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
fn htsjdk_samtools_util_pack_declares_interval_and_iterator_boundary() {
    let facts = htsjdk_samtools_util_pack_facts();

    let pack = facts
        .external_library_pack_for_type("htsjdk.samtools.util.Interval")
        .expect("samtools.util Interval should be owned by the external pack");
    assert_eq!(pack.pack_name, "htsjdk.samtools.util");
    assert_eq!(pack.mode, ExternalLibraryMode::ExternalFactModel);
    assert_eq!(pack.coverage, ExternalApiCoverage::Semantic);
    assert_eq!(pack.provenance, FactProvenance::ExternalLibraryPack);

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

    let peek = facts
        .external_api_effect_for_receiver(Some("PeekableIterator"), "peek")
        .expect("PeekableIterator peek should be modeled by the pack");
    assert_eq!(peek.role, ExternalApiRole::Getter);
    assert_eq!(peek.return_nullability, NullabilityState::DefinitelyNonNull);

    let close = facts
        .external_api_effect_for_receiver(Some("CloseableIterator"), "close")
        .expect("CloseableIterator close should be modeled by the pack");
    assert_eq!(close.role, ExternalApiRole::SideEffect);
}

#[test]
fn semantic_analysis_uses_samtools_util_pack_for_interval_workflows() {
    let semantic = analyze(
        r#"
import htsjdk.samtools.util.Interval;
import htsjdk.samtools.util.Locatable;
import htsjdk.samtools.util.PeekableIterator;
import java.util.Iterator;

class SamtoolsUtilPackUse {
    Interval interval(String contig, int start, int end) {
        return new Interval(contig, start, end);
    }

    int width(Locatable value) {
        return value.getEnd() - value.getStart();
    }

    boolean overlaps(Interval left, Interval right) {
        return left.intersects(right);
    }

    Object peek(Iterator<Interval> intervals) {
        return new PeekableIterator<Interval>(intervals).peek();
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

    let intersects = semantic
        .facts
        .external_api_effect_for_receiver(Some("Interval"), "intersects")
        .unwrap();
    assert_eq!(intersects.role, ExternalApiRole::BooleanGetter);
    assert_eq!(intersects.provenance, FactProvenance::ExternalLibraryPack);

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
fn user_override_beats_crate_mapping_and_builtin_external_facts() {
    let mut hdf5_facts = hdf5_pack_facts();
    hdf5_facts
        .external_api_effect_facts
        .push(new_java2rust::facts::ExternalApiEffectFact {
            method_name: "exists".to_string(),
            receiver_type: Some("HDF5File".to_string()),
            role: ExternalApiRole::Getter,
            receiver_mutability: ReceiverMutabilityFact::ReadOnly,
            return_type: None,
            return_nullability: NullabilityState::MaybeNull,
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
            role: ExternalApiRole::Getter,
            receiver_mutability: ReceiverMutabilityFact::ReadOnly,
            return_type: None,
            return_nullability: NullabilityState::DefinitelyNonNull,
            provenance: FactProvenance::BuiltinExternalModel,
        });
    builtin_facts
        .external_api_effect_facts
        .push(new_java2rust::facts::ExternalApiEffectFact {
            method_name: "size".to_string(),
            receiver_type: Some("List".to_string()),
            role: ExternalApiRole::Unknown,
            receiver_mutability: ReceiverMutabilityFact::Unknown,
            return_type: None,
            return_nullability: NullabilityState::Unknown,
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
import htsjdk.variant.variantcontext.VariantContext;
import htsjdk.variant.variantcontext.VariantContextBuilder;

class VariantPackUse {
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
}
"#,
    );

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
        return left.equals(right);
    }
}
"#,
    );

    assert!(
        semantic.diagnostics.is_empty(),
        "Object methods on pack-owned values should not be diagnosed as pack coverage gaps"
    );
    let equals = semantic
        .facts
        .external_api_effect_for_receiver(Some("Allele"), "equals")
        .expect("Object.equals should still be modeled for pack-owned values");
    assert_eq!(equals.provenance, FactProvenance::BuiltinExternalModel);
    assert_eq!(equals.role, ExternalApiRole::Getter);
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
        .expect("HDF5File constructor should be pack-owned by arity");
    assert_eq!(file.provenance, FactProvenance::ExternalLibraryPack);
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

    let write = facts
        .external_api_effect_for_receiver(Some("HDF5File"), "writeDoubleArray")
        .expect("HDF5 dataset write should be modeled by the pack");
    assert_eq!(write.role, ExternalApiRole::Mutator);
    assert_eq!(
        write.receiver_mutability,
        ReceiverMutabilityFact::MutatesReceiver
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
        hdf5.close();
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
}

#[test]
fn sqlite_pack_declares_sqlite_crate_mapping() {
    let facts = sqlite_pack_facts();

    let pack = facts
        .external_library_pack_for_type("org.sqlite.SQLiteConfig")
        .expect("SQLiteConfig should be owned by the SQLite crate-mapping pack");
    assert_eq!(pack.pack_name, "sqlite");
    assert_eq!(pack.mode, ExternalLibraryMode::RustCrateMapping);
    assert_eq!(pack.coverage, ExternalApiCoverage::Mapped);
    assert_eq!(pack.rust_target.as_deref(), Some("sqlite"));

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
        connect.return_nullability,
        NullabilityState::DefinitelyNonNull
    );
    assert_eq!(connect.provenance, FactProvenance::ExternalLibraryPack);
}

fn analyze(source: &str) -> semantic_analysis::SemanticFacts {
    let parsed = parser::parse_java_source(source);
    let resolved = resolve::resolve_program(&parsed.program, parsed.diagnostics);
    semantic_analysis::analyze_program(&resolved.program, &resolved.facts, &FactSet::default())
}
