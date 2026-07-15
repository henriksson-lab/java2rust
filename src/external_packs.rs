use crate::facts::{
    ExceptionFact, ExceptionKind, ExternalApiCoverage, ExternalApiEffectFact, ExternalApiRole,
    ExternalConstructorFact, ExternalFieldFact, ExternalLibraryMode, ExternalLibraryPackFact,
    FactProvenance, FactSet, FieldKey, HierarchyOverride, HierarchyStrategy, NullabilityState,
    ReceiverMutabilityFact, TypeCapabilityOverride, UnsupportedExternalApiFact,
};
use crate::java_ir::JavaType;

const VARIANTCONTEXT_PACK: &str = "htsjdk.variant.variantcontext";
const VCF_PACK: &str = "htsjdk.variant.vcf";
const SAM_HEADER_PACK: &str = "htsjdk.samtools.header";
const TRIBBLE_FEATURE_PACK: &str = "htsjdk.tribble.feature";
const SAMTOOLS_UTIL_PACK: &str = "htsjdk.samtools.util";
const HDF5_PACK: &str = "hdf5";
const SQLITE_PACK: &str = "sqlite";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalPackManifest {
    pub pack_name: &'static str,
    pub owned_type_patterns: &'static [&'static str],
    pub mode: ExternalLibraryMode,
    pub default_coverage: ExternalApiCoverage,
    pub rust_target: Option<&'static str>,
    pub unsupported_apis: &'static [ExternalPackUnsupportedApi],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExternalPackUnsupportedApi {
    pub receiver_type: &'static str,
    pub method_name: &'static str,
    pub reason: &'static str,
}

impl ExternalPackManifest {
    pub fn owns_type(&self, type_name: &str) -> bool {
        self.owned_type_patterns
            .iter()
            .any(|pattern| type_name == *pattern || type_name.starts_with(&format!("{pattern}.")))
    }

    pub fn unsupported_api(
        &self,
        receiver_type: &str,
        method_name: &str,
    ) -> Option<&ExternalPackUnsupportedApi> {
        self.unsupported_apis
            .iter()
            .find(|api| api.receiver_type == receiver_type && api.method_name == method_name)
    }

    pub fn coverage_for_api(&self, receiver_type: &str, method_name: &str) -> ExternalApiCoverage {
        if self.unsupported_api(receiver_type, method_name).is_some() {
            ExternalApiCoverage::Unsupported
        } else {
            self.default_coverage
        }
    }

    fn apply_boundary_facts(&self, facts: &mut FactSet) {
        for package_prefix in self.owned_type_patterns {
            facts.external_library_packs.push(ExternalLibraryPackFact {
                pack_name: self.pack_name.to_string(),
                package_prefix: (*package_prefix).to_string(),
                mode: self.mode,
                coverage: self.default_coverage,
                rust_target: self.rust_target.map(str::to_string),
                provenance: FactProvenance::ExternalLibraryPack,
            });
        }
        for api in self.unsupported_apis {
            facts
                .unsupported_external_api_facts
                .push(UnsupportedExternalApiFact {
                    pack_name: self.pack_name.to_string(),
                    receiver_type: api.receiver_type.to_string(),
                    method_name: api.method_name.to_string(),
                    reason: api.reason.to_string(),
                    provenance: FactProvenance::ExternalLibraryPack,
                });
        }
    }
}

const HDF5_OWNED_TYPES: &[&str] = &["HDF5Library", "HDF5File", "HDF5File.OpenMode", "OpenMode"];
const SQLITE_OWNED_TYPES: &[&str] = &[
    "org.sqlite.SQLiteConfig",
    "SQLiteConfig",
    "org.sqlite.SQLiteConnection",
    "SQLiteConnection",
    "org.sqlite.SQLiteStatement",
    "SQLiteStatement",
];
const SAMTOOLS_UTIL_OWNED_TYPES: &[&str] = &[
    "htsjdk.samtools.util.Locatable",
    "htsjdk.samtools.util.Interval",
    "htsjdk.samtools.util.IntervalList",
    "htsjdk.samtools.util.IntervalTree",
    "htsjdk.samtools.util.IntervalMergerIterator",
    "htsjdk.samtools.util.Histogram",
    "htsjdk.samtools.util.PeekableIterator",
    "htsjdk.samtools.util.MergingIterator",
    "htsjdk.samtools.util.RuntimeIOException",
];
const TRIBBLE_FEATURE_OWNED_TYPES: &[&str] = &[
    "htsjdk.tribble.Feature",
    "htsjdk.tribble.SimpleFeature",
    "htsjdk.tribble.NamedFeature",
    "htsjdk.tribble.bed.BEDFeature",
];
const SAM_HEADER_OWNED_TYPES: &[&str] = &[
    "htsjdk.samtools.SAMSequenceDictionary",
    "htsjdk.samtools.SAMSequenceRecord",
    "htsjdk.samtools.SAMFileHeader",
    "htsjdk.samtools.SAMReadGroupRecord",
];
const VCF_OWNED_TYPES: &[&str] = &[VCF_PACK];
const VARIANTCONTEXT_OWNED_TYPES: &[&str] = &[VARIANTCONTEXT_PACK];
const VCF_UNSUPPORTED_APIS: &[ExternalPackUnsupportedApi] = &[
    ExternalPackUnsupportedApi {
        receiver_type: "htsjdk.variant.vcf.VCFCodec",
        method_name: "readHeader",
        reason: "VCF codec parsing is an IO/runtime boundary; map through a facade",
    },
    ExternalPackUnsupportedApi {
        receiver_type: "VCFCodec",
        method_name: "readHeader",
        reason: "VCF codec parsing is an IO/runtime boundary; map through a facade",
    },
    ExternalPackUnsupportedApi {
        receiver_type: "htsjdk.variant.vcf.VCFRecordCodec",
        method_name: "readHeader",
        reason: "VCF codec parsing is an IO/runtime boundary; map through a facade",
    },
    ExternalPackUnsupportedApi {
        receiver_type: "VCFRecordCodec",
        method_name: "readHeader",
        reason: "VCF codec parsing is an IO/runtime boundary; map through a facade",
    },
];
const VARIANTCONTEXT_UNSUPPORTED_APIS: &[ExternalPackUnsupportedApi] = &[
    ExternalPackUnsupportedApi {
        receiver_type: "htsjdk.variant.variantcontext.VariantContext",
        method_name: "fullyDecode",
        reason: "requires HTSJDK codec/runtime state; keep as explicit external boundary",
    },
    ExternalPackUnsupportedApi {
        receiver_type: "VariantContext",
        method_name: "fullyDecode",
        reason: "requires HTSJDK codec/runtime state; keep as explicit external boundary",
    },
];

pub fn builtin_external_pack_manifests() -> Vec<ExternalPackManifest> {
    vec![
        ExternalPackManifest {
            pack_name: VARIANTCONTEXT_PACK,
            owned_type_patterns: VARIANTCONTEXT_OWNED_TYPES,
            mode: ExternalLibraryMode::ExternalFactModel,
            default_coverage: ExternalApiCoverage::Semantic,
            rust_target: Some("crate::external::htsjdk::variantcontext"),
            unsupported_apis: VARIANTCONTEXT_UNSUPPORTED_APIS,
        },
        ExternalPackManifest {
            pack_name: VCF_PACK,
            owned_type_patterns: VCF_OWNED_TYPES,
            mode: ExternalLibraryMode::ExternalFactModel,
            default_coverage: ExternalApiCoverage::Semantic,
            rust_target: Some("crate::external::htsjdk::vcf"),
            unsupported_apis: VCF_UNSUPPORTED_APIS,
        },
        ExternalPackManifest {
            pack_name: SAM_HEADER_PACK,
            owned_type_patterns: SAM_HEADER_OWNED_TYPES,
            mode: ExternalLibraryMode::ExternalFactModel,
            default_coverage: ExternalApiCoverage::Semantic,
            rust_target: Some("crate::external::htsjdk::sam_header"),
            unsupported_apis: &[],
        },
        ExternalPackManifest {
            pack_name: TRIBBLE_FEATURE_PACK,
            owned_type_patterns: TRIBBLE_FEATURE_OWNED_TYPES,
            mode: ExternalLibraryMode::ExternalFactModel,
            default_coverage: ExternalApiCoverage::Semantic,
            rust_target: Some("crate::external::htsjdk::tribble_feature"),
            unsupported_apis: &[],
        },
        ExternalPackManifest {
            pack_name: SAMTOOLS_UTIL_PACK,
            owned_type_patterns: SAMTOOLS_UTIL_OWNED_TYPES,
            mode: ExternalLibraryMode::ExternalFactModel,
            default_coverage: ExternalApiCoverage::Semantic,
            rust_target: Some("crate::external::htsjdk::samtools_util"),
            unsupported_apis: &[],
        },
        ExternalPackManifest {
            pack_name: HDF5_PACK,
            owned_type_patterns: HDF5_OWNED_TYPES,
            mode: ExternalLibraryMode::RustCrateMapping,
            default_coverage: ExternalApiCoverage::Mapped,
            rust_target: Some("hdf5_pure_rust"),
            unsupported_apis: &[],
        },
        ExternalPackManifest {
            pack_name: SQLITE_PACK,
            owned_type_patterns: SQLITE_OWNED_TYPES,
            mode: ExternalLibraryMode::RustCrateMapping,
            default_coverage: ExternalApiCoverage::Mapped,
            rust_target: Some("sqlite"),
            unsupported_apis: &[],
        },
    ]
}

fn manifest_by_name(pack_name: &str) -> ExternalPackManifest {
    builtin_external_pack_manifests()
        .into_iter()
        .find(|manifest| manifest.pack_name == pack_name)
        .unwrap_or_else(|| panic!("missing builtin external pack manifest for {pack_name}"))
}

pub fn builtin_external_library_packs() -> FactSet {
    let mut facts = FactSet::default();
    facts.extend_from(htsjdk_variantcontext_pack_facts());
    facts.extend_from(htsjdk_vcf_pack_facts());
    facts.extend_from(htsjdk_sam_header_pack_facts());
    facts.extend_from(htsjdk_tribble_feature_pack_facts());
    facts.extend_from(htsjdk_samtools_util_pack_facts());
    facts.extend_from(hdf5_pack_facts());
    facts.extend_from(sqlite_pack_facts());
    facts
}

pub fn external_api_effect_for_receiver(
    receiver_type: Option<&str>,
    method_name: &str,
) -> Option<ExternalApiEffectFact> {
    let receiver_type = receiver_type?;
    if let Some(fact) = htsjdk_variantcontext_pack_facts()
        .external_api_effect_for_receiver(Some(receiver_type), method_name)
        .cloned()
    {
        return Some(fact);
    }
    htsjdk_vcf_pack_facts()
        .external_api_effect_for_receiver(Some(receiver_type), method_name)
        .cloned()
        .or_else(|| {
            htsjdk_sam_header_pack_facts()
                .external_api_effect_for_receiver(Some(receiver_type), method_name)
                .cloned()
        })
        .or_else(|| {
            htsjdk_tribble_feature_pack_facts()
                .external_api_effect_for_receiver(Some(receiver_type), method_name)
                .cloned()
        })
        .or_else(|| {
            htsjdk_samtools_util_pack_facts()
                .external_api_effect_for_receiver(Some(receiver_type), method_name)
                .cloned()
        })
        .or_else(|| {
            hdf5_pack_facts()
                .external_api_effect_for_receiver(Some(receiver_type), method_name)
                .cloned()
        })
        .or_else(|| {
            sqlite_pack_facts()
                .external_api_effect_for_receiver(Some(receiver_type), method_name)
                .cloned()
        })
}

pub fn external_api_return_type_for_receiver(
    receiver_type: Option<&str>,
    method_name: &str,
) -> Option<JavaType> {
    external_api_effect_for_receiver(receiver_type, method_name).and_then(|fact| fact.return_type)
}

pub fn unsupported_external_api_for_receiver(
    receiver_type: Option<&str>,
    method_name: &str,
) -> Option<UnsupportedExternalApiFact> {
    let receiver_type = receiver_type?;
    if let Some(fact) = htsjdk_variantcontext_pack_facts()
        .unsupported_external_api_for_receiver(receiver_type, method_name)
        .cloned()
    {
        return Some(fact);
    }
    htsjdk_vcf_pack_facts()
        .unsupported_external_api_for_receiver(receiver_type, method_name)
        .cloned()
        .or_else(|| {
            htsjdk_sam_header_pack_facts()
                .unsupported_external_api_for_receiver(receiver_type, method_name)
                .cloned()
        })
        .or_else(|| {
            htsjdk_tribble_feature_pack_facts()
                .unsupported_external_api_for_receiver(receiver_type, method_name)
                .cloned()
        })
        .or_else(|| {
            htsjdk_samtools_util_pack_facts()
                .unsupported_external_api_for_receiver(receiver_type, method_name)
                .cloned()
        })
        .or_else(|| {
            hdf5_pack_facts()
                .unsupported_external_api_for_receiver(receiver_type, method_name)
                .cloned()
        })
        .or_else(|| {
            sqlite_pack_facts()
                .unsupported_external_api_for_receiver(receiver_type, method_name)
                .cloned()
        })
}

pub fn external_constructor_for(
    type_name: &str,
    arg_types: &[JavaType],
) -> Option<ExternalConstructorFact> {
    if let Some(fact) = htsjdk_variantcontext_pack_facts()
        .external_constructor_for(type_name, arg_types)
        .cloned()
    {
        return Some(fact);
    }
    htsjdk_vcf_pack_facts()
        .external_constructor_for(type_name, arg_types)
        .cloned()
        .or_else(|| {
            htsjdk_sam_header_pack_facts()
                .external_constructor_for(type_name, arg_types)
                .cloned()
        })
        .or_else(|| {
            htsjdk_tribble_feature_pack_facts()
                .external_constructor_for(type_name, arg_types)
                .cloned()
        })
        .or_else(|| {
            htsjdk_samtools_util_pack_facts()
                .external_constructor_for(type_name, arg_types)
                .cloned()
        })
        .or_else(|| {
            hdf5_pack_facts()
                .external_constructor_for(type_name, arg_types)
                .cloned()
        })
        .or_else(|| {
            sqlite_pack_facts()
                .external_constructor_for(type_name, arg_types)
                .cloned()
        })
}

pub fn external_field_for(type_name: &str, field_name: &str) -> Option<ExternalFieldFact> {
    if let Some(fact) = htsjdk_variantcontext_pack_facts()
        .external_field_for(type_name, field_name)
        .cloned()
    {
        return Some(fact);
    }
    if let Some(fact) = htsjdk_vcf_pack_facts()
        .external_field_for(type_name, field_name)
        .cloned()
    {
        return Some(fact);
    }
    htsjdk_sam_header_pack_facts()
        .external_field_for(type_name, field_name)
        .cloned()
        .or_else(|| {
            htsjdk_tribble_feature_pack_facts()
                .external_field_for(type_name, field_name)
                .cloned()
        })
        .or_else(|| {
            htsjdk_samtools_util_pack_facts()
                .external_field_for(type_name, field_name)
                .cloned()
        })
        .or_else(|| {
            hdf5_pack_facts()
                .external_field_for(type_name, field_name)
                .cloned()
        })
        .or_else(|| {
            sqlite_pack_facts()
                .external_field_for(type_name, field_name)
                .cloned()
        })
}

pub fn external_library_pack_for_type(type_name: &str) -> Option<ExternalLibraryPackFact> {
    pack_fact_for_type(htsjdk_variantcontext_pack_facts(), type_name)
        .or_else(|| pack_fact_for_type(htsjdk_vcf_pack_facts(), type_name))
        .or_else(|| pack_fact_for_type(htsjdk_sam_header_pack_facts(), type_name))
        .or_else(|| pack_fact_for_type(htsjdk_tribble_feature_pack_facts(), type_name))
        .or_else(|| pack_fact_for_type(htsjdk_samtools_util_pack_facts(), type_name))
        .or_else(|| pack_fact_for_type(hdf5_pack_facts(), type_name))
        .or_else(|| pack_fact_for_type(sqlite_pack_facts(), type_name))
}

fn pack_fact_for_type(facts: FactSet, type_name: &str) -> Option<ExternalLibraryPackFact> {
    if let Some(pack) = facts.external_library_pack_for_type(type_name) {
        return Some(pack.clone());
    }
    let owns_type = facts
        .hierarchy_overrides
        .iter()
        .any(|fact| fact.type_name == type_name)
        || facts
            .type_capability_overrides
            .iter()
            .any(|fact| fact.type_name == type_name)
        || facts
            .external_constructor_facts
            .iter()
            .any(|fact| fact.type_name == type_name)
        || facts
            .external_api_effect_facts
            .iter()
            .any(|fact| fact.receiver_type.as_deref() == Some(type_name))
        || facts
            .external_field_facts
            .iter()
            .any(|fact| fact.field.type_name == type_name)
        || facts
            .unsupported_external_api_facts
            .iter()
            .any(|fact| fact.receiver_type == type_name);
    owns_type
        .then(|| facts.external_library_packs.first().cloned())
        .flatten()
}

pub fn hdf5_pack_facts() -> FactSet {
    let mut facts = FactSet::default();
    manifest_by_name(HDF5_PACK).apply_boundary_facts(&mut facts);

    for type_name in ["HDF5Library", "HDF5File", "HDF5File.OpenMode", "OpenMode"] {
        facts.hierarchy_overrides.push(HierarchyOverride {
            type_name: type_name.to_string(),
            strategy: HierarchyStrategy::ErasedObject,
            provenance: FactProvenance::ExternalLibraryPack,
        });
    }
    push_cloneable_final(&mut facts, "OpenMode");
    push_cloneable_final(&mut facts, "HDF5File.OpenMode");

    for (type_name, arg_count) in [
        ("HDF5Library", 0usize),
        ("HDF5File", 1usize),
        ("HDF5File", 2usize),
    ] {
        facts
            .external_constructor_facts
            .push(ExternalConstructorFact {
                type_name: type_name.to_string(),
                arg_types: vec![JavaType::Unknown("_".to_string()); arg_count],
                provenance: FactProvenance::ExternalLibraryPack,
            });
    }

    for method in ["isOpen", "exists", "contains"] {
        push_effect_with_return_type(
            &mut facts,
            "HDF5File",
            method,
            ExternalApiRole::BooleanGetter,
            Some(JavaType::Primitive(crate::java_ir::PrimitiveType::Boolean)),
            NullabilityState::DefinitelyNonNull,
        );
    }
    for method in [
        "getFile",
        "getDataset",
        "read",
        "readDoubleArray",
        "readIntArray",
    ] {
        let return_type = match method {
            "readDoubleArray" => Some(JavaType::Array(Box::new(JavaType::Primitive(
                crate::java_ir::PrimitiveType::Double,
            )))),
            "readIntArray" => Some(JavaType::Array(Box::new(JavaType::Primitive(
                crate::java_ir::PrimitiveType::Int,
            )))),
            "getFile" => Some(JavaType::Named {
                name: "File".to_string(),
                args: Vec::new(),
            }),
            _ => None,
        };
        push_effect_with_return_type(
            &mut facts,
            "HDF5File",
            method,
            ExternalApiRole::Getter,
            return_type,
            NullabilityState::Unknown,
        );
    }
    for method in ["write", "writeDoubleArray", "writeIntArray", "flush"] {
        push_effect(
            &mut facts,
            "HDF5File",
            method,
            ExternalApiRole::Mutator,
            NullabilityState::Unknown,
        );
    }
    push_effect(
        &mut facts,
        "HDF5File",
        "close",
        ExternalApiRole::SideEffect,
        NullabilityState::Unknown,
    );

    for (type_name, field_name) in [
        ("HDF5File", "OpenMode"),
        ("HDF5File.OpenMode", "READ_ONLY"),
        ("HDF5File.OpenMode", "CREATE"),
        ("OpenMode", "READ_ONLY"),
        ("OpenMode", "CREATE"),
    ] {
        facts.external_field_facts.push(ExternalFieldFact {
            field: FieldKey {
                type_name: type_name.to_string(),
                field_name: field_name.to_string(),
            },
            provenance: FactProvenance::ExternalLibraryPack,
        });
    }

    facts
}

pub fn sqlite_pack_facts() -> FactSet {
    let mut facts = FactSet::default();
    manifest_by_name(SQLITE_PACK).apply_boundary_facts(&mut facts);

    for type_name in [
        "org.sqlite.SQLiteConfig",
        "SQLiteConfig",
        "org.sqlite.SQLiteConnection",
        "SQLiteConnection",
        "org.sqlite.SQLiteStatement",
        "SQLiteStatement",
    ] {
        facts.hierarchy_overrides.push(HierarchyOverride {
            type_name: type_name.to_string(),
            strategy: HierarchyStrategy::ErasedObject,
            provenance: FactProvenance::ExternalLibraryPack,
        });
    }

    for (type_name, arg_count) in [
        ("org.sqlite.SQLiteConfig", 0usize),
        ("SQLiteConfig", 0usize),
    ] {
        facts
            .external_constructor_facts
            .push(ExternalConstructorFact {
                type_name: type_name.to_string(),
                arg_types: vec![JavaType::Unknown("_".to_string()); arg_count],
                provenance: FactProvenance::ExternalLibraryPack,
            });
    }

    for receiver in ["org.sqlite.SQLiteConfig", "SQLiteConfig"] {
        for method in [
            "setReadOnly",
            "setBusyTimeout",
            "setSharedCache",
            "setJournalMode",
        ] {
            push_effect(
                &mut facts,
                receiver,
                method,
                ExternalApiRole::Setter,
                NullabilityState::Unknown,
            );
        }
        push_effect_with_return_type(
            &mut facts,
            receiver,
            "createConnection",
            ExternalApiRole::Getter,
            Some(JavaType::Named {
                name: "SQLiteConnection".to_string(),
                args: Vec::new(),
            }),
            NullabilityState::DefinitelyNonNull,
        );
    }
    for receiver in ["org.sqlite.SQLiteConnection", "SQLiteConnection"] {
        for method in ["prepareStatement", "createStatement"] {
            push_effect(
                &mut facts,
                receiver,
                method,
                ExternalApiRole::Getter,
                NullabilityState::DefinitelyNonNull,
            );
        }
        for method in ["commit", "rollback", "close"] {
            push_effect(
                &mut facts,
                receiver,
                method,
                ExternalApiRole::SideEffect,
                NullabilityState::Unknown,
            );
        }
    }
    for receiver in ["org.sqlite.SQLiteStatement", "SQLiteStatement"] {
        for method in ["execute", "executeUpdate"] {
            push_effect(
                &mut facts,
                receiver,
                method,
                ExternalApiRole::SideEffect,
                NullabilityState::Unknown,
            );
        }
        push_effect(
            &mut facts,
            receiver,
            "executeQuery",
            ExternalApiRole::Getter,
            NullabilityState::DefinitelyNonNull,
        );
    }

    facts
}

pub fn htsjdk_samtools_util_pack_facts() -> FactSet {
    let mut facts = FactSet::default();
    manifest_by_name(SAMTOOLS_UTIL_PACK).apply_boundary_facts(&mut facts);

    for type_name in [
        "htsjdk.samtools.util.Locatable",
        "Locatable",
        "htsjdk.samtools.util.Interval",
        "Interval",
        "htsjdk.samtools.util.IntervalList",
        "IntervalList",
        "htsjdk.samtools.util.IntervalTree",
        "IntervalTree",
        "htsjdk.samtools.util.IntervalMergerIterator",
        "IntervalMergerIterator",
        "htsjdk.samtools.util.Histogram",
        "Histogram",
        "htsjdk.samtools.util.PeekableIterator",
        "PeekableIterator",
        "htsjdk.samtools.util.MergingIterator",
        "MergingIterator",
        "htsjdk.samtools.util.CloseableIterator",
        "CloseableIterator",
        "htsjdk.samtools.util.RuntimeIOException",
        "RuntimeIOException",
    ] {
        facts.hierarchy_overrides.push(HierarchyOverride {
            type_name: type_name.to_string(),
            strategy: HierarchyStrategy::ErasedObject,
            provenance: FactProvenance::ExternalLibraryPack,
        });
    }

    for type_name in [
        "htsjdk.samtools.util.Interval",
        "Interval",
        "htsjdk.samtools.util.IntervalList",
        "IntervalList",
    ] {
        let mut override_fact = empty_capability(type_name);
        override_fact.clone = Some(true);
        override_fact.final_type = Some(true);
        override_fact.implements = vec![named("Locatable")];
        facts.type_capability_overrides.push(override_fact);
    }

    for type_name in [
        "htsjdk.samtools.util.IntervalTree",
        "IntervalTree",
        "htsjdk.samtools.util.Histogram",
        "Histogram",
        "htsjdk.samtools.util.PeekableIterator",
        "PeekableIterator",
        "htsjdk.samtools.util.MergingIterator",
        "MergingIterator",
    ] {
        push_cloneable_final(&mut facts, type_name);
    }

    for (type_name, arg_count) in [
        ("htsjdk.samtools.util.Interval", 1usize),
        ("htsjdk.samtools.util.Interval", 2usize),
        ("htsjdk.samtools.util.Interval", 3usize),
        ("htsjdk.samtools.util.Interval", 5usize),
        ("Interval", 1usize),
        ("Interval", 2usize),
        ("Interval", 3usize),
        ("Interval", 5usize),
        ("htsjdk.samtools.util.IntervalList", 1usize),
        ("IntervalList", 1usize),
        ("htsjdk.samtools.util.IntervalTree", 0usize),
        ("IntervalTree", 0usize),
        ("htsjdk.samtools.util.IntervalMergerIterator", 4usize),
        ("IntervalMergerIterator", 4usize),
        ("htsjdk.samtools.util.Histogram", 0usize),
        ("htsjdk.samtools.util.Histogram", 2usize),
        ("Histogram", 0usize),
        ("Histogram", 2usize),
        ("htsjdk.samtools.util.PeekableIterator", 1usize),
        ("PeekableIterator", 1usize),
        ("htsjdk.samtools.util.MergingIterator", 2usize),
        ("MergingIterator", 2usize),
        ("htsjdk.samtools.util.RuntimeIOException", 1usize),
        ("htsjdk.samtools.util.RuntimeIOException", 2usize),
        ("RuntimeIOException", 1usize),
        ("RuntimeIOException", 2usize),
    ] {
        facts
            .external_constructor_facts
            .push(ExternalConstructorFact {
                type_name: type_name.to_string(),
                arg_types: vec![JavaType::Unknown("_".to_string()); arg_count],
                provenance: FactProvenance::ExternalLibraryPack,
            });
    }

    for receiver in [
        "htsjdk.samtools.util.Locatable",
        "Locatable",
        "htsjdk.samtools.util.Interval",
        "Interval",
        "htsjdk.samtools.util.IntervalList",
        "IntervalList",
    ] {
        push_effect(
            &mut facts,
            receiver,
            "getContig",
            ExternalApiRole::Getter,
            NullabilityState::DefinitelyNonNull,
        );
        push_effect(
            &mut facts,
            receiver,
            "getStart",
            ExternalApiRole::Getter,
            NullabilityState::DefinitelyNonNull,
        );
        push_effect(
            &mut facts,
            receiver,
            "getEnd",
            ExternalApiRole::Getter,
            NullabilityState::DefinitelyNonNull,
        );
    }

    for method in ["getName", "getInterval", "toString"] {
        push_effect(
            &mut facts,
            "htsjdk.samtools.util.Interval",
            method,
            ExternalApiRole::Getter,
            if method == "getName" {
                NullabilityState::MaybeNull
            } else {
                NullabilityState::DefinitelyNonNull
            },
        );
        push_effect(
            &mut facts,
            "Interval",
            method,
            ExternalApiRole::Getter,
            if method == "getName" {
                NullabilityState::MaybeNull
            } else {
                NullabilityState::DefinitelyNonNull
            },
        );
    }

    for receiver in [
        "htsjdk.samtools.util.Interval",
        "Interval",
        "htsjdk.samtools.util.IntervalList",
        "IntervalList",
    ] {
        for method in ["isNegativeStrand", "intersects", "contains", "overlaps"] {
            push_effect(
                &mut facts,
                receiver,
                method,
                ExternalApiRole::BooleanGetter,
                NullabilityState::DefinitelyNonNull,
            );
        }
    }

    for receiver in [
        "htsjdk.samtools.util.IntervalList",
        "IntervalList",
        "htsjdk.samtools.util.IntervalTree",
        "IntervalTree",
        "htsjdk.samtools.util.Histogram",
        "Histogram",
    ] {
        for method in ["size", "isEmpty", "iterator"] {
            push_effect(
                &mut facts,
                receiver,
                method,
                ExternalApiRole::Getter,
                NullabilityState::DefinitelyNonNull,
            );
        }
    }

    for receiver in [
        "htsjdk.samtools.util.PeekableIterator",
        "PeekableIterator",
        "htsjdk.samtools.util.MergingIterator",
        "MergingIterator",
        "htsjdk.samtools.util.CloseableIterator",
        "CloseableIterator",
    ] {
        for method in ["hasNext", "next"] {
            push_effect(
                &mut facts,
                receiver,
                method,
                if method == "hasNext" {
                    ExternalApiRole::BooleanGetter
                } else {
                    ExternalApiRole::Getter
                },
                NullabilityState::DefinitelyNonNull,
            );
        }
    }
    for receiver in [
        "htsjdk.samtools.util.PeekableIterator",
        "PeekableIterator",
        "htsjdk.samtools.util.CloseableIterator",
        "CloseableIterator",
    ] {
        push_effect(
            &mut facts,
            receiver,
            "close",
            ExternalApiRole::SideEffect,
            NullabilityState::Unknown,
        );
    }
    push_effect(
        &mut facts,
        "htsjdk.samtools.util.PeekableIterator",
        "peek",
        ExternalApiRole::Getter,
        NullabilityState::DefinitelyNonNull,
    );
    push_effect(
        &mut facts,
        "PeekableIterator",
        "peek",
        ExternalApiRole::Getter,
        NullabilityState::DefinitelyNonNull,
    );

    facts.exception_facts.push(ExceptionFact {
        type_name: "htsjdk.samtools.util.RuntimeIOException".to_string(),
        kind: ExceptionKind::Unchecked,
        provenance: FactProvenance::ExternalLibraryPack,
    });
    facts.exception_facts.push(ExceptionFact {
        type_name: "RuntimeIOException".to_string(),
        kind: ExceptionKind::Unchecked,
        provenance: FactProvenance::ExternalLibraryPack,
    });

    facts
}

pub fn htsjdk_tribble_feature_pack_facts() -> FactSet {
    let mut facts = FactSet::default();
    manifest_by_name(TRIBBLE_FEATURE_PACK).apply_boundary_facts(&mut facts);

    for type_name in [
        "htsjdk.tribble.Feature",
        "Feature",
        "htsjdk.tribble.SimpleFeature",
        "SimpleFeature",
        "htsjdk.tribble.NamedFeature",
        "NamedFeature",
        "htsjdk.tribble.bed.BEDFeature",
        "BEDFeature",
    ] {
        facts.hierarchy_overrides.push(HierarchyOverride {
            type_name: type_name.to_string(),
            strategy: HierarchyStrategy::ErasedObject,
            provenance: FactProvenance::ExternalLibraryPack,
        });
    }

    for type_name in [
        "htsjdk.tribble.SimpleFeature",
        "SimpleFeature",
        "htsjdk.tribble.NamedFeature",
        "NamedFeature",
        "htsjdk.tribble.bed.BEDFeature",
        "BEDFeature",
    ] {
        push_cloneable_final(&mut facts, type_name);
    }

    for type_name in [
        "htsjdk.tribble.SimpleFeature",
        "SimpleFeature",
        "htsjdk.tribble.NamedFeature",
        "NamedFeature",
        "htsjdk.tribble.bed.BEDFeature",
        "BEDFeature",
    ] {
        let mut override_fact = empty_capability(type_name);
        override_fact.implements = vec![named("Feature"), named("Locatable")];
        facts.type_capability_overrides.push(override_fact);
    }

    for (type_name, arg_count) in [
        ("htsjdk.tribble.SimpleFeature", 3usize),
        ("SimpleFeature", 3usize),
    ] {
        facts
            .external_constructor_facts
            .push(ExternalConstructorFact {
                type_name: type_name.to_string(),
                arg_types: vec![JavaType::Unknown("_".to_string()); arg_count],
                provenance: FactProvenance::ExternalLibraryPack,
            });
    }

    for receiver in [
        "htsjdk.tribble.Feature",
        "Feature",
        "htsjdk.tribble.SimpleFeature",
        "SimpleFeature",
        "htsjdk.tribble.NamedFeature",
        "NamedFeature",
        "htsjdk.tribble.bed.BEDFeature",
        "BEDFeature",
    ] {
        push_effect(
            &mut facts,
            receiver,
            "getContig",
            ExternalApiRole::Getter,
            NullabilityState::DefinitelyNonNull,
        );
        push_effect(
            &mut facts,
            receiver,
            "getStart",
            ExternalApiRole::Getter,
            NullabilityState::DefinitelyNonNull,
        );
        push_effect(
            &mut facts,
            receiver,
            "getEnd",
            ExternalApiRole::Getter,
            NullabilityState::DefinitelyNonNull,
        );
    }

    for receiver in [
        "htsjdk.tribble.SimpleFeature",
        "SimpleFeature",
        "htsjdk.tribble.NamedFeature",
        "NamedFeature",
        "htsjdk.tribble.bed.BEDFeature",
        "BEDFeature",
    ] {
        push_effect(
            &mut facts,
            receiver,
            "getName",
            ExternalApiRole::Getter,
            NullabilityState::MaybeNull,
        );
    }

    facts
}

pub fn htsjdk_sam_header_pack_facts() -> FactSet {
    let mut facts = FactSet::default();
    manifest_by_name(SAM_HEADER_PACK).apply_boundary_facts(&mut facts);

    for type_name in [
        "htsjdk.samtools.SAMSequenceDictionary",
        "SAMSequenceDictionary",
        "htsjdk.samtools.SAMSequenceRecord",
        "SAMSequenceRecord",
        "htsjdk.samtools.SAMFileHeader",
        "SAMFileHeader",
        "htsjdk.samtools.SAMReadGroupRecord",
        "SAMReadGroupRecord",
        "htsjdk.samtools.SAMFileHeader.SortOrder",
        "SAMFileHeader.SortOrder",
        "SortOrder",
        "htsjdk.samtools.SAMFileHeader.GroupOrder",
        "SAMFileHeader.GroupOrder",
        "GroupOrder",
    ] {
        facts.hierarchy_overrides.push(HierarchyOverride {
            type_name: type_name.to_string(),
            strategy: HierarchyStrategy::ErasedObject,
            provenance: FactProvenance::ExternalLibraryPack,
        });
    }

    for type_name in [
        "htsjdk.samtools.SAMSequenceDictionary",
        "SAMSequenceDictionary",
        "htsjdk.samtools.SAMSequenceRecord",
        "SAMSequenceRecord",
        "htsjdk.samtools.SAMFileHeader",
        "SAMFileHeader",
        "htsjdk.samtools.SAMReadGroupRecord",
        "SAMReadGroupRecord",
        "htsjdk.samtools.SAMFileHeader.SortOrder",
        "SAMFileHeader.SortOrder",
        "SortOrder",
        "htsjdk.samtools.SAMFileHeader.GroupOrder",
        "SAMFileHeader.GroupOrder",
        "GroupOrder",
    ] {
        push_cloneable_final(&mut facts, type_name);
    }

    for (type_name, arg_count) in [
        ("htsjdk.samtools.SAMSequenceDictionary", 0usize),
        ("htsjdk.samtools.SAMSequenceDictionary", 1usize),
        ("SAMSequenceDictionary", 0usize),
        ("SAMSequenceDictionary", 1usize),
        ("htsjdk.samtools.SAMSequenceRecord", 2usize),
        ("SAMSequenceRecord", 2usize),
        ("htsjdk.samtools.SAMFileHeader", 0usize),
        ("htsjdk.samtools.SAMFileHeader", 1usize),
        ("SAMFileHeader", 0usize),
        ("SAMFileHeader", 1usize),
        ("htsjdk.samtools.SAMReadGroupRecord", 1usize),
        ("SAMReadGroupRecord", 1usize),
    ] {
        facts
            .external_constructor_facts
            .push(ExternalConstructorFact {
                type_name: type_name.to_string(),
                arg_types: vec![JavaType::Unknown("_".to_string()); arg_count],
                provenance: FactProvenance::ExternalLibraryPack,
            });
    }

    for method in [
        "getSequence",
        "getSequenceIndex",
        "getSequences",
        "size",
        "isEmpty",
    ] {
        push_effect(
            &mut facts,
            "htsjdk.samtools.SAMSequenceDictionary",
            method,
            if method == "getSequence" {
                ExternalApiRole::IndexedGetter
            } else {
                ExternalApiRole::Getter
            },
            NullabilityState::DefinitelyNonNull,
        );
        push_effect(
            &mut facts,
            "SAMSequenceDictionary",
            method,
            if method == "getSequence" {
                ExternalApiRole::IndexedGetter
            } else {
                ExternalApiRole::Getter
            },
            NullabilityState::DefinitelyNonNull,
        );
    }

    for method in [
        "getSequenceName",
        "getSequenceLength",
        "getSequenceIndex",
        "getAttribute",
        "getAttributes",
    ] {
        push_effect(
            &mut facts,
            "htsjdk.samtools.SAMSequenceRecord",
            method,
            ExternalApiRole::Getter,
            if method == "getAttribute" {
                NullabilityState::MaybeNull
            } else {
                NullabilityState::DefinitelyNonNull
            },
        );
        push_effect(
            &mut facts,
            "SAMSequenceRecord",
            method,
            ExternalApiRole::Getter,
            if method == "getAttribute" {
                NullabilityState::MaybeNull
            } else {
                NullabilityState::DefinitelyNonNull
            },
        );
    }

    for method in [
        "getSequenceDictionary",
        "getSortOrder",
        "getGroupOrder",
        "getReadGroups",
        "getProgramRecords",
    ] {
        push_effect(
            &mut facts,
            "htsjdk.samtools.SAMFileHeader",
            method,
            ExternalApiRole::Getter,
            NullabilityState::DefinitelyNonNull,
        );
        push_effect(
            &mut facts,
            "SAMFileHeader",
            method,
            ExternalApiRole::Getter,
            NullabilityState::DefinitelyNonNull,
        );
    }
    for method in [
        "setSequenceDictionary",
        "setSortOrder",
        "setGroupOrder",
        "addReadGroup",
        "addProgramRecord",
    ] {
        push_effect(
            &mut facts,
            "htsjdk.samtools.SAMFileHeader",
            method,
            ExternalApiRole::Mutator,
            NullabilityState::Unknown,
        );
        push_effect(
            &mut facts,
            "SAMFileHeader",
            method,
            ExternalApiRole::Mutator,
            NullabilityState::Unknown,
        );
    }

    for method in ["getId", "getSample", "getLibrary", "getPlatform"] {
        push_effect(
            &mut facts,
            "htsjdk.samtools.SAMReadGroupRecord",
            method,
            ExternalApiRole::Getter,
            if method == "getId" {
                NullabilityState::DefinitelyNonNull
            } else {
                NullabilityState::MaybeNull
            },
        );
        push_effect(
            &mut facts,
            "SAMReadGroupRecord",
            method,
            ExternalApiRole::Getter,
            if method == "getId" {
                NullabilityState::DefinitelyNonNull
            } else {
                NullabilityState::MaybeNull
            },
        );
    }
    for method in ["setSample", "setLibrary", "setPlatform"] {
        push_effect(
            &mut facts,
            "htsjdk.samtools.SAMReadGroupRecord",
            method,
            ExternalApiRole::Setter,
            NullabilityState::Unknown,
        );
        push_effect(
            &mut facts,
            "SAMReadGroupRecord",
            method,
            ExternalApiRole::Setter,
            NullabilityState::Unknown,
        );
    }

    for (type_name, field_name) in [
        (
            "htsjdk.samtools.SAMSequenceRecord",
            "UNKNOWN_SEQUENCE_LENGTH",
        ),
        ("SAMSequenceRecord", "UNKNOWN_SEQUENCE_LENGTH"),
        ("htsjdk.samtools.SAMFileHeader", "SortOrder"),
        ("SAMFileHeader", "SortOrder"),
        ("htsjdk.samtools.SAMFileHeader", "GroupOrder"),
        ("SAMFileHeader", "GroupOrder"),
    ] {
        facts.external_field_facts.push(ExternalFieldFact {
            field: FieldKey {
                type_name: type_name.to_string(),
                field_name: field_name.to_string(),
            },
            provenance: FactProvenance::ExternalLibraryPack,
        });
    }

    facts
}

pub fn htsjdk_vcf_pack_facts() -> FactSet {
    let mut facts = FactSet::default();
    manifest_by_name(VCF_PACK).apply_boundary_facts(&mut facts);

    for type_name in [
        "htsjdk.variant.vcf.VCFHeader",
        "VCFHeader",
        "htsjdk.variant.vcf.VCFHeaderLine",
        "VCFHeaderLine",
        "htsjdk.variant.vcf.VCFInfoHeaderLine",
        "VCFInfoHeaderLine",
        "htsjdk.variant.vcf.VCFFormatHeaderLine",
        "VCFFormatHeaderLine",
        "htsjdk.variant.vcf.VCFFilterHeaderLine",
        "VCFFilterHeaderLine",
        "htsjdk.variant.vcf.VCFHeaderLineType",
        "VCFHeaderLineType",
        "htsjdk.variant.vcf.VCFHeaderLineCount",
        "VCFHeaderLineCount",
        "htsjdk.variant.vcf.VCFHeaderVersion",
        "VCFHeaderVersion",
        "htsjdk.variant.vcf.VCFConstants",
        "VCFConstants",
    ] {
        facts.hierarchy_overrides.push(HierarchyOverride {
            type_name: type_name.to_string(),
            strategy: HierarchyStrategy::ErasedObject,
            provenance: FactProvenance::ExternalLibraryPack,
        });
    }

    for type_name in [
        "htsjdk.variant.vcf.VCFHeaderLine",
        "VCFHeaderLine",
        "htsjdk.variant.vcf.VCFInfoHeaderLine",
        "VCFInfoHeaderLine",
        "htsjdk.variant.vcf.VCFFormatHeaderLine",
        "VCFFormatHeaderLine",
        "htsjdk.variant.vcf.VCFFilterHeaderLine",
        "VCFFilterHeaderLine",
        "htsjdk.variant.vcf.VCFHeaderLineType",
        "VCFHeaderLineType",
        "htsjdk.variant.vcf.VCFHeaderLineCount",
        "VCFHeaderLineCount",
        "htsjdk.variant.vcf.VCFHeaderVersion",
        "VCFHeaderVersion",
    ] {
        push_cloneable_final(&mut facts, type_name);
    }

    for (type_name, arg_count) in [
        ("htsjdk.variant.vcf.VCFInfoHeaderLine", 3usize),
        ("htsjdk.variant.vcf.VCFInfoHeaderLine", 4usize),
        ("VCFInfoHeaderLine", 3usize),
        ("VCFInfoHeaderLine", 4usize),
        ("htsjdk.variant.vcf.VCFFormatHeaderLine", 3usize),
        ("htsjdk.variant.vcf.VCFFormatHeaderLine", 4usize),
        ("VCFFormatHeaderLine", 3usize),
        ("VCFFormatHeaderLine", 4usize),
        ("htsjdk.variant.vcf.VCFFilterHeaderLine", 2usize),
        ("VCFFilterHeaderLine", 2usize),
        ("htsjdk.variant.vcf.VCFHeaderLine", 2usize),
        ("VCFHeaderLine", 2usize),
        ("htsjdk.variant.vcf.VCFHeader", 0usize),
        ("htsjdk.variant.vcf.VCFHeader", 1usize),
        ("htsjdk.variant.vcf.VCFHeader", 2usize),
        ("VCFHeader", 0usize),
        ("VCFHeader", 1usize),
        ("VCFHeader", 2usize),
        ("htsjdk.variant.vcf.VCFCodec", 0usize),
        ("VCFCodec", 0usize),
        ("htsjdk.variant.vcf.VCFRecordCodec", 2usize),
        ("VCFRecordCodec", 2usize),
    ] {
        facts
            .external_constructor_facts
            .push(ExternalConstructorFact {
                type_name: type_name.to_string(),
                arg_types: vec![JavaType::Unknown("_".to_string()); arg_count],
                provenance: FactProvenance::ExternalLibraryPack,
            });
    }

    for method in [
        "getGenotypeSamples",
        "getMetaDataInInputOrder",
        "getInfoHeaderLines",
        "getFormatHeaderLines",
        "getFilterLines",
        "getContigLines",
        "getSequenceDictionary",
        "getVCFHeaderVersion",
        "getColumnCount",
    ] {
        push_effect(
            &mut facts,
            "htsjdk.variant.vcf.VCFHeader",
            method,
            ExternalApiRole::Getter,
            NullabilityState::DefinitelyNonNull,
        );
        push_effect(
            &mut facts,
            "VCFHeader",
            method,
            ExternalApiRole::Getter,
            NullabilityState::DefinitelyNonNull,
        );
    }
    push_effect(
        &mut facts,
        "htsjdk.variant.vcf.VCFHeader",
        "addMetaDataLine",
        ExternalApiRole::Mutator,
        NullabilityState::Unknown,
    );
    push_effect(
        &mut facts,
        "VCFHeader",
        "addMetaDataLine",
        ExternalApiRole::Mutator,
        NullabilityState::Unknown,
    );

    for method in ["getKey", "getValue", "getID", "getDescription", "toString"] {
        for receiver in [
            "htsjdk.variant.vcf.VCFHeaderLine",
            "VCFHeaderLine",
            "htsjdk.variant.vcf.VCFInfoHeaderLine",
            "VCFInfoHeaderLine",
            "htsjdk.variant.vcf.VCFFormatHeaderLine",
            "VCFFormatHeaderLine",
            "htsjdk.variant.vcf.VCFFilterHeaderLine",
            "VCFFilterHeaderLine",
        ] {
            push_effect(
                &mut facts,
                receiver,
                method,
                ExternalApiRole::Getter,
                NullabilityState::DefinitelyNonNull,
            );
        }
    }

    for receiver in [
        "htsjdk.variant.vcf.VCFInfoHeaderLine",
        "VCFInfoHeaderLine",
        "htsjdk.variant.vcf.VCFFormatHeaderLine",
        "VCFFormatHeaderLine",
    ] {
        for method in ["getCount", "getCountType", "getType"] {
            push_effect(
                &mut facts,
                receiver,
                method,
                ExternalApiRole::Getter,
                NullabilityState::DefinitelyNonNull,
            );
        }
    }

    facts
}

pub fn htsjdk_variantcontext_pack_facts() -> FactSet {
    let mut facts = FactSet::default();
    manifest_by_name(VARIANTCONTEXT_PACK).apply_boundary_facts(&mut facts);

    for type_name in [
        "htsjdk.variant.variantcontext.Allele",
        "Allele",
        "htsjdk.variant.variantcontext.VariantContext",
        "VariantContext",
        "htsjdk.variant.variantcontext.VariantContextBuilder",
        "VariantContextBuilder",
        "htsjdk.variant.variantcontext.Genotype",
        "Genotype",
        "htsjdk.variant.variantcontext.GenotypeBuilder",
        "GenotypeBuilder",
        "htsjdk.variant.variantcontext.GenotypesContext",
        "GenotypesContext",
        "htsjdk.variant.variantcontext.VariantContext.Type",
        "VariantContext.Type",
        "htsjdk.variant.variantcontext.GenotypeType",
        "GenotypeType",
    ] {
        facts.hierarchy_overrides.push(HierarchyOverride {
            type_name: type_name.to_string(),
            strategy: HierarchyStrategy::ErasedObject,
            provenance: FactProvenance::ExternalLibraryPack,
        });
    }

    push_cloneable_final(&mut facts, "htsjdk.variant.variantcontext.Allele");
    push_cloneable_final(&mut facts, "Allele");
    push_cloneable_final(&mut facts, "htsjdk.variant.variantcontext.VariantContext");
    push_cloneable_final(&mut facts, "VariantContext");
    push_cloneable_final(&mut facts, "htsjdk.variant.variantcontext.Genotype");
    push_cloneable_final(&mut facts, "Genotype");
    push_sequence_like(
        &mut facts,
        "htsjdk.variant.variantcontext.GenotypesContext",
        named("htsjdk.variant.variantcontext.Genotype"),
    );
    push_sequence_like(&mut facts, "GenotypesContext", named("Genotype"));

    for (receiver, method, arg_count) in [
        ("htsjdk.variant.variantcontext.Allele", "create", 1usize),
        ("htsjdk.variant.variantcontext.Allele", "create", 2usize),
        (
            "htsjdk.variant.variantcontext.VariantContextBuilder",
            "<init>",
            0usize,
        ),
        (
            "htsjdk.variant.variantcontext.VariantContextBuilder",
            "<init>",
            1usize,
        ),
        (
            "htsjdk.variant.variantcontext.VariantContextBuilder",
            "<init>",
            5usize,
        ),
        (
            "htsjdk.variant.variantcontext.GenotypeBuilder",
            "<init>",
            0usize,
        ),
        (
            "htsjdk.variant.variantcontext.GenotypeBuilder",
            "<init>",
            1usize,
        ),
        (
            "htsjdk.variant.variantcontext.GenotypeBuilder",
            "<init>",
            2usize,
        ),
        ("VariantContextBuilder", "<init>", 0usize),
        ("VariantContextBuilder", "<init>", 1usize),
        ("VariantContextBuilder", "<init>", 5usize),
        ("GenotypeBuilder", "<init>", 0usize),
        ("GenotypeBuilder", "<init>", 1usize),
        ("GenotypeBuilder", "<init>", 2usize),
    ] {
        if method == "<init>" {
            facts
                .external_constructor_facts
                .push(ExternalConstructorFact {
                    type_name: receiver.to_string(),
                    arg_types: vec![JavaType::Unknown("_".to_string()); arg_count],
                    provenance: FactProvenance::ExternalLibraryPack,
                });
        }
    }

    for method in [
        "getBaseString",
        "getBases",
        "isReference",
        "isNoCall",
        "isNonReference",
        "length",
    ] {
        let return_type = variantcontext_allele_return_type(method);
        push_effect_with_return_type(
            &mut facts,
            "htsjdk.variant.variantcontext.Allele",
            method,
            if method.starts_with("is") {
                ExternalApiRole::BooleanGetter
            } else {
                ExternalApiRole::Getter
            },
            return_type.clone(),
            NullabilityState::DefinitelyNonNull,
        );
        push_effect_with_return_type(
            &mut facts,
            "Allele",
            method,
            if method.starts_with("is") {
                ExternalApiRole::BooleanGetter
            } else {
                ExternalApiRole::Getter
            },
            return_type,
            NullabilityState::DefinitelyNonNull,
        );
    }
    push_effect_with_return_type(
        &mut facts,
        "htsjdk.variant.variantcontext.Allele",
        "create",
        ExternalApiRole::Getter,
        Some(named_type("Allele")),
        NullabilityState::DefinitelyNonNull,
    );
    push_effect_with_return_type(
        &mut facts,
        "Allele",
        "create",
        ExternalApiRole::Getter,
        Some(named_type("Allele")),
        NullabilityState::DefinitelyNonNull,
    );

    for method in [
        "getContig",
        "getStart",
        "getEnd",
        "getAlleles",
        "getAlternateAlleles",
        "getReference",
        "getGenotypes",
        "getAttributes",
        "getType",
        "hasAttribute",
        "isBiallelic",
        "isSNP",
        "isIndel",
    ] {
        let return_type = variantcontext_return_type(method);
        push_effect_with_return_type(
            &mut facts,
            "htsjdk.variant.variantcontext.VariantContext",
            method,
            if method.starts_with("has") || method.starts_with("is") {
                ExternalApiRole::BooleanGetter
            } else {
                ExternalApiRole::Getter
            },
            return_type.clone(),
            NullabilityState::DefinitelyNonNull,
        );
        push_effect_with_return_type(
            &mut facts,
            "VariantContext",
            method,
            if method.starts_with("has") || method.starts_with("is") {
                ExternalApiRole::BooleanGetter
            } else {
                ExternalApiRole::Getter
            },
            return_type,
            NullabilityState::DefinitelyNonNull,
        );
    }
    for method in ["getAttribute", "getAttributeAsString"] {
        let return_type = if method == "getAttributeAsString" {
            Some(JavaType::String)
        } else {
            Some(named_type("Object"))
        };
        push_effect_with_return_type(
            &mut facts,
            "htsjdk.variant.variantcontext.VariantContext",
            method,
            ExternalApiRole::Getter,
            return_type.clone(),
            NullabilityState::MaybeNull,
        );
        push_effect_with_return_type(
            &mut facts,
            "VariantContext",
            method,
            ExternalApiRole::Getter,
            return_type,
            NullabilityState::MaybeNull,
        );
    }

    for method in [
        "chr",
        "start",
        "stop",
        "alleles",
        "genotypes",
        "attribute",
        "attributes",
        "id",
        "log10PError",
        "filters",
    ] {
        push_effect_with_return_type(
            &mut facts,
            "htsjdk.variant.variantcontext.VariantContextBuilder",
            method,
            ExternalApiRole::FluentMutator,
            Some(named_type("VariantContextBuilder")),
            NullabilityState::DefinitelyNonNull,
        );
        push_effect_with_return_type(
            &mut facts,
            "VariantContextBuilder",
            method,
            ExternalApiRole::FluentMutator,
            Some(named_type("VariantContextBuilder")),
            NullabilityState::DefinitelyNonNull,
        );
    }
    push_effect_with_return_type(
        &mut facts,
        "htsjdk.variant.variantcontext.VariantContextBuilder",
        "make",
        ExternalApiRole::Getter,
        Some(named_type("VariantContext")),
        NullabilityState::DefinitelyNonNull,
    );
    push_effect_with_return_type(
        &mut facts,
        "VariantContextBuilder",
        "make",
        ExternalApiRole::Getter,
        Some(named_type("VariantContext")),
        NullabilityState::DefinitelyNonNull,
    );

    for method in [
        "name",
        "alleles",
        "attribute",
        "attributes",
        "DP",
        "GQ",
        "PL",
        "AD",
        "noAD",
        "noPL",
    ] {
        push_effect_with_return_type(
            &mut facts,
            "htsjdk.variant.variantcontext.GenotypeBuilder",
            method,
            ExternalApiRole::FluentMutator,
            Some(named_type("GenotypeBuilder")),
            NullabilityState::DefinitelyNonNull,
        );
        push_effect_with_return_type(
            &mut facts,
            "GenotypeBuilder",
            method,
            ExternalApiRole::FluentMutator,
            Some(named_type("GenotypeBuilder")),
            NullabilityState::DefinitelyNonNull,
        );
    }
    push_effect_with_return_type(
        &mut facts,
        "htsjdk.variant.variantcontext.GenotypeBuilder",
        "make",
        ExternalApiRole::Getter,
        Some(named_type("Genotype")),
        NullabilityState::DefinitelyNonNull,
    );
    push_effect_with_return_type(
        &mut facts,
        "GenotypeBuilder",
        "make",
        ExternalApiRole::Getter,
        Some(named_type("Genotype")),
        NullabilityState::DefinitelyNonNull,
    );

    for method in [
        "getSampleName",
        "getAlleles",
        "getExtendedAttributes",
        "hasAD",
        "hasPL",
        "hasGQ",
        "isCalled",
        "isNoCall",
    ] {
        push_effect(
            &mut facts,
            "htsjdk.variant.variantcontext.Genotype",
            method,
            if method.starts_with("has") || method.starts_with("is") {
                ExternalApiRole::BooleanGetter
            } else {
                ExternalApiRole::Getter
            },
            NullabilityState::DefinitelyNonNull,
        );
        push_effect(
            &mut facts,
            "Genotype",
            method,
            if method.starts_with("has") || method.starts_with("is") {
                ExternalApiRole::BooleanGetter
            } else {
                ExternalApiRole::Getter
            },
            NullabilityState::DefinitelyNonNull,
        );
    }

    for method in ["size", "iterator", "stream", "get", "isEmpty"] {
        push_effect(
            &mut facts,
            "htsjdk.variant.variantcontext.GenotypesContext",
            method,
            if method == "get" {
                ExternalApiRole::IndexedGetter
            } else {
                ExternalApiRole::Getter
            },
            NullabilityState::DefinitelyNonNull,
        );
        push_effect(
            &mut facts,
            "GenotypesContext",
            method,
            if method == "get" {
                ExternalApiRole::IndexedGetter
            } else {
                ExternalApiRole::Getter
            },
            NullabilityState::DefinitelyNonNull,
        );
    }

    facts.exception_facts.push(ExceptionFact {
        type_name: "htsjdk.variant.variantcontext.VariantContextException".to_string(),
        kind: ExceptionKind::Unchecked,
        provenance: FactProvenance::ExternalLibraryPack,
    });
    facts
}

fn push_effect(
    facts: &mut FactSet,
    receiver_type: &str,
    method_name: &str,
    role: ExternalApiRole,
    return_nullability: NullabilityState,
) {
    push_effect_with_return_type(
        facts,
        receiver_type,
        method_name,
        role,
        None,
        return_nullability,
    );
}

fn push_effect_with_return_type(
    facts: &mut FactSet,
    receiver_type: &str,
    method_name: &str,
    role: ExternalApiRole,
    return_type: Option<JavaType>,
    return_nullability: NullabilityState,
) {
    facts.external_api_effect_facts.push(ExternalApiEffectFact {
        method_name: method_name.to_string(),
        receiver_type: Some(receiver_type.to_string()),
        role,
        receiver_mutability: match role {
            ExternalApiRole::Setter
            | ExternalApiRole::Mutator
            | ExternalApiRole::FluentMutator
            | ExternalApiRole::IndexedSetter => ReceiverMutabilityFact::MutatesReceiver,
            ExternalApiRole::Unknown => ReceiverMutabilityFact::Unknown,
            ExternalApiRole::Getter
            | ExternalApiRole::BooleanGetter
            | ExternalApiRole::SideEffect
            | ExternalApiRole::IndexedGetter => ReceiverMutabilityFact::ReadOnly,
        },
        return_type,
        return_nullability,
        provenance: FactProvenance::ExternalLibraryPack,
    });
}

fn named_type(name: &str) -> JavaType {
    JavaType::Named {
        name: name.to_string(),
        args: Vec::new(),
    }
}

fn list_type(element: JavaType) -> JavaType {
    JavaType::Named {
        name: "List".to_string(),
        args: vec![element],
    }
}

fn map_type(key: JavaType, value: JavaType) -> JavaType {
    JavaType::Named {
        name: "Map".to_string(),
        args: vec![key, value],
    }
}

fn variantcontext_allele_return_type(method: &str) -> Option<JavaType> {
    match method {
        "getBaseString" => Some(JavaType::String),
        "getBases" => Some(JavaType::Array(Box::new(JavaType::Primitive(
            crate::java_ir::PrimitiveType::Byte,
        )))),
        "isReference" | "isNoCall" | "isNonReference" => {
            Some(JavaType::Primitive(crate::java_ir::PrimitiveType::Boolean))
        }
        "length" => Some(JavaType::Primitive(crate::java_ir::PrimitiveType::Int)),
        _ => None,
    }
}

fn variantcontext_return_type(method: &str) -> Option<JavaType> {
    match method {
        "getContig" | "getType" => Some(JavaType::String),
        "getStart" | "getEnd" => Some(JavaType::Primitive(crate::java_ir::PrimitiveType::Int)),
        "getAlleles" | "getAlternateAlleles" => Some(list_type(named_type("Allele"))),
        "getReference" => Some(named_type("Allele")),
        "getGenotypes" => Some(named_type("GenotypesContext")),
        "getAttributes" => Some(map_type(JavaType::String, named_type("Object"))),
        "hasAttribute" | "isBiallelic" | "isSNP" | "isIndel" => {
            Some(JavaType::Primitive(crate::java_ir::PrimitiveType::Boolean))
        }
        _ => None,
    }
}

fn push_cloneable_final(facts: &mut FactSet, type_name: &str) {
    let mut override_fact = empty_capability(type_name);
    override_fact.clone = Some(true);
    override_fact.final_type = Some(true);
    facts.type_capability_overrides.push(override_fact);
}

fn push_sequence_like(facts: &mut FactSet, type_name: &str, element: JavaType) {
    let mut override_fact = empty_capability(type_name);
    override_fact.sequence_like = Some(element);
    override_fact.final_type = Some(true);
    facts.type_capability_overrides.push(override_fact);
}

fn empty_capability(type_name: &str) -> TypeCapabilityOverride {
    TypeCapabilityOverride {
        type_name: type_name.to_string(),
        copy: None,
        clone: None,
        string_like: None,
        sequence_like: None,
        implements: Vec::new(),
        extends: None,
        final_type: None,
        object_safe_trait: None,
        provenance: FactProvenance::ExternalLibraryPack,
    }
}

fn named(name: &str) -> JavaType {
    JavaType::Named {
        name: name.to_string(),
        args: Vec::new(),
    }
}
