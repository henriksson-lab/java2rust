use std::collections::HashMap;
use std::sync::OnceLock;

use crate::diagnostics::DiagnosticBag;
use crate::facts::{
    ClosedHierarchyFact, ExceptionFact, ExceptionKind, ExternalApiArgumentEffectFact,
    ExternalApiCoverage, ExternalApiEffectFact, ExternalApiRole, ExternalArgumentAliasPolicy,
    ExternalConstructorFact, ExternalCrateApiMappingFact, ExternalCrateConstructorMappingFact,
    ExternalFieldFact, ExternalLibraryMode, ExternalLibraryPackFact, FactProvenance, FactSet,
    FieldKey, FieldNullabilityFact, FieldNullabilityProvenance, HierarchyOverride,
    HierarchyStrategy, NullabilityState, ObjectSafetyFact, ObjectSafetyViolation,
    OwnedRecoveryCapability, OwnedRecoveryCapabilityFact, ReceiverMutabilityFact,
    TypeCapabilityOverride, UnsupportedExternalApiFact, UnsupportedExternalConstructorFact,
    UnsupportedExternalFieldFact,
};
use crate::java_ir::{JavaType, PrimitiveType};
use crate::parser::parse_type;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalPackManifestParse {
    pub facts: FactSet,
    pub diagnostics: DiagnosticBag,
}

const VARIANTCONTEXT_PACK: &str = "htsjdk.variant.variantcontext";
const VCF_PACK: &str = "htsjdk.variant.vcf";
const BCF_PACK: &str = "htsjdk.variant.bcf2";
const SAM_HEADER_PACK: &str = "htsjdk.samtools.header";
const SAM_RECORD_PACK: &str = "htsjdk.samtools.record";
const SAM_REFERENCE_PACK: &str = "htsjdk.samtools.reference";
const SAM_READER_PACK: &str = "htsjdk.samtools.reader";
const SAM_WRITER_PACK: &str = "htsjdk.samtools.writer";
const SAM_BAM_PACK: &str = "htsjdk.samtools.bam";
const SAM_CRAM_PACK: &str = "htsjdk.samtools.cram";
const SAM_METRICS_PACK: &str = "htsjdk.samtools.metrics";
const HTSJDK_NESTED_HELPER_PACK: &str = "htsjdk.nested_helper";
const TRIBBLE_FEATURE_PACK: &str = "htsjdk.tribble.feature";
const TRIBBLE_INDEX_PACK: &str = "htsjdk.tribble.index";
const SAMTOOLS_UTIL_PACK: &str = "htsjdk.samtools.util";
const HDF5_PACK: &str = "hdf5";
const SQLITE_PACK: &str = "sqlite";
const JAVA_SQL_PACK: &str = "java.sql";
const APACHE_COMMONS_MATH_PACK: &str = "org.apache.commons.math3";
const APACHE_COMMONS_MATH_OPTIM_PACK: &str = "org.apache.commons.math3.optim.univariate";
const APACHE_COMMONS_MATH_STATS_PACK: &str = "org.apache.commons.math3.stat.descriptive.rank";
const APACHE_COMMONS_COLLECTIONS_PACK: &str = "org.apache.commons.collections4";
const APACHE_COMMONS_COMPRESS_PACK: &str = "org.apache.commons.compress";
const APACHE_COMMONS_LANG_PACK: &str = "org.apache.commons.lang3";
const APACHE_COMMONS_CODEC_PACK: &str = "org.apache.commons.codec";
const JAMA_PACK: &str = "Jama";
const HIPPARCHUS_DISTRIBUTION_PACK: &str = "org.hipparchus.random";
const APACHE_HTTP_PACK: &str = "org.apache.http";
const GOOGLE_BIGQUERY_PACK: &str = "com.google.cloud.bigquery";
const GOOGLE_BIGQUERY_STORAGE_PACK: &str = "com.google.cloud.bigquery.storage.v1";
const GOOGLE_STORAGE_PACK: &str = "com.google.cloud.storage";
const GOOGLE_CLOUD_NIO_PACK: &str = "com.google.cloud.storage.contrib.nio";
const GENOMICSDB_PACK: &str = "com.intel.genomicsdb";
const PICARD_PROGRAMGROUPS_PACK: &str = "picard.cmdline.programgroups";
const PICARD_MARKDUPLICATES_PACK: &str = "picard.sam.markduplicates";
const PICARD_METRICS_PACK: &str = "picard.analysis.metrics";
const AVRO_PACK: &str = "org.apache.avro";
const BARCLAY_HELP_PACK: &str = "org.broadinstitute.barclay.help";
const BARCLAY_ARGPARSER_PACK: &str = "org.broadinstitute.barclay.argparser";
const BARCLAY_UTILS_PACK: &str = "org.broadinstitute.barclay.utils";
const SCALA_TUPLE_PACK: &str = "scala";
const KRYO_PACK: &str = "com.esotericsoftware.kryo";
const OPENCSV_PACK: &str = "com.opencsv";
const ORG_JSON_PACK: &str = "org.json";
const REFLECTIONS_PACK: &str = "org.reflections";
const HADOOP_CONF_PACK: &str = "org.apache.hadoop.conf";
const APACHE_COMMONS_CONFIG_PACK: &str = "org.apache.commons.configuration2";
const SPARK_PACK: &str = "org.apache.spark";
const BWA_PACK: &str = "org.broadinstitute.hellbender.utils.bwa";
const JGRAPHT_PACK: &str = "org.jgrapht";
const FASTUTIL_PACK: &str = "it.unimi.dsi.fastutil";
const KRYO_SERIALIZERS_PACK: &str = "de.javakaffee.kryoserializers";
const DISQ_WRITE_PACK: &str = "org.disq_bio.disq";
const INTEL_GKL_PACK: &str = "com.intel.gkl";
const GUAVA_IMMUTABLE_COLLECT_PACK: &str = "com.google.common.collect";
const GUAVA_CONCURRENT_PACK: &str = "com.google.common.util.concurrent";
const BROAD_HELLBENDER_UTILS_PACK: &str = "org.broadinstitute.hellbender.utils";
const BROAD_HTTP_PACK: &str = "org.broadinstitute.http.nio";
const PICARD_VCF_PACK: &str = "picard.vcf";
const JACKSON_DATABIND_PACK: &str = "com.fasterxml.jackson.databind";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalPackManifest {
    pub pack_name: &'static str,
    pub owned_type_patterns: &'static [&'static str],
    pub mode: ExternalLibraryMode,
    pub default_coverage: ExternalApiCoverage,
    pub rust_target: Option<&'static str>,
    pub object_strategies: &'static [ExternalPackObjectStrategy],
    pub closed_hierarchy: &'static [ExternalPackClosedHierarchy],
    pub object_safety: &'static [ExternalPackObjectSafety],
    pub type_capabilities: &'static [ExternalPackTypeCapability],
    pub field_nullability: &'static [ExternalPackFieldNullability],
    pub argument_effects: &'static [ExternalPackArgumentEffect],
    pub exceptions: &'static [ExternalPackException],
    pub api_effects: &'static [ExternalPackApiEffect],
    pub modeled_apis: &'static [ExternalPackModeledApi],
    pub modeled_constructors: &'static [ExternalPackModeledConstructor],
    pub modeled_fields: &'static [ExternalPackModeledField],
    pub unsupported_apis: &'static [ExternalPackUnsupportedApi],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExternalPackObjectStrategy {
    pub type_name: &'static str,
    pub strategy: ExternalPackObjectStrategyKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExternalPackClosedHierarchy {
    pub type_name: &'static str,
    pub closed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExternalPackObjectSafety {
    pub type_name: &'static str,
    pub violation: ExternalPackObjectSafetyViolation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternalPackObjectSafetyViolation {
    StaticMethod {
        method_name: &'static str,
    },
    SelfReferentialReturn {
        method_name: &'static str,
    },
    NestedSelfReferentialReturn {
        method_name: &'static str,
    },
    SelfReferentialParameter {
        method_name: &'static str,
        param_name: &'static str,
    },
    NestedSelfReferentialParameter {
        method_name: &'static str,
        param_name: &'static str,
    },
    FieldMember {
        field_name: &'static str,
    },
    ConstructorMember,
    UnsupportedMember,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternalPackObjectStrategyKind {
    ConcreteStruct,
    ClosedEnum {
        enum_name: &'static str,
        variants: &'static [&'static str],
    },
    DynTrait {
        trait_name: &'static str,
    },
    ErasedObject,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExternalPackTypeCapability {
    pub type_name: &'static str,
    pub copy: Option<bool>,
    pub clone: Option<bool>,
    pub string_like: Option<bool>,
    pub sequence_like: Option<&'static str>,
    pub implements: &'static [&'static str],
    pub extends: Option<&'static str>,
    pub final_type: Option<bool>,
    pub object_safe_trait: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExternalPackFieldNullability {
    pub type_name: &'static str,
    pub field_name: &'static str,
    pub state: NullabilityState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExternalPackArgumentEffect {
    pub receiver_type: &'static str,
    pub method_name: &'static str,
    pub parameter_index: usize,
    pub parameter_type: Option<&'static str>,
    pub parameter_nullability: NullabilityState,
    pub argument_mutability: ReceiverMutabilityFact,
    pub alias_policy: ExternalArgumentAliasPolicy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExternalPackException {
    pub type_name: &'static str,
    pub kind: ExceptionKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExternalPackApiEffect {
    pub receiver_type: &'static str,
    pub method_name: &'static str,
    pub arg_count: Option<usize>,
    pub rust_name: Option<&'static str>,
    pub role: ExternalApiRole,
    pub receiver_mutability: ReceiverMutabilityFact,
    pub return_type: Option<&'static str>,
    pub return_nullability: NullabilityState,
    pub thrown: &'static [&'static str],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExternalPackModeledApi {
    pub receiver_type: &'static str,
    pub method_name: &'static str,
    pub coverage: ExternalApiCoverage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExternalPackModeledConstructor {
    pub type_name: &'static str,
    pub arg_count: usize,
    pub rust_type_name: Option<&'static str>,
    pub rust_name: Option<&'static str>,
    pub coverage: ExternalApiCoverage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExternalPackModeledField {
    pub type_name: &'static str,
    pub field_name: &'static str,
    pub coverage: ExternalApiCoverage,
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

    pub fn modeled_api(
        &self,
        receiver_type: &str,
        method_name: &str,
    ) -> Option<&ExternalPackModeledApi> {
        self.modeled_apis
            .iter()
            .find(|api| api.receiver_type == receiver_type && api.method_name == method_name)
    }

    pub fn coverage_for_api(&self, receiver_type: &str, method_name: &str) -> ExternalApiCoverage {
        if self.unsupported_api(receiver_type, method_name).is_some() {
            ExternalApiCoverage::Unsupported
        } else if let Some(api) = self.modeled_api(receiver_type, method_name) {
            api.coverage
        } else {
            self.default_coverage
        }
    }

    pub fn modeled_constructor(
        &self,
        type_name: &str,
        arg_count: usize,
    ) -> Option<&ExternalPackModeledConstructor> {
        self.modeled_constructors
            .iter()
            .find(|ctor| ctor.type_name == type_name && ctor.arg_count == arg_count)
    }

    pub fn coverage_for_constructor(
        &self,
        type_name: &str,
        arg_count: usize,
    ) -> ExternalApiCoverage {
        self.modeled_constructor(type_name, arg_count)
            .map(|constructor| constructor.coverage)
            .unwrap_or(self.default_coverage)
    }

    pub fn modeled_field(
        &self,
        type_name: &str,
        field_name: &str,
    ) -> Option<&ExternalPackModeledField> {
        self.modeled_fields
            .iter()
            .find(|field| field.type_name == type_name && field.field_name == field_name)
    }

    pub fn coverage_for_field(&self, type_name: &str, field_name: &str) -> ExternalApiCoverage {
        self.modeled_field(type_name, field_name)
            .map(|field| field.coverage)
            .unwrap_or(self.default_coverage)
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
        for strategy in self.object_strategies {
            facts.hierarchy_overrides.push(HierarchyOverride {
                type_name: strategy.type_name.to_string(),
                strategy: strategy.kind_as_hierarchy_strategy(),
                provenance: FactProvenance::ExternalLibraryPack,
            });
        }
        for closed in self.closed_hierarchy {
            facts.closed_hierarchy_facts.push(ClosedHierarchyFact {
                type_name: closed.type_name.to_string(),
                closed: closed.closed,
                provenance: FactProvenance::ExternalLibraryPack,
            });
        }
        for object_safety in self.object_safety {
            object_safety.push_facts(facts);
        }
        for capability in self.type_capabilities {
            facts
                .type_capability_overrides
                .push(capability.as_type_capability_override());
        }
        for field in self.field_nullability {
            facts
                .field_nullability_facts
                .push(field.as_field_nullability_fact());
        }
        for argument in self.argument_effects {
            facts
                .external_api_argument_effect_facts
                .push(argument.as_external_api_argument_effect_fact());
        }
        for exception in self.exceptions {
            facts.exception_facts.push(exception.as_exception_fact());
        }
        for effect in self.api_effects {
            facts
                .external_api_effect_facts
                .push(effect.as_external_api_effect_fact());
            if self.mode == ExternalLibraryMode::RustCrateMapping {
                facts
                    .external_crate_api_mapping_facts
                    .push(effect.as_external_crate_api_mapping_fact());
            }
        }
        for constructor in self.modeled_constructors {
            if constructor.coverage == ExternalApiCoverage::Unsupported {
                facts
                    .unsupported_external_constructor_facts
                    .push(constructor.as_unsupported_external_constructor_fact(self.pack_name));
                continue;
            }
            facts
                .external_constructor_facts
                .push(ExternalConstructorFact {
                    type_name: constructor.type_name.to_string(),
                    arg_types: vec![JavaType::Unknown("_".to_string()); constructor.arg_count],
                    provenance: FactProvenance::ExternalLibraryPack,
                });
            if self.mode == ExternalLibraryMode::RustCrateMapping {
                facts
                    .external_crate_constructor_mapping_facts
                    .push(constructor.as_external_crate_constructor_mapping_fact());
            }
        }
        for field in self.modeled_fields {
            if field.coverage == ExternalApiCoverage::Unsupported {
                facts
                    .unsupported_external_field_facts
                    .push(field.as_unsupported_external_field_fact(self.pack_name));
                continue;
            }
            facts.external_field_facts.push(ExternalFieldFact {
                field: FieldKey {
                    type_name: field.type_name.to_string(),
                    field_name: field.field_name.to_string(),
                },
                field_type: external_pack_modeled_field_type(field.type_name, field.field_name),
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

/// Checks that pack-owned member facts are backed by explicit manifest coverage entries.
///
/// `facts` should be the fact set emitted for `manifest`, not a merged global fact set.
/// This keeps the audit focused on the selected pack boundary and avoids treating
/// user overrides or source-summary facts as manifest coverage gaps.
pub fn audit_external_pack_manifest_coverage(
    manifest: &ExternalPackManifest,
    facts: &FactSet,
) -> DiagnosticBag {
    let mut diagnostics = DiagnosticBag::default();

    for effect in facts.external_api_effect_facts.iter().filter(|effect| {
        effect.provenance == FactProvenance::ExternalLibraryPack && effect.receiver_type.is_some()
    }) {
        let receiver_type = effect.receiver_type.as_deref().unwrap();
        match manifest.modeled_api(receiver_type, &effect.method_name) {
            Some(modeled) if modeled.coverage != ExternalApiCoverage::Unsupported => {
                match manifest.api_effects.iter().find(|modeled_effect| {
                    modeled_effect.receiver_type == receiver_type
                        && modeled_effect.method_name == effect.method_name
                        && modeled_effect.arg_count == effect.arg_count
                }) {
                    Some(modeled_effect)
                        if modeled_effect.as_external_api_effect_fact() == *effect => {}
                    Some(modeled_effect) => diagnostics.warning(
                        "external_pack_manifest_api_effect_mismatch",
                        format!(
                            "{}.{} in {} has API effect {:?} but manifest declares {:?}",
                            receiver_type,
                            effect.method_name,
                            manifest.pack_name,
                            effect,
                            modeled_effect.as_external_api_effect_fact()
                        ),
                    ),
                    None => diagnostics.warning(
                        "external_pack_manifest_missing_api_effect_entry",
                        format!(
                            "{}.{} in {} has a pack-owned API effect fact but no matching manifest API effect entry",
                            receiver_type, effect.method_name, manifest.pack_name
                        ),
                    ),
                }
            }
            Some(_) => diagnostics.warning(
                "external_pack_manifest_unsupported_api_emitted",
                format!(
                    "{}.{} in {} is marked unsupported but has an emitted API effect fact",
                    receiver_type, effect.method_name, manifest.pack_name
                ),
            ),
            None => diagnostics.warning(
                "external_pack_manifest_missing_api_coverage",
                format!(
                    "{}.{} in {} has a pack-owned API effect fact but no manifest coverage entry",
                    receiver_type, effect.method_name, manifest.pack_name
                ),
            ),
        }
    }

    for argument in facts
        .external_api_argument_effect_facts
        .iter()
        .filter(|argument| {
            argument.provenance == FactProvenance::ExternalLibraryPack
                && argument.receiver_type.is_some()
        })
    {
        let receiver_type = argument.receiver_type.as_deref().unwrap();
        match manifest.modeled_api(receiver_type, &argument.method_name) {
            Some(modeled) if modeled.coverage != ExternalApiCoverage::Unsupported => {}
            Some(_) => diagnostics.warning(
                "external_pack_manifest_unsupported_argument_effect_emitted",
                format!(
                    "{}.{} argument {} in {} is marked unsupported but has an emitted argument-effect fact",
                    receiver_type,
                    argument.method_name,
                    argument.parameter_index,
                    manifest.pack_name
                ),
            ),
            None => diagnostics.warning(
                "external_pack_manifest_missing_argument_effect_coverage",
                format!(
                    "{}.{} argument {} in {} has a pack-owned argument-effect fact but no manifest API coverage entry",
                    receiver_type,
                    argument.method_name,
                    argument.parameter_index,
                    manifest.pack_name
                ),
            ),
        }
    }

    for mapping in facts
        .external_crate_api_mapping_facts
        .iter()
        .filter(|mapping| mapping.provenance == FactProvenance::ExternalLibraryPack)
    {
        if manifest.mode != ExternalLibraryMode::RustCrateMapping {
            diagnostics.warning(
                "external_pack_manifest_unexpected_crate_api_mapping",
                format!(
                    "{}.{} in {} has a crate API mapping fact but manifest mode is {:?}",
                    mapping.receiver_type, mapping.method_name, manifest.pack_name, manifest.mode
                ),
            );
            continue;
        }
        match manifest.modeled_api(&mapping.receiver_type, &mapping.method_name) {
            Some(modeled) if modeled.coverage != ExternalApiCoverage::Unsupported => {
                match manifest.api_effects.iter().find(|modeled_effect| {
                    modeled_effect.receiver_type == mapping.receiver_type
                        && modeled_effect.method_name == mapping.method_name
                        && modeled_effect.arg_count == mapping.arg_count
                }) {
                    Some(modeled_effect)
                        if modeled_effect.as_external_crate_api_mapping_fact() == *mapping => {}
                    Some(modeled_effect) => diagnostics.warning(
                        "external_pack_manifest_crate_api_mapping_mismatch",
                        format!(
                            "{}.{} in {} has crate API mapping {:?} but manifest declares {:?}",
                            mapping.receiver_type,
                            mapping.method_name,
                            manifest.pack_name,
                            mapping,
                            modeled_effect.as_external_crate_api_mapping_fact()
                        ),
                    ),
                    None => diagnostics.warning(
                        "external_pack_manifest_missing_crate_api_mapping_entry",
                        format!(
                            "{}.{} in {} has a crate API mapping fact but no matching manifest API effect entry",
                            mapping.receiver_type, mapping.method_name, manifest.pack_name
                        ),
                    ),
                }
            }
            Some(_) => diagnostics.warning(
                "external_pack_manifest_unsupported_crate_api_mapping_emitted",
                format!(
                    "{}.{} in {} is marked unsupported but has an emitted crate API mapping fact",
                    mapping.receiver_type, mapping.method_name, manifest.pack_name
                ),
            ),
            None => diagnostics.warning(
                "external_pack_manifest_missing_crate_api_mapping_coverage",
                format!(
                    "{}.{} in {} has a crate API mapping fact but no manifest API coverage entry",
                    mapping.receiver_type, mapping.method_name, manifest.pack_name
                ),
            ),
        }
    }

    for constructor in facts
        .external_constructor_facts
        .iter()
        .filter(|constructor| constructor.provenance == FactProvenance::ExternalLibraryPack)
    {
        match manifest.modeled_constructor(&constructor.type_name, constructor.arg_types.len()) {
            Some(modeled) if modeled.coverage != ExternalApiCoverage::Unsupported => {}
            Some(_) => diagnostics.warning(
                "external_pack_manifest_unsupported_constructor_emitted",
                format!(
                    "{} constructor with {} argument(s) in {} is marked unsupported but has an emitted constructor fact",
                    constructor.type_name,
                    constructor.arg_types.len(),
                    manifest.pack_name
                ),
            ),
            None => diagnostics.warning(
                "external_pack_manifest_missing_constructor_coverage",
                format!(
                    "{} constructor with {} argument(s) in {} has a pack-owned constructor fact but no manifest coverage entry",
                    constructor.type_name,
                    constructor.arg_types.len(),
                    manifest.pack_name
                ),
            ),
        }
    }

    for mapping in facts
        .external_crate_constructor_mapping_facts
        .iter()
        .filter(|mapping| mapping.provenance == FactProvenance::ExternalLibraryPack)
    {
        if manifest.mode != ExternalLibraryMode::RustCrateMapping {
            diagnostics.warning(
                "external_pack_manifest_unexpected_crate_constructor_mapping",
                format!(
                    "{} constructor with {} argument(s) in {} has a crate constructor mapping fact but manifest mode is {:?}",
                    mapping.type_name, mapping.arg_count, manifest.pack_name, manifest.mode
                ),
            );
            continue;
        }
        match manifest.modeled_constructor(&mapping.type_name, mapping.arg_count) {
            Some(modeled) if modeled.coverage != ExternalApiCoverage::Unsupported => {
                if modeled.as_external_crate_constructor_mapping_fact() != *mapping {
                    diagnostics.warning(
                        "external_pack_manifest_crate_constructor_mapping_mismatch",
                        format!(
                            "{} constructor with {} argument(s) in {} has crate constructor mapping {:?} but manifest declares {:?}",
                            mapping.type_name,
                            mapping.arg_count,
                            manifest.pack_name,
                            mapping,
                            modeled.as_external_crate_constructor_mapping_fact()
                        ),
                    );
                }
            }
            Some(_) => diagnostics.warning(
                "external_pack_manifest_unsupported_crate_constructor_mapping_emitted",
                format!(
                    "{} constructor with {} argument(s) in {} is marked unsupported but has an emitted crate constructor mapping fact",
                    mapping.type_name, mapping.arg_count, manifest.pack_name
                ),
            ),
            None => diagnostics.warning(
                "external_pack_manifest_missing_crate_constructor_mapping_coverage",
                format!(
                    "{} constructor with {} argument(s) in {} has a crate constructor mapping fact but no manifest constructor coverage entry",
                    mapping.type_name, mapping.arg_count, manifest.pack_name
                ),
            ),
        }
    }

    for field in facts
        .external_field_facts
        .iter()
        .filter(|field| field.provenance == FactProvenance::ExternalLibraryPack)
    {
        match manifest.modeled_field(&field.field.type_name, &field.field.field_name) {
            Some(modeled) if modeled.coverage != ExternalApiCoverage::Unsupported => {}
            Some(_) => diagnostics.warning(
                "external_pack_manifest_unsupported_field_emitted",
                format!(
                    "{}.{} in {} is marked unsupported but has an emitted field fact",
                    field.field.type_name, field.field.field_name, manifest.pack_name
                ),
            ),
            None => diagnostics.warning(
                "external_pack_manifest_missing_field_coverage",
                format!(
                    "{}.{} in {} has a pack-owned field fact but no manifest coverage entry",
                    field.field.type_name, field.field.field_name, manifest.pack_name
                ),
            ),
        }
    }

    for unsupported in facts
        .unsupported_external_api_facts
        .iter()
        .filter(|unsupported| {
            unsupported.provenance == FactProvenance::ExternalLibraryPack
                && unsupported.pack_name == manifest.pack_name
        })
    {
        let expected_reason = manifest
            .unsupported_apis
            .iter()
            .find(|modeled| {
                modeled.receiver_type == unsupported.receiver_type
                    && modeled.method_name == unsupported.method_name
            })
            .map(|modeled| modeled.reason)
            .or_else(|| {
                manifest
                    .modeled_api(&unsupported.receiver_type, &unsupported.method_name)
                    .filter(|modeled| modeled.coverage == ExternalApiCoverage::Unsupported)
                    .map(|_| "declared unsupported by external pack manifest")
            });
        match expected_reason {
            Some(expected_reason) if unsupported.reason == expected_reason => {}
            Some(expected_reason) => diagnostics.warning(
                "external_pack_manifest_unsupported_api_mismatch",
                format!(
                    "{}.{} in {} has unsupported API reason {:?} but manifest declares {:?}",
                    unsupported.receiver_type,
                    unsupported.method_name,
                    manifest.pack_name,
                    unsupported.reason,
                    expected_reason
                ),
            ),
            None => diagnostics.warning(
                "external_pack_manifest_missing_unsupported_api_coverage",
                format!(
                    "{}.{} in {} has an unsupported API fact but no unsupported manifest coverage entry",
                    unsupported.receiver_type, unsupported.method_name, manifest.pack_name
                ),
            ),
        }
    }

    for unsupported in facts
        .unsupported_external_constructor_facts
        .iter()
        .filter(|unsupported| {
            unsupported.provenance == FactProvenance::ExternalLibraryPack
                && unsupported.pack_name == manifest.pack_name
        })
    {
        match manifest.modeled_constructor(&unsupported.type_name, unsupported.arg_count) {
            Some(modeled) if modeled.coverage == ExternalApiCoverage::Unsupported => {
                if modeled.as_unsupported_external_constructor_fact(manifest.pack_name)
                    != *unsupported
                {
                    diagnostics.warning(
                        "external_pack_manifest_unsupported_constructor_mismatch",
                        format!(
                            "{} constructor with {} argument(s) in {} has unsupported constructor fact {:?} but manifest declares {:?}",
                            unsupported.type_name,
                            unsupported.arg_count,
                            manifest.pack_name,
                            unsupported,
                            modeled.as_unsupported_external_constructor_fact(manifest.pack_name)
                        ),
                    );
                }
            }
            _ => diagnostics.warning(
                "external_pack_manifest_missing_unsupported_constructor_coverage",
                format!(
                    "{} constructor with {} argument(s) in {} has an unsupported constructor fact but no unsupported manifest coverage entry",
                    unsupported.type_name, unsupported.arg_count, manifest.pack_name
                ),
            ),
        }
    }

    for unsupported in facts
        .unsupported_external_field_facts
        .iter()
        .filter(|unsupported| {
            unsupported.provenance == FactProvenance::ExternalLibraryPack
                && unsupported.pack_name == manifest.pack_name
        })
    {
        match manifest.modeled_field(&unsupported.field.type_name, &unsupported.field.field_name) {
            Some(modeled) if modeled.coverage == ExternalApiCoverage::Unsupported => {
                if modeled.as_unsupported_external_field_fact(manifest.pack_name) != *unsupported {
                    diagnostics.warning(
                        "external_pack_manifest_unsupported_field_mismatch",
                        format!(
                            "{}.{} in {} has unsupported field fact {:?} but manifest declares {:?}",
                            unsupported.field.type_name,
                            unsupported.field.field_name,
                            manifest.pack_name,
                            unsupported,
                            modeled.as_unsupported_external_field_fact(manifest.pack_name)
                        ),
                    );
                }
            }
            _ => diagnostics.warning(
                "external_pack_manifest_missing_unsupported_field_coverage",
                format!(
                    "{}.{} in {} has an unsupported field fact but no unsupported manifest coverage entry",
                    unsupported.field.type_name,
                    unsupported.field.field_name,
                    manifest.pack_name
                ),
            ),
        }
    }

    for strategy in facts
        .hierarchy_overrides
        .iter()
        .filter(|strategy| strategy.provenance == FactProvenance::ExternalLibraryPack)
    {
        match manifest
            .object_strategies
            .iter()
            .find(|modeled| modeled.type_name == strategy.type_name)
        {
            Some(modeled) if modeled.kind_as_hierarchy_strategy() == strategy.strategy => {}
            Some(modeled) => diagnostics.warning(
                "external_pack_manifest_object_strategy_mismatch",
                format!(
                    "{} in {} has object strategy {:?} but manifest declares {:?}",
                    strategy.type_name,
                    manifest.pack_name,
                    strategy.strategy,
                    modeled.kind_as_hierarchy_strategy()
                ),
            ),
            None => diagnostics.warning(
                "external_pack_manifest_missing_object_strategy_coverage",
                format!(
                    "{} in {} has a pack-owned object strategy fact but no manifest object strategy entry",
                    strategy.type_name, manifest.pack_name
                ),
            ),
        }
    }

    for closed in facts
        .closed_hierarchy_facts
        .iter()
        .filter(|closed| closed.provenance == FactProvenance::ExternalLibraryPack)
    {
        match manifest
            .closed_hierarchy
            .iter()
            .find(|modeled| modeled.type_name == closed.type_name)
        {
            Some(modeled) if modeled.closed == closed.closed => {}
            Some(modeled) => diagnostics.warning(
                "external_pack_manifest_closed_hierarchy_mismatch",
                format!(
                    "{} in {} has closed-hierarchy value {:?} but manifest declares {:?}",
                    closed.type_name, manifest.pack_name, closed.closed, modeled.closed
                ),
            ),
            None => diagnostics.warning(
                "external_pack_manifest_missing_closed_hierarchy_coverage",
                format!(
                    "{} in {} has a pack-owned closed-hierarchy fact but no manifest closed-hierarchy entry",
                    closed.type_name, manifest.pack_name
                ),
            ),
        }
    }

    for object_safety in facts
        .object_safety_facts
        .iter()
        .filter(|object_safety| object_safety.provenance == FactProvenance::ExternalLibraryPack)
    {
        let manifest_object_safety = manifest
            .object_safety
            .iter()
            .filter(|modeled| modeled.type_name == object_safety.type_name)
            .collect::<Vec<_>>();
        if manifest_object_safety.is_empty() {
            diagnostics.warning(
                "external_pack_manifest_missing_object_safety_coverage",
                format!(
                    "{} in {} has a pack-owned object-safety fact but no manifest object-safety entry",
                    object_safety.type_name, manifest.pack_name
                ),
            );
        } else {
            let manifest_violations = manifest_object_safety
                .iter()
                .map(|modeled| modeled.as_object_safety_violation())
                .collect::<Vec<_>>();
            if !object_safety.violations.iter().all(|violation| {
                manifest_violations
                    .iter()
                    .any(|modeled| modeled == violation)
            }) || !manifest_violations.iter().all(|modeled| {
                object_safety
                    .violations
                    .iter()
                    .any(|violation| violation == modeled)
            }) {
                diagnostics.warning(
                    "external_pack_manifest_object_safety_mismatch",
                    format!(
                        "{} in {} has object-safety violations {:?} but manifest declares {:?}",
                        object_safety.type_name,
                        manifest.pack_name,
                        object_safety.violations,
                        manifest_violations
                    ),
                );
            }
        }
    }

    for capability in facts
        .type_capability_overrides
        .iter()
        .filter(|capability| capability.provenance == FactProvenance::ExternalLibraryPack)
    {
        let matching_manifest_capabilities = manifest
            .type_capabilities
            .iter()
            .filter(|modeled| modeled.type_name == capability.type_name)
            .collect::<Vec<_>>();
        if matching_manifest_capabilities.is_empty() {
            diagnostics.warning(
                "external_pack_manifest_missing_type_capability_coverage",
                format!(
                    "{} in {} has a pack-owned type capability fact but no manifest type capability entry",
                    capability.type_name, manifest.pack_name
                ),
            );
        } else if !matching_manifest_capabilities
            .iter()
            .any(|modeled| modeled.as_type_capability_override() == *capability)
        {
            diagnostics.warning(
                "external_pack_manifest_type_capability_mismatch",
                format!(
                    "{} in {} has type capability {:?} but no matching manifest type capability entry",
                    capability.type_name, manifest.pack_name, capability
                ),
            );
        }
    }

    for capability in facts
        .owned_recovery_capability_facts
        .iter()
        .filter(|capability| capability.provenance == FactProvenance::ExternalLibraryPack)
    {
        if !fact_set_owns_type(facts, &capability.type_name) {
            diagnostics.warning(
                "external_pack_manifest_unowned_owned_recovery_capability_fact",
                format!(
                    "{} has an external-pack owned-recovery capability fact but no declared pack owns type {}",
                    capability.type_name, capability.type_name
                ),
            );
        }
    }

    for field in facts
        .field_nullability_facts
        .iter()
        .filter(|field| field.provenance == FieldNullabilityProvenance::ExternalLibraryPack)
    {
        match manifest.field_nullability.iter().find(|modeled| {
            modeled.type_name == field.field.type_name
                && modeled.field_name == field.field.field_name
        }) {
            Some(modeled) if modeled.state == field.state => {}
            Some(modeled) => diagnostics.warning(
                "external_pack_manifest_field_nullability_mismatch",
                format!(
                    "{}.{} in {} has field nullability {:?} but manifest declares {:?}",
                    field.field.type_name,
                    field.field.field_name,
                    manifest.pack_name,
                    field.state,
                    modeled.state
                ),
            ),
            None => diagnostics.warning(
                "external_pack_manifest_missing_field_nullability_coverage",
                format!(
                    "{}.{} in {} has a pack-owned field-nullability fact but no manifest field-nullability entry",
                    field.field.type_name, field.field.field_name, manifest.pack_name
                ),
            ),
        }
    }

    for exception in facts
        .exception_facts
        .iter()
        .filter(|exception| exception.provenance == FactProvenance::ExternalLibraryPack)
    {
        match manifest
            .exceptions
            .iter()
            .find(|modeled| modeled.type_name == exception.type_name)
        {
            Some(modeled) if modeled.kind == exception.kind => {}
            Some(modeled) => diagnostics.warning(
                "external_pack_manifest_exception_kind_mismatch",
                format!(
                    "{} in {} has exception kind {:?} but manifest declares {:?}",
                    exception.type_name, manifest.pack_name, exception.kind, modeled.kind
                ),
            ),
            None => diagnostics.warning(
                "external_pack_manifest_missing_exception_coverage",
                format!(
                    "{} in {} has a pack-owned exception fact but no manifest exception entry",
                    exception.type_name, manifest.pack_name
                ),
            ),
        }
    }

    diagnostics
}

pub fn audit_external_pack_fact_boundaries(facts: &FactSet) -> DiagnosticBag {
    let mut diagnostics = DiagnosticBag::default();

    for effect in facts.external_api_effect_facts.iter().filter(|effect| {
        effect.provenance == FactProvenance::ExternalLibraryPack && effect.receiver_type.is_some()
    }) {
        let receiver_type = effect.receiver_type.as_deref().unwrap();
        if !fact_set_owns_type(facts, receiver_type) {
            diagnostics.warning(
                "external_pack_manifest_unowned_api_fact",
                format!(
                    "{}.{} has an external-pack API fact but no declared pack owns receiver type {}",
                    receiver_type, effect.method_name, receiver_type
                ),
            );
        }
    }

    for argument in facts
        .external_api_argument_effect_facts
        .iter()
        .filter(|argument| {
            argument.provenance == FactProvenance::ExternalLibraryPack
                && argument.receiver_type.is_some()
        })
    {
        let receiver_type = argument.receiver_type.as_deref().unwrap();
        if !fact_set_owns_type(facts, receiver_type) {
            diagnostics.warning(
                "external_pack_manifest_unowned_argument_effect_fact",
                format!(
                    "{}.{} argument {} has an external-pack argument-effect fact but no declared pack owns receiver type {}",
                    receiver_type,
                    argument.method_name,
                    argument.parameter_index,
                    receiver_type
                ),
            );
        }
    }

    for mapping in facts
        .external_crate_api_mapping_facts
        .iter()
        .filter(|mapping| mapping.provenance == FactProvenance::ExternalLibraryPack)
    {
        if !fact_set_owns_type(facts, &mapping.receiver_type) {
            diagnostics.warning(
                "external_pack_manifest_unowned_crate_api_mapping_fact",
                format!(
                    "{}.{} has an external-pack crate API mapping fact but no declared pack owns receiver type {}",
                    mapping.receiver_type, mapping.method_name, mapping.receiver_type
                ),
            );
        }
    }

    for constructor in facts
        .external_constructor_facts
        .iter()
        .filter(|constructor| constructor.provenance == FactProvenance::ExternalLibraryPack)
    {
        if !fact_set_owns_type(facts, &constructor.type_name) {
            diagnostics.warning(
                "external_pack_manifest_unowned_constructor_fact",
                format!(
                    "{} constructor with {} argument(s) has an external-pack constructor fact but no declared pack owns type {}",
                    constructor.type_name,
                    constructor.arg_types.len(),
                    constructor.type_name
                ),
            );
        }
    }

    for mapping in facts
        .external_crate_constructor_mapping_facts
        .iter()
        .filter(|mapping| mapping.provenance == FactProvenance::ExternalLibraryPack)
    {
        if !fact_set_owns_type(facts, &mapping.type_name) {
            diagnostics.warning(
                "external_pack_manifest_unowned_crate_constructor_mapping_fact",
                format!(
                    "{} constructor with {} argument(s) has an external-pack crate constructor mapping fact but no declared pack owns type {}",
                    mapping.type_name, mapping.arg_count, mapping.type_name
                ),
            );
        }
    }

    for field in facts
        .external_field_facts
        .iter()
        .filter(|field| field.provenance == FactProvenance::ExternalLibraryPack)
    {
        if !fact_set_owns_type(facts, &field.field.type_name) {
            diagnostics.warning(
                "external_pack_manifest_unowned_field_fact",
                format!(
                    "{}.{} has an external-pack field fact but no declared pack owns type {}",
                    field.field.type_name, field.field.field_name, field.field.type_name
                ),
            );
        }
    }

    for unsupported in facts
        .unsupported_external_api_facts
        .iter()
        .filter(|unsupported| unsupported.provenance == FactProvenance::ExternalLibraryPack)
    {
        if !pack_named_owns_type(facts, &unsupported.pack_name, &unsupported.receiver_type) {
            diagnostics.warning(
                "external_pack_manifest_unowned_unsupported_api_fact",
                format!(
                    "{}.{} is declared unsupported in pack {} but that pack does not own receiver type {}",
                    unsupported.receiver_type,
                    unsupported.method_name,
                    unsupported.pack_name,
                    unsupported.receiver_type
                ),
            );
        }
    }

    for unsupported in facts
        .unsupported_external_constructor_facts
        .iter()
        .filter(|unsupported| unsupported.provenance == FactProvenance::ExternalLibraryPack)
    {
        if !pack_named_owns_type(facts, &unsupported.pack_name, &unsupported.type_name) {
            diagnostics.warning(
                "external_pack_manifest_unowned_unsupported_constructor_fact",
                format!(
                    "{} constructor with {} argument(s) is declared unsupported in pack {} but that pack does not own type {}",
                    unsupported.type_name,
                    unsupported.arg_count,
                    unsupported.pack_name,
                    unsupported.type_name
                ),
            );
        }
    }

    for unsupported in facts
        .unsupported_external_field_facts
        .iter()
        .filter(|unsupported| unsupported.provenance == FactProvenance::ExternalLibraryPack)
    {
        if !pack_named_owns_type(facts, &unsupported.pack_name, &unsupported.field.type_name) {
            diagnostics.warning(
                "external_pack_manifest_unowned_unsupported_field_fact",
                format!(
                    "{}.{} is declared unsupported in pack {} but that pack does not own type {}",
                    unsupported.field.type_name,
                    unsupported.field.field_name,
                    unsupported.pack_name,
                    unsupported.field.type_name
                ),
            );
        }
    }

    for strategy in facts
        .hierarchy_overrides
        .iter()
        .filter(|strategy| strategy.provenance == FactProvenance::ExternalLibraryPack)
    {
        if !fact_set_owns_type(facts, &strategy.type_name) {
            diagnostics.warning(
                "external_pack_manifest_unowned_object_strategy_fact",
                format!(
                    "{} has an external-pack object strategy fact but no declared pack owns type {}",
                    strategy.type_name, strategy.type_name
                ),
            );
        }
    }

    for closed in facts
        .closed_hierarchy_facts
        .iter()
        .filter(|closed| closed.provenance == FactProvenance::ExternalLibraryPack)
    {
        if !fact_set_owns_type(facts, &closed.type_name) {
            diagnostics.warning(
                "external_pack_manifest_unowned_closed_hierarchy_fact",
                format!(
                    "{} has an external-pack closed-hierarchy fact but no declared pack owns type {}",
                    closed.type_name, closed.type_name
                ),
            );
        }
    }

    for object_safety in facts
        .object_safety_facts
        .iter()
        .filter(|object_safety| object_safety.provenance == FactProvenance::ExternalLibraryPack)
    {
        if !fact_set_owns_type(facts, &object_safety.type_name) {
            diagnostics.warning(
                "external_pack_manifest_unowned_object_safety_fact",
                format!(
                    "{} has an external-pack object-safety fact but no declared pack owns type {}",
                    object_safety.type_name, object_safety.type_name
                ),
            );
        }
    }

    for capability in facts
        .type_capability_overrides
        .iter()
        .filter(|capability| capability.provenance == FactProvenance::ExternalLibraryPack)
    {
        if !fact_set_owns_type(facts, &capability.type_name) {
            diagnostics.warning(
                "external_pack_manifest_unowned_type_capability_fact",
                format!(
                    "{} has an external-pack type capability fact but no declared pack owns type {}",
                    capability.type_name, capability.type_name
                ),
            );
        }
    }

    for capability in facts
        .owned_recovery_capability_facts
        .iter()
        .filter(|capability| capability.provenance == FactProvenance::ExternalLibraryPack)
    {
        if !fact_set_owns_type(facts, &capability.type_name) {
            diagnostics.warning(
                "external_pack_manifest_unowned_owned_recovery_capability_fact",
                format!(
                    "{} has an external-pack owned-recovery capability fact but no declared pack owns type {}",
                    capability.type_name, capability.type_name
                ),
            );
        }
    }

    for field in facts
        .field_nullability_facts
        .iter()
        .filter(|field| field.provenance == FieldNullabilityProvenance::ExternalLibraryPack)
    {
        if !fact_set_owns_type(facts, &field.field.type_name) {
            diagnostics.warning(
                "external_pack_manifest_unowned_field_nullability_fact",
                format!(
                    "{}.{} has an external-pack field-nullability fact but no declared pack owns type {}",
                    field.field.type_name, field.field.field_name, field.field.type_name
                ),
            );
        }
    }

    for exception in facts
        .exception_facts
        .iter()
        .filter(|exception| exception.provenance == FactProvenance::ExternalLibraryPack)
    {
        if !fact_set_owns_type(facts, &exception.type_name) {
            diagnostics.warning(
                "external_pack_manifest_unowned_exception_fact",
                format!(
                    "{} has an external-pack exception fact but no declared pack owns type {}",
                    exception.type_name, exception.type_name
                ),
            );
        }
    }

    diagnostics
}

fn fact_set_owns_type(facts: &FactSet, type_name: &str) -> bool {
    facts.external_library_pack_for_type(type_name).is_some()
        || builtin_external_pack_boundary_audit_matchers()
            .iter()
            .any(|matcher| {
                matcher.matches(type_name) && fact_set_contains_pack(facts, &matcher.pack.pack_name)
            })
}

fn fact_set_contains_pack(facts: &FactSet, pack_name: &str) -> bool {
    facts.external_library_packs.iter().any(|pack| {
        pack.pack_name == pack_name && pack.provenance == FactProvenance::ExternalLibraryPack
    })
}

fn pack_named_owns_type(facts: &FactSet, pack_name: &str, type_name: &str) -> bool {
    facts.external_library_packs.iter().any(|pack| {
        pack.pack_name == pack_name
            && (type_name == pack.package_prefix
                || type_name.starts_with(&format!("{}.", pack.package_prefix)))
    }) || builtin_external_pack_boundary_audit_matchers()
        .iter()
        .any(|matcher| {
            matcher.pack.pack_name == pack_name
                && matcher.matches(type_name)
                && fact_set_contains_pack(facts, pack_name)
        })
}

impl ExternalPackTypeCapability {
    fn as_type_capability_override(&self) -> TypeCapabilityOverride {
        TypeCapabilityOverride {
            type_name: self.type_name.to_string(),
            copy: self.copy,
            clone: self.clone,
            string_like: self.string_like,
            sequence_like: self.sequence_like.map(parse_type),
            implements: self.implements.iter().map(|ty| parse_type(ty)).collect(),
            extends: self.extends.map(parse_type),
            final_type: self.final_type,
            object_safe_trait: self.object_safe_trait,
            provenance: FactProvenance::ExternalLibraryPack,
        }
    }
}

impl ExternalPackArgumentEffect {
    fn as_external_api_argument_effect_fact(&self) -> ExternalApiArgumentEffectFact {
        ExternalApiArgumentEffectFact {
            method_name: self.method_name.to_string(),
            receiver_type: Some(self.receiver_type.to_string()),
            parameter_index: self.parameter_index,
            parameter_type: self.parameter_type.map(parse_type),
            parameter_nullability: self.parameter_nullability,
            argument_mutability: self.argument_mutability,
            alias_policy: self.alias_policy,
            provenance: FactProvenance::ExternalLibraryPack,
        }
    }
}

impl ExternalPackFieldNullability {
    fn as_field_nullability_fact(&self) -> FieldNullabilityFact {
        FieldNullabilityFact {
            field: FieldKey {
                type_name: self.type_name.to_string(),
                field_name: self.field_name.to_string(),
            },
            state: self.state,
            provenance: FieldNullabilityProvenance::ExternalLibraryPack,
        }
    }
}

impl ExternalPackException {
    fn as_exception_fact(&self) -> ExceptionFact {
        ExceptionFact {
            type_name: self.type_name.to_string(),
            kind: self.kind,
            provenance: FactProvenance::ExternalLibraryPack,
        }
    }
}

impl ExternalPackApiEffect {
    fn as_external_api_effect_fact(&self) -> ExternalApiEffectFact {
        ExternalApiEffectFact {
            method_name: self.method_name.to_string(),
            receiver_type: Some(self.receiver_type.to_string()),
            arg_count: self.arg_count,
            role: self.role,
            receiver_mutability: self.receiver_mutability,
            return_type: self.return_type.map(parse_type),
            return_nullability: self.return_nullability,
            thrown: self.thrown.iter().map(|ty| parse_type(ty)).collect(),
            provenance: FactProvenance::ExternalLibraryPack,
        }
    }

    fn as_external_crate_api_mapping_fact(&self) -> ExternalCrateApiMappingFact {
        ExternalCrateApiMappingFact {
            receiver_type: self.receiver_type.to_string(),
            method_name: self.method_name.to_string(),
            arg_count: self.arg_count,
            rust_name: self
                .rust_name
                .map(str::to_string)
                .unwrap_or_else(|| crate::parser::to_snake_case(self.method_name)),
            provenance: FactProvenance::ExternalLibraryPack,
        }
    }
}

impl ExternalPackModeledConstructor {
    fn as_external_crate_constructor_mapping_fact(&self) -> ExternalCrateConstructorMappingFact {
        ExternalCrateConstructorMappingFact {
            type_name: self.type_name.to_string(),
            arg_count: self.arg_count,
            rust_type_name: self.rust_type_name.map(str::to_string),
            rust_name: self.rust_name.unwrap_or("new").to_string(),
            provenance: FactProvenance::ExternalLibraryPack,
        }
    }

    fn as_unsupported_external_constructor_fact(
        &self,
        pack_name: &str,
    ) -> UnsupportedExternalConstructorFact {
        UnsupportedExternalConstructorFact {
            pack_name: pack_name.to_string(),
            type_name: self.type_name.to_string(),
            arg_count: self.arg_count,
            reason: "declared unsupported by external pack manifest".to_string(),
            provenance: FactProvenance::ExternalLibraryPack,
        }
    }
}

impl ExternalPackModeledField {
    fn as_unsupported_external_field_fact(&self, pack_name: &str) -> UnsupportedExternalFieldFact {
        UnsupportedExternalFieldFact {
            pack_name: pack_name.to_string(),
            field: FieldKey {
                type_name: self.type_name.to_string(),
                field_name: self.field_name.to_string(),
            },
            reason: "declared unsupported by external pack manifest".to_string(),
            provenance: FactProvenance::ExternalLibraryPack,
        }
    }
}

impl ExternalPackObjectStrategy {
    fn kind_as_hierarchy_strategy(&self) -> HierarchyStrategy {
        match self.strategy {
            ExternalPackObjectStrategyKind::ConcreteStruct => HierarchyStrategy::ConcreteStruct,
            ExternalPackObjectStrategyKind::ClosedEnum {
                enum_name,
                variants,
            } => HierarchyStrategy::ClosedEnum {
                enum_name: enum_name.to_string(),
                variants: variants
                    .iter()
                    .map(|variant| (*variant).to_string())
                    .collect(),
            },
            ExternalPackObjectStrategyKind::DynTrait { trait_name } => {
                HierarchyStrategy::DynTrait {
                    trait_name: trait_name.to_string(),
                }
            }
            ExternalPackObjectStrategyKind::ErasedObject => HierarchyStrategy::ErasedObject,
        }
    }
}

impl ExternalPackObjectSafety {
    fn as_object_safety_violation(&self) -> ObjectSafetyViolation {
        match self.violation {
            ExternalPackObjectSafetyViolation::StaticMethod { method_name } => {
                ObjectSafetyViolation::StaticMethod {
                    method_name: method_name.to_string(),
                }
            }
            ExternalPackObjectSafetyViolation::SelfReferentialReturn { method_name } => {
                ObjectSafetyViolation::SelfReferentialReturn {
                    method_name: method_name.to_string(),
                }
            }
            ExternalPackObjectSafetyViolation::NestedSelfReferentialReturn { method_name } => {
                ObjectSafetyViolation::NestedSelfReferentialReturn {
                    method_name: method_name.to_string(),
                }
            }
            ExternalPackObjectSafetyViolation::SelfReferentialParameter {
                method_name,
                param_name,
            } => ObjectSafetyViolation::SelfReferentialParameter {
                method_name: method_name.to_string(),
                param_name: param_name.to_string(),
            },
            ExternalPackObjectSafetyViolation::NestedSelfReferentialParameter {
                method_name,
                param_name,
            } => ObjectSafetyViolation::NestedSelfReferentialParameter {
                method_name: method_name.to_string(),
                param_name: param_name.to_string(),
            },
            ExternalPackObjectSafetyViolation::FieldMember { field_name } => {
                ObjectSafetyViolation::FieldMember {
                    field_name: field_name.to_string(),
                }
            }
            ExternalPackObjectSafetyViolation::ConstructorMember => {
                ObjectSafetyViolation::ConstructorMember
            }
            ExternalPackObjectSafetyViolation::UnsupportedMember => {
                ObjectSafetyViolation::UnsupportedMember
            }
        }
    }

    fn push_facts(&self, facts: &mut FactSet) {
        push_external_pack_object_safety_violation(
            facts,
            self.type_name,
            self.as_object_safety_violation(),
        );
    }
}

pub fn parse_external_pack_manifest(source: &str) -> ExternalPackManifestParse {
    let mut facts = FactSet::default();
    let mut diagnostics = DiagnosticBag::default();

    for (line_index, raw_line) in source.lines().enumerate() {
        let line = strip_comment(raw_line).trim();
        if line.is_empty() {
            continue;
        }
        let parts = line.split_whitespace().collect::<Vec<_>>();
        match parts.as_slice() {
            [
                "external-pack",
                pack_name,
                package_prefix,
                mode,
                coverage,
                metadata @ ..,
            ] => {
                push_external_pack_fact(
                    &mut facts,
                    &mut diagnostics,
                    line_index,
                    pack_name,
                    package_prefix,
                    mode,
                    coverage,
                    metadata,
                );
            }
            [
                "external-pack-api",
                pack_name,
                receiver_type,
                method_name,
                coverage,
                role,
                receiver_mutability,
            ] => {
                push_external_pack_api_fact(
                    &mut facts,
                    &mut diagnostics,
                    line_index,
                    pack_name,
                    receiver_type,
                    method_name,
                    coverage,
                    role,
                    receiver_mutability,
                    "unknown",
                    &[],
                );
            }
            [
                "external-pack-api",
                pack_name,
                receiver_type,
                method_name,
                coverage,
                role,
                receiver_mutability,
                return_nullability,
                metadata @ ..,
            ] => {
                push_external_pack_api_fact(
                    &mut facts,
                    &mut diagnostics,
                    line_index,
                    pack_name,
                    receiver_type,
                    method_name,
                    coverage,
                    role,
                    receiver_mutability,
                    return_nullability,
                    metadata,
                );
            }
            [
                "external-pack-constructor",
                pack_name,
                type_name,
                arg_count,
                coverage,
                metadata @ ..,
            ] => {
                push_external_pack_constructor_fact(
                    &mut facts,
                    &mut diagnostics,
                    line_index,
                    pack_name,
                    type_name,
                    arg_count,
                    coverage,
                    metadata,
                );
            }
            [
                "external-pack-field",
                pack_name,
                type_name,
                field_name,
                coverage,
            ] => {
                push_external_pack_field_fact(
                    &mut facts,
                    &mut diagnostics,
                    line_index,
                    pack_name,
                    type_name,
                    field_name,
                    coverage,
                );
            }
            [
                "external-pack-field-nullability",
                _pack_name,
                type_name,
                field_name,
                state,
            ] => {
                push_external_pack_field_nullability_fact(
                    &mut facts,
                    &mut diagnostics,
                    line_index,
                    type_name,
                    field_name,
                    state,
                );
            }
            [
                "external-pack-object",
                _pack_name,
                type_name,
                strategy,
                metadata @ ..,
            ] => {
                push_external_pack_object_strategy_fact(
                    &mut facts,
                    &mut diagnostics,
                    line_index,
                    type_name,
                    strategy,
                    metadata,
                );
            }
            ["external-pack-closed", _pack_name, type_name] => {
                push_external_pack_closed_hierarchy_fact(
                    &mut facts,
                    &mut diagnostics,
                    line_index,
                    type_name,
                    "true",
                );
            }
            ["external-pack-closed", _pack_name, type_name, closed] => {
                push_external_pack_closed_hierarchy_fact(
                    &mut facts,
                    &mut diagnostics,
                    line_index,
                    type_name,
                    closed,
                );
            }
            [
                "external-pack-capability",
                _pack_name,
                type_name,
                metadata @ ..,
            ] => {
                push_external_pack_type_capability_fact(
                    &mut facts,
                    &mut diagnostics,
                    line_index,
                    type_name,
                    metadata,
                );
            }
            [
                "external-pack-owned-recovery",
                _pack_name,
                type_name,
                kind,
                metadata @ ..,
            ] => {
                push_external_pack_owned_recovery_fact(
                    &mut facts,
                    &mut diagnostics,
                    line_index,
                    type_name,
                    kind,
                    metadata,
                );
            }
            [
                "external-pack-object-safety",
                _pack_name,
                type_name,
                violation_kind,
                metadata @ ..,
            ] => {
                push_external_pack_object_safety_fact(
                    &mut facts,
                    &mut diagnostics,
                    line_index,
                    type_name,
                    violation_kind,
                    metadata,
                );
            }
            [
                "external-pack-argument",
                _pack_name,
                receiver_type,
                method_name,
                parameter_index,
                parameter_nullability,
                argument_mutability,
                alias_policy,
                metadata @ ..,
            ] => {
                push_external_pack_argument_effect_fact(
                    &mut facts,
                    &mut diagnostics,
                    line_index,
                    receiver_type,
                    method_name,
                    parameter_index,
                    parameter_nullability,
                    argument_mutability,
                    alias_policy,
                    metadata,
                );
            }
            ["external-pack-exception", _pack_name, type_name, kind] => {
                push_external_pack_exception_fact(
                    &mut facts,
                    &mut diagnostics,
                    line_index,
                    type_name,
                    kind,
                );
            }
            [
                "external-pack-unsupported",
                pack_name,
                receiver_type,
                method_name,
                reason @ ..,
            ] if !reason.is_empty() => {
                let reason = reason.join(" ");
                facts
                    .unsupported_external_api_facts
                    .push(UnsupportedExternalApiFact {
                        pack_name: (*pack_name).to_string(),
                        receiver_type: (*receiver_type).to_string(),
                        method_name: (*method_name).to_string(),
                        reason,
                        provenance: FactProvenance::ExternalLibraryPack,
                    });
            }
            _ => push_external_pack_parse_error(
                &mut diagnostics,
                line_index,
                "unrecognized external pack manifest line",
            ),
        }
    }

    diagnostics.extend(audit_external_pack_fact_boundaries(&facts));

    ExternalPackManifestParse { facts, diagnostics }
}

fn strip_comment(line: &str) -> &str {
    line.split_once('#')
        .map(|(before_comment, _)| before_comment)
        .unwrap_or(line)
}

fn push_external_pack_fact(
    facts: &mut FactSet,
    diagnostics: &mut DiagnosticBag,
    line_index: usize,
    pack_name: &str,
    package_prefix: &str,
    mode: &str,
    coverage: &str,
    metadata: &[&str],
) {
    let Some(mode) = parse_external_library_mode(mode) else {
        push_external_pack_parse_error(diagnostics, line_index, "invalid external pack mode");
        return;
    };
    let Some(coverage) = parse_external_api_coverage(coverage) else {
        push_external_pack_parse_error(diagnostics, line_index, "invalid external pack coverage");
        return;
    };
    let Some(rust_target) = parse_pack_metadata(diagnostics, line_index, metadata) else {
        return;
    };
    facts.external_library_packs.push(ExternalLibraryPackFact {
        pack_name: pack_name.to_string(),
        package_prefix: package_prefix.to_string(),
        mode,
        coverage,
        rust_target,
        provenance: FactProvenance::ExternalLibraryPack,
    });
}

fn push_external_pack_api_fact(
    facts: &mut FactSet,
    diagnostics: &mut DiagnosticBag,
    line_index: usize,
    pack_name: &str,
    receiver_type: &str,
    method_name: &str,
    coverage: &str,
    role: &str,
    receiver_mutability: &str,
    return_nullability: &str,
    metadata: &[&str],
) {
    let Some(coverage) = parse_external_api_coverage(coverage) else {
        push_external_pack_parse_error(diagnostics, line_index, "invalid API coverage");
        return;
    };
    if coverage == ExternalApiCoverage::Unsupported {
        if !metadata.is_empty() {
            push_external_pack_parse_error(
                diagnostics,
                line_index,
                "unsupported external-pack-api rows do not accept args, return, throws, or rust metadata",
            );
            return;
        }
        if parse_external_api_role(role).is_none() {
            push_external_pack_parse_error(diagnostics, line_index, "invalid external API role");
            return;
        }
        if parse_receiver_mutability(receiver_mutability).is_none() {
            push_external_pack_parse_error(
                diagnostics,
                line_index,
                "invalid external API receiver mutability",
            );
            return;
        }
        facts
            .unsupported_external_api_facts
            .push(UnsupportedExternalApiFact {
                pack_name: pack_name.to_string(),
                receiver_type: receiver_type.to_string(),
                method_name: method_name.to_string(),
                reason: "declared unsupported by external pack manifest".to_string(),
                provenance: FactProvenance::ExternalLibraryPack,
            });
        return;
    }
    let Some(role) = parse_external_api_role(role) else {
        push_external_pack_parse_error(diagnostics, line_index, "invalid external API role");
        return;
    };
    let Some(receiver_mutability) = parse_receiver_mutability(receiver_mutability) else {
        push_external_pack_parse_error(
            diagnostics,
            line_index,
            "invalid external API receiver mutability",
        );
        return;
    };
    let Some(return_nullability) = parse_nullability_state(return_nullability) else {
        push_external_pack_parse_error(
            diagnostics,
            line_index,
            "invalid external API return nullability",
        );
        return;
    };
    let Some(parsed_metadata) = parse_external_api_metadata(diagnostics, line_index, metadata)
    else {
        return;
    };
    facts.external_api_effect_facts.push(ExternalApiEffectFact {
        method_name: method_name.to_string(),
        receiver_type: Some(receiver_type.to_string()),
        arg_count: parsed_metadata.arg_count,
        role,
        receiver_mutability,
        return_type: parsed_metadata.return_type,
        return_nullability,
        thrown: parsed_metadata.thrown,
        provenance: FactProvenance::ExternalLibraryPack,
    });
    if parsed_metadata.rust_name.is_some()
        || (coverage == ExternalApiCoverage::Mapped && pack_is_rust_crate_mapping(facts, pack_name))
    {
        facts
            .external_crate_api_mapping_facts
            .push(ExternalCrateApiMappingFact {
                receiver_type: receiver_type.to_string(),
                method_name: method_name.to_string(),
                arg_count: parsed_metadata.arg_count,
                rust_name: parsed_metadata
                    .rust_name
                    .unwrap_or_else(|| crate::parser::to_snake_case(method_name)),
                provenance: FactProvenance::ExternalLibraryPack,
            });
    }
}

fn push_external_pack_constructor_fact(
    facts: &mut FactSet,
    diagnostics: &mut DiagnosticBag,
    line_index: usize,
    pack_name: &str,
    type_name: &str,
    arg_count: &str,
    coverage: &str,
    metadata: &[&str],
) {
    let Some(coverage) = parse_external_api_coverage(coverage) else {
        push_external_pack_parse_error(diagnostics, line_index, "invalid constructor coverage");
        return;
    };
    let Ok(arg_count) = arg_count.parse::<usize>() else {
        push_external_pack_parse_error(diagnostics, line_index, "invalid constructor arg count");
        return;
    };
    if coverage == ExternalApiCoverage::Unsupported {
        if !metadata.is_empty() {
            push_external_pack_parse_error(
                diagnostics,
                line_index,
                "unsupported external-pack-constructor rows do not accept rust or rust-type metadata",
            );
            return;
        }
        facts
            .unsupported_external_constructor_facts
            .push(UnsupportedExternalConstructorFact {
                pack_name: pack_name.to_string(),
                type_name: type_name.to_string(),
                arg_count,
                reason: "declared unsupported by external pack manifest".to_string(),
                provenance: FactProvenance::ExternalLibraryPack,
            });
        return;
    }
    let Some(parsed_metadata) =
        parse_external_constructor_metadata(diagnostics, line_index, metadata)
    else {
        return;
    };
    facts
        .external_constructor_facts
        .push(ExternalConstructorFact {
            type_name: type_name.to_string(),
            arg_types: vec![JavaType::Unknown("_".to_string()); arg_count],
            provenance: FactProvenance::ExternalLibraryPack,
        });
    if parsed_metadata.rust_name.is_some()
        || parsed_metadata.rust_type_name.is_some()
        || (coverage == ExternalApiCoverage::Mapped && pack_is_rust_crate_mapping(facts, pack_name))
    {
        facts
            .external_crate_constructor_mapping_facts
            .push(ExternalCrateConstructorMappingFact {
                type_name: type_name.to_string(),
                arg_count,
                rust_type_name: parsed_metadata.rust_type_name,
                rust_name: parsed_metadata
                    .rust_name
                    .unwrap_or_else(|| "new".to_string()),
                provenance: FactProvenance::ExternalLibraryPack,
            });
    }
}

fn pack_is_rust_crate_mapping(facts: &FactSet, pack_name: &str) -> bool {
    facts.external_library_packs.iter().rev().any(|pack| {
        pack.pack_name == pack_name && pack.mode == ExternalLibraryMode::RustCrateMapping
    })
}

fn push_external_pack_field_fact(
    facts: &mut FactSet,
    diagnostics: &mut DiagnosticBag,
    line_index: usize,
    pack_name: &str,
    type_name: &str,
    field_name: &str,
    coverage: &str,
) {
    let Some(coverage) = parse_external_api_coverage(coverage) else {
        push_external_pack_parse_error(diagnostics, line_index, "invalid field coverage");
        return;
    };
    if coverage == ExternalApiCoverage::Unsupported {
        facts
            .unsupported_external_field_facts
            .push(UnsupportedExternalFieldFact {
                pack_name: pack_name.to_string(),
                field: FieldKey {
                    type_name: type_name.to_string(),
                    field_name: field_name.to_string(),
                },
                reason: "declared unsupported by external pack manifest".to_string(),
                provenance: FactProvenance::ExternalLibraryPack,
            });
        return;
    }
    facts.external_field_facts.push(ExternalFieldFact {
        field: FieldKey {
            type_name: type_name.to_string(),
            field_name: field_name.to_string(),
        },
        field_type: external_pack_modeled_field_type(type_name, field_name),
        provenance: FactProvenance::ExternalLibraryPack,
    });
}

fn push_external_pack_field_nullability_fact(
    facts: &mut FactSet,
    diagnostics: &mut DiagnosticBag,
    line_index: usize,
    type_name: &str,
    field_name: &str,
    state: &str,
) {
    let Some(state) = parse_nullability_state(state) else {
        push_external_pack_parse_error(diagnostics, line_index, "invalid field nullability state");
        return;
    };
    facts.field_nullability_facts.push(FieldNullabilityFact {
        field: FieldKey {
            type_name: type_name.to_string(),
            field_name: field_name.to_string(),
        },
        state,
        provenance: FieldNullabilityProvenance::ExternalLibraryPack,
    });
}

fn external_pack_modeled_field_type(type_name: &str, field_name: &str) -> Option<JavaType> {
    let simple_type_name = type_name.rsplit('.').next().unwrap_or(type_name);
    if matches!(
        simple_type_name,
        "TruthAndCallStates" | "GenotypeConcordanceStates.TruthAndCallStates"
    ) {
        return match field_name {
            "callState" => Some(JavaType::Named {
                name: "CallState".to_string(),
                args: Vec::new(),
            }),
            "truthState" => Some(JavaType::Named {
                name: "TruthState".to_string(),
                args: Vec::new(),
            }),
            _ => None,
        };
    }
    if simple_type_name == "VariantContext" && field_name == "Type" {
        return Some(JavaType::Named {
            name: "Type".to_string(),
            args: Vec::new(),
        });
    }
    if simple_type_name == "Type"
        && matches!(
            field_name,
            "SNP" | "MNP" | "SYMBOLIC" | "INDEL" | "NO_VARIATION"
        )
    {
        return Some(JavaType::Named {
            name: "Type".to_string(),
            args: Vec::new(),
        });
    }
    if simple_type_name == "DuplicationMetrics" {
        return match field_name {
            "LIBRARY" => Some(JavaType::String),
            "UNPAIRED_READS_EXAMINED"
            | "READ_PAIRS_EXAMINED"
            | "SECONDARY_OR_SUPPLEMENTARY_RDS"
            | "UNMAPPED_READS"
            | "UNPAIRED_READ_DUPLICATES"
            | "READ_PAIR_DUPLICATES"
            | "READ_PAIR_OPTICAL_DUPLICATES"
            | "ESTIMATED_LIBRARY_SIZE" => Some(JavaType::Primitive(PrimitiveType::Long)),
            "PERCENT_DUPLICATION" => Some(JavaType::Primitive(PrimitiveType::Double)),
            _ => None,
        };
    }
    if simple_type_name == "ReadEnds" && matches!(field_name, "FF" | "FR" | "RF" | "RR" | "F" | "R")
    {
        return Some(JavaType::Primitive(PrimitiveType::Byte));
    }
    if matches!(simple_type_name, "SAMTagAndValue" | "SAMBinaryTagAndValue") {
        return match field_name {
            "tag" => Some(JavaType::String),
            "value" => Some(JavaType::Named {
                name: "Object".to_string(),
                args: Vec::new(),
            }),
            _ => None,
        };
    }
    if simple_type_name == "GenotypeLikelihoodsAllelePair" {
        return match field_name {
            "alleleIndex1" | "alleleIndex2" => Some(JavaType::Primitive(PrimitiveType::Int)),
            _ => None,
        };
    }
    if simple_type_name == "Allele" {
        return match field_name {
            "NON_REF_STRING" | "UNSPECIFIED_ALTERNATE_ALLELE_STRING" => Some(JavaType::String),
            "NO_CALL"
            | "NON_REF_ALLELE"
            | "SPAN_DEL"
            | "SV_SIMPLE_DEL"
            | "SV_SIMPLE_DUP"
            | "UNSPECIFIED_ALTERNATE_ALLELE" => Some(JavaType::Named {
                name: "Allele".to_string(),
                args: Vec::new(),
            }),
            _ => None,
        };
    }
    if simple_type_name == "GenotypeLikelihoods"
        && field_name == "MAX_DIPLOID_ALT_ALLELES_THAT_CAN_BE_GENOTYPED"
    {
        return Some(JavaType::Primitive(PrimitiveType::Int));
    }
    if simple_type_name == "GenotypesContext" && field_name == "NO_GENOTYPES" {
        return Some(JavaType::Named {
            name: "GenotypesContext".to_string(),
            args: Vec::new(),
        });
    }
    if simple_type_name == "JexlVCMatchExp" {
        return match field_name {
            "name" => Some(JavaType::String),
            "exp" => Some(JavaType::Named {
                name: "Expression".to_string(),
                args: Vec::new(),
            }),
            _ => None,
        };
    }
    if simple_type_name == "JexlMissingValueTreatment"
        && matches!(field_name, "TREAT_AS_MATCH" | "TREAT_AS_MISMATCH")
    {
        return Some(JavaType::Named {
            name: "JexlMissingValueTreatment".to_string(),
            args: Vec::new(),
        });
    }
    if simple_type_name == "GoalType" && matches!(field_name, "MAXIMIZE" | "MINIMIZE") {
        return Some(JavaType::Named {
            name: "GoalType".to_string(),
            args: Vec::new(),
        });
    }
    if matches!(simple_type_name, "CommonInfo" | "VariantContext")
        && field_name == "NO_LOG10_PERROR"
    {
        return Some(JavaType::Primitive(PrimitiveType::Double));
    }
    if simple_type_name == "Genotype" && field_name == "UNPHASED_ALLELE_SEPARATOR" {
        return Some(JavaType::Primitive(PrimitiveType::Char));
    }
    if simple_type_name == "Type" && matches!(field_name, "INDEL" | "NO_VARIATION") {
        return Some(JavaType::Named {
            name: "Type".to_string(),
            args: Vec::new(),
        });
    }
    if simple_type_name == "BwaMemAligner" && field_name == "MEM_F_ALL" {
        return Some(JavaType::Primitive(crate::java_ir::PrimitiveType::Int));
    }
    if simple_type_name == "SystemUtils" {
        return match field_name {
            "OS_ARCH" | "OS_NAME" => Some(JavaType::String),
            "IS_OS_MAC" | "IS_OS_LINUX" | "IS_OS_WINDOWS" => {
                Some(JavaType::Primitive(crate::java_ir::PrimitiveType::Boolean))
            }
            _ => None,
        };
    }
    if simple_type_name == "Defaults" {
        return match field_name {
            "COMPRESSION_LEVEL" => Some(JavaType::Primitive(PrimitiveType::Int)),
            "USE_ASYNC_IO_READ_FOR_SAMTOOLS"
            | "USE_ASYNC_IO_WRITE_FOR_SAMTOOLS"
            | "USE_ASYNC_IO_WRITE_FOR_TRIBBLE" => Some(JavaType::Primitive(PrimitiveType::Boolean)),
            _ => None,
        };
    }
    if simple_type_name == "FeatureCodecHeader" && field_name == "NO_HEADER_END" {
        return Some(JavaType::Primitive(PrimitiveType::Long));
    }
    if simple_type_name == "SAMReadGroupRecord"
        && matches!(field_name, "READ_GROUP_ID_TAG" | "READ_GROUP_SAMPLE_TAG")
    {
        return Some(JavaType::String);
    }
    if simple_type_name == "MessageDigestAlgorithms" && field_name == "SHA_256" {
        return Some(JavaType::String);
    }
    if simple_type_name == "GenotypeConcordanceSummaryMetrics"
        && matches!(
            field_name,
            "GENOTYPE_CONCORDANCE"
                | "NON_REF_GENOTYPE_CONCORDANCE"
                | "HET_PPV"
                | "HET_SENSITIVITY"
                | "HOMVAR_PPV"
                | "HOMVAR_SENSITIVITY"
                | "VAR_PPV"
                | "VAR_SENSITIVITY"
                | "VAR_SPECIFICITY"
        )
    {
        return Some(JavaType::String);
    }
    if simple_type_name == "GenotypeConcordance" && field_name == "CONTINGENCY_STATE_TAG" {
        return Some(JavaType::String);
    }
    if simple_type_name == "VCFConstants" {
        return match field_name {
            "PASSES_FILTERS_v4"
            | "MISSING_VALUE_v4"
            | "EMPTY_ID_FIELD"
            | "EMPTY_INFO_FIELD"
            | "GENOTYPE_ALLELE_DEPTHS"
            | "SVTYPE"
            | "NULL_ALLELE"
            | "UNFILTERED"
            | "DEPTH_KEY"
            | "ALLELE_NUMBER_KEY"
            | "ALLELE_COUNT_KEY"
            | "ALLELE_FREQUENCY_KEY"
            | "END_KEY"
            | "GENOTYPE_KEY"
            | "RMS_MAPPING_QUALITY_KEY"
            | "DBSNP_KEY"
            | "GENOTYPE_FILTER_KEY"
            | "GENOTYPE_PL_KEY"
            | "GENOTYPE_POSTERIORS_KEY"
            | "ID_FIELD_SEPARATOR"
            | "INFO_FIELD_ARRAY_SEPARATOR" => Some(JavaType::String),
            "INFO_FIELD_ARRAY_SEPARATOR_CHAR" => Some(JavaType::Primitive(PrimitiveType::Char)),
            "MAX_GENOTYPE_QUAL" => Some(JavaType::Primitive(PrimitiveType::Int)),
            _ => None,
        };
    }
    if simple_type_name == "TemplateProperties"
        && matches!(
            field_name,
            "ARGUMENTS_POSITIONAL" | "ARGUMENT_DEFAULT_VALUE" | "ARGUMENT_TYPE"
        )
    {
        return Some(JavaType::String);
    }
    if simple_type_name == "SpecialArgumentsCollection" && field_name == "SHOW_HIDDEN" {
        return Some(JavaType::Primitive(PrimitiveType::Boolean));
    }
    if simple_type_name == "SAMUtils" && field_name == "MAX_PHRED_SCORE" {
        return Some(JavaType::Primitive(PrimitiveType::Int));
    }
    if simple_type_name == "ReservedTagConstants"
        && matches!(field_name, "XT" | "PROGRAM_GROUP_ID" | "READ_GROUP_ID")
    {
        return Some(JavaType::String);
    }
    if simple_type_name == "BlockCompressedStreamConstants" {
        return match field_name {
            "BLOCK_HEADER_LENGTH" | "BLOCK_LENGTH_OFFSET" | "DEFAULT_COMPRESSION_LEVEL" => {
                Some(JavaType::Primitive(PrimitiveType::Int))
            }
            "EMPTY_GZIP_BLOCK" => Some(JavaType::Array(Box::new(JavaType::Primitive(
                PrimitiveType::Byte,
            )))),
            _ => None,
        };
    }
    if simple_type_name == "FastqConstants"
        && matches!(field_name, "FIRST_OF_PAIR" | "SECOND_OF_PAIR")
    {
        return Some(JavaType::String);
    }
    if simple_type_name == "SequenceUtil" && field_name == "VALID_BASES_UPPER" {
        return Some(JavaType::Array(Box::new(JavaType::Primitive(
            PrimitiveType::Byte,
        ))));
    }
    if simple_type_name == "BAMFileConstants" && field_name == "FIXED_BLOCK_SIZE" {
        return Some(JavaType::Primitive(PrimitiveType::Int));
    }
    if simple_type_name == "GenotypeConcordanceStates" {
        return match field_name {
            "CallState" => Some(JavaType::Named {
                name: "CallState".to_string(),
                args: Vec::new(),
            }),
            "TruthState" => Some(JavaType::Named {
                name: "TruthState".to_string(),
                args: Vec::new(),
            }),
            _ => None,
        };
    }
    if simple_type_name == "CallState" && matches!(field_name, "HOM_REF" | "HET_REF_VAR1") {
        return Some(JavaType::Named {
            name: "CallState".to_string(),
            args: Vec::new(),
        });
    }
    if simple_type_name == "TruthState" && matches!(field_name, "HOM_REF" | "NO_CALL") {
        return Some(JavaType::Named {
            name: "TruthState".to_string(),
            args: Vec::new(),
        });
    }
    if simple_type_name == "SAMSequenceRecord" && field_name == "UNKNOWN_SEQUENCE_LENGTH" {
        return Some(JavaType::Primitive(crate::java_ir::PrimitiveType::Int));
    }
    if simple_type_name == "SAMFileHeader" {
        return match field_name {
            "SortOrder" => Some(JavaType::Named {
                name: "SortOrder".to_string(),
                args: Vec::new(),
            }),
            "GroupOrder" => Some(JavaType::Named {
                name: "GroupOrder".to_string(),
                args: Vec::new(),
            }),
            _ => None,
        };
    }
    if simple_type_name == "SortOrder" && is_sam_sort_order_modeled_field(field_name) {
        return Some(JavaType::Named {
            name: "SortOrder".to_string(),
            args: Vec::new(),
        });
    }
    if simple_type_name == "GroupOrder" && is_sam_group_order_modeled_field(field_name) {
        return Some(JavaType::Named {
            name: "GroupOrder".to_string(),
            args: Vec::new(),
        });
    }
    if simple_type_name == "CigarOperator" && is_cigar_operator_modeled_field(field_name) {
        return Some(JavaType::Named {
            name: "CigarOperator".to_string(),
            args: Vec::new(),
        });
    }
    if simple_type_name == "SAMRecord" {
        return sam_record_modeled_field_type(field_name);
    }
    if simple_type_name == "SBIIndexWriter" && field_name == "DEFAULT_GRANULARITY" {
        return Some(JavaType::Primitive(crate::java_ir::PrimitiveType::Long));
    }
    if simple_type_name == "DataSeriesType" && field_name == "BYTE_ARRAY" {
        return Some(JavaType::Named {
            name: "DataSeriesType".to_string(),
            args: Vec::new(),
        });
    }
    if simple_type_name == "DataFormat" && matches!(field_name, "AVRO" | "ARROW") {
        return Some(JavaType::Named {
            name: "DataFormat".to_string(),
            args: Vec::new(),
        });
    }
    if simple_type_name == "MapperFeature" && field_name == "ACCEPT_CASE_INSENSITIVE_PROPERTIES" {
        return Some(JavaType::Named {
            name: "MapperFeature".to_string(),
            args: Vec::new(),
        });
    }
    if simple_type_name == "DeserializationFeature" && field_name == "UNWRAP_ROOT_VALUE" {
        return Some(JavaType::Named {
            name: "DeserializationFeature".to_string(),
            args: Vec::new(),
        });
    }
    if simple_type_name == "SamReaderFactory" && field_name == "Option" {
        return Some(JavaType::Named {
            name: "Option".to_string(),
            args: Vec::new(),
        });
    }
    if matches!(
        type_name,
        "htsjdk.samtools.SamReaderFactory.Option" | "SamReaderFactory.Option"
    ) && matches!(
        field_name,
        "CACHE_FILE_BASED_INDEXES" | "INCLUDE_SOURCE_IN_RECORDS"
    ) {
        return Some(JavaType::Named {
            name: "Option".to_string(),
            args: Vec::new(),
        });
    }
    if simple_type_name == "ValidationStringency"
        && matches!(field_name, "LENIENT" | "SILENT" | "STRICT")
    {
        return Some(JavaType::Named {
            name: "ValidationStringency".to_string(),
            args: Vec::new(),
        });
    }
    if simple_type_name == "IndexFactory" {
        return match field_name {
            "IndexType" => Some(JavaType::Named {
                name: "IndexType".to_string(),
                args: Vec::new(),
            }),
            "IndexBalanceApproach" => Some(JavaType::Named {
                name: "IndexBalanceApproach".to_string(),
                args: Vec::new(),
            }),
            _ => None,
        };
    }
    if simple_type_name == "IndexType" && field_name == "TABIX" {
        return Some(JavaType::Named {
            name: "IndexType".to_string(),
            args: Vec::new(),
        });
    }
    if simple_type_name == "IndexBalanceApproach" && field_name == "FOR_SEEK_TIME" {
        return Some(JavaType::Named {
            name: "IndexBalanceApproach".to_string(),
            args: Vec::new(),
        });
    }
    if simple_type_name == "TabixFormat" && matches!(field_name, "GENERIC_FLAGS" | "ZERO_BASED") {
        return Some(JavaType::Primitive(PrimitiveType::Int));
    }
    if simple_type_name == "Percentile" && field_name == "EstimationType" {
        return Some(JavaType::Named {
            name: "EstimationType".to_string(),
            args: Vec::new(),
        });
    }
    if simple_type_name == "EstimationType" && field_name == "R_1" {
        return Some(JavaType::Named {
            name: "EstimationType".to_string(),
            args: Vec::new(),
        });
    }
    if simple_type_name == "FieldValue" && field_name == "Attribute" {
        return Some(JavaType::Named {
            name: "Attribute".to_string(),
            args: Vec::new(),
        });
    }
    if matches!(
        type_name,
        "com.google.cloud.bigquery.FieldValue.Attribute" | "FieldValue.Attribute"
    ) && field_name == "PRIMITIVE"
    {
        return Some(JavaType::Named {
            name: "Attribute".to_string(),
            args: Vec::new(),
        });
    }
    if simple_type_name == "QueryJobConfiguration" && field_name == "Priority" {
        return Some(JavaType::Named {
            name: "Priority".to_string(),
            args: Vec::new(),
        });
    }
    if matches!(
        type_name,
        "com.google.cloud.bigquery.QueryJobConfiguration.Priority"
            | "QueryJobConfiguration.Priority"
    ) && matches!(field_name, "BATCH" | "INTERACTIVE")
    {
        return Some(JavaType::Named {
            name: "Priority".to_string(),
            args: Vec::new(),
        });
    }
    if simple_type_name == "Storage" && field_name == "SignUrlOption" {
        return Some(JavaType::Named {
            name: "SignUrlOption".to_string(),
            args: Vec::new(),
        });
    }
    if simple_type_name == "CloudStorageFileSystem" && field_name == "URI_SCHEME" {
        return Some(JavaType::String);
    }
    if simple_type_name == "CloudStorageConfiguration" && field_name == "DEFAULT" {
        return Some(JavaType::Named {
            name: "CloudStorageConfiguration".to_string(),
            args: Vec::new(),
        });
    }
    if simple_type_name == "ReadsFormatWriteOption" && field_name == "BAM" {
        return Some(JavaType::Named {
            name: "ReadsFormatWriteOption".to_string(),
            args: Vec::new(),
        });
    }
    if simple_type_name == "FileCardinalityWriteOption"
        && matches!(field_name, "MULTIPLE" | "SINGLE")
    {
        return Some(JavaType::Named {
            name: "FileCardinalityWriteOption".to_string(),
            args: Vec::new(),
        });
    }
    if simple_type_name == "PDHMMNativeArguments" && field_name == "avxLevel" {
        return Some(JavaType::Named {
            name: "AVXLevel".to_string(),
            args: Vec::new(),
        });
    }
    if simple_type_name == "PDHMMNativeArguments" && field_name == "openMPSetting" {
        return Some(JavaType::Named {
            name: "OpenMPSetting".to_string(),
            args: Vec::new(),
        });
    }
    if matches!(
        (simple_type_name, field_name),
        ("PDHMMNativeArguments", "maxNumberOfThreads")
            | ("PairHMMNativeArguments", "maxNumberOfThreads")
    ) {
        return Some(JavaType::Primitive(crate::java_ir::PrimitiveType::Int));
    }
    if simple_type_name == "PairHMMNativeArguments" && field_name == "useDoublePrecision" {
        return Some(JavaType::Primitive(crate::java_ir::PrimitiveType::Boolean));
    }
    if simple_type_name == "ReadDataHolder"
        && matches!(
            field_name,
            "readBases" | "readQuals" | "insertionGOP" | "deletionGOP" | "overallGCP"
        )
    {
        return Some(JavaType::Array(Box::new(JavaType::Primitive(
            PrimitiveType::Byte,
        ))));
    }
    if simple_type_name == "HaplotypeDataHolder"
        && matches!(field_name, "haplotypeBases" | "haplotypePDBases")
    {
        return Some(JavaType::Array(Box::new(JavaType::Primitive(
            PrimitiveType::Byte,
        ))));
    }
    if simple_type_name == "BatchCompletionCallbackFunctionArgument"
        && matches!(field_name, "batchCount" | "totalBatchCount")
    {
        return Some(JavaType::Primitive(PrimitiveType::Int));
    }
    if simple_type_name == "SWNativeAlignerResult" {
        return match field_name {
            "alignment_offset" => Some(JavaType::Primitive(PrimitiveType::Int)),
            "cigar" => Some(JavaType::String),
            _ => None,
        };
    }
    if simple_type_name == "Log" && field_name == "LogLevel" {
        return Some(JavaType::Named {
            name: "LogLevel".to_string(),
            args: Vec::new(),
        });
    }
    if simple_type_name == "LogLevel"
        && matches!(field_name, "ERROR" | "WARNING" | "INFO" | "DEBUG")
    {
        return Some(JavaType::Named {
            name: "LogLevel".to_string(),
            args: Vec::new(),
        });
    }
    if simple_type_name == "Coordinates" {
        return match field_name {
            "GenomicsDBColumn" => Some(JavaType::Named {
                name: "GenomicsDBColumn".to_string(),
                args: Vec::new(),
            }),
            "ContigPosition" => Some(JavaType::Named {
                name: "ContigPosition".to_string(),
                args: Vec::new(),
            }),
            _ => None,
        };
    }
    if simple_type_name == "GenomicsDBExportConfiguration" && field_name == "ExportConfiguration" {
        return Some(JavaType::Named {
            name: "ExportConfiguration".to_string(),
            args: Vec::new(),
        });
    }
    if simple_type_name == "GenomicsDBImportConfiguration" {
        return match field_name {
            "ImportConfiguration" => Some(JavaType::Named {
                name: "ImportConfiguration".to_string(),
                args: Vec::new(),
            }),
            "Partition" => Some(JavaType::Named {
                name: "Partition".to_string(),
                args: Vec::new(),
            }),
            _ => None,
        };
    }
    if simple_type_name == "GenomicsDBVidMapProto" {
        return match field_name {
            "VidMappingPB" => Some(JavaType::Named {
                name: "VidMappingPB".to_string(),
                args: Vec::new(),
            }),
            "FieldLengthDescriptorComponentPB" => Some(JavaType::Named {
                name: "FieldLengthDescriptorComponentPB".to_string(),
                args: Vec::new(),
            }),
            _ => None,
        };
    }
    if simple_type_name == "ReadSession" && field_name == "TableReadOptions" {
        return Some(JavaType::Named {
            name: "TableReadOptions".to_string(),
            args: Vec::new(),
        });
    }
    if simple_type_name == "SamPairUtil" && field_name == "PairOrientation" {
        return Some(JavaType::Named {
            name: "PairOrientation".to_string(),
            args: Vec::new(),
        });
    }
    if simple_type_name == "PairOrientation" && is_pair_orientation_modeled_field(field_name) {
        return Some(JavaType::Named {
            name: "PairOrientation".to_string(),
            args: Vec::new(),
        });
    }
    if simple_type_name == "TarArchiveOutputStream"
        && matches!(field_name, "BIGNUMBER_STAR" | "LONGFILE_GNU")
    {
        return Some(JavaType::Primitive(PrimitiveType::Int));
    }
    if simple_type_name == "NullOutputStream"
        && matches!(field_name, "INSTANCE" | "NULL_OUTPUT_STREAM")
    {
        return Some(JavaType::Named {
            name: "OutputStream".to_string(),
            args: Vec::new(),
        });
    }
    if simple_type_name == "NullWriter" && field_name == "INSTANCE" {
        return Some(JavaType::Named {
            name: "Writer".to_string(),
            args: Vec::new(),
        });
    }
    if simple_type_name == "BlockCompressedInputStream" && field_name == "FileTermination" {
        return Some(JavaType::Named {
            name: "FileTermination".to_string(),
            args: Vec::new(),
        });
    }
    if simple_type_name == "FileTermination" && is_file_termination_modeled_field(field_name) {
        return Some(JavaType::Named {
            name: "FileTermination".to_string(),
            args: Vec::new(),
        });
    }
    if simple_type_name == "HttpFileSystemProviderSettings" && field_name == "DEFAULT_SETTINGS" {
        return Some(JavaType::Named {
            name: "HttpFileSystemProviderSettings".to_string(),
            args: Vec::new(),
        });
    }
    if matches!(
        simple_type_name,
        "HttpFileSystemProvider" | "HttpsFileSystemProvider"
    ) && field_name == "SCHEME"
    {
        return Some(JavaType::String);
    }
    if simple_type_name == "RetryHandler"
        && matches!(
            field_name,
            "DEFAULT_RETRYABLE_HTTP_CODES"
                | "DEFAULT_RETRYABLE_EXCEPTIONS"
                | "DEFALT_RETRYABLE_MESSAGES"
        )
    {
        return Some(JavaType::Named {
            name: "Object".to_string(),
            args: Vec::new(),
        });
    }
    if simple_type_name == "HDF5File" && field_name == "OpenMode" {
        return Some(JavaType::Named {
            name: "OpenMode".to_string(),
            args: Vec::new(),
        });
    }
    if matches!(simple_type_name, "OpenMode") && matches!(field_name, "READ_ONLY" | "CREATE") {
        return Some(JavaType::Named {
            name: "OpenMode".to_string(),
            args: Vec::new(),
        });
    }
    if simple_type_name == "VCFHeader" {
        return match field_name {
            "HEADER_FIELDS" => Some(JavaType::Named {
                name: "HEADER_FIELDS".to_string(),
                args: Vec::new(),
            }),
            "REFERENCE_KEY" => Some(JavaType::String),
            _ => None,
        };
    }
    if simple_type_name == "VCFCompoundHeaderLine" && field_name == "SupportedHeaderLineType" {
        return Some(JavaType::Named {
            name: "SupportedHeaderLineType".to_string(),
            args: Vec::new(),
        });
    }
    if simple_type_name == "SupportedHeaderLineType" && matches!(field_name, "FORMAT" | "INFO") {
        return Some(JavaType::Named {
            name: "SupportedHeaderLineType".to_string(),
            args: Vec::new(),
        });
    }
    if simple_type_name == "VariantContextWriterBuilder" && field_name == "OutputType" {
        return Some(JavaType::Named {
            name: "OutputType".to_string(),
            args: Vec::new(),
        });
    }
    if simple_type_name == "VariantContextWriterBuilder" && field_name == "DEFAULT_OPTIONS" {
        return Some(JavaType::Named {
            name: "EnumSet".to_string(),
            args: vec![JavaType::Named {
                name: "Options".to_string(),
                args: Vec::new(),
            }],
        });
    }
    if simple_type_name == "Options"
        && matches!(
            field_name,
            "INDEX_ON_THE_FLY" | "DO_NOT_WRITE_GENOTYPES" | "ALLOW_MISSING_FIELDS_IN_HEADER"
        )
    {
        return Some(JavaType::Named {
            name: "Options".to_string(),
            args: Vec::new(),
        });
    }
    if simple_type_name == "MarkDuplicates" && field_name == "DuplicateTaggingPolicy" {
        return Some(JavaType::Named {
            name: "DuplicateTaggingPolicy".to_string(),
            args: Vec::new(),
        });
    }
    if simple_type_name == "DuplicateTaggingPolicy"
        && matches!(field_name, "DontTag" | "OpticalOnly" | "All")
    {
        return Some(JavaType::Named {
            name: "DuplicateTaggingPolicy".to_string(),
            args: Vec::new(),
        });
    }
    crate::stdlib::known_external_field_type_for_receiver(
        &JavaType::Named {
            name: type_name.to_string(),
            args: Vec::new(),
        },
        field_name,
    )
}

fn is_sam_sort_order_modeled_field(field_name: &str) -> bool {
    matches!(
        field_name,
        "coordinate" | "queryname" | "unsorted" | "unknown" | "duplicate"
    )
}

fn is_sam_group_order_modeled_field(field_name: &str) -> bool {
    matches!(field_name, "none" | "query")
}

fn is_pair_orientation_modeled_field(field_name: &str) -> bool {
    matches!(field_name, "FR" | "RF" | "TANDEM")
}

fn is_file_termination_modeled_field(field_name: &str) -> bool {
    matches!(field_name, "DEFECTIVE" | "HAS_TERMINATOR_BLOCK")
}

fn sam_record_modeled_field_type(field_name: &str) -> Option<JavaType> {
    match field_name {
        "NO_ALIGNMENT_CIGAR" => Some(JavaType::Named {
            name: "Cigar".to_string(),
            args: Vec::new(),
        }),
        "NO_ALIGNMENT_REFERENCE_NAME" | "NULL_QUALS_STRING" => Some(JavaType::String),
        "NULL_QUALS" | "NULL_SEQUENCE" => Some(JavaType::Array(Box::new(JavaType::Primitive(
            crate::java_ir::PrimitiveType::Byte,
        )))),
        "NO_ALIGNMENT_START"
        | "NO_ALIGNMENT_REFERENCE_INDEX"
        | "UNKNOWN_MAPPING_QUALITY"
        | "NO_MAPPING_QUALITY" => Some(JavaType::Primitive(crate::java_ir::PrimitiveType::Int)),
        _ => None,
    }
}

fn is_cigar_operator_modeled_field(field_name: &str) -> bool {
    matches!(
        field_name,
        "M" | "I"
            | "D"
            | "N"
            | "S"
            | "H"
            | "P"
            | "EQ"
            | "X"
            | "MATCH"
            | "MATCH_OR_MISMATCH"
            | "INSERTION"
            | "DELETION"
            | "SKIPPED_REGION"
            | "SOFT_CLIP"
            | "HARD_CLIP"
            | "PADDING"
    )
}

fn push_external_pack_object_strategy_fact(
    facts: &mut FactSet,
    diagnostics: &mut DiagnosticBag,
    line_index: usize,
    type_name: &str,
    strategy: &str,
    metadata: &[&str],
) {
    let Some(strategy) =
        parse_external_pack_object_strategy(diagnostics, line_index, strategy, metadata)
    else {
        return;
    };
    facts.hierarchy_overrides.push(HierarchyOverride {
        type_name: type_name.to_string(),
        strategy,
        provenance: FactProvenance::ExternalLibraryPack,
    });
}

fn push_external_pack_closed_hierarchy_fact(
    facts: &mut FactSet,
    diagnostics: &mut DiagnosticBag,
    line_index: usize,
    type_name: &str,
    closed: &str,
) {
    let Some(closed) = parse_external_pack_closed_bool(closed) else {
        push_external_pack_parse_error(
            diagnostics,
            line_index,
            "external-pack-closed value must be true, false, closed, or open",
        );
        return;
    };
    facts.closed_hierarchy_facts.push(ClosedHierarchyFact {
        type_name: type_name.to_string(),
        closed,
        provenance: FactProvenance::ExternalLibraryPack,
    });
}

fn push_external_pack_type_capability_fact(
    facts: &mut FactSet,
    diagnostics: &mut DiagnosticBag,
    line_index: usize,
    type_name: &str,
    metadata: &[&str],
) {
    let Some(capability) =
        parse_external_pack_type_capability(diagnostics, line_index, type_name, metadata)
    else {
        return;
    };
    facts.type_capability_overrides.push(capability);
}

fn push_external_pack_owned_recovery_fact(
    facts: &mut FactSet,
    diagnostics: &mut DiagnosticBag,
    line_index: usize,
    type_name: &str,
    kind: &str,
    metadata: &[&str],
) {
    let Some(capability) = parse_external_pack_owned_recovery_capability(
        diagnostics,
        line_index,
        type_name,
        kind,
        metadata,
    ) else {
        return;
    };
    facts
        .owned_recovery_capability_facts
        .push(OwnedRecoveryCapabilityFact {
            type_name: type_name.to_string(),
            capability,
            provenance: FactProvenance::ExternalLibraryPack,
        });
}

fn push_external_pack_object_safety_fact(
    facts: &mut FactSet,
    diagnostics: &mut DiagnosticBag,
    line_index: usize,
    type_name: &str,
    violation_kind: &str,
    metadata: &[&str],
) {
    let Some(violation) = parse_external_pack_object_safety_violation(
        diagnostics,
        line_index,
        violation_kind,
        metadata,
    ) else {
        return;
    };
    push_external_pack_object_safety_violation(facts, type_name, violation);
}

fn push_external_pack_object_safety_violation(
    facts: &mut FactSet,
    type_name: &str,
    violation: ObjectSafetyViolation,
) {
    if let Some(fact) = facts.object_safety_facts.iter_mut().rev().find(|fact| {
        fact.type_name == type_name && fact.provenance == FactProvenance::ExternalLibraryPack
    }) {
        fact.object_safe = false;
        fact.violations.push(violation);
    } else {
        facts.object_safety_facts.push(ObjectSafetyFact {
            type_name: type_name.to_string(),
            object_safe: false,
            violations: vec![violation],
            provenance: FactProvenance::ExternalLibraryPack,
        });
    }

    let mut capability = empty_capability(type_name);
    capability.object_safe_trait = Some(false);
    facts.type_capability_overrides.push(capability);
}

fn push_external_pack_argument_effect_fact(
    facts: &mut FactSet,
    diagnostics: &mut DiagnosticBag,
    line_index: usize,
    receiver_type: &str,
    method_name: &str,
    parameter_index: &str,
    parameter_nullability: &str,
    argument_mutability: &str,
    alias_policy: &str,
    metadata: &[&str],
) {
    let Ok(parameter_index) = parameter_index.parse::<usize>() else {
        push_external_pack_parse_error(diagnostics, line_index, "invalid parameter index");
        return;
    };
    let Some(parameter_nullability) = parse_nullability_state(parameter_nullability) else {
        push_external_pack_parse_error(
            diagnostics,
            line_index,
            "invalid argument parameter nullability",
        );
        return;
    };
    let Some(argument_mutability) = parse_receiver_mutability(argument_mutability) else {
        push_external_pack_parse_error(diagnostics, line_index, "invalid argument mutability");
        return;
    };
    let Some(alias_policy) = parse_external_argument_alias_policy(alias_policy) else {
        push_external_pack_parse_error(diagnostics, line_index, "invalid argument alias policy");
        return;
    };
    let Some(parameter_type) = parse_external_argument_metadata(diagnostics, line_index, metadata)
    else {
        return;
    };
    facts
        .external_api_argument_effect_facts
        .push(ExternalApiArgumentEffectFact {
            method_name: method_name.to_string(),
            receiver_type: Some(receiver_type.to_string()),
            parameter_index,
            parameter_type,
            parameter_nullability,
            argument_mutability,
            alias_policy,
            provenance: FactProvenance::ExternalLibraryPack,
        });
}

fn push_external_pack_exception_fact(
    facts: &mut FactSet,
    diagnostics: &mut DiagnosticBag,
    line_index: usize,
    type_name: &str,
    kind: &str,
) {
    let Some(kind) = parse_exception_kind(kind) else {
        push_external_pack_parse_error(diagnostics, line_index, "invalid exception kind");
        return;
    };
    facts.exception_facts.push(ExceptionFact {
        type_name: type_name.to_string(),
        kind,
        provenance: FactProvenance::ExternalLibraryPack,
    });
}

fn parse_external_library_mode(value: &str) -> Option<ExternalLibraryMode> {
    match value {
        "external-fact-model" | "external_fact_model" => {
            Some(ExternalLibraryMode::ExternalFactModel)
        }
        "rust-crate-mapping" | "rust_crate_mapping" => Some(ExternalLibraryMode::RustCrateMapping),
        "ffi-boundary" | "ffi_boundary" => Some(ExternalLibraryMode::FfiBoundary),
        "conservative-boundary" | "conservative_boundary" => {
            Some(ExternalLibraryMode::ConservativeBoundary)
        }
        _ => None,
    }
}

fn parse_external_pack_type_capability(
    diagnostics: &mut DiagnosticBag,
    line_index: usize,
    type_name: &str,
    metadata: &[&str],
) -> Option<TypeCapabilityOverride> {
    let mut capability = empty_capability(type_name);
    let mut saw_metadata = false;
    for token in metadata {
        saw_metadata = true;
        if let Some(value) = token.strip_prefix("copy=") {
            capability.copy = Some(parse_bool_metadata(diagnostics, line_index, "copy", value)?);
        } else if let Some(value) = token.strip_prefix("clone=") {
            capability.clone = Some(parse_bool_metadata(
                diagnostics,
                line_index,
                "clone",
                value,
            )?);
        } else if let Some(value) = token.strip_prefix("string-like=") {
            capability.string_like = Some(parse_bool_metadata(
                diagnostics,
                line_index,
                "string-like",
                value,
            )?);
        } else if let Some(value) = token.strip_prefix("sequence=") {
            if value.is_empty() {
                push_external_pack_parse_error(
                    diagnostics,
                    line_index,
                    "external-pack-capability sequence metadata must name an element type",
                );
                return None;
            }
            capability.sequence_like = Some(parse_type(value));
        } else if let Some(value) = token.strip_prefix("implements=") {
            capability.implements =
                parse_type_list_metadata(diagnostics, line_index, "implements", value)?;
        } else if let Some(value) = token.strip_prefix("extends=") {
            if value.is_empty() {
                push_external_pack_parse_error(
                    diagnostics,
                    line_index,
                    "external-pack-capability extends metadata must name a type",
                );
                return None;
            }
            capability.extends = Some(parse_type(value));
        } else if let Some(value) = token.strip_prefix("final=") {
            capability.final_type = Some(parse_bool_metadata(
                diagnostics,
                line_index,
                "final",
                value,
            )?);
        } else if let Some(value) = token.strip_prefix("object-safe=") {
            capability.object_safe_trait = Some(parse_bool_metadata(
                diagnostics,
                line_index,
                "object-safe",
                value,
            )?);
        } else {
            push_external_pack_parse_error(
                diagnostics,
                line_index,
                "unrecognized external-pack-capability metadata",
            );
            return None;
        }
    }
    if !saw_metadata {
        push_external_pack_parse_error(
            diagnostics,
            line_index,
            "external-pack-capability requires at least one metadata assignment",
        );
        return None;
    }
    Some(capability)
}

fn parse_external_pack_owned_recovery_capability(
    diagnostics: &mut DiagnosticBag,
    line_index: usize,
    type_name: &str,
    kind: &str,
    metadata: &[&str],
) -> Option<OwnedRecoveryCapability> {
    let java_ty = parse_owned_recovery_java_type(diagnostics, line_index, type_name, metadata)?;
    match kind {
        "copy" => Some(OwnedRecoveryCapability::Copy),
        "explicit-clone" | "explicit_clone" => {
            Some(OwnedRecoveryCapability::ExplicitClone { java_ty })
        }
        "clone-box-adapter" | "clone_box_adapter" => {
            Some(OwnedRecoveryCapability::CloneBoxAdapter { java_ty })
        }
        "factory-copy" | "factory_copy" => Some(OwnedRecoveryCapability::FactoryCopy { java_ty }),
        "immutable-shared-handle" | "immutable_shared_handle" => {
            Some(OwnedRecoveryCapability::ImmutableSharedHandle { java_ty })
        }
        "facade-owned-external-handle" | "facade_owned_external_handle" => {
            Some(OwnedRecoveryCapability::FacadeOwnedExternalHandle { java_ty })
        }
        "ffi-owned-handle" | "ffi_owned_handle" => {
            Some(OwnedRecoveryCapability::FfiOwnedHandle { java_ty })
        }
        "conservative-clone" | "conservative_clone" => {
            Some(OwnedRecoveryCapability::ConservativeClone)
        }
        "non-cloneable" | "non_cloneable" => Some(OwnedRecoveryCapability::NonCloneable),
        _ => {
            push_external_pack_parse_error(
                diagnostics,
                line_index,
                "invalid external-pack-owned-recovery capability kind",
            );
            None
        }
    }
}

fn parse_owned_recovery_java_type(
    diagnostics: &mut DiagnosticBag,
    line_index: usize,
    type_name: &str,
    metadata: &[&str],
) -> Option<JavaType> {
    let mut java_ty = parse_type(type_name);
    for token in metadata {
        if let Some(value) = token.strip_prefix("java-type=") {
            if value.is_empty() {
                push_external_pack_parse_error(
                    diagnostics,
                    line_index,
                    "external-pack-owned-recovery java-type metadata must name a type",
                );
                return None;
            }
            java_ty = parse_type(value);
        } else {
            push_external_pack_parse_error(
                diagnostics,
                line_index,
                "unrecognized external-pack-owned-recovery metadata",
            );
            return None;
        }
    }
    Some(java_ty)
}

fn parse_external_pack_object_safety_violation(
    diagnostics: &mut DiagnosticBag,
    line_index: usize,
    violation_kind: &str,
    metadata: &[&str],
) -> Option<ObjectSafetyViolation> {
    let wrong_arity = |diagnostics: &mut DiagnosticBag, line_index: usize, expected: &str| {
        push_external_pack_parse_error(
            diagnostics,
            line_index,
            &format!("external-pack-object-safety {violation_kind} requires {expected}"),
        );
    };

    match (violation_kind, metadata) {
        ("static-method", [method_name]) => Some(ObjectSafetyViolation::StaticMethod {
            method_name: (*method_name).to_string(),
        }),
        ("self-return", [method_name]) => Some(ObjectSafetyViolation::SelfReferentialReturn {
            method_name: (*method_name).to_string(),
        }),
        ("nested-self-return", [method_name]) => {
            Some(ObjectSafetyViolation::NestedSelfReferentialReturn {
                method_name: (*method_name).to_string(),
            })
        }
        ("self-param", [method_name, param_name]) => {
            Some(ObjectSafetyViolation::SelfReferentialParameter {
                method_name: (*method_name).to_string(),
                param_name: (*param_name).to_string(),
            })
        }
        ("nested-self-param", [method_name, param_name]) => {
            Some(ObjectSafetyViolation::NestedSelfReferentialParameter {
                method_name: (*method_name).to_string(),
                param_name: (*param_name).to_string(),
            })
        }
        ("field", [field_name]) => Some(ObjectSafetyViolation::FieldMember {
            field_name: (*field_name).to_string(),
        }),
        ("constructor", []) => Some(ObjectSafetyViolation::ConstructorMember),
        ("unsupported-member", []) => Some(ObjectSafetyViolation::UnsupportedMember),
        ("static-method" | "self-return" | "nested-self-return" | "field", _) => {
            wrong_arity(diagnostics, line_index, "one member name");
            None
        }
        ("self-param" | "nested-self-param", _) => {
            wrong_arity(diagnostics, line_index, "a method name and parameter name");
            None
        }
        ("constructor" | "unsupported-member", _) => {
            wrong_arity(diagnostics, line_index, "no trailing metadata");
            None
        }
        _ => {
            push_external_pack_parse_error(
                diagnostics,
                line_index,
                "unrecognized external-pack-object-safety violation kind",
            );
            None
        }
    }
}

fn parse_bool_metadata(
    diagnostics: &mut DiagnosticBag,
    line_index: usize,
    field_name: &str,
    value: &str,
) -> Option<bool> {
    match value {
        "true" => Some(true),
        "false" => Some(false),
        _ => {
            push_external_pack_parse_error(
                diagnostics,
                line_index,
                &format!("external-pack-capability {field_name} metadata must be true or false"),
            );
            None
        }
    }
}

fn parse_external_pack_closed_bool(value: &str) -> Option<bool> {
    match value {
        "true" | "yes" | "closed" => Some(true),
        "false" | "no" | "open" => Some(false),
        _ => None,
    }
}

fn parse_type_list_metadata(
    diagnostics: &mut DiagnosticBag,
    line_index: usize,
    field_name: &str,
    value: &str,
) -> Option<Vec<JavaType>> {
    let values = value
        .split(',')
        .filter(|ty| !ty.is_empty())
        .map(parse_type)
        .collect::<Vec<_>>();
    if values.is_empty() {
        push_external_pack_parse_error(
            diagnostics,
            line_index,
            &format!("external-pack-capability {field_name} metadata must name at least one type"),
        );
        return None;
    }
    Some(values)
}

fn parse_external_pack_object_strategy(
    diagnostics: &mut DiagnosticBag,
    line_index: usize,
    strategy: &str,
    metadata: &[&str],
) -> Option<HierarchyStrategy> {
    match strategy {
        "concrete" => {
            if !metadata.is_empty() {
                push_external_pack_parse_error(
                    diagnostics,
                    line_index,
                    "concrete object strategy does not accept metadata",
                );
                return None;
            }
            Some(HierarchyStrategy::ConcreteStruct)
        }
        "erased-object" => {
            if !metadata.is_empty() {
                push_external_pack_parse_error(
                    diagnostics,
                    line_index,
                    "erased-object strategy does not accept metadata",
                );
                return None;
            }
            Some(HierarchyStrategy::ErasedObject)
        }
        "dyn-trait" => {
            let [trait_name] = metadata else {
                push_external_pack_parse_error(
                    diagnostics,
                    line_index,
                    "dyn-trait object strategy requires exactly one trait name",
                );
                return None;
            };
            Some(HierarchyStrategy::DynTrait {
                trait_name: (*trait_name).to_string(),
            })
        }
        "closed-enum" => {
            let [enum_name, variants] = metadata else {
                push_external_pack_parse_error(
                    diagnostics,
                    line_index,
                    "closed-enum object strategy requires enum name and comma-separated variants",
                );
                return None;
            };
            let variants = variants
                .split(',')
                .filter(|variant| !variant.is_empty())
                .map(str::to_string)
                .collect::<Vec<_>>();
            if variants.is_empty() {
                push_external_pack_parse_error(
                    diagnostics,
                    line_index,
                    "closed-enum object strategy requires at least one variant",
                );
                return None;
            }
            Some(HierarchyStrategy::ClosedEnum {
                enum_name: (*enum_name).to_string(),
                variants,
            })
        }
        _ => {
            push_external_pack_parse_error(
                diagnostics,
                line_index,
                "invalid external pack object strategy",
            );
            None
        }
    }
}

fn parse_external_api_coverage(value: &str) -> Option<ExternalApiCoverage> {
    match value {
        "declared" => Some(ExternalApiCoverage::Declared),
        "conservative" => Some(ExternalApiCoverage::Conservative),
        "semantic" => Some(ExternalApiCoverage::Semantic),
        "mapped" => Some(ExternalApiCoverage::Mapped),
        "unsupported" => Some(ExternalApiCoverage::Unsupported),
        _ => None,
    }
}

fn parse_external_api_role(value: &str) -> Option<ExternalApiRole> {
    match value {
        "getter" => Some(ExternalApiRole::Getter),
        "boolean-getter" => Some(ExternalApiRole::BooleanGetter),
        "setter" => Some(ExternalApiRole::Setter),
        "mutator" => Some(ExternalApiRole::Mutator),
        "fluent-mutator" => Some(ExternalApiRole::FluentMutator),
        "side-effect" => Some(ExternalApiRole::SideEffect),
        "indexed-getter" => Some(ExternalApiRole::IndexedGetter),
        "indexed-setter" => Some(ExternalApiRole::IndexedSetter),
        "unknown" => Some(ExternalApiRole::Unknown),
        _ => None,
    }
}

fn parse_exception_kind(value: &str) -> Option<ExceptionKind> {
    match value {
        "checked" => Some(ExceptionKind::Checked),
        "unchecked" => Some(ExceptionKind::Unchecked),
        "unknown" => Some(ExceptionKind::Unknown),
        _ => None,
    }
}

fn parse_receiver_mutability(value: &str) -> Option<ReceiverMutabilityFact> {
    match value {
        "read-only" => Some(ReceiverMutabilityFact::ReadOnly),
        "mutates-receiver" => Some(ReceiverMutabilityFact::MutatesReceiver),
        "unknown" => Some(ReceiverMutabilityFact::Unknown),
        _ => None,
    }
}

fn parse_external_argument_alias_policy(value: &str) -> Option<ExternalArgumentAliasPolicy> {
    match value {
        "unknown" => Some(ExternalArgumentAliasPolicy::Unknown),
        "no-alias" | "no_alias" => Some(ExternalArgumentAliasPolicy::NoAlias),
        "borrowed" => Some(ExternalArgumentAliasPolicy::Borrowed),
        "escapes" => Some(ExternalArgumentAliasPolicy::Escapes),
        _ => None,
    }
}

fn parse_nullability_state(value: &str) -> Option<NullabilityState> {
    match value {
        "definitely-non-null" | "non-null" => Some(NullabilityState::DefinitelyNonNull),
        "maybe-null" | "nullable" => Some(NullabilityState::MaybeNull),
        "definitely-null" | "null" => Some(NullabilityState::DefinitelyNull),
        "unknown" => Some(NullabilityState::Unknown),
        _ => None,
    }
}

fn parse_pack_metadata(
    diagnostics: &mut DiagnosticBag,
    line_index: usize,
    metadata: &[&str],
) -> Option<Option<String>> {
    let mut rust_target = None;
    for token in metadata {
        if let Some(target) = token.strip_prefix("target=") {
            if target.is_empty() {
                push_external_pack_parse_error(
                    diagnostics,
                    line_index,
                    "external-pack target metadata must name a Rust target",
                );
                return None;
            }
            rust_target = Some(target.to_string());
        } else {
            push_external_pack_parse_error(
                diagnostics,
                line_index,
                "unrecognized external-pack metadata; expected target=RustPath",
            );
            return None;
        }
    }
    Some(rust_target)
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct ExternalApiMetadata {
    arg_count: Option<usize>,
    return_type: Option<JavaType>,
    thrown: Vec<JavaType>,
    rust_name: Option<String>,
}

fn parse_external_api_metadata(
    diagnostics: &mut DiagnosticBag,
    line_index: usize,
    metadata: &[&str],
) -> Option<ExternalApiMetadata> {
    let mut parsed = ExternalApiMetadata::default();
    for token in metadata {
        if let Some(arg_count) = token.strip_prefix("args=") {
            let Ok(arg_count) = arg_count.parse::<usize>() else {
                push_external_pack_parse_error(
                    diagnostics,
                    line_index,
                    "external-pack-api args metadata must be a non-negative integer",
                );
                return None;
            };
            parsed.arg_count = Some(arg_count);
        } else if let Some(return_type) = token.strip_prefix("return=") {
            if return_type.is_empty() {
                push_external_pack_parse_error(
                    diagnostics,
                    line_index,
                    "external-pack-api return metadata must name a Java type",
                );
                return None;
            }
            parsed.return_type = Some(parse_type(return_type));
        } else if let Some(thrown) = token.strip_prefix("throws=") {
            if thrown.is_empty() {
                push_external_pack_parse_error(
                    diagnostics,
                    line_index,
                    "external-pack-api throws metadata must name at least one Java type",
                );
                return None;
            }
            let mut parsed_any = false;
            for thrown_type in thrown.split(',').filter(|ty| !ty.is_empty()) {
                parsed_any = true;
                let ty = parse_type(thrown_type);
                if !parsed.thrown.contains(&ty) {
                    parsed.thrown.push(ty);
                }
            }
            if !parsed_any {
                push_external_pack_parse_error(
                    diagnostics,
                    line_index,
                    "external-pack-api throws metadata must name at least one Java type",
                );
                return None;
            }
        } else if let Some(rust_name) = token.strip_prefix("rust=") {
            if rust_name.is_empty() {
                push_external_pack_parse_error(
                    diagnostics,
                    line_index,
                    "external-pack-api rust metadata must name a Rust facade function",
                );
                return None;
            }
            parsed.rust_name = Some(rust_name.to_string());
        } else {
            push_external_pack_parse_error(
                diagnostics,
                line_index,
                "unrecognized external-pack-api metadata; expected args=N, return=Type, throws=TypeA,TypeB or rust=facade_name",
            );
            return None;
        }
    }
    Some(parsed)
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct ExternalConstructorMetadata {
    rust_type_name: Option<String>,
    rust_name: Option<String>,
}

fn parse_external_constructor_metadata(
    diagnostics: &mut DiagnosticBag,
    line_index: usize,
    metadata: &[&str],
) -> Option<ExternalConstructorMetadata> {
    let mut parsed = ExternalConstructorMetadata::default();
    for token in metadata {
        if let Some(rust_name) = token.strip_prefix("rust=") {
            if rust_name.is_empty() {
                push_external_pack_parse_error(
                    diagnostics,
                    line_index,
                    "external-pack-constructor rust metadata must name a Rust constructor function",
                );
                return None;
            }
            parsed.rust_name = Some(rust_name.to_string());
        } else if let Some(rust_type_name) = token.strip_prefix("rust-type=") {
            if rust_type_name.is_empty() {
                push_external_pack_parse_error(
                    diagnostics,
                    line_index,
                    "external-pack-constructor rust-type metadata must name a Rust type",
                );
                return None;
            }
            parsed.rust_type_name = Some(rust_type_name.to_string());
        } else {
            push_external_pack_parse_error(
                diagnostics,
                line_index,
                "unrecognized external-pack-constructor metadata; expected rust=constructor_name or rust-type=RustType",
            );
            return None;
        }
    }
    Some(parsed)
}

fn parse_external_argument_metadata(
    diagnostics: &mut DiagnosticBag,
    line_index: usize,
    metadata: &[&str],
) -> Option<Option<JavaType>> {
    let mut parameter_type = None;
    for token in metadata {
        if let Some(value) = token.strip_prefix("type=") {
            if value.is_empty() {
                push_external_pack_parse_error(
                    diagnostics,
                    line_index,
                    "external-pack-argument type metadata must name a Java type",
                );
                return None;
            }
            parameter_type = Some(parse_type(value));
        } else {
            push_external_pack_parse_error(
                diagnostics,
                line_index,
                "unrecognized external-pack-argument metadata; expected type=Type",
            );
            return None;
        }
    }
    Some(parameter_type)
}

fn push_external_pack_parse_error(
    diagnostics: &mut DiagnosticBag,
    line_index: usize,
    message: &str,
) {
    diagnostics.error(
        "external_pack_manifest_parse_error",
        format!("external pack manifest line {}: {message}", line_index + 1),
    );
}

const HDF5_OWNED_TYPES: &[&str] = &["HDF5Library", "HDF5File", "HDF5File.OpenMode", "OpenMode"];
const SQLITE_OWNED_TYPES: &[&str] = &[
    "org.sqlite.SQLiteConfig",
    "SQLiteConfig",
    "org.sqlite.SQLiteConnection",
    "SQLiteConnection",
    "org.sqlite.SQLiteStatement",
    "SQLiteStatement",
    "org.sqlite.SQLiteResultSet",
    "SQLiteResultSet",
    "org.sqlite.SQLException",
    "SQLException",
];
const JAVA_SQL_OWNED_TYPES: &[&str] = &[
    "java.sql.DriverManager",
    "driver_manager",
    "java.sql.Connection",
    "java.sql.Statement",
    "java.sql.PreparedStatement",
    "java.sql.ResultSet",
    "java.sql.ResultSetMetaData",
    "java.sql.DatabaseMetaData",
    "java.sql.Date",
    "java.sql.Timestamp",
    "java.sql.SQLException",
];
const SAMTOOLS_UTIL_OWNED_TYPES: &[&str] = &[
    "htsjdk.samtools.util.Locatable",
    "htsjdk.samtools.util.Interval",
    "htsjdk.samtools.util.IntervalList",
    "htsjdk.samtools.util.IntervalTree",
    "htsjdk.samtools.util.IntervalMergerIterator",
    "htsjdk.samtools.util.Histogram",
    "htsjdk.samtools.util.PeekableIterator",
    "htsjdk.samtools.util.DelegatingIterator",
    "DelegatingIterator",
    "htsjdk.samtools.util.MergingIterator",
    "htsjdk.samtools.util.RuntimeIOException",
    "htsjdk.samtools.util.DefaultingMap",
    "DefaultingMap",
    "htsjdk.samtools.util.Lazy",
    "Lazy",
    "htsjdk.samtools.seekablestream.SeekableBufferedStream",
    "SeekableBufferedStream",
    "htsjdk.samtools.seekablestream.SeekablePathStream",
    "SeekablePathStream",
    "htsjdk.samtools.util.BinaryCodec",
    "BinaryCodec",
    "htsjdk.samtools.util.BufferedLineReader",
    "BufferedLineReader",
    "htsjdk.samtools.util.AsciiLineReaderIterator",
    "AsciiLineReaderIterator",
    "htsjdk.samtools.util.LineIteratorImpl",
    "LineIteratorImpl",
    "htsjdk.samtools.util.SynchronousLineReader",
    "SynchronousLineReader",
    "htsjdk.samtools.Defaults",
    "Defaults",
    "htsjdk.samtools.SAMUtils",
    "SAMUtils",
    "htsjdk.samtools.ReservedTagConstants",
    "ReservedTagConstants",
    "htsjdk.samtools.fastq.FastqConstants",
    "FastqConstants",
    "htsjdk.samtools.util.SequenceUtil",
    "SequenceUtil",
    "htsjdk.samtools.util.BlockCompressedStreamConstants",
    "BlockCompressedStreamConstants",
    "htsjdk.samtools.util.BlockCompressedInputStream",
    "BlockCompressedInputStream",
    "htsjdk.samtools.util.BlockCompressedInputStream.FileTermination",
    "BlockCompressedInputStream.FileTermination",
    "FileTermination",
    "htsjdk.samtools.util.BlockCompressedOutputStream",
    "BlockCompressedOutputStream",
    "htsjdk.samtools.util.Log",
    "Log",
    "htsjdk.samtools.util.Log.LogLevel",
    "Log.LogLevel",
    "LogLevel",
    "htsjdk.samtools.SamPairUtil",
    "SamPairUtil",
    "htsjdk.samtools.SamPairUtil.PairOrientation",
    "SamPairUtil.PairOrientation",
    "PairOrientation",
];
const TRIBBLE_FEATURE_OWNED_TYPES: &[&str] = &[
    "htsjdk.tribble.Feature",
    "htsjdk.tribble.SimpleFeature",
    "htsjdk.tribble.NamedFeature",
    "htsjdk.tribble.AbstractFeatureReader",
    "AbstractFeatureReader",
    "htsjdk.tribble.FeatureReader",
    "FeatureReader",
    "htsjdk.tribble.bed.BEDFeature",
    "htsjdk.tribble.FeatureCodecHeader",
    "FeatureCodecHeader",
    "htsjdk.tribble.readers.CodecLineParsingException",
    "CodecLineParsingException",
    "htsjdk.tribble.gff.Gff3BaseData",
    "Gff3BaseData",
];

const TRIBBLE_FEATURE_ARGUMENT_EFFECTS: &[ExternalPackArgumentEffect] = &[
    ExternalPackArgumentEffect {
        receiver_type: "htsjdk.tribble.AbstractFeatureReader",
        method_name: "getFeatureReader",
        parameter_index: 1,
        parameter_type: Some("String"),
        parameter_nullability: NullabilityState::MaybeNull,
        argument_mutability: ReceiverMutabilityFact::ReadOnly,
        alias_policy: ExternalArgumentAliasPolicy::Borrowed,
    },
    ExternalPackArgumentEffect {
        receiver_type: "AbstractFeatureReader",
        method_name: "getFeatureReader",
        parameter_index: 1,
        parameter_type: Some("String"),
        parameter_nullability: NullabilityState::MaybeNull,
        argument_mutability: ReceiverMutabilityFact::ReadOnly,
        alias_policy: ExternalArgumentAliasPolicy::Borrowed,
    },
];

const TRIBBLE_INDEX_OWNED_TYPES: &[&str] = &[
    "htsjdk.tribble.index.IndexFactory",
    "IndexFactory",
    "htsjdk.tribble.index.IndexFactory.IndexType",
    "IndexFactory.IndexType",
    "IndexType",
    "htsjdk.tribble.index.IndexFactory.IndexBalanceApproach",
    "IndexFactory.IndexBalanceApproach",
    "IndexBalanceApproach",
    "htsjdk.tribble.index.tabix.TabixFormat",
    "TabixFormat",
];
const SAM_HEADER_OWNED_TYPES: &[&str] = &[
    "htsjdk.samtools.SAMSequenceDictionary",
    "htsjdk.samtools.SAMSequenceRecord",
    "htsjdk.samtools.SAMFileHeader",
    "htsjdk.samtools.SAMReadGroupRecord",
    "htsjdk.samtools.SAMProgramRecord",
    "SAMProgramRecord",
    "htsjdk.samtools.SAMTextHeaderCodec",
    "SAMTextHeaderCodec",
    "htsjdk.samtools.SAMFormatException",
    "SAMFormatException",
];
const SAM_RECORD_OWNED_TYPES: &[&str] = &[
    "htsjdk.samtools.SAMRecord",
    "SAMRecord",
    "htsjdk.samtools.SAMTagAndValue",
    "SAMTagAndValue",
    "htsjdk.samtools.SAMBinaryTagAndValue",
    "SAMBinaryTagAndValue",
    "htsjdk.samtools.Cigar",
    "Cigar",
    "htsjdk.samtools.CigarElement",
    "CigarElement",
    "htsjdk.samtools.CigarOperator",
    "CigarOperator",
    "htsjdk.samtools.DefaultSAMRecordFactory",
    "DefaultSAMRecordFactory",
    "htsjdk.samtools.SAMRecordCoordinateComparator",
    "SAMRecordCoordinateComparator",
    "htsjdk.samtools.SAMRecordQueryNameComparator",
    "SAMRecordQueryNameComparator",
    "htsjdk.samtools.filter.SamRecordFilter",
    "SamRecordFilter",
    "htsjdk.samtools.filter.FilteringSamIterator",
    "FilteringSamIterator",
];
const SAM_REFERENCE_OWNED_TYPES: &[&str] = &[
    "htsjdk.samtools.reference.FastaSequenceFile",
    "FastaSequenceFile",
    "htsjdk.samtools.reference.IndexedFastaSequenceFile",
    "IndexedFastaSequenceFile",
    "htsjdk.samtools.reference.FastaReferenceWriterBuilder",
    "FastaReferenceWriterBuilder",
    "htsjdk.samtools.reference.ReferenceSequence",
    "ReferenceSequence",
    "htsjdk.samtools.reference.ReferenceSequenceFileWalker",
    "ReferenceSequenceFileWalker",
];
const SAM_READER_OWNED_TYPES: &[&str] = &[
    "htsjdk.samtools.SamReaderFactory",
    "SamReaderFactory",
    "htsjdk.samtools.SamReaderFactory.Option",
    "SamReaderFactory.Option",
    "htsjdk.samtools.ValidationStringency",
    "ValidationStringency",
    "htsjdk.samtools.SamFileHeaderMerger",
    "SamFileHeaderMerger",
    "htsjdk.samtools.MergingSamRecordIterator",
    "MergingSamRecordIterator",
];
const SAM_WRITER_OWNED_TYPES: &[&str] = &[
    "htsjdk.samtools.SAMFileWriterFactory",
    "SAMFileWriterFactory",
];
const SAM_BAM_OWNED_TYPES: &[&str] = &[
    "htsjdk.samtools.BAMRecordCodec",
    "BAMRecordCodec",
    "htsjdk.samtools.BAMIndexer",
    "BAMIndexer",
    "htsjdk.samtools.SBIIndexWriter",
    "SBIIndexWriter",
    "htsjdk.samtools.BAMFileConstants",
    "BAMFileConstants",
];
const SAM_CRAM_OWNED_TYPES: &[&str] = &[
    "htsjdk.samtools.cram.build.CramContainerIterator",
    "CramContainerIterator",
    "htsjdk.samtools.cram.structure.Container",
    "Container",
    "htsjdk.samtools.cram.structure.DataSeriesType",
    "DataSeriesType",
    "htsjdk.samtools.cram.ref.CRAMReferenceRegion",
    "CRAMReferenceRegion",
];
const SAM_METRICS_OWNED_TYPES: &[&str] = &["htsjdk.samtools.metrics.MetricsFile", "MetricsFile"];
const HTSJDK_NESTED_HELPER_OWNED_TYPES: &[&str] = &["ComponentMethods"];
const VCF_OWNED_TYPES: &[&str] = &[
    VCF_PACK,
    "htsjdk.variant.vcf.VCFConstants",
    "VCFConstants",
    "htsjdk.variant.vcf.VCFCompoundHeaderLine",
    "VCFCompoundHeaderLine",
    "htsjdk.variant.vcf.VCFCompoundHeaderLine.SupportedHeaderLineType",
    "VCFCompoundHeaderLine.SupportedHeaderLineType",
    "SupportedHeaderLineType",
    "htsjdk.variant.variantcontext.writer.VariantContextWriterBuilder",
    "VariantContextWriterBuilder",
    "htsjdk.variant.variantcontext.writer.VariantContextWriter",
    "VariantContextWriter",
    "htsjdk.variant.variantcontext.writer.VariantContextWriterBuilder.OutputType",
    "VariantContextWriterBuilder.OutputType",
    "OutputType",
    "htsjdk.variant.variantcontext.writer.Options",
    "Options",
];
const BCF_OWNED_TYPES: &[&str] = &["htsjdk.variant.bcf2.BCF2Codec", "BCF2Codec"];
const VARIANTCONTEXT_OWNED_TYPES: &[&str] = &[
    VARIANTCONTEXT_PACK,
    "htsjdk.variant.variantcontext.VariantContextComparator",
    "VariantContextComparator",
    "htsjdk.variant.variantcontext.GenotypeLikelihoodsAllelePair",
    "GenotypeLikelihoodsAllelePair",
    "htsjdk.variant.variantcontext.JexlVCMatchExp",
    "JexlVCMatchExp",
    "htsjdk.variant.variantcontext.JexlMissingValueTreatment",
    "JexlMissingValueTreatment",
];
const APACHE_COMMONS_MATH_OWNED_TYPES: &[&str] = &[
    "org.apache.commons.math3.linear.Array2DRowRealMatrix",
    "Array2DRowRealMatrix",
    "org.apache.commons.math3.linear.ArrayRealVector",
    "ArrayRealVector",
    "org.apache.commons.math3.linear.DefaultRealMatrixChangingVisitor",
    "DefaultRealMatrixChangingVisitor",
    "org.apache.commons.math3.stat.descriptive.moment.Mean",
    "Mean",
    "org.apache.commons.math3.distribution.BinomialDistribution",
    "BinomialDistribution",
    "org.apache.commons.math3.distribution.BetaDistribution",
    "BetaDistribution",
    "org.apache.commons.math3.distribution.AbstractIntegerDistribution",
    "AbstractIntegerDistribution",
    "org.apache.commons.math3.stat.descriptive.DescriptiveStatistics",
    "DescriptiveStatistics",
    "org.apache.commons.math3.exception.DimensionMismatchException",
    "DimensionMismatchException",
    "org.apache.commons.math3.distribution.EnumeratedDistribution",
    "EnumeratedDistribution",
    "org.apache.commons.math3.distribution.ExponentialDistribution",
    "ExponentialDistribution",
    "org.apache.commons.math3.distribution.HypergeometricDistribution",
    "HypergeometricDistribution",
    "org.apache.commons.math3.random.JDKRandomGenerator",
    "JDKRandomGenerator",
    "org.apache.commons.math3.optim.MaxEval",
    "MaxEval",
    "org.apache.commons.math3.stat.descriptive.rank.Median",
    "Median",
    "org.apache.commons.math3.distribution.NormalDistribution",
    "NormalDistribution",
    "org.apache.commons.math3.distribution.ChiSquaredDistribution",
    "ChiSquaredDistribution",
    "org.apache.commons.math3.distribution.PoissonDistribution",
    "PoissonDistribution",
    "org.apache.commons.math3.linear.SingularValueDecomposition",
    "SingularValueDecomposition",
    "org.apache.commons.math3.stat.descriptive.moment.StandardDeviation",
    "StandardDeviation",
    "org.apache.commons.math3.distribution.TDistribution",
    "TDistribution",
];
const APACHE_COMMONS_MATH_OPTIM_OWNED_TYPES: &[&str] = &[
    "org.apache.commons.math3.optim.univariate.SearchInterval",
    "SearchInterval",
    "org.apache.commons.math3.optim.univariate.UnivariateObjectiveFunction",
    "UnivariateObjectiveFunction",
    "org.apache.commons.math3.optim.univariate.BrentOptimizer",
    "BrentOptimizer",
    "org.apache.commons.math3.optim.nonlinear.scalar.GoalType",
    "GoalType",
];
const APACHE_COMMONS_MATH_STATS_OWNED_TYPES: &[&str] = &[
    "org.apache.commons.math3.stat.descriptive.rank.Percentile",
    "Percentile",
    "org.apache.commons.math3.stat.descriptive.rank.Percentile.EstimationType",
    "Percentile.EstimationType",
    "EstimationType",
];
const APACHE_COMMONS_COLLECTIONS_OWNED_TYPES: &[&str] = &[
    "org.apache.commons.collections4.map.DefaultedMap",
    "DefaultedMap",
    "org.apache.commons.collections4.iterators.FilterIterator",
    "FilterIterator",
    "org.apache.commons.collections4.iterators.SingletonIterator",
    "SingletonIterator",
    "org.apache.commons.collections4.MultiMap",
    "MultiMap",
];
const APACHE_COMMONS_COMPRESS_OWNED_TYPES: &[&str] = &[
    "org.apache.commons.compress.compressors.gzip.GzipCompressorInputStream",
    "GzipCompressorInputStream",
    "org.apache.commons.compress.compressors.gzip.GzipCompressorOutputStream",
    "GzipCompressorOutputStream",
    "org.apache.commons.io.input.NullInputStream",
    "NullInputStream",
    "org.apache.commons.io.output.NullOutputStream",
    "NullOutputStream",
    "org.apache.commons.io.output.NullWriter",
    "NullWriter",
    "org.apache.commons.compress.archivers.tar.TarArchiveInputStream",
    "TarArchiveInputStream",
    "org.apache.commons.compress.archivers.tar.TarArchiveOutputStream",
    "TarArchiveOutputStream",
    "org.apache.commons.compress.archivers.tar.TarArchiveEntry",
    "TarArchiveEntry",
];
const APACHE_COMMONS_LANG_OWNED_TYPES: &[&str] = &[
    "org.apache.commons.lang3.builder.HashCodeBuilder",
    "HashCodeBuilder",
    "org.apache.commons.lang3.SystemUtils",
    "SystemUtils",
    "org.apache.commons.lang3.mutable.MutableInt",
    "MutableInt",
    "org.apache.commons.lang3.mutable.MutableLong",
    "MutableLong",
    "org.apache.commons.lang3.mutable.MutableDouble",
    "MutableDouble",
    "org.apache.commons.lang3.NotImplementedException",
    "NotImplementedException",
    "org.apache.commons.lang3.tuple.Pair",
    "Pair",
    "org.apache.commons.lang3.tuple.ImmutablePair",
    "ImmutablePair",
    "org.apache.commons.lang3.tuple.MutablePair",
    "MutablePair",
    "org.apache.commons.lang3.tuple.Triple",
    "Triple",
    "org.apache.commons.lang3.tuple.ImmutableTriple",
    "ImmutableTriple",
    "org.apache.commons.lang3.tuple.MutableTriple",
    "MutableTriple",
];
const APACHE_COMMONS_CODEC_OWNED_TYPES: &[&str] = &[
    "org.apache.commons.codec.digest.MessageDigestAlgorithms",
    "MessageDigestAlgorithms",
];
const JAMA_OWNED_TYPES: &[&str] = &["Jama.Matrix", "Matrix"];
const HIPPARCHUS_DISTRIBUTION_OWNED_TYPES: &[&str] = &[
    "org.hipparchus.random.EmpiricalDistribution",
    "EmpiricalDistribution",
];
const APACHE_HTTP_OWNED_TYPES: &[&str] = &[
    "org.apache.http.client.methods.HttpGet",
    "HttpGet",
    "org.apache.http.impl.conn.PoolingHttpClientConnectionManager",
    "PoolingHttpClientConnectionManager",
];
const GOOGLE_BIGQUERY_OWNED_TYPES: &[&str] = &[
    "com.google.cloud.bigquery.FieldValue",
    "FieldValue",
    "com.google.cloud.bigquery.FieldValue.Attribute",
    "FieldValue.Attribute",
    "com.google.cloud.bigquery.QueryJobConfiguration",
    "QueryJobConfiguration",
    "com.google.cloud.bigquery.QueryJobConfiguration.Priority",
    "QueryJobConfiguration.Priority",
];
const GOOGLE_BIGQUERY_STORAGE_OWNED_TYPES: &[&str] = &[
    "com.google.cloud.bigquery.storage.v1.DataFormat",
    "DataFormat",
];
const GOOGLE_STORAGE_OWNED_TYPES: &[&str] = &[
    "com.google.cloud.storage.Storage",
    "Storage",
    "com.google.cloud.storage.Storage.SignUrlOption",
    "Storage.SignUrlOption",
    "SignUrlOption",
];
const GOOGLE_CLOUD_NIO_OWNED_TYPES: &[&str] = &[
    "com.google.cloud.storage.contrib.nio.CloudStorageFileSystem",
    "CloudStorageFileSystem",
    "com.google.cloud.storage.contrib.nio.CloudStorageConfiguration",
    "CloudStorageConfiguration",
];
const GENOMICSDB_OWNED_TYPES: &[&str] = &[
    "org.genomicsdb.GenomicsDBUtils",
    "GenomicsDBUtils",
    "com.intel.genomicsdb.GenomicsDBFeatureReader",
    "GenomicsDBFeatureReader",
    "com.intel.genomicsdb.GenomicsDBImporter",
    "GenomicsDBImporter",
    "com.intel.genomicsdb.importer.model.ImportConfig",
    "ImportConfig",
    "com.intel.genomicsdb.model.Coordinates",
    "Coordinates",
    "com.intel.genomicsdb.model.Coordinates.GenomicsDBColumn",
    "Coordinates.GenomicsDBColumn",
    "GenomicsDBColumn",
    "com.intel.genomicsdb.model.Coordinates.ContigPosition",
    "Coordinates.ContigPosition",
    "ContigPosition",
    "com.intel.genomicsdb.model.GenomicsDBExportConfiguration",
    "GenomicsDBExportConfiguration",
    "com.intel.genomicsdb.model.GenomicsDBExportConfiguration.ExportConfiguration",
    "GenomicsDBExportConfiguration.ExportConfiguration",
    "ExportConfiguration",
    "com.intel.genomicsdb.model.GenomicsDBImportConfiguration",
    "GenomicsDBImportConfiguration",
    "com.intel.genomicsdb.model.GenomicsDBImportConfiguration.ImportConfiguration",
    "GenomicsDBImportConfiguration.ImportConfiguration",
    "ImportConfiguration",
    "com.intel.genomicsdb.model.GenomicsDBImportConfiguration.Partition",
    "GenomicsDBImportConfiguration.Partition",
    "Partition",
    "com.intel.genomicsdb.model.GenomicsDBVidMapProto",
    "GenomicsDBVidMapProto",
    "com.intel.genomicsdb.model.GenomicsDBVidMapProto.VidMappingPB",
    "GenomicsDBVidMapProto.VidMappingPB",
    "VidMappingPB",
    "com.intel.genomicsdb.model.GenomicsDBVidMapProto.Chromosome",
    "org.genomicsdb.model.GenomicsDBVidMapProto.Chromosome",
    "GenomicsDBVidMapProto.Chromosome",
    "Chromosome",
    "com.intel.genomicsdb.model.GenomicsDBVidMapProto.FieldInfo",
    "GenomicsDBVidMapProto.FieldInfo",
    "FieldInfo",
    "com.intel.genomicsdb.model.GenomicsDBVidMapProto.FieldLengthDescriptorComponentPB",
    "GenomicsDBVidMapProto.FieldLengthDescriptorComponentPB",
    "FieldLengthDescriptorComponentPB",
    "com.intel.genomicsdb.model.ReadSession",
    "ReadSession",
    "com.intel.genomicsdb.model.ReadSession.TableReadOptions",
    "ReadSession.TableReadOptions",
    "TableReadOptions",
];
const PICARD_PROGRAMGROUPS_OWNED_TYPES: &[&str] = &[
    "picard.cmdline.programgroups.DiagnosticsAndQCProgramGroup",
    "DiagnosticsAndQCProgramGroup",
    "picard.cmdline.programgroups.IntervalsManipulationProgramGroup",
    "IntervalsManipulationProgramGroup",
    "picard.cmdline.programgroups.OtherProgramGroup",
    "OtherProgramGroup",
    "picard.cmdline.programgroups.ReadDataManipulationProgramGroup",
    "ReadDataManipulationProgramGroup",
    "picard.cmdline.programgroups.ReferenceProgramGroup",
    "ReferenceProgramGroup",
    "picard.cmdline.programgroups.VariantEvaluationProgramGroup",
    "VariantEvaluationProgramGroup",
    "picard.cmdline.programgroups.VariantFilteringProgramGroup",
    "VariantFilteringProgramGroup",
    "picard.cmdline.programgroups.VariantManipulationProgramGroup",
    "VariantManipulationProgramGroup",
];
const PICARD_MARKDUPLICATES_OWNED_TYPES: &[&str] = &[
    "picard.sam.markduplicates.MarkDuplicates",
    "MarkDuplicates",
    "picard.sam.markduplicates.util.OpticalDuplicateFinder",
    "OpticalDuplicateFinder",
    "picard.sam.markduplicates.util.ReadEnds",
    "ReadEnds",
    "picard.sam.markduplicates.MarkDuplicates.DuplicateTaggingPolicy",
    "MarkDuplicates.DuplicateTaggingPolicy",
    "DuplicateTaggingPolicy",
];
const PICARD_METRICS_OWNED_TYPES: &[&str] =
    &["picard.sam.DuplicationMetrics", "DuplicationMetrics"];
const AVRO_OWNED_TYPES: &[&str] = &[
    "org.apache.avro.Schema",
    "org.apache.avro.Schema.Parser",
    "Schema.Parser",
    "Parser",
    "org.apache.avro.generic.GenericDatumReader",
    "GenericDatumReader",
];
const BARCLAY_HELP_OWNED_TYPES: &[&str] = &[
    "org.broadinstitute.barclay.help.DocException",
    "DocException",
    "org.broadinstitute.barclay.help.TemplateProperties",
    "TemplateProperties",
];
const BARCLAY_ARGPARSER_OWNED_TYPES: &[&str] = &[
    "org.broadinstitute.barclay.argparser.CommandLineException",
    "CommandLineException",
    "org.broadinstitute.barclay.argparser.CommandLineException.BadArgumentValue",
    "CommandLineException.BadArgumentValue",
    "BadArgumentValue",
    "org.broadinstitute.barclay.argparser.CommandLineException.MissingArgument",
    "CommandLineException.MissingArgument",
    "MissingArgument",
    "org.broadinstitute.barclay.argparser.CommandLineArgumentParser",
    "CommandLineArgumentParser",
    "org.broadinstitute.barclay.argparser.SpecialArgumentsCollection",
    "SpecialArgumentsCollection",
];
const BARCLAY_UTILS_OWNED_TYPES: &[&str] = &[
    "org.broadinstitute.barclay.utils.ClassFinder",
    "ClassFinder",
];
const SCALA_TUPLE_OWNED_TYPES: &[&str] = &[
    "scala.Tuple2",
    "Tuple2",
    "scala.Tuple3",
    "Tuple3",
    "scala.Tuple4",
    "Tuple4",
];
const KRYO_OWNED_TYPES: &[&str] = &[
    "com.esotericsoftware.kryo.Kryo",
    "Kryo",
    "com.esotericsoftware.kryo.Serializer",
    "com.esotericsoftware.kryo.io.Input",
    "Input",
    "com.esotericsoftware.kryo.io.Output",
    "Output",
    "com.esotericsoftware.kryo.serializers.FieldSerializer",
    "FieldSerializer",
    "com.esotericsoftware.kryo.serializers.ImmutableMapSerializer",
    "ImmutableMapSerializer",
];
const OPENCSV_OWNED_TYPES: &[&str] = &[
    "com.opencsv.CSVReader",
    "CSVReader",
    "com.opencsv.CSVWriter",
    "CSVWriter",
];
const ORG_JSON_OWNED_TYPES: &[&str] = &[
    "org.json.JSONObject",
    "JSONObject",
    "org.json.JSONArray",
    "JSONArray",
    "org.json.JSONTokener",
    "JSONTokener",
];
const REFLECTIONS_OWNED_TYPES: &[&str] = &["org.reflections.Reflections", "Reflections"];
const HADOOP_CONF_OWNED_TYPES: &[&str] = &["org.apache.hadoop.conf.Configuration", "Configuration"];
const APACHE_COMMONS_CONFIG_OWNED_TYPES: &[&str] = &[
    "org.apache.commons.configuration2.builder.fluent.Configurations",
    "Configurations",
    "org.apache.commons.configuration2.PropertiesConfiguration",
    "PropertiesConfiguration",
    "org.apache.commons.configuration2.XMLConfiguration",
    "XMLConfiguration",
    "org.apache.commons.configuration2.INIConfiguration",
    "INIConfiguration",
];
const SPARK_OWNED_TYPES: &[&str] = &[
    "org.apache.spark.SparkConf",
    "SparkConf",
    "org.apache.spark.HashPartitioner",
    "HashPartitioner",
    "org.apache.spark.api.java.JavaSparkContext",
    "JavaSparkContext",
    "org.apache.spark.api.java.JavaRDD",
    "JavaRDD",
    "org.apache.spark.api.java.JavaPairRDD",
    "JavaPairRDD",
    "org.apache.spark.rdd.PartitionGroup",
    "PartitionGroup",
    "org.apache.spark.mllib.linalg.distributed.RowMatrix",
    "RowMatrix",
    "org.apache.spark.broadcast.Broadcast",
    "Broadcast",
];
const BWA_OWNED_TYPES: &[&str] = &[
    "org.broadinstitute.hellbender.utils.bwa.BwaMemIndex",
    "BwaMemIndex",
    "org.broadinstitute.hellbender.utils.bwa.BwaMemAligner",
    "BwaMemAligner",
    "org.broadinstitute.hellbender.utils.bwa.BwaMemAlignment",
    "BwaMemAlignment",
];
const JGRAPHT_OWNED_TYPES: &[&str] = &[
    "org.jgrapht.Graph",
    "Graph",
    "org.jgrapht.alg.ConnectivityInspector",
    "ConnectivityInspector",
    "org.jgrapht.alg.CycleDetector",
    "CycleDetector",
    "org.jgrapht.graph.SimpleGraph",
    "SimpleGraph",
    "org.jgrapht.graph.DefaultDirectedWeightedGraph",
    "DefaultDirectedWeightedGraph",
    "org.jgrapht.io.IntegerComponentNameProvider",
    "IntegerComponentNameProvider",
    "org.jgrapht.io.DOTExporter",
    "DOTExporter",
];
const FASTUTIL_OWNED_TYPES: &[&str] = &[
    "it.unimi.dsi.fastutil.ints.Int2ObjectArrayMap",
    "Int2ObjectArrayMap",
    "it.unimi.dsi.fastutil.ints.Int2ObjectOpenHashMap",
    "Int2ObjectOpenHashMap",
    "it.unimi.dsi.fastutil.ints.IntArrayList",
    "IntArrayList",
    "it.unimi.dsi.fastutil.ints.Int2IntArrayMap",
    "Int2IntArrayMap",
    "it.unimi.dsi.fastutil.objects.Object2IntOpenHashMap",
    "Object2IntOpenHashMap",
];
const KRYO_SERIALIZERS_OWNED_TYPES: &[&str] = &[
    "de.javakaffee.kryoserializers.UnmodifiableCollectionsSerializer",
    "UnmodifiableCollectionsSerializer",
];
const DISQ_WRITE_OWNED_TYPES: &[&str] = &[
    "org.disq_bio.disq.HtsjdkReadsRdd",
    "HtsjdkReadsRdd",
    "org.disq_bio.disq.HtsjdkVariantsRdd",
    "HtsjdkVariantsRdd",
    "org.disq_bio.disq.HtsjdkReadsTraversalParameters",
    "HtsjdkReadsTraversalParameters",
    "org.disq_bio.disq.TempPartsDirectoryWriteOption",
    "TempPartsDirectoryWriteOption",
    "org.disq_bio.disq.ReadsFormatWriteOption",
    "ReadsFormatWriteOption",
    "org.disq_bio.disq.FileCardinalityWriteOption",
    "FileCardinalityWriteOption",
];
const INTEL_GKL_OWNED_TYPES: &[&str] = &[
    "com.intel.gkl.compression.IntelDeflaterFactory",
    "IntelDeflaterFactory",
    "com.intel.gkl.compression.IntelInflaterFactory",
    "IntelInflaterFactory",
    "com.intel.gkl.pdhmm.IntelPDHMM",
    "IntelPDHMM",
    "com.intel.gkl.pairhmm.IntelPairHmm",
    "IntelPairHmm",
    "com.intel.gkl.pairhmm.IntelPairHmmOMP",
    "IntelPairHmmOMP",
    "com.intel.gkl.pdhmm.PDHMMNativeArguments",
    "PDHMMNativeArguments",
    "com.intel.gkl.pairhmm.PairHMMNativeArguments",
    "PairHMMNativeArguments",
    "com.intel.gkl.pairhmm.IntelPairHmm.ReadDataHolder",
    "ReadDataHolder",
    "com.intel.gkl.pairhmm.IntelPairHmm.HaplotypeDataHolder",
    "HaplotypeDataHolder",
    "com.intel.gkl.pairhmm.IntelPairHmm.BatchCompletionCallbackFunctionArgument",
    "BatchCompletionCallbackFunctionArgument",
    "com.intel.gkl.smithwaterman.SWNativeAlignerResult",
    "SWNativeAlignerResult",
    "com.intel.gkl.pdhmm.AVXLevel",
    "AVXLevel",
    "com.intel.gkl.pdhmm.OpenMPSetting",
    "OpenMPSetting",
];
const GUAVA_IMMUTABLE_COLLECT_OWNED_TYPES: &[&str] = &[
    "com.google.common.collect.ImmutableMap.Builder",
    "ImmutableMap.Builder",
    "com.google.common.collect.AbstractIterator",
    "AbstractIterator",
];
const GUAVA_CONCURRENT_OWNED_TYPES: &[&str] = &[
    "com.google.common.util.concurrent.ThreadFactoryBuilder",
    "ThreadFactoryBuilder",
];
const BROAD_HELLBENDER_UTILS_OWNED_TYPES: &[&str] =
    &["org.broadinstitute.hellbender.utils.Utils", "Utils"];
const BROAD_HTTP_OWNED_TYPES: &[&str] = &[
    "org.broadinstitute.http.nio.HttpFileSystemProviderSettings",
    "HttpFileSystemProviderSettings",
    "org.broadinstitute.http.nio.HttpFileSystemProviderSettings.RetrySettings",
    "HttpFileSystemProviderSettings.RetrySettings",
    "RetrySettings",
    "org.broadinstitute.http.nio.RetryHandler",
    "RetryHandler",
    "org.broadinstitute.http.nio.HttpFileSystemProvider",
    "HttpFileSystemProvider",
    "org.broadinstitute.http.nio.HttpsFileSystemProvider",
    "HttpsFileSystemProvider",
    "org.broadinstitute.hellbender.utils.gcs.BucketUtils",
    "BucketUtils",
];
const PICARD_VCF_OWNED_TYPES: &[&str] = &[
    "picard.vcf.GenotypeConcordanceStates",
    "GenotypeConcordanceStates",
    "picard.vcf.GenotypeConcordance",
    "GenotypeConcordance",
    "picard.vcf.GenotypeConcordanceCounts",
    "GenotypeConcordanceCounts",
    "picard.vcf.GenotypeConcordanceSummaryMetrics",
    "GenotypeConcordanceSummaryMetrics",
    "picard.vcf.GenotypeConcordanceStates.TruthAndCallStates",
    "GenotypeConcordanceStates.TruthAndCallStates",
    "TruthAndCallStates",
    "picard.vcf.GenotypeConcordanceStates.TruthState",
    "GenotypeConcordanceStates.TruthState",
    "TruthState",
    "picard.vcf.GenotypeConcordanceStates.CallState",
    "GenotypeConcordanceStates.CallState",
    "CallState",
];
const JACKSON_DATABIND_OWNED_TYPES: &[&str] = &[
    "com.fasterxml.jackson.databind.MapperFeature",
    "MapperFeature",
    "com.fasterxml.jackson.databind.DeserializationFeature",
    "DeserializationFeature",
];

const fn erased_object_strategy(type_name: &'static str) -> ExternalPackObjectStrategy {
    ExternalPackObjectStrategy {
        type_name,
        strategy: ExternalPackObjectStrategyKind::ErasedObject,
    }
}

const fn concrete_object_strategy(type_name: &'static str) -> ExternalPackObjectStrategy {
    ExternalPackObjectStrategy {
        type_name,
        strategy: ExternalPackObjectStrategyKind::ConcreteStruct,
    }
}

const NO_TYPE_CAPABILITIES: &[ExternalPackTypeCapability] = &[];
const NO_CLOSED_HIERARCHY: &[ExternalPackClosedHierarchy] = &[];
const NO_OBJECT_SAFETY: &[ExternalPackObjectSafety] = &[];

const LOCATABLE_IMPL: &[&str] = &["Locatable"];
const FEATURE_LOCATABLE_IMPL: &[&str] = &["Feature", "Locatable"];

const fn empty_manifest_capability(type_name: &'static str) -> ExternalPackTypeCapability {
    ExternalPackTypeCapability {
        type_name,
        copy: None,
        clone: None,
        string_like: None,
        sequence_like: None,
        implements: &[],
        extends: None,
        final_type: None,
        object_safe_trait: None,
    }
}

const fn cloneable_final_capability(type_name: &'static str) -> ExternalPackTypeCapability {
    let mut capability = empty_manifest_capability(type_name);
    capability.clone = Some(true);
    capability.final_type = Some(true);
    capability
}

const fn sequence_like_capability(
    type_name: &'static str,
    element_type: &'static str,
) -> ExternalPackTypeCapability {
    let mut capability = empty_manifest_capability(type_name);
    capability.sequence_like = Some(element_type);
    capability.final_type = Some(true);
    capability
}

const fn cloneable_final_implements_capability(
    type_name: &'static str,
    implements: &'static [&'static str],
) -> ExternalPackTypeCapability {
    let mut capability = cloneable_final_capability(type_name);
    capability.implements = implements;
    capability
}

const fn implements_capability(
    type_name: &'static str,
    implements: &'static [&'static str],
) -> ExternalPackTypeCapability {
    let mut capability = empty_manifest_capability(type_name);
    capability.implements = implements;
    capability
}

const fn extends_capability(
    type_name: &'static str,
    extends: &'static str,
) -> ExternalPackTypeCapability {
    let mut capability = empty_manifest_capability(type_name);
    capability.extends = Some(extends);
    capability
}

const fn readonly_borrowed_argument(
    receiver_type: &'static str,
    method_name: &'static str,
    parameter_index: usize,
    parameter_type: &'static str,
) -> ExternalPackArgumentEffect {
    ExternalPackArgumentEffect {
        receiver_type,
        method_name,
        parameter_index,
        parameter_type: Some(parameter_type),
        parameter_nullability: NullabilityState::DefinitelyNonNull,
        argument_mutability: ReceiverMutabilityFact::ReadOnly,
        alias_policy: ExternalArgumentAliasPolicy::Borrowed,
    }
}

const fn non_null_field(
    type_name: &'static str,
    field_name: &'static str,
) -> ExternalPackFieldNullability {
    ExternalPackFieldNullability {
        type_name,
        field_name,
        state: NullabilityState::DefinitelyNonNull,
    }
}

const fn api_effect(
    receiver_type: &'static str,
    method_name: &'static str,
    role: ExternalApiRole,
    receiver_mutability: ReceiverMutabilityFact,
    return_type: Option<&'static str>,
    return_nullability: NullabilityState,
    thrown: &'static [&'static str],
) -> ExternalPackApiEffect {
    ExternalPackApiEffect {
        receiver_type,
        method_name,
        arg_count: None,
        rust_name: None,
        role,
        receiver_mutability,
        return_type,
        return_nullability,
        thrown,
    }
}

const fn api_effect_with_arity(
    receiver_type: &'static str,
    method_name: &'static str,
    arg_count: usize,
    role: ExternalApiRole,
    receiver_mutability: ReceiverMutabilityFact,
    return_type: Option<&'static str>,
    return_nullability: NullabilityState,
    thrown: &'static [&'static str],
) -> ExternalPackApiEffect {
    ExternalPackApiEffect {
        receiver_type,
        method_name,
        arg_count: Some(arg_count),
        rust_name: None,
        role,
        receiver_mutability,
        return_type,
        return_nullability,
        thrown,
    }
}

const fn semantic_typed_effect(
    receiver_type: &'static str,
    method_name: &'static str,
    role: ExternalApiRole,
    receiver_mutability: ReceiverMutabilityFact,
    return_type: &'static str,
    return_nullability: NullabilityState,
) -> ExternalPackApiEffect {
    api_effect(
        receiver_type,
        method_name,
        role,
        receiver_mutability,
        Some(return_type),
        return_nullability,
        &[],
    )
}

const fn semantic_typed_effect_with_arity(
    receiver_type: &'static str,
    method_name: &'static str,
    arg_count: usize,
    role: ExternalApiRole,
    receiver_mutability: ReceiverMutabilityFact,
    return_type: &'static str,
    return_nullability: NullabilityState,
) -> ExternalPackApiEffect {
    api_effect_with_arity(
        receiver_type,
        method_name,
        arg_count,
        role,
        receiver_mutability,
        Some(return_type),
        return_nullability,
        &[],
    )
}

const fn semantic_fluent_mutator_effect(
    receiver_type: &'static str,
    method_name: &'static str,
    return_type: &'static str,
) -> ExternalPackApiEffect {
    semantic_typed_effect(
        receiver_type,
        method_name,
        ExternalApiRole::FluentMutator,
        ReceiverMutabilityFact::MutatesReceiver,
        return_type,
        NullabilityState::DefinitelyNonNull,
    )
}

const HDF5_TYPE_CAPABILITIES: &[ExternalPackTypeCapability] = &[
    cloneable_final_capability("OpenMode"),
    cloneable_final_capability("HDF5File.OpenMode"),
];

const VARIANTCONTEXT_TYPE_CAPABILITIES: &[ExternalPackTypeCapability] = &[
    cloneable_final_capability("htsjdk.variant.variantcontext.Allele"),
    cloneable_final_capability("Allele"),
    cloneable_final_capability("htsjdk.variant.variantcontext.VariantContext"),
    cloneable_final_capability("VariantContext"),
    cloneable_final_capability("htsjdk.variant.variantcontext.VariantContext.Type"),
    cloneable_final_capability("VariantContext.Type"),
    cloneable_final_capability("Type"),
    cloneable_final_capability("htsjdk.variant.variantcontext.VariantContextComparator"),
    cloneable_final_capability("VariantContextComparator"),
    cloneable_final_capability("htsjdk.variant.variantcontext.GenotypeLikelihoodsAllelePair"),
    cloneable_final_capability("GenotypeLikelihoodsAllelePair"),
    cloneable_final_capability("htsjdk.variant.variantcontext.JexlVCMatchExp"),
    cloneable_final_capability("JexlVCMatchExp"),
    cloneable_final_capability("htsjdk.variant.variantcontext.JexlMissingValueTreatment"),
    cloneable_final_capability("JexlMissingValueTreatment"),
    cloneable_final_capability("htsjdk.variant.variantcontext.Genotype"),
    cloneable_final_capability("Genotype"),
    sequence_like_capability(
        "htsjdk.variant.variantcontext.GenotypesContext",
        "htsjdk.variant.variantcontext.Genotype",
    ),
    sequence_like_capability("GenotypesContext", "Genotype"),
    extends_capability(
        "htsjdk.variant.variantcontext.VariantContextException",
        "RuntimeException",
    ),
    extends_capability("VariantContextException", "RuntimeException"),
    extends_capability(
        "htsjdk.tribble.TribbleException.InternalCodecException",
        "RuntimeException",
    ),
    extends_capability("InternalCodecException", "RuntimeException"),
];

const VCF_TYPE_CAPABILITIES: &[ExternalPackTypeCapability] = &[
    cloneable_final_capability("htsjdk.variant.vcf.VCFFileReader"),
    cloneable_final_capability("VCFFileReader"),
    cloneable_final_capability("htsjdk.variant.vcf.VCFSimpleHeaderLine"),
    cloneable_final_capability("VCFSimpleHeaderLine"),
    cloneable_final_capability("htsjdk.variant.vcf.VCFHeaderLine"),
    cloneable_final_capability("VCFHeaderLine"),
    cloneable_final_capability("htsjdk.variant.vcf.VCFInfoHeaderLine"),
    cloneable_final_capability("VCFInfoHeaderLine"),
    cloneable_final_capability("htsjdk.variant.vcf.VCFFormatHeaderLine"),
    cloneable_final_capability("VCFFormatHeaderLine"),
    cloneable_final_capability("htsjdk.variant.vcf.VCFFilterHeaderLine"),
    cloneable_final_capability("VCFFilterHeaderLine"),
    cloneable_final_capability("htsjdk.variant.vcf.VCFHeaderLineType"),
    cloneable_final_capability("VCFHeaderLineType"),
    cloneable_final_capability("htsjdk.variant.vcf.VCFHeaderLineCount"),
    cloneable_final_capability("VCFHeaderLineCount"),
    cloneable_final_capability("htsjdk.variant.vcf.VCFHeaderVersion"),
    cloneable_final_capability("VCFHeaderVersion"),
    cloneable_final_capability("htsjdk.variant.vcf.VCFCompoundHeaderLine"),
    cloneable_final_capability("VCFCompoundHeaderLine"),
    cloneable_final_capability("htsjdk.variant.vcf.VCFCompoundHeaderLine.SupportedHeaderLineType"),
    cloneable_final_capability("VCFCompoundHeaderLine.SupportedHeaderLineType"),
    cloneable_final_capability("SupportedHeaderLineType"),
    cloneable_final_capability("htsjdk.variant.variantcontext.writer.VariantContextWriterBuilder"),
    cloneable_final_capability("VariantContextWriterBuilder"),
    cloneable_final_capability("htsjdk.variant.variantcontext.writer.VariantContextWriter"),
    cloneable_final_capability("VariantContextWriter"),
    cloneable_final_capability(
        "htsjdk.variant.variantcontext.writer.VariantContextWriterBuilder.OutputType",
    ),
    cloneable_final_capability("VariantContextWriterBuilder.OutputType"),
    cloneable_final_capability("OutputType"),
    cloneable_final_capability("htsjdk.variant.variantcontext.writer.Options"),
    cloneable_final_capability("Options"),
];

const APACHE_COMMONS_MATH_TYPE_CAPABILITIES: &[ExternalPackTypeCapability] = &[
    cloneable_final_capability("org.apache.commons.math3.linear.Array2DRowRealMatrix"),
    cloneable_final_capability("Array2DRowRealMatrix"),
    cloneable_final_capability("org.apache.commons.math3.linear.ArrayRealVector"),
    cloneable_final_capability("ArrayRealVector"),
    cloneable_final_capability("org.apache.commons.math3.linear.DefaultRealMatrixChangingVisitor"),
    cloneable_final_capability("DefaultRealMatrixChangingVisitor"),
    cloneable_final_capability("org.apache.commons.math3.stat.descriptive.moment.Mean"),
    cloneable_final_capability("Mean"),
    cloneable_final_capability("org.apache.commons.math3.distribution.BinomialDistribution"),
    cloneable_final_capability("BinomialDistribution"),
    cloneable_final_capability("org.apache.commons.math3.distribution.BetaDistribution"),
    cloneable_final_capability("BetaDistribution"),
    cloneable_final_capability("org.apache.commons.math3.distribution.AbstractIntegerDistribution"),
    cloneable_final_capability("AbstractIntegerDistribution"),
    cloneable_final_capability("org.apache.commons.math3.stat.descriptive.DescriptiveStatistics"),
    cloneable_final_capability("DescriptiveStatistics"),
    cloneable_final_capability("org.apache.commons.math3.exception.DimensionMismatchException"),
    cloneable_final_capability("DimensionMismatchException"),
    cloneable_final_capability("org.apache.commons.math3.distribution.EnumeratedDistribution"),
    cloneable_final_capability("EnumeratedDistribution"),
    cloneable_final_capability("org.apache.commons.math3.distribution.ExponentialDistribution"),
    cloneable_final_capability("ExponentialDistribution"),
    cloneable_final_capability("org.apache.commons.math3.distribution.HypergeometricDistribution"),
    cloneable_final_capability("HypergeometricDistribution"),
    cloneable_final_capability("org.apache.commons.math3.random.JDKRandomGenerator"),
    cloneable_final_capability("JDKRandomGenerator"),
    cloneable_final_capability("org.apache.commons.math3.optim.MaxEval"),
    cloneable_final_capability("MaxEval"),
    cloneable_final_capability("org.apache.commons.math3.stat.descriptive.rank.Median"),
    cloneable_final_capability("Median"),
    cloneable_final_capability("org.apache.commons.math3.distribution.NormalDistribution"),
    cloneable_final_capability("NormalDistribution"),
    cloneable_final_capability("org.apache.commons.math3.distribution.ChiSquaredDistribution"),
    cloneable_final_capability("ChiSquaredDistribution"),
    cloneable_final_capability("org.apache.commons.math3.distribution.PoissonDistribution"),
    cloneable_final_capability("PoissonDistribution"),
    cloneable_final_capability("org.apache.commons.math3.linear.SingularValueDecomposition"),
    cloneable_final_capability("SingularValueDecomposition"),
    cloneable_final_capability(
        "org.apache.commons.math3.stat.descriptive.moment.StandardDeviation",
    ),
    cloneable_final_capability("StandardDeviation"),
    cloneable_final_capability("org.apache.commons.math3.distribution.TDistribution"),
    cloneable_final_capability("TDistribution"),
];

const APACHE_COMMONS_MATH_OPTIM_TYPE_CAPABILITIES: &[ExternalPackTypeCapability] = &[
    cloneable_final_capability("org.apache.commons.math3.optim.univariate.SearchInterval"),
    cloneable_final_capability("SearchInterval"),
    cloneable_final_capability(
        "org.apache.commons.math3.optim.univariate.UnivariateObjectiveFunction",
    ),
    cloneable_final_capability("UnivariateObjectiveFunction"),
    cloneable_final_capability("org.apache.commons.math3.optim.univariate.BrentOptimizer"),
    cloneable_final_capability("BrentOptimizer"),
    cloneable_final_capability("org.apache.commons.math3.optim.nonlinear.scalar.GoalType"),
    cloneable_final_capability("GoalType"),
];

const APACHE_COMMONS_MATH_STATS_TYPE_CAPABILITIES: &[ExternalPackTypeCapability] = &[
    cloneable_final_capability("org.apache.commons.math3.stat.descriptive.rank.Percentile"),
    cloneable_final_capability("Percentile"),
    cloneable_final_capability(
        "org.apache.commons.math3.stat.descriptive.rank.Percentile.EstimationType",
    ),
    cloneable_final_capability("Percentile.EstimationType"),
    cloneable_final_capability("EstimationType"),
];

const APACHE_COMMONS_COLLECTIONS_TYPE_CAPABILITIES: &[ExternalPackTypeCapability] = &[
    cloneable_final_capability("org.apache.commons.collections4.map.DefaultedMap"),
    cloneable_final_capability("DefaultedMap"),
    cloneable_final_capability("org.apache.commons.collections4.iterators.FilterIterator"),
    cloneable_final_capability("FilterIterator"),
    cloneable_final_capability("org.apache.commons.collections4.iterators.SingletonIterator"),
    cloneable_final_capability("SingletonIterator"),
    cloneable_final_capability("org.apache.commons.collections4.MultiMap"),
    cloneable_final_capability("MultiMap"),
];

const APACHE_COMMONS_COMPRESS_TYPE_CAPABILITIES: &[ExternalPackTypeCapability] = &[
    cloneable_final_capability(
        "org.apache.commons.compress.compressors.gzip.GzipCompressorInputStream",
    ),
    cloneable_final_capability("GzipCompressorInputStream"),
    cloneable_final_capability(
        "org.apache.commons.compress.compressors.gzip.GzipCompressorOutputStream",
    ),
    cloneable_final_capability("GzipCompressorOutputStream"),
    cloneable_final_capability("org.apache.commons.io.input.NullInputStream"),
    cloneable_final_capability("NullInputStream"),
    cloneable_final_capability("org.apache.commons.io.output.NullOutputStream"),
    cloneable_final_capability("NullOutputStream"),
    cloneable_final_capability("org.apache.commons.io.output.NullWriter"),
    cloneable_final_capability("NullWriter"),
    cloneable_final_capability("org.apache.commons.compress.archivers.tar.TarArchiveInputStream"),
    cloneable_final_capability("TarArchiveInputStream"),
    cloneable_final_capability("org.apache.commons.compress.archivers.tar.TarArchiveOutputStream"),
    cloneable_final_capability("TarArchiveOutputStream"),
    cloneable_final_capability("org.apache.commons.compress.archivers.tar.TarArchiveEntry"),
    cloneable_final_capability("TarArchiveEntry"),
];

const APACHE_COMMONS_LANG_TYPE_CAPABILITIES: &[ExternalPackTypeCapability] = &[
    cloneable_final_capability("org.apache.commons.lang3.builder.HashCodeBuilder"),
    cloneable_final_capability("HashCodeBuilder"),
    cloneable_final_capability("org.apache.commons.lang3.SystemUtils"),
    cloneable_final_capability("SystemUtils"),
    cloneable_final_capability("org.apache.commons.lang3.mutable.MutableInt"),
    cloneable_final_capability("MutableInt"),
    cloneable_final_capability("org.apache.commons.lang3.mutable.MutableLong"),
    cloneable_final_capability("MutableLong"),
    cloneable_final_capability("org.apache.commons.lang3.mutable.MutableDouble"),
    cloneable_final_capability("MutableDouble"),
    cloneable_final_capability("org.apache.commons.lang3.NotImplementedException"),
    cloneable_final_capability("NotImplementedException"),
    cloneable_final_capability("org.apache.commons.lang3.tuple.Pair"),
    cloneable_final_capability("Pair"),
    cloneable_final_capability("org.apache.commons.lang3.tuple.ImmutablePair"),
    cloneable_final_capability("ImmutablePair"),
    cloneable_final_capability("org.apache.commons.lang3.tuple.MutablePair"),
    cloneable_final_capability("MutablePair"),
    cloneable_final_capability("org.apache.commons.lang3.tuple.Triple"),
    cloneable_final_capability("Triple"),
    cloneable_final_capability("org.apache.commons.lang3.tuple.ImmutableTriple"),
    cloneable_final_capability("ImmutableTriple"),
    cloneable_final_capability("org.apache.commons.lang3.tuple.MutableTriple"),
    cloneable_final_capability("MutableTriple"),
];

const APACHE_COMMONS_CODEC_TYPE_CAPABILITIES: &[ExternalPackTypeCapability] = &[
    cloneable_final_capability("org.apache.commons.codec.digest.MessageDigestAlgorithms"),
    cloneable_final_capability("MessageDigestAlgorithms"),
];

const JAMA_TYPE_CAPABILITIES: &[ExternalPackTypeCapability] = &[
    cloneable_final_capability("Jama.Matrix"),
    cloneable_final_capability("Matrix"),
];

const HIPPARCHUS_DISTRIBUTION_TYPE_CAPABILITIES: &[ExternalPackTypeCapability] = &[
    cloneable_final_capability("org.hipparchus.random.EmpiricalDistribution"),
    cloneable_final_capability("EmpiricalDistribution"),
];

const APACHE_HTTP_TYPE_CAPABILITIES: &[ExternalPackTypeCapability] = &[
    cloneable_final_capability("org.apache.http.client.methods.HttpGet"),
    cloneable_final_capability("HttpGet"),
    cloneable_final_capability("org.apache.http.impl.conn.PoolingHttpClientConnectionManager"),
    cloneable_final_capability("PoolingHttpClientConnectionManager"),
];

const GOOGLE_BIGQUERY_TYPE_CAPABILITIES: &[ExternalPackTypeCapability] = &[
    cloneable_final_capability("com.google.cloud.bigquery.FieldValue"),
    cloneable_final_capability("FieldValue"),
    cloneable_final_capability("com.google.cloud.bigquery.FieldValue.Attribute"),
    cloneable_final_capability("FieldValue.Attribute"),
    cloneable_final_capability("com.google.cloud.bigquery.QueryJobConfiguration"),
    cloneable_final_capability("QueryJobConfiguration"),
    cloneable_final_capability("com.google.cloud.bigquery.QueryJobConfiguration.Priority"),
    cloneable_final_capability("QueryJobConfiguration.Priority"),
];

const GOOGLE_BIGQUERY_STORAGE_TYPE_CAPABILITIES: &[ExternalPackTypeCapability] = &[
    cloneable_final_capability("com.google.cloud.bigquery.storage.v1.DataFormat"),
    cloneable_final_capability("DataFormat"),
];

const GOOGLE_STORAGE_TYPE_CAPABILITIES: &[ExternalPackTypeCapability] = &[
    cloneable_final_capability("com.google.cloud.storage.Storage"),
    cloneable_final_capability("Storage"),
    cloneable_final_capability("com.google.cloud.storage.Storage.SignUrlOption"),
    cloneable_final_capability("Storage.SignUrlOption"),
    cloneable_final_capability("SignUrlOption"),
];

const GOOGLE_CLOUD_NIO_TYPE_CAPABILITIES: &[ExternalPackTypeCapability] = &[
    cloneable_final_capability("com.google.cloud.storage.contrib.nio.CloudStorageFileSystem"),
    cloneable_final_capability("CloudStorageFileSystem"),
    cloneable_final_capability("com.google.cloud.storage.contrib.nio.CloudStorageConfiguration"),
    cloneable_final_capability("CloudStorageConfiguration"),
];

const GENOMICSDB_TYPE_CAPABILITIES: &[ExternalPackTypeCapability] = &[
    cloneable_final_capability("com.intel.genomicsdb.GenomicsDBFeatureReader"),
    cloneable_final_capability("GenomicsDBFeatureReader"),
    cloneable_final_capability("com.intel.genomicsdb.GenomicsDBImporter"),
    cloneable_final_capability("GenomicsDBImporter"),
    cloneable_final_capability("com.intel.genomicsdb.importer.model.ImportConfig"),
    cloneable_final_capability("ImportConfig"),
    cloneable_final_capability("com.intel.genomicsdb.model.Coordinates"),
    cloneable_final_capability("Coordinates"),
    cloneable_final_capability("com.intel.genomicsdb.model.Coordinates.GenomicsDBColumn"),
    cloneable_final_capability("Coordinates.GenomicsDBColumn"),
    cloneable_final_capability("GenomicsDBColumn"),
    cloneable_final_capability("com.intel.genomicsdb.model.Coordinates.ContigPosition"),
    cloneable_final_capability("Coordinates.ContigPosition"),
    cloneable_final_capability("ContigPosition"),
    cloneable_final_capability("com.intel.genomicsdb.model.GenomicsDBExportConfiguration"),
    cloneable_final_capability("GenomicsDBExportConfiguration"),
    cloneable_final_capability(
        "com.intel.genomicsdb.model.GenomicsDBExportConfiguration.ExportConfiguration",
    ),
    cloneable_final_capability("GenomicsDBExportConfiguration.ExportConfiguration"),
    cloneable_final_capability("ExportConfiguration"),
    cloneable_final_capability("com.intel.genomicsdb.model.GenomicsDBImportConfiguration"),
    cloneable_final_capability("GenomicsDBImportConfiguration"),
    cloneable_final_capability(
        "com.intel.genomicsdb.model.GenomicsDBImportConfiguration.ImportConfiguration",
    ),
    cloneable_final_capability("GenomicsDBImportConfiguration.ImportConfiguration"),
    cloneable_final_capability("ImportConfiguration"),
    cloneable_final_capability(
        "com.intel.genomicsdb.model.GenomicsDBImportConfiguration.Partition",
    ),
    cloneable_final_capability("GenomicsDBImportConfiguration.Partition"),
    cloneable_final_capability("Partition"),
    cloneable_final_capability("com.intel.genomicsdb.model.GenomicsDBVidMapProto"),
    cloneable_final_capability("GenomicsDBVidMapProto"),
    cloneable_final_capability("com.intel.genomicsdb.model.GenomicsDBVidMapProto.VidMappingPB"),
    cloneable_final_capability("GenomicsDBVidMapProto.VidMappingPB"),
    cloneable_final_capability("VidMappingPB"),
    cloneable_final_capability("com.intel.genomicsdb.model.GenomicsDBVidMapProto.Chromosome"),
    cloneable_final_capability("org.genomicsdb.model.GenomicsDBVidMapProto.Chromosome"),
    cloneable_final_capability("GenomicsDBVidMapProto.Chromosome"),
    cloneable_final_capability("Chromosome"),
    cloneable_final_capability("com.intel.genomicsdb.model.GenomicsDBVidMapProto.FieldInfo"),
    cloneable_final_capability("GenomicsDBVidMapProto.FieldInfo"),
    cloneable_final_capability("FieldInfo"),
    cloneable_final_capability(
        "com.intel.genomicsdb.model.GenomicsDBVidMapProto.FieldLengthDescriptorComponentPB",
    ),
    cloneable_final_capability("GenomicsDBVidMapProto.FieldLengthDescriptorComponentPB"),
    cloneable_final_capability("FieldLengthDescriptorComponentPB"),
    cloneable_final_capability("com.intel.genomicsdb.model.ReadSession"),
    cloneable_final_capability("ReadSession"),
    cloneable_final_capability("com.intel.genomicsdb.model.ReadSession.TableReadOptions"),
    cloneable_final_capability("ReadSession.TableReadOptions"),
    cloneable_final_capability("TableReadOptions"),
];

const PICARD_PROGRAMGROUPS_TYPE_CAPABILITIES: &[ExternalPackTypeCapability] = &[
    cloneable_final_capability("picard.cmdline.programgroups.DiagnosticsAndQCProgramGroup"),
    cloneable_final_capability("DiagnosticsAndQCProgramGroup"),
    cloneable_final_capability("picard.cmdline.programgroups.IntervalsManipulationProgramGroup"),
    cloneable_final_capability("IntervalsManipulationProgramGroup"),
    cloneable_final_capability("picard.cmdline.programgroups.OtherProgramGroup"),
    cloneable_final_capability("OtherProgramGroup"),
    cloneable_final_capability("picard.cmdline.programgroups.ReadDataManipulationProgramGroup"),
    cloneable_final_capability("ReadDataManipulationProgramGroup"),
    cloneable_final_capability("picard.cmdline.programgroups.ReferenceProgramGroup"),
    cloneable_final_capability("ReferenceProgramGroup"),
    cloneable_final_capability("picard.cmdline.programgroups.VariantEvaluationProgramGroup"),
    cloneable_final_capability("VariantEvaluationProgramGroup"),
    cloneable_final_capability("picard.cmdline.programgroups.VariantFilteringProgramGroup"),
    cloneable_final_capability("VariantFilteringProgramGroup"),
    cloneable_final_capability("picard.cmdline.programgroups.VariantManipulationProgramGroup"),
    cloneable_final_capability("VariantManipulationProgramGroup"),
];

const PICARD_MARKDUPLICATES_TYPE_CAPABILITIES: &[ExternalPackTypeCapability] = &[
    cloneable_final_capability("picard.sam.markduplicates.MarkDuplicates"),
    cloneable_final_capability("MarkDuplicates"),
    cloneable_final_capability("picard.sam.markduplicates.util.OpticalDuplicateFinder"),
    cloneable_final_capability("OpticalDuplicateFinder"),
    cloneable_final_capability("picard.sam.markduplicates.util.ReadEnds"),
    cloneable_final_capability("ReadEnds"),
    cloneable_final_capability("picard.sam.markduplicates.MarkDuplicates.DuplicateTaggingPolicy"),
    cloneable_final_capability("MarkDuplicates.DuplicateTaggingPolicy"),
    cloneable_final_capability("DuplicateTaggingPolicy"),
];

const PICARD_METRICS_TYPE_CAPABILITIES: &[ExternalPackTypeCapability] = &[
    cloneable_final_capability("picard.sam.DuplicationMetrics"),
    cloneable_final_capability("DuplicationMetrics"),
];

const AVRO_TYPE_CAPABILITIES: &[ExternalPackTypeCapability] = &[
    cloneable_final_capability("org.apache.avro.Schema"),
    cloneable_final_capability("org.apache.avro.Schema.Parser"),
    cloneable_final_capability("Schema.Parser"),
    cloneable_final_capability("Parser"),
    cloneable_final_capability("org.apache.avro.generic.GenericDatumReader"),
    cloneable_final_capability("GenericDatumReader"),
];

const BARCLAY_HELP_TYPE_CAPABILITIES: &[ExternalPackTypeCapability] = &[
    cloneable_final_capability("org.broadinstitute.barclay.help.DocException"),
    cloneable_final_capability("DocException"),
    cloneable_final_capability("org.broadinstitute.barclay.help.TemplateProperties"),
    cloneable_final_capability("TemplateProperties"),
];

const BARCLAY_ARGPARSER_TYPE_CAPABILITIES: &[ExternalPackTypeCapability] = &[
    cloneable_final_capability("org.broadinstitute.barclay.argparser.CommandLineException"),
    cloneable_final_capability("CommandLineException"),
    cloneable_final_capability(
        "org.broadinstitute.barclay.argparser.CommandLineException.BadArgumentValue",
    ),
    cloneable_final_capability("CommandLineException.BadArgumentValue"),
    cloneable_final_capability("BadArgumentValue"),
    cloneable_final_capability(
        "org.broadinstitute.barclay.argparser.CommandLineException.MissingArgument",
    ),
    cloneable_final_capability("CommandLineException.MissingArgument"),
    cloneable_final_capability("MissingArgument"),
    cloneable_final_capability("org.broadinstitute.barclay.argparser.CommandLineArgumentParser"),
    cloneable_final_capability("CommandLineArgumentParser"),
    cloneable_final_capability("org.broadinstitute.barclay.argparser.SpecialArgumentsCollection"),
    cloneable_final_capability("SpecialArgumentsCollection"),
];

const BARCLAY_UTILS_TYPE_CAPABILITIES: &[ExternalPackTypeCapability] = &[
    cloneable_final_capability("org.broadinstitute.barclay.utils.ClassFinder"),
    cloneable_final_capability("ClassFinder"),
];

const SCALA_TUPLE_TYPE_CAPABILITIES: &[ExternalPackTypeCapability] = &[
    cloneable_final_capability("scala.Tuple2"),
    cloneable_final_capability("Tuple2"),
    cloneable_final_capability("scala.Tuple3"),
    cloneable_final_capability("Tuple3"),
    cloneable_final_capability("scala.Tuple4"),
    cloneable_final_capability("Tuple4"),
];

const KRYO_TYPE_CAPABILITIES: &[ExternalPackTypeCapability] = &[
    cloneable_final_capability("com.esotericsoftware.kryo.Kryo"),
    cloneable_final_capability("Kryo"),
    cloneable_final_capability("com.esotericsoftware.kryo.Serializer"),
    cloneable_final_capability("com.esotericsoftware.kryo.io.Input"),
    cloneable_final_capability("Input"),
    cloneable_final_capability("com.esotericsoftware.kryo.io.Output"),
    cloneable_final_capability("Output"),
    cloneable_final_capability("com.esotericsoftware.kryo.serializers.FieldSerializer"),
    cloneable_final_capability("FieldSerializer"),
    cloneable_final_capability("com.esotericsoftware.kryo.serializers.ImmutableMapSerializer"),
    cloneable_final_capability("ImmutableMapSerializer"),
];

const OPENCSV_TYPE_CAPABILITIES: &[ExternalPackTypeCapability] = &[
    cloneable_final_capability("com.opencsv.CSVReader"),
    cloneable_final_capability("CSVReader"),
    cloneable_final_capability("com.opencsv.CSVWriter"),
    cloneable_final_capability("CSVWriter"),
];

const ORG_JSON_TYPE_CAPABILITIES: &[ExternalPackTypeCapability] = &[
    cloneable_final_capability("org.json.JSONObject"),
    cloneable_final_capability("JSONObject"),
    cloneable_final_capability("org.json.JSONArray"),
    cloneable_final_capability("JSONArray"),
    cloneable_final_capability("org.json.JSONTokener"),
    cloneable_final_capability("JSONTokener"),
];

const REFLECTIONS_TYPE_CAPABILITIES: &[ExternalPackTypeCapability] = &[
    cloneable_final_capability("org.reflections.Reflections"),
    cloneable_final_capability("Reflections"),
];

const HADOOP_CONF_TYPE_CAPABILITIES: &[ExternalPackTypeCapability] = &[
    cloneable_final_capability("org.apache.hadoop.conf.Configuration"),
    cloneable_final_capability("Configuration"),
];

const APACHE_COMMONS_CONFIG_TYPE_CAPABILITIES: &[ExternalPackTypeCapability] = &[
    cloneable_final_capability("org.apache.commons.configuration2.builder.fluent.Configurations"),
    cloneable_final_capability("Configurations"),
    cloneable_final_capability("org.apache.commons.configuration2.PropertiesConfiguration"),
    cloneable_final_capability("PropertiesConfiguration"),
    cloneable_final_capability("org.apache.commons.configuration2.XMLConfiguration"),
    cloneable_final_capability("XMLConfiguration"),
    cloneable_final_capability("org.apache.commons.configuration2.INIConfiguration"),
    cloneable_final_capability("INIConfiguration"),
];

const SPARK_TYPE_CAPABILITIES: &[ExternalPackTypeCapability] = &[
    cloneable_final_capability("org.apache.spark.SparkConf"),
    cloneable_final_capability("SparkConf"),
    cloneable_final_capability("org.apache.spark.HashPartitioner"),
    cloneable_final_capability("HashPartitioner"),
    cloneable_final_capability("org.apache.spark.api.java.JavaSparkContext"),
    cloneable_final_capability("JavaSparkContext"),
    cloneable_final_capability("org.apache.spark.api.java.JavaRDD"),
    cloneable_final_capability("JavaRDD"),
    cloneable_final_capability("org.apache.spark.api.java.JavaPairRDD"),
    cloneable_final_capability("JavaPairRDD"),
    cloneable_final_capability("org.apache.spark.rdd.PartitionGroup"),
    cloneable_final_capability("PartitionGroup"),
    cloneable_final_capability("org.apache.spark.mllib.linalg.distributed.RowMatrix"),
    cloneable_final_capability("RowMatrix"),
    cloneable_final_capability("org.apache.spark.broadcast.Broadcast"),
    cloneable_final_capability("Broadcast"),
];

const BWA_TYPE_CAPABILITIES: &[ExternalPackTypeCapability] = &[
    cloneable_final_capability("org.broadinstitute.hellbender.utils.bwa.BwaMemIndex"),
    cloneable_final_capability("BwaMemIndex"),
    cloneable_final_capability("org.broadinstitute.hellbender.utils.bwa.BwaMemAligner"),
    cloneable_final_capability("BwaMemAligner"),
];

const JGRAPHT_TYPE_CAPABILITIES: &[ExternalPackTypeCapability] = &[
    cloneable_final_capability("org.jgrapht.Graph"),
    cloneable_final_capability("Graph"),
    cloneable_final_capability("org.jgrapht.alg.ConnectivityInspector"),
    cloneable_final_capability("ConnectivityInspector"),
    cloneable_final_capability("org.jgrapht.alg.CycleDetector"),
    cloneable_final_capability("CycleDetector"),
    cloneable_final_capability("org.jgrapht.graph.SimpleGraph"),
    cloneable_final_capability("SimpleGraph"),
    cloneable_final_capability("org.jgrapht.graph.DefaultDirectedWeightedGraph"),
    cloneable_final_capability("DefaultDirectedWeightedGraph"),
    cloneable_final_capability("org.jgrapht.io.IntegerComponentNameProvider"),
    cloneable_final_capability("IntegerComponentNameProvider"),
    cloneable_final_capability("org.jgrapht.io.DOTExporter"),
    cloneable_final_capability("DOTExporter"),
];

const FASTUTIL_TYPE_CAPABILITIES: &[ExternalPackTypeCapability] = &[
    cloneable_final_capability("it.unimi.dsi.fastutil.ints.Int2ObjectArrayMap"),
    cloneable_final_capability("Int2ObjectArrayMap"),
    cloneable_final_capability("it.unimi.dsi.fastutil.ints.Int2ObjectOpenHashMap"),
    cloneable_final_capability("Int2ObjectOpenHashMap"),
    cloneable_final_capability("it.unimi.dsi.fastutil.ints.IntArrayList"),
    cloneable_final_capability("IntArrayList"),
    cloneable_final_capability("it.unimi.dsi.fastutil.ints.Int2IntArrayMap"),
    cloneable_final_capability("Int2IntArrayMap"),
    cloneable_final_capability("it.unimi.dsi.fastutil.objects.Object2IntOpenHashMap"),
    cloneable_final_capability("Object2IntOpenHashMap"),
];

const KRYO_SERIALIZERS_TYPE_CAPABILITIES: &[ExternalPackTypeCapability] = &[
    cloneable_final_capability("de.javakaffee.kryoserializers.UnmodifiableCollectionsSerializer"),
    cloneable_final_capability("UnmodifiableCollectionsSerializer"),
];

const DISQ_WRITE_TYPE_CAPABILITIES: &[ExternalPackTypeCapability] = &[
    cloneable_final_capability("org.disq_bio.disq.HtsjdkReadsRdd"),
    cloneable_final_capability("HtsjdkReadsRdd"),
    cloneable_final_capability("org.disq_bio.disq.HtsjdkVariantsRdd"),
    cloneable_final_capability("HtsjdkVariantsRdd"),
    cloneable_final_capability("org.disq_bio.disq.HtsjdkReadsTraversalParameters"),
    cloneable_final_capability("HtsjdkReadsTraversalParameters"),
    cloneable_final_capability("org.disq_bio.disq.TempPartsDirectoryWriteOption"),
    cloneable_final_capability("TempPartsDirectoryWriteOption"),
    cloneable_final_capability("org.disq_bio.disq.ReadsFormatWriteOption"),
    cloneable_final_capability("ReadsFormatWriteOption"),
    cloneable_final_capability("org.disq_bio.disq.FileCardinalityWriteOption"),
    cloneable_final_capability("FileCardinalityWriteOption"),
];

const INTEL_GKL_TYPE_CAPABILITIES: &[ExternalPackTypeCapability] = &[
    cloneable_final_capability("com.intel.gkl.compression.IntelDeflaterFactory"),
    cloneable_final_capability("IntelDeflaterFactory"),
    cloneable_final_capability("com.intel.gkl.compression.IntelInflaterFactory"),
    cloneable_final_capability("IntelInflaterFactory"),
    cloneable_final_capability("com.intel.gkl.pdhmm.IntelPDHMM"),
    cloneable_final_capability("IntelPDHMM"),
    cloneable_final_capability("com.intel.gkl.pairhmm.IntelPairHmm"),
    cloneable_final_capability("IntelPairHmm"),
    cloneable_final_capability("com.intel.gkl.pairhmm.IntelPairHmmOMP"),
    cloneable_final_capability("IntelPairHmmOMP"),
    cloneable_final_capability("com.intel.gkl.pdhmm.PDHMMNativeArguments"),
    cloneable_final_capability("PDHMMNativeArguments"),
    cloneable_final_capability("com.intel.gkl.pairhmm.PairHMMNativeArguments"),
    cloneable_final_capability("PairHMMNativeArguments"),
    cloneable_final_capability("com.intel.gkl.pairhmm.IntelPairHmm.ReadDataHolder"),
    cloneable_final_capability("ReadDataHolder"),
    cloneable_final_capability("com.intel.gkl.pairhmm.IntelPairHmm.HaplotypeDataHolder"),
    cloneable_final_capability("HaplotypeDataHolder"),
    cloneable_final_capability(
        "com.intel.gkl.pairhmm.IntelPairHmm.BatchCompletionCallbackFunctionArgument",
    ),
    cloneable_final_capability("BatchCompletionCallbackFunctionArgument"),
    cloneable_final_capability("com.intel.gkl.smithwaterman.SWNativeAlignerResult"),
    cloneable_final_capability("SWNativeAlignerResult"),
    cloneable_final_capability("com.intel.gkl.pdhmm.AVXLevel"),
    cloneable_final_capability("AVXLevel"),
    cloneable_final_capability("com.intel.gkl.pdhmm.OpenMPSetting"),
    cloneable_final_capability("OpenMPSetting"),
];

const GUAVA_IMMUTABLE_COLLECT_TYPE_CAPABILITIES: &[ExternalPackTypeCapability] = &[
    cloneable_final_capability("com.google.common.collect.ImmutableMap.Builder"),
    cloneable_final_capability("ImmutableMap.Builder"),
];

const GUAVA_CONCURRENT_TYPE_CAPABILITIES: &[ExternalPackTypeCapability] = &[
    cloneable_final_capability("com.google.common.util.concurrent.ThreadFactoryBuilder"),
    cloneable_final_capability("ThreadFactoryBuilder"),
];

const BROAD_HTTP_TYPE_CAPABILITIES: &[ExternalPackTypeCapability] = &[
    cloneable_final_capability("org.broadinstitute.http.nio.HttpFileSystemProviderSettings"),
    cloneable_final_capability("HttpFileSystemProviderSettings"),
    cloneable_final_capability(
        "org.broadinstitute.http.nio.HttpFileSystemProviderSettings.RetrySettings",
    ),
    cloneable_final_capability("HttpFileSystemProviderSettings.RetrySettings"),
    cloneable_final_capability("RetrySettings"),
    cloneable_final_capability("org.broadinstitute.http.nio.RetryHandler"),
    cloneable_final_capability("RetryHandler"),
    cloneable_final_capability("org.broadinstitute.http.nio.HttpFileSystemProvider"),
    cloneable_final_capability("HttpFileSystemProvider"),
    cloneable_final_capability("org.broadinstitute.http.nio.HttpsFileSystemProvider"),
    cloneable_final_capability("HttpsFileSystemProvider"),
    cloneable_final_capability("org.broadinstitute.hellbender.utils.gcs.BucketUtils"),
    cloneable_final_capability("BucketUtils"),
];

const PICARD_VCF_TYPE_CAPABILITIES: &[ExternalPackTypeCapability] = &[
    cloneable_final_capability("picard.vcf.GenotypeConcordanceStates"),
    cloneable_final_capability("GenotypeConcordanceStates"),
    cloneable_final_capability("picard.vcf.GenotypeConcordance"),
    cloneable_final_capability("GenotypeConcordance"),
    cloneable_final_capability("picard.vcf.GenotypeConcordanceCounts"),
    cloneable_final_capability("GenotypeConcordanceCounts"),
    cloneable_final_capability("picard.vcf.GenotypeConcordanceSummaryMetrics"),
    cloneable_final_capability("GenotypeConcordanceSummaryMetrics"),
    cloneable_final_capability("picard.vcf.GenotypeConcordanceStates.TruthAndCallStates"),
    cloneable_final_capability("GenotypeConcordanceStates.TruthAndCallStates"),
    cloneable_final_capability("TruthAndCallStates"),
    cloneable_final_capability("picard.vcf.GenotypeConcordanceStates.TruthState"),
    cloneable_final_capability("GenotypeConcordanceStates.TruthState"),
    cloneable_final_capability("TruthState"),
    cloneable_final_capability("picard.vcf.GenotypeConcordanceStates.CallState"),
    cloneable_final_capability("GenotypeConcordanceStates.CallState"),
    cloneable_final_capability("CallState"),
];
const JACKSON_DATABIND_TYPE_CAPABILITIES: &[ExternalPackTypeCapability] = &[
    cloneable_final_capability("com.fasterxml.jackson.databind.MapperFeature"),
    cloneable_final_capability("MapperFeature"),
    cloneable_final_capability("com.fasterxml.jackson.databind.DeserializationFeature"),
    cloneable_final_capability("DeserializationFeature"),
];

const SAM_HEADER_TYPE_CAPABILITIES: &[ExternalPackTypeCapability] = &[
    cloneable_final_capability("htsjdk.samtools.SAMSequenceDictionary"),
    cloneable_final_capability("SAMSequenceDictionary"),
    cloneable_final_capability("htsjdk.samtools.SAMSequenceRecord"),
    cloneable_final_capability("SAMSequenceRecord"),
    cloneable_final_capability("htsjdk.samtools.SAMFileHeader"),
    cloneable_final_capability("SAMFileHeader"),
    cloneable_final_capability("htsjdk.samtools.SAMReadGroupRecord"),
    cloneable_final_capability("SAMReadGroupRecord"),
    cloneable_final_capability("htsjdk.samtools.SAMProgramRecord"),
    cloneable_final_capability("SAMProgramRecord"),
    cloneable_final_capability("htsjdk.samtools.SAMTextHeaderCodec"),
    cloneable_final_capability("SAMTextHeaderCodec"),
    extends_capability("htsjdk.samtools.SAMFormatException", "RuntimeException"),
    extends_capability("SAMFormatException", "RuntimeException"),
    cloneable_final_capability("htsjdk.samtools.SAMFileHeader.SortOrder"),
    cloneable_final_capability("SAMFileHeader.SortOrder"),
    cloneable_final_capability("SortOrder"),
    cloneable_final_capability("htsjdk.samtools.SAMFileHeader.GroupOrder"),
    cloneable_final_capability("SAMFileHeader.GroupOrder"),
    cloneable_final_capability("GroupOrder"),
];

const SAM_READER_TYPE_CAPABILITIES: &[ExternalPackTypeCapability] = &[
    cloneable_final_capability("htsjdk.samtools.SamReaderFactory"),
    cloneable_final_capability("SamReaderFactory"),
    cloneable_final_capability("htsjdk.samtools.SamReaderFactory.Option"),
    cloneable_final_capability("SamReaderFactory.Option"),
    cloneable_final_capability("htsjdk.samtools.ValidationStringency"),
    cloneable_final_capability("ValidationStringency"),
    cloneable_final_capability("htsjdk.samtools.SamFileHeaderMerger"),
    cloneable_final_capability("SamFileHeaderMerger"),
    cloneable_final_capability("htsjdk.samtools.MergingSamRecordIterator"),
    cloneable_final_capability("MergingSamRecordIterator"),
];

const SAM_WRITER_TYPE_CAPABILITIES: &[ExternalPackTypeCapability] = &[
    cloneable_final_capability("htsjdk.samtools.SAMFileWriterFactory"),
    cloneable_final_capability("SAMFileWriterFactory"),
];

const SAM_BAM_TYPE_CAPABILITIES: &[ExternalPackTypeCapability] = &[
    cloneable_final_capability("htsjdk.samtools.BAMRecordCodec"),
    cloneable_final_capability("BAMRecordCodec"),
    cloneable_final_capability("htsjdk.samtools.BAMIndexer"),
    cloneable_final_capability("BAMIndexer"),
    cloneable_final_capability("htsjdk.samtools.SBIIndexWriter"),
    cloneable_final_capability("SBIIndexWriter"),
    cloneable_final_capability("htsjdk.samtools.BAMFileConstants"),
    cloneable_final_capability("BAMFileConstants"),
];

const SAM_CRAM_TYPE_CAPABILITIES: &[ExternalPackTypeCapability] = &[
    cloneable_final_capability("htsjdk.samtools.cram.build.CramContainerIterator"),
    cloneable_final_capability("CramContainerIterator"),
    cloneable_final_capability("htsjdk.samtools.cram.structure.Container"),
    cloneable_final_capability("Container"),
    cloneable_final_capability("htsjdk.samtools.cram.structure.DataSeriesType"),
    cloneable_final_capability("DataSeriesType"),
    cloneable_final_capability("htsjdk.samtools.cram.ref.CRAMReferenceRegion"),
    cloneable_final_capability("CRAMReferenceRegion"),
];

const SAM_METRICS_TYPE_CAPABILITIES: &[ExternalPackTypeCapability] = &[
    cloneable_final_capability("htsjdk.samtools.metrics.MetricsFile"),
    cloneable_final_capability("MetricsFile"),
];

const HTSJDK_NESTED_HELPER_TYPE_CAPABILITIES: &[ExternalPackTypeCapability] =
    &[cloneable_final_capability("ComponentMethods")];

const SAM_RECORD_TYPE_CAPABILITIES: &[ExternalPackTypeCapability] = &[
    cloneable_final_capability("htsjdk.samtools.SAMRecord"),
    cloneable_final_capability("SAMRecord"),
    cloneable_final_capability("htsjdk.samtools.SAMTagAndValue"),
    cloneable_final_capability("SAMTagAndValue"),
    cloneable_final_capability("htsjdk.samtools.SAMBinaryTagAndValue"),
    cloneable_final_capability("SAMBinaryTagAndValue"),
    cloneable_final_capability("htsjdk.samtools.Cigar"),
    cloneable_final_capability("Cigar"),
    cloneable_final_capability("htsjdk.samtools.CigarElement"),
    cloneable_final_capability("CigarElement"),
    cloneable_final_capability("htsjdk.samtools.CigarOperator"),
    cloneable_final_capability("CigarOperator"),
    cloneable_final_capability("htsjdk.samtools.DefaultSAMRecordFactory"),
    cloneable_final_capability("DefaultSAMRecordFactory"),
    cloneable_final_capability("htsjdk.samtools.SAMRecordCoordinateComparator"),
    cloneable_final_capability("SAMRecordCoordinateComparator"),
    cloneable_final_capability("htsjdk.samtools.SAMRecordQueryNameComparator"),
    cloneable_final_capability("SAMRecordQueryNameComparator"),
    cloneable_final_capability("htsjdk.samtools.filter.FilteringSamIterator"),
    cloneable_final_capability("FilteringSamIterator"),
];

const SAM_REFERENCE_TYPE_CAPABILITIES: &[ExternalPackTypeCapability] = &[
    cloneable_final_capability("htsjdk.samtools.reference.FastaSequenceFile"),
    cloneable_final_capability("FastaSequenceFile"),
    cloneable_final_capability("htsjdk.samtools.reference.IndexedFastaSequenceFile"),
    cloneable_final_capability("IndexedFastaSequenceFile"),
    cloneable_final_capability("htsjdk.samtools.reference.FastaReferenceWriterBuilder"),
    cloneable_final_capability("FastaReferenceWriterBuilder"),
    cloneable_final_capability("htsjdk.samtools.reference.ReferenceSequence"),
    cloneable_final_capability("ReferenceSequence"),
    cloneable_final_capability("htsjdk.samtools.reference.ReferenceSequenceFileWalker"),
    cloneable_final_capability("ReferenceSequenceFileWalker"),
];

const TRIBBLE_FEATURE_TYPE_CAPABILITIES: &[ExternalPackTypeCapability] = &[
    cloneable_final_capability("htsjdk.tribble.SimpleFeature"),
    cloneable_final_capability("SimpleFeature"),
    cloneable_final_capability("htsjdk.tribble.NamedFeature"),
    cloneable_final_capability("NamedFeature"),
    cloneable_final_capability("htsjdk.tribble.bed.BEDFeature"),
    cloneable_final_capability("BEDFeature"),
    cloneable_final_capability("htsjdk.tribble.FeatureCodecHeader"),
    cloneable_final_capability("FeatureCodecHeader"),
    cloneable_final_capability("htsjdk.tribble.gff.Gff3BaseData"),
    cloneable_final_capability("Gff3BaseData"),
    extends_capability(
        "htsjdk.tribble.readers.CodecLineParsingException",
        "RuntimeException",
    ),
    extends_capability("CodecLineParsingException", "RuntimeException"),
    implements_capability("htsjdk.tribble.SimpleFeature", FEATURE_LOCATABLE_IMPL),
    implements_capability("SimpleFeature", FEATURE_LOCATABLE_IMPL),
    implements_capability("htsjdk.tribble.NamedFeature", FEATURE_LOCATABLE_IMPL),
    implements_capability("NamedFeature", FEATURE_LOCATABLE_IMPL),
    implements_capability("htsjdk.tribble.bed.BEDFeature", FEATURE_LOCATABLE_IMPL),
    implements_capability("BEDFeature", FEATURE_LOCATABLE_IMPL),
];

const TRIBBLE_INDEX_TYPE_CAPABILITIES: &[ExternalPackTypeCapability] = &[
    cloneable_final_capability("htsjdk.tribble.index.IndexFactory"),
    cloneable_final_capability("IndexFactory"),
    cloneable_final_capability("htsjdk.tribble.index.IndexFactory.IndexType"),
    cloneable_final_capability("IndexFactory.IndexType"),
    cloneable_final_capability("IndexType"),
    cloneable_final_capability("htsjdk.tribble.index.IndexFactory.IndexBalanceApproach"),
    cloneable_final_capability("IndexFactory.IndexBalanceApproach"),
    cloneable_final_capability("IndexBalanceApproach"),
    cloneable_final_capability("htsjdk.tribble.index.tabix.TabixFormat"),
    cloneable_final_capability("TabixFormat"),
];

const BCF_TYPE_CAPABILITIES: &[ExternalPackTypeCapability] = &[
    cloneable_final_capability("htsjdk.variant.bcf2.BCF2Codec"),
    cloneable_final_capability("BCF2Codec"),
];

const SAMTOOLS_UTIL_TYPE_CAPABILITIES: &[ExternalPackTypeCapability] = &[
    cloneable_final_implements_capability("htsjdk.samtools.util.Interval", LOCATABLE_IMPL),
    cloneable_final_implements_capability("Interval", LOCATABLE_IMPL),
    cloneable_final_implements_capability("htsjdk.samtools.util.IntervalList", LOCATABLE_IMPL),
    cloneable_final_implements_capability("IntervalList", LOCATABLE_IMPL),
    cloneable_final_capability("htsjdk.samtools.util.IntervalTree"),
    cloneable_final_capability("IntervalTree"),
    cloneable_final_capability("htsjdk.samtools.util.Histogram"),
    cloneable_final_capability("Histogram"),
    cloneable_final_capability("htsjdk.samtools.util.PeekableIterator"),
    cloneable_final_capability("PeekableIterator"),
    cloneable_final_capability("htsjdk.samtools.util.MergingIterator"),
    cloneable_final_capability("MergingIterator"),
    cloneable_final_capability("htsjdk.samtools.util.DefaultingMap"),
    cloneable_final_capability("DefaultingMap"),
    cloneable_final_capability("htsjdk.samtools.util.Lazy"),
    cloneable_final_capability("Lazy"),
    cloneable_final_capability("htsjdk.samtools.seekablestream.SeekableBufferedStream"),
    cloneable_final_capability("SeekableBufferedStream"),
    cloneable_final_capability("htsjdk.samtools.seekablestream.SeekablePathStream"),
    cloneable_final_capability("SeekablePathStream"),
    cloneable_final_capability("htsjdk.samtools.util.BinaryCodec"),
    cloneable_final_capability("BinaryCodec"),
    cloneable_final_capability("htsjdk.samtools.util.BufferedLineReader"),
    cloneable_final_capability("BufferedLineReader"),
    cloneable_final_capability("htsjdk.samtools.util.AsciiLineReaderIterator"),
    cloneable_final_capability("AsciiLineReaderIterator"),
    cloneable_final_capability("htsjdk.samtools.util.LineIteratorImpl"),
    cloneable_final_capability("LineIteratorImpl"),
    cloneable_final_capability("htsjdk.samtools.util.SynchronousLineReader"),
    cloneable_final_capability("SynchronousLineReader"),
    cloneable_final_capability("htsjdk.samtools.Defaults"),
    cloneable_final_capability("Defaults"),
    cloneable_final_capability("htsjdk.samtools.SAMUtils"),
    cloneable_final_capability("SAMUtils"),
    cloneable_final_capability("htsjdk.samtools.ReservedTagConstants"),
    cloneable_final_capability("ReservedTagConstants"),
    cloneable_final_capability("htsjdk.samtools.fastq.FastqConstants"),
    cloneable_final_capability("FastqConstants"),
    cloneable_final_capability("htsjdk.samtools.util.SequenceUtil"),
    cloneable_final_capability("SequenceUtil"),
    cloneable_final_capability("htsjdk.samtools.util.BlockCompressedStreamConstants"),
    cloneable_final_capability("BlockCompressedStreamConstants"),
    cloneable_final_capability("htsjdk.samtools.util.BlockCompressedInputStream"),
    cloneable_final_capability("BlockCompressedInputStream"),
    cloneable_final_capability("htsjdk.samtools.util.BlockCompressedInputStream.FileTermination"),
    cloneable_final_capability("BlockCompressedInputStream.FileTermination"),
    cloneable_final_capability("FileTermination"),
    cloneable_final_capability("htsjdk.samtools.util.BlockCompressedOutputStream"),
    cloneable_final_capability("BlockCompressedOutputStream"),
    cloneable_final_capability("htsjdk.samtools.util.Log"),
    cloneable_final_capability("Log"),
    cloneable_final_capability("htsjdk.samtools.util.Log.LogLevel"),
    cloneable_final_capability("Log.LogLevel"),
    cloneable_final_capability("LogLevel"),
    cloneable_final_capability("htsjdk.samtools.SamPairUtil"),
    cloneable_final_capability("SamPairUtil"),
    cloneable_final_capability("htsjdk.samtools.SamPairUtil.PairOrientation"),
    cloneable_final_capability("SamPairUtil.PairOrientation"),
    cloneable_final_capability("PairOrientation"),
];

const NO_ARGUMENT_EFFECTS: &[ExternalPackArgumentEffect] = &[];
const NO_FIELD_NULLABILITY: &[ExternalPackFieldNullability] = &[];
const NO_EXCEPTIONS: &[ExternalPackException] = &[];

const fn semantic_getter_effect(
    receiver_type: &'static str,
    method_name: &'static str,
    return_nullability: NullabilityState,
) -> ExternalPackApiEffect {
    api_effect(
        receiver_type,
        method_name,
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        None,
        return_nullability,
        &[],
    )
}

const fn semantic_boolean_getter_effect(
    receiver_type: &'static str,
    method_name: &'static str,
    return_nullability: NullabilityState,
) -> ExternalPackApiEffect {
    api_effect(
        receiver_type,
        method_name,
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        None,
        return_nullability,
        &[],
    )
}

const fn semantic_indexed_getter_effect(
    receiver_type: &'static str,
    method_name: &'static str,
    return_nullability: NullabilityState,
) -> ExternalPackApiEffect {
    api_effect(
        receiver_type,
        method_name,
        ExternalApiRole::IndexedGetter,
        ReceiverMutabilityFact::ReadOnly,
        None,
        return_nullability,
        &[],
    )
}

const fn semantic_mutator_effect(
    receiver_type: &'static str,
    method_name: &'static str,
    return_nullability: NullabilityState,
) -> ExternalPackApiEffect {
    api_effect(
        receiver_type,
        method_name,
        ExternalApiRole::Mutator,
        ReceiverMutabilityFact::MutatesReceiver,
        None,
        return_nullability,
        &[],
    )
}

const fn semantic_side_effect(
    receiver_type: &'static str,
    method_name: &'static str,
    return_nullability: NullabilityState,
) -> ExternalPackApiEffect {
    api_effect(
        receiver_type,
        method_name,
        ExternalApiRole::SideEffect,
        ReceiverMutabilityFact::ReadOnly,
        None,
        return_nullability,
        &[],
    )
}

const fn semantic_side_effect_with_arity(
    receiver_type: &'static str,
    method_name: &'static str,
    arg_count: usize,
    return_nullability: NullabilityState,
) -> ExternalPackApiEffect {
    api_effect_with_arity(
        receiver_type,
        method_name,
        arg_count,
        ExternalApiRole::SideEffect,
        ReceiverMutabilityFact::ReadOnly,
        None,
        return_nullability,
        &[],
    )
}

const fn semantic_setter_effect(
    receiver_type: &'static str,
    method_name: &'static str,
    return_nullability: NullabilityState,
) -> ExternalPackApiEffect {
    api_effect(
        receiver_type,
        method_name,
        ExternalApiRole::Setter,
        ReceiverMutabilityFact::MutatesReceiver,
        None,
        return_nullability,
        &[],
    )
}

const HDF5_THROWN: &[&str] = &["HDF5Error"];
const SQLITE_THROWN: &[&str] = &["SQLException"];
const JAVA_SQL_THROWN: &[&str] = &["java.sql.SQLException"];
const VARIANT_VALIDATION_THROWN: &[&str] =
    &["htsjdk.tribble.TribbleException.InternalCodecException"];
const SAM_DICTIONARY_ASSERT_THROWN: &[&str] = &["AssertionError"];

const PICARD_PROGRAMGROUPS_API_EFFECTS: &[ExternalPackApiEffect] = &[
    api_effect(
        "picard.cmdline.programgroups.DiagnosticsAndQCProgramGroup",
        "getName",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        Some("String"),
        NullabilityState::DefinitelyNonNull,
        &[],
    ),
    api_effect(
        "DiagnosticsAndQCProgramGroup",
        "getName",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        Some("String"),
        NullabilityState::DefinitelyNonNull,
        &[],
    ),
    api_effect(
        "picard.cmdline.programgroups.IntervalsManipulationProgramGroup",
        "getName",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        Some("String"),
        NullabilityState::DefinitelyNonNull,
        &[],
    ),
    api_effect(
        "IntervalsManipulationProgramGroup",
        "getName",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        Some("String"),
        NullabilityState::DefinitelyNonNull,
        &[],
    ),
    api_effect(
        "picard.cmdline.programgroups.OtherProgramGroup",
        "getName",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        Some("String"),
        NullabilityState::DefinitelyNonNull,
        &[],
    ),
    api_effect(
        "OtherProgramGroup",
        "getName",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        Some("String"),
        NullabilityState::DefinitelyNonNull,
        &[],
    ),
    api_effect(
        "picard.cmdline.programgroups.ReadDataManipulationProgramGroup",
        "getName",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        Some("String"),
        NullabilityState::DefinitelyNonNull,
        &[],
    ),
    api_effect(
        "ReadDataManipulationProgramGroup",
        "getName",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        Some("String"),
        NullabilityState::DefinitelyNonNull,
        &[],
    ),
    api_effect(
        "picard.cmdline.programgroups.ReferenceProgramGroup",
        "getName",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        Some("String"),
        NullabilityState::DefinitelyNonNull,
        &[],
    ),
    api_effect(
        "ReferenceProgramGroup",
        "getName",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        Some("String"),
        NullabilityState::DefinitelyNonNull,
        &[],
    ),
    api_effect(
        "picard.cmdline.programgroups.VariantEvaluationProgramGroup",
        "getName",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        Some("String"),
        NullabilityState::DefinitelyNonNull,
        &[],
    ),
    api_effect(
        "VariantEvaluationProgramGroup",
        "getName",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        Some("String"),
        NullabilityState::DefinitelyNonNull,
        &[],
    ),
    api_effect(
        "picard.cmdline.programgroups.VariantFilteringProgramGroup",
        "getName",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        Some("String"),
        NullabilityState::DefinitelyNonNull,
        &[],
    ),
    api_effect(
        "VariantFilteringProgramGroup",
        "getName",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        Some("String"),
        NullabilityState::DefinitelyNonNull,
        &[],
    ),
    api_effect(
        "picard.cmdline.programgroups.VariantManipulationProgramGroup",
        "getName",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        Some("String"),
        NullabilityState::DefinitelyNonNull,
        &[],
    ),
    api_effect(
        "VariantManipulationProgramGroup",
        "getName",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        Some("String"),
        NullabilityState::DefinitelyNonNull,
        &[],
    ),
    api_effect(
        "picard.cmdline.programgroups.DiagnosticsAndQCProgramGroup",
        "getDescription",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        Some("String"),
        NullabilityState::DefinitelyNonNull,
        &[],
    ),
    api_effect(
        "DiagnosticsAndQCProgramGroup",
        "getDescription",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        Some("String"),
        NullabilityState::DefinitelyNonNull,
        &[],
    ),
    api_effect(
        "picard.cmdline.programgroups.IntervalsManipulationProgramGroup",
        "getDescription",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        Some("String"),
        NullabilityState::DefinitelyNonNull,
        &[],
    ),
    api_effect(
        "IntervalsManipulationProgramGroup",
        "getDescription",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        Some("String"),
        NullabilityState::DefinitelyNonNull,
        &[],
    ),
    api_effect(
        "picard.cmdline.programgroups.OtherProgramGroup",
        "getDescription",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        Some("String"),
        NullabilityState::DefinitelyNonNull,
        &[],
    ),
    api_effect(
        "OtherProgramGroup",
        "getDescription",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        Some("String"),
        NullabilityState::DefinitelyNonNull,
        &[],
    ),
    api_effect(
        "picard.cmdline.programgroups.ReadDataManipulationProgramGroup",
        "getDescription",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        Some("String"),
        NullabilityState::DefinitelyNonNull,
        &[],
    ),
    api_effect(
        "ReadDataManipulationProgramGroup",
        "getDescription",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        Some("String"),
        NullabilityState::DefinitelyNonNull,
        &[],
    ),
    api_effect(
        "picard.cmdline.programgroups.ReferenceProgramGroup",
        "getDescription",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        Some("String"),
        NullabilityState::DefinitelyNonNull,
        &[],
    ),
    api_effect(
        "ReferenceProgramGroup",
        "getDescription",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        Some("String"),
        NullabilityState::DefinitelyNonNull,
        &[],
    ),
    api_effect(
        "picard.cmdline.programgroups.VariantEvaluationProgramGroup",
        "getDescription",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        Some("String"),
        NullabilityState::DefinitelyNonNull,
        &[],
    ),
    api_effect(
        "VariantEvaluationProgramGroup",
        "getDescription",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        Some("String"),
        NullabilityState::DefinitelyNonNull,
        &[],
    ),
    api_effect(
        "picard.cmdline.programgroups.VariantFilteringProgramGroup",
        "getDescription",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        Some("String"),
        NullabilityState::DefinitelyNonNull,
        &[],
    ),
    api_effect(
        "VariantFilteringProgramGroup",
        "getDescription",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        Some("String"),
        NullabilityState::DefinitelyNonNull,
        &[],
    ),
    api_effect(
        "picard.cmdline.programgroups.VariantManipulationProgramGroup",
        "getDescription",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        Some("String"),
        NullabilityState::DefinitelyNonNull,
        &[],
    ),
    api_effect(
        "VariantManipulationProgramGroup",
        "getDescription",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        Some("String"),
        NullabilityState::DefinitelyNonNull,
        &[],
    ),
];

const PICARD_PROGRAMGROUPS_MODELED_APIS: &[ExternalPackModeledApi] = &[
    modeled_semantic(
        "picard.cmdline.programgroups.DiagnosticsAndQCProgramGroup",
        "getName",
    ),
    modeled_semantic("DiagnosticsAndQCProgramGroup", "getName"),
    modeled_semantic(
        "picard.cmdline.programgroups.IntervalsManipulationProgramGroup",
        "getName",
    ),
    modeled_semantic("IntervalsManipulationProgramGroup", "getName"),
    modeled_semantic("picard.cmdline.programgroups.OtherProgramGroup", "getName"),
    modeled_semantic("OtherProgramGroup", "getName"),
    modeled_semantic(
        "picard.cmdline.programgroups.ReadDataManipulationProgramGroup",
        "getName",
    ),
    modeled_semantic("ReadDataManipulationProgramGroup", "getName"),
    modeled_semantic(
        "picard.cmdline.programgroups.ReferenceProgramGroup",
        "getName",
    ),
    modeled_semantic("ReferenceProgramGroup", "getName"),
    modeled_semantic(
        "picard.cmdline.programgroups.VariantEvaluationProgramGroup",
        "getName",
    ),
    modeled_semantic("VariantEvaluationProgramGroup", "getName"),
    modeled_semantic(
        "picard.cmdline.programgroups.VariantFilteringProgramGroup",
        "getName",
    ),
    modeled_semantic("VariantFilteringProgramGroup", "getName"),
    modeled_semantic(
        "picard.cmdline.programgroups.VariantManipulationProgramGroup",
        "getName",
    ),
    modeled_semantic("VariantManipulationProgramGroup", "getName"),
    modeled_semantic(
        "picard.cmdline.programgroups.DiagnosticsAndQCProgramGroup",
        "getDescription",
    ),
    modeled_semantic("DiagnosticsAndQCProgramGroup", "getDescription"),
    modeled_semantic(
        "picard.cmdline.programgroups.IntervalsManipulationProgramGroup",
        "getDescription",
    ),
    modeled_semantic("IntervalsManipulationProgramGroup", "getDescription"),
    modeled_semantic(
        "picard.cmdline.programgroups.OtherProgramGroup",
        "getDescription",
    ),
    modeled_semantic("OtherProgramGroup", "getDescription"),
    modeled_semantic(
        "picard.cmdline.programgroups.ReadDataManipulationProgramGroup",
        "getDescription",
    ),
    modeled_semantic("ReadDataManipulationProgramGroup", "getDescription"),
    modeled_semantic(
        "picard.cmdline.programgroups.ReferenceProgramGroup",
        "getDescription",
    ),
    modeled_semantic("ReferenceProgramGroup", "getDescription"),
    modeled_semantic(
        "picard.cmdline.programgroups.VariantEvaluationProgramGroup",
        "getDescription",
    ),
    modeled_semantic("VariantEvaluationProgramGroup", "getDescription"),
    modeled_semantic(
        "picard.cmdline.programgroups.VariantFilteringProgramGroup",
        "getDescription",
    ),
    modeled_semantic("VariantFilteringProgramGroup", "getDescription"),
    modeled_semantic(
        "picard.cmdline.programgroups.VariantManipulationProgramGroup",
        "getDescription",
    ),
    modeled_semantic("VariantManipulationProgramGroup", "getDescription"),
];

const AVRO_API_EFFECTS: &[ExternalPackApiEffect] = &[
    api_effect(
        "org.apache.avro.Schema.Parser",
        "parse",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        Some("org.apache.avro.Schema"),
        NullabilityState::DefinitelyNonNull,
        &[],
    ),
    api_effect(
        "Schema.Parser",
        "parse",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        Some("Schema"),
        NullabilityState::DefinitelyNonNull,
        &[],
    ),
    api_effect(
        "Parser",
        "parse",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        Some("Schema"),
        NullabilityState::DefinitelyNonNull,
        &[],
    ),
];

const AVRO_MODELED_APIS: &[ExternalPackModeledApi] = &[
    modeled_semantic("org.apache.avro.Schema.Parser", "parse"),
    modeled_semantic("Schema.Parser", "parse"),
    modeled_semantic("Parser", "parse"),
];

const SCALA_TUPLE_API_EFFECTS: &[ExternalPackApiEffect] = &[
    api_effect(
        "scala.Tuple2",
        "_1",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        Some("Object"),
        NullabilityState::MaybeNull,
        &[],
    ),
    api_effect(
        "Tuple2",
        "_1",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        Some("Object"),
        NullabilityState::MaybeNull,
        &[],
    ),
    api_effect(
        "scala.Tuple2",
        "_2",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        Some("Object"),
        NullabilityState::MaybeNull,
        &[],
    ),
    api_effect(
        "Tuple2",
        "_2",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        Some("Object"),
        NullabilityState::MaybeNull,
        &[],
    ),
    api_effect(
        "scala.Tuple3",
        "_1",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        Some("Object"),
        NullabilityState::MaybeNull,
        &[],
    ),
    api_effect(
        "Tuple3",
        "_1",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        Some("Object"),
        NullabilityState::MaybeNull,
        &[],
    ),
    api_effect(
        "scala.Tuple3",
        "_2",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        Some("Object"),
        NullabilityState::MaybeNull,
        &[],
    ),
    api_effect(
        "Tuple3",
        "_2",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        Some("Object"),
        NullabilityState::MaybeNull,
        &[],
    ),
    api_effect(
        "scala.Tuple3",
        "_3",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        Some("Object"),
        NullabilityState::MaybeNull,
        &[],
    ),
    api_effect(
        "Tuple3",
        "_3",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        Some("Object"),
        NullabilityState::MaybeNull,
        &[],
    ),
    api_effect(
        "scala.Tuple4",
        "_1",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        Some("Object"),
        NullabilityState::MaybeNull,
        &[],
    ),
    api_effect(
        "Tuple4",
        "_1",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        Some("Object"),
        NullabilityState::MaybeNull,
        &[],
    ),
    api_effect(
        "scala.Tuple4",
        "_2",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        Some("Object"),
        NullabilityState::MaybeNull,
        &[],
    ),
    api_effect(
        "Tuple4",
        "_2",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        Some("Object"),
        NullabilityState::MaybeNull,
        &[],
    ),
    api_effect(
        "scala.Tuple4",
        "_3",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        Some("Object"),
        NullabilityState::MaybeNull,
        &[],
    ),
    api_effect(
        "Tuple4",
        "_3",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        Some("Object"),
        NullabilityState::MaybeNull,
        &[],
    ),
    api_effect(
        "scala.Tuple4",
        "_4",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        Some("Object"),
        NullabilityState::MaybeNull,
        &[],
    ),
    api_effect(
        "Tuple4",
        "_4",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        Some("Object"),
        NullabilityState::MaybeNull,
        &[],
    ),
];

const SCALA_TUPLE_MODELED_APIS: &[ExternalPackModeledApi] = &[
    modeled_semantic("scala.Tuple2", "_1"),
    modeled_semantic("Tuple2", "_1"),
    modeled_semantic("scala.Tuple2", "_2"),
    modeled_semantic("Tuple2", "_2"),
    modeled_semantic("scala.Tuple3", "_1"),
    modeled_semantic("Tuple3", "_1"),
    modeled_semantic("scala.Tuple3", "_2"),
    modeled_semantic("Tuple3", "_2"),
    modeled_semantic("scala.Tuple3", "_3"),
    modeled_semantic("Tuple3", "_3"),
    modeled_semantic("scala.Tuple4", "_1"),
    modeled_semantic("Tuple4", "_1"),
    modeled_semantic("scala.Tuple4", "_2"),
    modeled_semantic("Tuple4", "_2"),
    modeled_semantic("scala.Tuple4", "_3"),
    modeled_semantic("Tuple4", "_3"),
    modeled_semantic("scala.Tuple4", "_4"),
    modeled_semantic("Tuple4", "_4"),
];

const KRYO_API_EFFECTS: &[ExternalPackApiEffect] = &[
    semantic_typed_effect_with_arity(
        "com.esotericsoftware.kryo.Kryo",
        "register",
        1,
        ExternalApiRole::Mutator,
        ReceiverMutabilityFact::MutatesReceiver,
        "com.esotericsoftware.kryo.Registration",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect_with_arity(
        "Kryo",
        "register",
        1,
        ExternalApiRole::Mutator,
        ReceiverMutabilityFact::MutatesReceiver,
        "Registration",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect_with_arity(
        "com.esotericsoftware.kryo.Kryo",
        "register",
        2,
        ExternalApiRole::Mutator,
        ReceiverMutabilityFact::MutatesReceiver,
        "com.esotericsoftware.kryo.Registration",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect_with_arity(
        "Kryo",
        "register",
        2,
        ExternalApiRole::Mutator,
        ReceiverMutabilityFact::MutatesReceiver,
        "Registration",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "com.esotericsoftware.kryo.Kryo",
        "readClassAndObject",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Object",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "Kryo",
        "readClassAndObject",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Object",
        NullabilityState::MaybeNull,
    ),
    semantic_side_effect(
        "com.esotericsoftware.kryo.Kryo",
        "writeClassAndObject",
        NullabilityState::Unknown,
    ),
    semantic_side_effect("Kryo", "writeClassAndObject", NullabilityState::Unknown),
    semantic_typed_effect(
        "com.esotericsoftware.kryo.io.Input",
        "readBoolean",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::MutatesReceiver,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "Input",
        "readBoolean",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::MutatesReceiver,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "com.esotericsoftware.kryo.io.Input",
        "readByte",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::MutatesReceiver,
        "byte",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "Input",
        "readByte",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::MutatesReceiver,
        "byte",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "com.esotericsoftware.kryo.io.Input",
        "readShort",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::MutatesReceiver,
        "short",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "Input",
        "readShort",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::MutatesReceiver,
        "short",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "com.esotericsoftware.kryo.io.Input",
        "readInt",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::MutatesReceiver,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "Input",
        "readInt",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::MutatesReceiver,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "com.esotericsoftware.kryo.io.Input",
        "readLong",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::MutatesReceiver,
        "long",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "Input",
        "readLong",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::MutatesReceiver,
        "long",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "com.esotericsoftware.kryo.io.Input",
        "readFloat",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::MutatesReceiver,
        "float",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "Input",
        "readFloat",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::MutatesReceiver,
        "float",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "com.esotericsoftware.kryo.io.Input",
        "readBytes",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::MutatesReceiver,
        "byte[]",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "Input",
        "readBytes",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::MutatesReceiver,
        "byte[]",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "com.esotericsoftware.kryo.io.Input",
        "readString",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::MutatesReceiver,
        "String",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "Input",
        "readString",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::MutatesReceiver,
        "String",
        NullabilityState::MaybeNull,
    ),
    semantic_side_effect(
        "com.esotericsoftware.kryo.io.Output",
        "writeBoolean",
        NullabilityState::Unknown,
    ),
    semantic_side_effect("Output", "writeBoolean", NullabilityState::Unknown),
    semantic_side_effect(
        "com.esotericsoftware.kryo.io.Output",
        "writeByte",
        NullabilityState::Unknown,
    ),
    semantic_side_effect("Output", "writeByte", NullabilityState::Unknown),
    semantic_side_effect(
        "com.esotericsoftware.kryo.io.Output",
        "writeShort",
        NullabilityState::Unknown,
    ),
    semantic_side_effect("Output", "writeShort", NullabilityState::Unknown),
    semantic_side_effect(
        "com.esotericsoftware.kryo.io.Output",
        "writeInt",
        NullabilityState::Unknown,
    ),
    semantic_side_effect("Output", "writeInt", NullabilityState::Unknown),
    semantic_side_effect(
        "com.esotericsoftware.kryo.io.Output",
        "writeLong",
        NullabilityState::Unknown,
    ),
    semantic_side_effect("Output", "writeLong", NullabilityState::Unknown),
    semantic_side_effect(
        "com.esotericsoftware.kryo.io.Output",
        "writeFloat",
        NullabilityState::Unknown,
    ),
    semantic_side_effect("Output", "writeFloat", NullabilityState::Unknown),
    semantic_side_effect(
        "com.esotericsoftware.kryo.io.Output",
        "writeBytes",
        NullabilityState::Unknown,
    ),
    semantic_side_effect("Output", "writeBytes", NullabilityState::Unknown),
    semantic_side_effect(
        "com.esotericsoftware.kryo.io.Output",
        "writeString",
        NullabilityState::Unknown,
    ),
    semantic_side_effect("Output", "writeString", NullabilityState::Unknown),
    semantic_side_effect(
        "com.esotericsoftware.kryo.io.Output",
        "writeAscii",
        NullabilityState::Unknown,
    ),
    semantic_side_effect("Output", "writeAscii", NullabilityState::Unknown),
    semantic_side_effect(
        "com.esotericsoftware.kryo.io.Output",
        "close",
        NullabilityState::Unknown,
    ),
    semantic_side_effect("Output", "close", NullabilityState::Unknown),
];

const KRYO_MODELED_APIS: &[ExternalPackModeledApi] = &[
    modeled_semantic("com.esotericsoftware.kryo.Kryo", "register"),
    modeled_semantic("Kryo", "register"),
    modeled_semantic("com.esotericsoftware.kryo.Kryo", "readClassAndObject"),
    modeled_semantic("Kryo", "readClassAndObject"),
    modeled_semantic("com.esotericsoftware.kryo.Kryo", "writeClassAndObject"),
    modeled_semantic("Kryo", "writeClassAndObject"),
    modeled_semantic("com.esotericsoftware.kryo.io.Input", "readBoolean"),
    modeled_semantic("Input", "readBoolean"),
    modeled_semantic("com.esotericsoftware.kryo.io.Input", "readByte"),
    modeled_semantic("Input", "readByte"),
    modeled_semantic("com.esotericsoftware.kryo.io.Input", "readShort"),
    modeled_semantic("Input", "readShort"),
    modeled_semantic("com.esotericsoftware.kryo.io.Input", "readInt"),
    modeled_semantic("Input", "readInt"),
    modeled_semantic("com.esotericsoftware.kryo.io.Input", "readLong"),
    modeled_semantic("Input", "readLong"),
    modeled_semantic("com.esotericsoftware.kryo.io.Input", "readFloat"),
    modeled_semantic("Input", "readFloat"),
    modeled_semantic("com.esotericsoftware.kryo.io.Input", "readBytes"),
    modeled_semantic("Input", "readBytes"),
    modeled_semantic("com.esotericsoftware.kryo.io.Input", "readString"),
    modeled_semantic("Input", "readString"),
    modeled_semantic("com.esotericsoftware.kryo.io.Output", "writeBoolean"),
    modeled_semantic("Output", "writeBoolean"),
    modeled_semantic("com.esotericsoftware.kryo.io.Output", "writeByte"),
    modeled_semantic("Output", "writeByte"),
    modeled_semantic("com.esotericsoftware.kryo.io.Output", "writeShort"),
    modeled_semantic("Output", "writeShort"),
    modeled_semantic("com.esotericsoftware.kryo.io.Output", "writeInt"),
    modeled_semantic("Output", "writeInt"),
    modeled_semantic("com.esotericsoftware.kryo.io.Output", "writeLong"),
    modeled_semantic("Output", "writeLong"),
    modeled_semantic("com.esotericsoftware.kryo.io.Output", "writeFloat"),
    modeled_semantic("Output", "writeFloat"),
    modeled_semantic("com.esotericsoftware.kryo.io.Output", "writeBytes"),
    modeled_semantic("Output", "writeBytes"),
    modeled_semantic("com.esotericsoftware.kryo.io.Output", "writeString"),
    modeled_semantic("Output", "writeString"),
    modeled_semantic("com.esotericsoftware.kryo.io.Output", "writeAscii"),
    modeled_semantic("Output", "writeAscii"),
    modeled_semantic("com.esotericsoftware.kryo.io.Output", "close"),
    modeled_semantic("Output", "close"),
];

const APACHE_COMMONS_COLLECTIONS_API_EFFECTS: &[ExternalPackApiEffect] = &[
    semantic_typed_effect(
        "org.apache.commons.collections4.MultiMap",
        "append",
        ExternalApiRole::Mutator,
        ReceiverMutabilityFact::MutatesReceiver,
        "Object",
        NullabilityState::Unknown,
    ),
    semantic_typed_effect(
        "MultiMap",
        "append",
        ExternalApiRole::Mutator,
        ReceiverMutabilityFact::MutatesReceiver,
        "Object",
        NullabilityState::Unknown,
    ),
    semantic_typed_effect(
        "org.apache.commons.collections4.MultiMap",
        "get",
        ExternalApiRole::IndexedGetter,
        ReceiverMutabilityFact::ReadOnly,
        "Object",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "MultiMap",
        "get",
        ExternalApiRole::IndexedGetter,
        ReceiverMutabilityFact::ReadOnly,
        "Object",
        NullabilityState::MaybeNull,
    ),
];

const APACHE_COMMONS_COLLECTIONS_MODELED_APIS: &[ExternalPackModeledApi] = &[
    modeled_semantic("org.apache.commons.collections4.MultiMap", "append"),
    modeled_semantic("MultiMap", "append"),
    modeled_semantic("org.apache.commons.collections4.MultiMap", "get"),
    modeled_semantic("MultiMap", "get"),
];

const OPENCSV_API_EFFECTS: &[ExternalPackApiEffect] = &[
    semantic_typed_effect_with_arity(
        "com.opencsv.CSVReader",
        "readNext",
        0,
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::MutatesReceiver,
        "String[]",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect_with_arity(
        "CSVReader",
        "readNext",
        0,
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::MutatesReceiver,
        "String[]",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect_with_arity(
        "com.opencsv.CSVReader",
        "readAll",
        0,
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::MutatesReceiver,
        "List<String[]>",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect_with_arity(
        "CSVReader",
        "readAll",
        0,
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::MutatesReceiver,
        "List<String[]>",
        NullabilityState::DefinitelyNonNull,
    ),
    api_effect_with_arity(
        "com.opencsv.CSVReader",
        "close",
        0,
        ExternalApiRole::SideEffect,
        ReceiverMutabilityFact::MutatesReceiver,
        None,
        NullabilityState::Unknown,
        &[],
    ),
    api_effect_with_arity(
        "CSVReader",
        "close",
        0,
        ExternalApiRole::SideEffect,
        ReceiverMutabilityFact::MutatesReceiver,
        None,
        NullabilityState::Unknown,
        &[],
    ),
    api_effect_with_arity(
        "com.opencsv.CSVWriter",
        "writeNext",
        1,
        ExternalApiRole::Mutator,
        ReceiverMutabilityFact::MutatesReceiver,
        None,
        NullabilityState::Unknown,
        &[],
    ),
    api_effect_with_arity(
        "CSVWriter",
        "writeNext",
        1,
        ExternalApiRole::Mutator,
        ReceiverMutabilityFact::MutatesReceiver,
        None,
        NullabilityState::Unknown,
        &[],
    ),
    api_effect_with_arity(
        "com.opencsv.CSVWriter",
        "writeNext",
        2,
        ExternalApiRole::Mutator,
        ReceiverMutabilityFact::MutatesReceiver,
        None,
        NullabilityState::Unknown,
        &[],
    ),
    api_effect_with_arity(
        "CSVWriter",
        "writeNext",
        2,
        ExternalApiRole::Mutator,
        ReceiverMutabilityFact::MutatesReceiver,
        None,
        NullabilityState::Unknown,
        &[],
    ),
    api_effect_with_arity(
        "com.opencsv.CSVWriter",
        "writeAll",
        1,
        ExternalApiRole::Mutator,
        ReceiverMutabilityFact::MutatesReceiver,
        None,
        NullabilityState::Unknown,
        &[],
    ),
    api_effect_with_arity(
        "CSVWriter",
        "writeAll",
        1,
        ExternalApiRole::Mutator,
        ReceiverMutabilityFact::MutatesReceiver,
        None,
        NullabilityState::Unknown,
        &[],
    ),
    api_effect_with_arity(
        "com.opencsv.CSVWriter",
        "flush",
        0,
        ExternalApiRole::SideEffect,
        ReceiverMutabilityFact::MutatesReceiver,
        None,
        NullabilityState::Unknown,
        &[],
    ),
    api_effect_with_arity(
        "CSVWriter",
        "flush",
        0,
        ExternalApiRole::SideEffect,
        ReceiverMutabilityFact::MutatesReceiver,
        None,
        NullabilityState::Unknown,
        &[],
    ),
    api_effect_with_arity(
        "com.opencsv.CSVWriter",
        "close",
        0,
        ExternalApiRole::SideEffect,
        ReceiverMutabilityFact::MutatesReceiver,
        None,
        NullabilityState::Unknown,
        &[],
    ),
    api_effect_with_arity(
        "CSVWriter",
        "close",
        0,
        ExternalApiRole::SideEffect,
        ReceiverMutabilityFact::MutatesReceiver,
        None,
        NullabilityState::Unknown,
        &[],
    ),
];

const OPENCSV_MODELED_APIS: &[ExternalPackModeledApi] = &[
    modeled_semantic("com.opencsv.CSVReader", "readNext"),
    modeled_semantic("CSVReader", "readNext"),
    modeled_semantic("com.opencsv.CSVReader", "readAll"),
    modeled_semantic("CSVReader", "readAll"),
    modeled_semantic("com.opencsv.CSVReader", "close"),
    modeled_semantic("CSVReader", "close"),
    modeled_semantic("com.opencsv.CSVWriter", "writeNext"),
    modeled_semantic("CSVWriter", "writeNext"),
    modeled_semantic("com.opencsv.CSVWriter", "writeAll"),
    modeled_semantic("CSVWriter", "writeAll"),
    modeled_semantic("com.opencsv.CSVWriter", "flush"),
    modeled_semantic("CSVWriter", "flush"),
    modeled_semantic("com.opencsv.CSVWriter", "close"),
    modeled_semantic("CSVWriter", "close"),
];

const ORG_JSON_API_EFFECTS: &[ExternalPackApiEffect] = &[
    semantic_typed_effect(
        "org.json.JSONObject",
        "put",
        ExternalApiRole::FluentMutator,
        ReceiverMutabilityFact::MutatesReceiver,
        "JSONObject",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "JSONObject",
        "put",
        ExternalApiRole::FluentMutator,
        ReceiverMutabilityFact::MutatesReceiver,
        "JSONObject",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "org.json.JSONObject",
        "get",
        ExternalApiRole::IndexedGetter,
        ReceiverMutabilityFact::ReadOnly,
        "Object",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "JSONObject",
        "get",
        ExternalApiRole::IndexedGetter,
        ReceiverMutabilityFact::ReadOnly,
        "Object",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "org.json.JSONObject",
        "opt",
        ExternalApiRole::IndexedGetter,
        ReceiverMutabilityFact::ReadOnly,
        "Object",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "JSONObject",
        "opt",
        ExternalApiRole::IndexedGetter,
        ReceiverMutabilityFact::ReadOnly,
        "Object",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "org.json.JSONObject",
        "getString",
        ExternalApiRole::IndexedGetter,
        ReceiverMutabilityFact::ReadOnly,
        "String",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "JSONObject",
        "getString",
        ExternalApiRole::IndexedGetter,
        ReceiverMutabilityFact::ReadOnly,
        "String",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "org.json.JSONObject",
        "optString",
        ExternalApiRole::IndexedGetter,
        ReceiverMutabilityFact::ReadOnly,
        "String",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "JSONObject",
        "optString",
        ExternalApiRole::IndexedGetter,
        ReceiverMutabilityFact::ReadOnly,
        "String",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "org.json.JSONObject",
        "getInt",
        ExternalApiRole::IndexedGetter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "JSONObject",
        "getInt",
        ExternalApiRole::IndexedGetter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "org.json.JSONObject",
        "has",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "JSONObject",
        "has",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "org.json.JSONObject",
        "length",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "JSONObject",
        "length",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "org.json.JSONArray",
        "put",
        ExternalApiRole::FluentMutator,
        ReceiverMutabilityFact::MutatesReceiver,
        "JSONArray",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "JSONArray",
        "put",
        ExternalApiRole::FluentMutator,
        ReceiverMutabilityFact::MutatesReceiver,
        "JSONArray",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "org.json.JSONArray",
        "get",
        ExternalApiRole::IndexedGetter,
        ReceiverMutabilityFact::ReadOnly,
        "Object",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "JSONArray",
        "get",
        ExternalApiRole::IndexedGetter,
        ReceiverMutabilityFact::ReadOnly,
        "Object",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "org.json.JSONArray",
        "opt",
        ExternalApiRole::IndexedGetter,
        ReceiverMutabilityFact::ReadOnly,
        "Object",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "JSONArray",
        "opt",
        ExternalApiRole::IndexedGetter,
        ReceiverMutabilityFact::ReadOnly,
        "Object",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "org.json.JSONArray",
        "getString",
        ExternalApiRole::IndexedGetter,
        ReceiverMutabilityFact::ReadOnly,
        "String",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "JSONArray",
        "getString",
        ExternalApiRole::IndexedGetter,
        ReceiverMutabilityFact::ReadOnly,
        "String",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "org.json.JSONArray",
        "length",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "JSONArray",
        "length",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "org.json.JSONTokener",
        "nextValue",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::MutatesReceiver,
        "Object",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "JSONTokener",
        "nextValue",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::MutatesReceiver,
        "Object",
        NullabilityState::MaybeNull,
    ),
];

const ORG_JSON_MODELED_APIS: &[ExternalPackModeledApi] = &[
    modeled_semantic("org.json.JSONObject", "put"),
    modeled_semantic("JSONObject", "put"),
    modeled_semantic("org.json.JSONObject", "get"),
    modeled_semantic("JSONObject", "get"),
    modeled_semantic("org.json.JSONObject", "opt"),
    modeled_semantic("JSONObject", "opt"),
    modeled_semantic("org.json.JSONObject", "getString"),
    modeled_semantic("JSONObject", "getString"),
    modeled_semantic("org.json.JSONObject", "optString"),
    modeled_semantic("JSONObject", "optString"),
    modeled_semantic("org.json.JSONObject", "getInt"),
    modeled_semantic("JSONObject", "getInt"),
    modeled_semantic("org.json.JSONObject", "has"),
    modeled_semantic("JSONObject", "has"),
    modeled_semantic("org.json.JSONObject", "length"),
    modeled_semantic("JSONObject", "length"),
    modeled_semantic("org.json.JSONArray", "put"),
    modeled_semantic("JSONArray", "put"),
    modeled_semantic("org.json.JSONArray", "get"),
    modeled_semantic("JSONArray", "get"),
    modeled_semantic("org.json.JSONArray", "opt"),
    modeled_semantic("JSONArray", "opt"),
    modeled_semantic("org.json.JSONArray", "getString"),
    modeled_semantic("JSONArray", "getString"),
    modeled_semantic("org.json.JSONArray", "length"),
    modeled_semantic("JSONArray", "length"),
    modeled_semantic("org.json.JSONTokener", "nextValue"),
    modeled_semantic("JSONTokener", "nextValue"),
];

const REFLECTIONS_API_EFFECTS: &[ExternalPackApiEffect] = &[
    semantic_typed_effect(
        "org.reflections.Reflections",
        "getSubTypesOf",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Set<Class>",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "Reflections",
        "getSubTypesOf",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Set<Class>",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "org.reflections.Reflections",
        "getTypesAnnotatedWith",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Set<Class>",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "Reflections",
        "getTypesAnnotatedWith",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Set<Class>",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "org.reflections.Reflections",
        "getMethodsAnnotatedWith",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Set<Method>",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "Reflections",
        "getMethodsAnnotatedWith",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Set<Method>",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "org.reflections.Reflections",
        "getFieldsAnnotatedWith",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Set<Field>",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "Reflections",
        "getFieldsAnnotatedWith",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Set<Field>",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "org.reflections.Reflections",
        "getConstructorsAnnotatedWith",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Set<Constructor>",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "Reflections",
        "getConstructorsAnnotatedWith",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Set<Constructor>",
        NullabilityState::DefinitelyNonNull,
    ),
];

const REFLECTIONS_MODELED_APIS: &[ExternalPackModeledApi] = &[
    modeled_semantic("org.reflections.Reflections", "getSubTypesOf"),
    modeled_semantic("Reflections", "getSubTypesOf"),
    modeled_semantic("org.reflections.Reflections", "getTypesAnnotatedWith"),
    modeled_semantic("Reflections", "getTypesAnnotatedWith"),
    modeled_semantic("org.reflections.Reflections", "getMethodsAnnotatedWith"),
    modeled_semantic("Reflections", "getMethodsAnnotatedWith"),
    modeled_semantic("org.reflections.Reflections", "getFieldsAnnotatedWith"),
    modeled_semantic("Reflections", "getFieldsAnnotatedWith"),
    modeled_semantic(
        "org.reflections.Reflections",
        "getConstructorsAnnotatedWith",
    ),
    modeled_semantic("Reflections", "getConstructorsAnnotatedWith"),
];

const HADOOP_CONF_API_EFFECTS: &[ExternalPackApiEffect] = &[
    semantic_typed_effect_with_arity(
        "org.apache.hadoop.conf.Configuration",
        "get",
        1,
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "String",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect_with_arity(
        "Configuration",
        "get",
        1,
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "String",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect_with_arity(
        "org.apache.hadoop.conf.Configuration",
        "get",
        2,
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "String",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect_with_arity(
        "Configuration",
        "get",
        2,
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "String",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect_with_arity(
        "org.apache.hadoop.conf.Configuration",
        "getBoolean",
        2,
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect_with_arity(
        "Configuration",
        "getBoolean",
        2,
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect_with_arity(
        "org.apache.hadoop.conf.Configuration",
        "getInt",
        2,
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect_with_arity(
        "Configuration",
        "getInt",
        2,
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    api_effect_with_arity(
        "org.apache.hadoop.conf.Configuration",
        "set",
        2,
        ExternalApiRole::Setter,
        ReceiverMutabilityFact::MutatesReceiver,
        None,
        NullabilityState::Unknown,
        &[],
    ),
    api_effect_with_arity(
        "Configuration",
        "set",
        2,
        ExternalApiRole::Setter,
        ReceiverMutabilityFact::MutatesReceiver,
        None,
        NullabilityState::Unknown,
        &[],
    ),
    api_effect_with_arity(
        "org.apache.hadoop.conf.Configuration",
        "setBoolean",
        2,
        ExternalApiRole::Setter,
        ReceiverMutabilityFact::MutatesReceiver,
        None,
        NullabilityState::Unknown,
        &[],
    ),
    api_effect_with_arity(
        "Configuration",
        "setBoolean",
        2,
        ExternalApiRole::Setter,
        ReceiverMutabilityFact::MutatesReceiver,
        None,
        NullabilityState::Unknown,
        &[],
    ),
    api_effect_with_arity(
        "org.apache.hadoop.conf.Configuration",
        "setInt",
        2,
        ExternalApiRole::Setter,
        ReceiverMutabilityFact::MutatesReceiver,
        None,
        NullabilityState::Unknown,
        &[],
    ),
    api_effect_with_arity(
        "Configuration",
        "setInt",
        2,
        ExternalApiRole::Setter,
        ReceiverMutabilityFact::MutatesReceiver,
        None,
        NullabilityState::Unknown,
        &[],
    ),
];

const HADOOP_CONF_MODELED_APIS: &[ExternalPackModeledApi] = &[
    modeled_semantic("org.apache.hadoop.conf.Configuration", "get"),
    modeled_semantic("Configuration", "get"),
    modeled_semantic("org.apache.hadoop.conf.Configuration", "getBoolean"),
    modeled_semantic("Configuration", "getBoolean"),
    modeled_semantic("org.apache.hadoop.conf.Configuration", "getInt"),
    modeled_semantic("Configuration", "getInt"),
    modeled_semantic("org.apache.hadoop.conf.Configuration", "set"),
    modeled_semantic("Configuration", "set"),
    modeled_semantic("org.apache.hadoop.conf.Configuration", "setBoolean"),
    modeled_semantic("Configuration", "setBoolean"),
    modeled_semantic("org.apache.hadoop.conf.Configuration", "setInt"),
    modeled_semantic("Configuration", "setInt"),
];

const APACHE_COMMONS_CONFIG_API_EFFECTS: &[ExternalPackApiEffect] = &[
    semantic_typed_effect(
        "org.apache.commons.configuration2.builder.fluent.Configurations",
        "properties",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "org.apache.commons.configuration2.PropertiesConfiguration",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "Configurations",
        "properties",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "PropertiesConfiguration",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "org.apache.commons.configuration2.builder.fluent.Configurations",
        "xml",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "org.apache.commons.configuration2.XMLConfiguration",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "Configurations",
        "xml",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "XMLConfiguration",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "org.apache.commons.configuration2.builder.fluent.Configurations",
        "ini",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "org.apache.commons.configuration2.INIConfiguration",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "Configurations",
        "ini",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "INIConfiguration",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "PropertiesConfiguration",
        "getString",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "String",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "XMLConfiguration",
        "getString",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "String",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "INIConfiguration",
        "getString",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "String",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "PropertiesConfiguration",
        "getBoolean",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "XMLConfiguration",
        "getBoolean",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "INIConfiguration",
        "getBoolean",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "PropertiesConfiguration",
        "getInt",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "XMLConfiguration",
        "getInt",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "INIConfiguration",
        "getInt",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    api_effect(
        "PropertiesConfiguration",
        "setProperty",
        ExternalApiRole::Setter,
        ReceiverMutabilityFact::MutatesReceiver,
        None,
        NullabilityState::Unknown,
        &[],
    ),
    api_effect(
        "XMLConfiguration",
        "setProperty",
        ExternalApiRole::Setter,
        ReceiverMutabilityFact::MutatesReceiver,
        None,
        NullabilityState::Unknown,
        &[],
    ),
    api_effect(
        "INIConfiguration",
        "setProperty",
        ExternalApiRole::Setter,
        ReceiverMutabilityFact::MutatesReceiver,
        None,
        NullabilityState::Unknown,
        &[],
    ),
];

const APACHE_COMMONS_CONFIG_MODELED_APIS: &[ExternalPackModeledApi] = &[
    modeled_semantic(
        "org.apache.commons.configuration2.builder.fluent.Configurations",
        "properties",
    ),
    modeled_semantic("Configurations", "properties"),
    modeled_semantic(
        "org.apache.commons.configuration2.builder.fluent.Configurations",
        "xml",
    ),
    modeled_semantic("Configurations", "xml"),
    modeled_semantic(
        "org.apache.commons.configuration2.builder.fluent.Configurations",
        "ini",
    ),
    modeled_semantic("Configurations", "ini"),
    modeled_semantic("PropertiesConfiguration", "getString"),
    modeled_semantic("XMLConfiguration", "getString"),
    modeled_semantic("INIConfiguration", "getString"),
    modeled_semantic("PropertiesConfiguration", "getBoolean"),
    modeled_semantic("XMLConfiguration", "getBoolean"),
    modeled_semantic("INIConfiguration", "getBoolean"),
    modeled_semantic("PropertiesConfiguration", "getInt"),
    modeled_semantic("XMLConfiguration", "getInt"),
    modeled_semantic("INIConfiguration", "getInt"),
    modeled_semantic("PropertiesConfiguration", "setProperty"),
    modeled_semantic("XMLConfiguration", "setProperty"),
    modeled_semantic("INIConfiguration", "setProperty"),
];

const SPARK_API_EFFECTS: &[ExternalPackApiEffect] = &[
    semantic_fluent_mutator_effect("org.apache.spark.SparkConf", "setAppName", "SparkConf"),
    semantic_fluent_mutator_effect("SparkConf", "setAppName", "SparkConf"),
    semantic_fluent_mutator_effect("org.apache.spark.SparkConf", "setMaster", "SparkConf"),
    semantic_fluent_mutator_effect("SparkConf", "setMaster", "SparkConf"),
    semantic_fluent_mutator_effect("org.apache.spark.SparkConf", "set", "SparkConf"),
    semantic_fluent_mutator_effect("SparkConf", "set", "SparkConf"),
    semantic_typed_effect(
        "org.apache.spark.api.java.JavaSparkContext",
        "parallelize",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "JavaRDD",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "JavaSparkContext",
        "parallelize",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "JavaRDD",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "org.apache.spark.api.java.JavaSparkContext",
        "broadcast",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Broadcast",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "JavaSparkContext",
        "broadcast",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Broadcast",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "org.apache.spark.api.java.JavaSparkContext",
        "hadoopConfiguration",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Configuration",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "JavaSparkContext",
        "hadoopConfiguration",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Configuration",
        NullabilityState::DefinitelyNonNull,
    ),
    api_effect(
        "org.apache.spark.api.java.JavaSparkContext",
        "stop",
        ExternalApiRole::SideEffect,
        ReceiverMutabilityFact::MutatesReceiver,
        None,
        NullabilityState::Unknown,
        &[],
    ),
    api_effect(
        "JavaSparkContext",
        "stop",
        ExternalApiRole::SideEffect,
        ReceiverMutabilityFact::MutatesReceiver,
        None,
        NullabilityState::Unknown,
        &[],
    ),
    semantic_typed_effect(
        "org.apache.spark.api.java.JavaRDD",
        "context",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "JavaSparkContext",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "JavaRDD",
        "context",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "JavaSparkContext",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "org.apache.spark.api.java.JavaRDD",
        "getNumPartitions",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "JavaRDD",
        "getNumPartitions",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "org.apache.spark.api.java.JavaRDD",
        "rdd",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Object",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "JavaRDD",
        "rdd",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Object",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "org.apache.spark.api.java.JavaRDD",
        "map",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "JavaRDD",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "JavaRDD",
        "map",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "JavaRDD",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "org.apache.spark.api.java.JavaRDD",
        "flatMap",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "JavaRDD",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "JavaRDD",
        "flatMap",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "JavaRDD",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "org.apache.spark.api.java.JavaRDD",
        "filter",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "JavaRDD",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "JavaRDD",
        "filter",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "JavaRDD",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "org.apache.spark.api.java.JavaRDD",
        "cache",
        ExternalApiRole::FluentMutator,
        ReceiverMutabilityFact::MutatesReceiver,
        "JavaRDD",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "JavaRDD",
        "cache",
        ExternalApiRole::FluentMutator,
        ReceiverMutabilityFact::MutatesReceiver,
        "JavaRDD",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "org.apache.spark.api.java.JavaRDD",
        "repartition",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "JavaRDD",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "JavaRDD",
        "repartition",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "JavaRDD",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "org.apache.spark.api.java.JavaRDD",
        "partitionBy",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "JavaRDD",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "JavaRDD",
        "partitionBy",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "JavaRDD",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect_with_arity(
        "org.apache.spark.api.java.JavaRDD",
        "mapPartitions",
        1,
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "JavaRDD",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect_with_arity(
        "JavaRDD",
        "mapPartitions",
        1,
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "JavaRDD",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect_with_arity(
        "org.apache.spark.api.java.JavaRDD",
        "mapPartitions",
        2,
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "JavaRDD",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect_with_arity(
        "JavaRDD",
        "mapPartitions",
        2,
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "JavaRDD",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect_with_arity(
        "org.apache.spark.api.java.JavaRDD",
        "mapPartitionsWithIndex",
        2,
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "JavaRDD",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect_with_arity(
        "JavaRDD",
        "mapPartitionsWithIndex",
        2,
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "JavaRDD",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect_with_arity(
        "org.apache.spark.api.java.JavaRDD",
        "mapToPair",
        1,
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "JavaPairRDD",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect_with_arity(
        "JavaRDD",
        "mapToPair",
        1,
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "JavaPairRDD",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect_with_arity(
        "org.apache.spark.api.java.JavaRDD",
        "mapPartitionsToPair",
        1,
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "JavaPairRDD",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect_with_arity(
        "JavaRDD",
        "mapPartitionsToPair",
        1,
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "JavaPairRDD",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect_with_arity(
        "org.apache.spark.api.java.JavaRDD",
        "mapPartitionsToPair",
        2,
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "JavaPairRDD",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect_with_arity(
        "JavaRDD",
        "mapPartitionsToPair",
        2,
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "JavaPairRDD",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect_with_arity(
        "org.apache.spark.api.java.JavaRDD",
        "zipPartitions",
        2,
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "JavaRDD",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect_with_arity(
        "JavaRDD",
        "zipPartitions",
        2,
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "JavaRDD",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect_with_arity(
        "org.apache.spark.api.java.JavaPairRDD",
        "sortByKey",
        1,
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "JavaPairRDD",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect_with_arity(
        "JavaPairRDD",
        "sortByKey",
        1,
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "JavaPairRDD",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect_with_arity(
        "org.apache.spark.api.java.JavaPairRDD",
        "sortByKey",
        3,
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "JavaPairRDD",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect_with_arity(
        "JavaPairRDD",
        "sortByKey",
        3,
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "JavaPairRDD",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect_with_arity(
        "org.apache.spark.api.java.JavaPairRDD",
        "keys",
        0,
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "JavaRDD",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect_with_arity(
        "JavaPairRDD",
        "keys",
        0,
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "JavaRDD",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect_with_arity(
        "org.apache.spark.api.java.JavaPairRDD",
        "mapPartitionsToPair",
        1,
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "JavaPairRDD",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect_with_arity(
        "JavaPairRDD",
        "mapPartitionsToPair",
        1,
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "JavaPairRDD",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect_with_arity(
        "org.apache.spark.api.java.JavaPairRDD",
        "mapPartitions",
        1,
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "JavaRDD",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect_with_arity(
        "JavaPairRDD",
        "mapPartitions",
        1,
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "JavaRDD",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect_with_arity(
        "org.apache.spark.api.java.JavaPairRDD",
        "filter",
        1,
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "JavaPairRDD",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect_with_arity(
        "JavaPairRDD",
        "filter",
        1,
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "JavaPairRDD",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect_with_arity(
        "org.apache.spark.api.java.JavaPairRDD",
        "flatMap",
        1,
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "JavaRDD",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect_with_arity(
        "JavaPairRDD",
        "flatMap",
        1,
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "JavaRDD",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect_with_arity(
        "org.apache.spark.api.java.JavaPairRDD",
        "map",
        1,
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "JavaRDD",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect_with_arity(
        "JavaPairRDD",
        "map",
        1,
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "JavaRDD",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect_with_arity(
        "org.apache.spark.api.java.JavaPairRDD",
        "reduceByKey",
        1,
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "JavaPairRDD",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect_with_arity(
        "org.apache.spark.api.java.JavaPairRDD",
        "combineByKey",
        3,
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "JavaPairRDD",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect_with_arity(
        "JavaPairRDD",
        "combineByKey",
        3,
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "JavaPairRDD",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect_with_arity(
        "org.apache.spark.api.java.JavaPairRDD",
        "combineByKey",
        6,
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "JavaPairRDD",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect_with_arity(
        "JavaPairRDD",
        "combineByKey",
        6,
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "JavaPairRDD",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect_with_arity(
        "JavaPairRDD",
        "reduceByKey",
        1,
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "JavaPairRDD",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect_with_arity(
        "org.apache.spark.api.java.JavaPairRDD",
        "partitionBy",
        1,
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "JavaPairRDD",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect_with_arity(
        "JavaPairRDD",
        "partitionBy",
        1,
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "JavaPairRDD",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "org.apache.spark.api.java.JavaRDD",
        "collect",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "List",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "JavaRDD",
        "collect",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "List",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "org.apache.spark.api.java.JavaRDD",
        "count",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "long",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "JavaRDD",
        "count",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "long",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "org.apache.spark.api.java.JavaRDD",
        "unpersist",
        ExternalApiRole::FluentMutator,
        ReceiverMutabilityFact::MutatesReceiver,
        "JavaRDD",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "JavaRDD",
        "unpersist",
        ExternalApiRole::FluentMutator,
        ReceiverMutabilityFact::MutatesReceiver,
        "JavaRDD",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_side_effect(
        "org.apache.spark.api.java.JavaRDD",
        "saveAsTextFile",
        NullabilityState::Unknown,
    ),
    semantic_side_effect("JavaRDD", "saveAsTextFile", NullabilityState::Unknown),
    api_effect(
        "org.apache.spark.broadcast.Broadcast",
        "destroy",
        ExternalApiRole::SideEffect,
        ReceiverMutabilityFact::MutatesReceiver,
        None,
        NullabilityState::Unknown,
        &[],
    ),
    api_effect(
        "Broadcast",
        "destroy",
        ExternalApiRole::SideEffect,
        ReceiverMutabilityFact::MutatesReceiver,
        None,
        NullabilityState::Unknown,
        &[],
    ),
    semantic_typed_effect(
        "org.apache.spark.broadcast.Broadcast",
        "value",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Object",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "Broadcast",
        "value",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Object",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "org.apache.spark.broadcast.Broadcast",
        "getValue",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Object",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "Broadcast",
        "getValue",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Object",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "org.apache.spark.mllib.linalg.distributed.RowMatrix",
        "rows",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "JavaRDD",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "RowMatrix",
        "rows",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "JavaRDD",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "org.apache.spark.mllib.linalg.distributed.RowMatrix",
        "numRows",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "long",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "RowMatrix",
        "numRows",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "long",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "org.apache.spark.mllib.linalg.distributed.RowMatrix",
        "numCols",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "long",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "RowMatrix",
        "numCols",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "long",
        NullabilityState::DefinitelyNonNull,
    ),
];

const SPARK_MODELED_APIS: &[ExternalPackModeledApi] = &[
    modeled_semantic("org.apache.spark.SparkConf", "setAppName"),
    modeled_semantic("SparkConf", "setAppName"),
    modeled_semantic("org.apache.spark.SparkConf", "setMaster"),
    modeled_semantic("SparkConf", "setMaster"),
    modeled_semantic("org.apache.spark.SparkConf", "set"),
    modeled_semantic("SparkConf", "set"),
    modeled_semantic("org.apache.spark.api.java.JavaSparkContext", "parallelize"),
    modeled_semantic("JavaSparkContext", "parallelize"),
    modeled_semantic("org.apache.spark.api.java.JavaSparkContext", "broadcast"),
    modeled_semantic("JavaSparkContext", "broadcast"),
    modeled_semantic(
        "org.apache.spark.api.java.JavaSparkContext",
        "hadoopConfiguration",
    ),
    modeled_semantic("JavaSparkContext", "hadoopConfiguration"),
    modeled_semantic("org.apache.spark.api.java.JavaSparkContext", "stop"),
    modeled_semantic("JavaSparkContext", "stop"),
    modeled_semantic("org.apache.spark.api.java.JavaRDD", "context"),
    modeled_semantic("JavaRDD", "context"),
    modeled_semantic("org.apache.spark.api.java.JavaRDD", "getNumPartitions"),
    modeled_semantic("JavaRDD", "getNumPartitions"),
    modeled_semantic("org.apache.spark.api.java.JavaRDD", "rdd"),
    modeled_semantic("JavaRDD", "rdd"),
    modeled_semantic("org.apache.spark.api.java.JavaRDD", "map"),
    modeled_semantic("JavaRDD", "map"),
    modeled_semantic("org.apache.spark.api.java.JavaRDD", "flatMap"),
    modeled_semantic("JavaRDD", "flatMap"),
    modeled_semantic("org.apache.spark.api.java.JavaRDD", "mapPartitions"),
    modeled_semantic("JavaRDD", "mapPartitions"),
    modeled_semantic(
        "org.apache.spark.api.java.JavaRDD",
        "mapPartitionsWithIndex",
    ),
    modeled_semantic("JavaRDD", "mapPartitionsWithIndex"),
    modeled_semantic("org.apache.spark.api.java.JavaRDD", "mapToPair"),
    modeled_semantic("JavaRDD", "mapToPair"),
    modeled_semantic("org.apache.spark.api.java.JavaRDD", "mapPartitionsToPair"),
    modeled_semantic("JavaRDD", "mapPartitionsToPair"),
    modeled_semantic("org.apache.spark.api.java.JavaRDD", "zipPartitions"),
    modeled_semantic("JavaRDD", "zipPartitions"),
    modeled_semantic("org.apache.spark.api.java.JavaPairRDD", "sortByKey"),
    modeled_semantic("JavaPairRDD", "sortByKey"),
    modeled_semantic("org.apache.spark.api.java.JavaPairRDD", "keys"),
    modeled_semantic("JavaPairRDD", "keys"),
    modeled_semantic(
        "org.apache.spark.api.java.JavaPairRDD",
        "mapPartitionsToPair",
    ),
    modeled_semantic("JavaPairRDD", "mapPartitionsToPair"),
    modeled_semantic("org.apache.spark.api.java.JavaPairRDD", "mapPartitions"),
    modeled_semantic("JavaPairRDD", "mapPartitions"),
    modeled_semantic("org.apache.spark.api.java.JavaPairRDD", "filter"),
    modeled_semantic("JavaPairRDD", "filter"),
    modeled_semantic("org.apache.spark.api.java.JavaPairRDD", "flatMap"),
    modeled_semantic("JavaPairRDD", "flatMap"),
    modeled_semantic("org.apache.spark.api.java.JavaPairRDD", "map"),
    modeled_semantic("JavaPairRDD", "map"),
    modeled_semantic("org.apache.spark.api.java.JavaPairRDD", "reduceByKey"),
    modeled_semantic("JavaPairRDD", "reduceByKey"),
    modeled_semantic("org.apache.spark.api.java.JavaPairRDD", "combineByKey"),
    modeled_semantic("JavaPairRDD", "combineByKey"),
    modeled_semantic("org.apache.spark.api.java.JavaPairRDD", "partitionBy"),
    modeled_semantic("JavaPairRDD", "partitionBy"),
    modeled_semantic("org.apache.spark.api.java.JavaRDD", "filter"),
    modeled_semantic("JavaRDD", "filter"),
    modeled_semantic("org.apache.spark.api.java.JavaRDD", "cache"),
    modeled_semantic("JavaRDD", "cache"),
    modeled_semantic("org.apache.spark.api.java.JavaRDD", "repartition"),
    modeled_semantic("JavaRDD", "repartition"),
    modeled_semantic("org.apache.spark.api.java.JavaRDD", "partitionBy"),
    modeled_semantic("JavaRDD", "partitionBy"),
    modeled_semantic("org.apache.spark.api.java.JavaRDD", "collect"),
    modeled_semantic("JavaRDD", "collect"),
    modeled_semantic("org.apache.spark.api.java.JavaRDD", "count"),
    modeled_semantic("JavaRDD", "count"),
    modeled_semantic("org.apache.spark.api.java.JavaRDD", "unpersist"),
    modeled_semantic("JavaRDD", "unpersist"),
    modeled_semantic("org.apache.spark.api.java.JavaRDD", "saveAsTextFile"),
    modeled_semantic("JavaRDD", "saveAsTextFile"),
    modeled_semantic("org.apache.spark.broadcast.Broadcast", "destroy"),
    modeled_semantic("Broadcast", "destroy"),
    modeled_semantic("org.apache.spark.broadcast.Broadcast", "value"),
    modeled_semantic("Broadcast", "value"),
    modeled_semantic("org.apache.spark.broadcast.Broadcast", "getValue"),
    modeled_semantic("Broadcast", "getValue"),
    modeled_semantic(
        "org.apache.spark.mllib.linalg.distributed.RowMatrix",
        "rows",
    ),
    modeled_semantic("RowMatrix", "rows"),
    modeled_semantic(
        "org.apache.spark.mllib.linalg.distributed.RowMatrix",
        "numRows",
    ),
    modeled_semantic("RowMatrix", "numRows"),
    modeled_semantic(
        "org.apache.spark.mllib.linalg.distributed.RowMatrix",
        "numCols",
    ),
    modeled_semantic("RowMatrix", "numCols"),
];

const JGRAPHT_API_EFFECTS: &[ExternalPackApiEffect] = &[
    semantic_typed_effect(
        "org.jgrapht.alg.ConnectivityInspector",
        "connectedSets",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "List<Set<Object>>",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "ConnectivityInspector",
        "connectedSets",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "List<Set<Object>>",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "org.jgrapht.alg.CycleDetector",
        "detectCycles",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "CycleDetector",
        "detectCycles",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "org.jgrapht.Graph",
        "addVertex",
        ExternalApiRole::Mutator,
        ReceiverMutabilityFact::MutatesReceiver,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "Graph",
        "addVertex",
        ExternalApiRole::Mutator,
        ReceiverMutabilityFact::MutatesReceiver,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "org.jgrapht.graph.SimpleGraph",
        "addVertex",
        ExternalApiRole::Mutator,
        ReceiverMutabilityFact::MutatesReceiver,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "SimpleGraph",
        "addVertex",
        ExternalApiRole::Mutator,
        ReceiverMutabilityFact::MutatesReceiver,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "org.jgrapht.graph.DefaultDirectedWeightedGraph",
        "addVertex",
        ExternalApiRole::Mutator,
        ReceiverMutabilityFact::MutatesReceiver,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "DefaultDirectedWeightedGraph",
        "addVertex",
        ExternalApiRole::Mutator,
        ReceiverMutabilityFact::MutatesReceiver,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "org.jgrapht.Graph",
        "addEdge",
        ExternalApiRole::Mutator,
        ReceiverMutabilityFact::MutatesReceiver,
        "Object",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "Graph",
        "addEdge",
        ExternalApiRole::Mutator,
        ReceiverMutabilityFact::MutatesReceiver,
        "Object",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "org.jgrapht.graph.SimpleGraph",
        "addEdge",
        ExternalApiRole::Mutator,
        ReceiverMutabilityFact::MutatesReceiver,
        "Object",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "SimpleGraph",
        "addEdge",
        ExternalApiRole::Mutator,
        ReceiverMutabilityFact::MutatesReceiver,
        "Object",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "org.jgrapht.graph.DefaultDirectedWeightedGraph",
        "addEdge",
        ExternalApiRole::Mutator,
        ReceiverMutabilityFact::MutatesReceiver,
        "Object",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "DefaultDirectedWeightedGraph",
        "addEdge",
        ExternalApiRole::Mutator,
        ReceiverMutabilityFact::MutatesReceiver,
        "Object",
        NullabilityState::MaybeNull,
    ),
    api_effect(
        "org.jgrapht.graph.DefaultDirectedWeightedGraph",
        "setEdgeWeight",
        ExternalApiRole::Mutator,
        ReceiverMutabilityFact::MutatesReceiver,
        None,
        NullabilityState::Unknown,
        &[],
    ),
    api_effect(
        "DefaultDirectedWeightedGraph",
        "setEdgeWeight",
        ExternalApiRole::Mutator,
        ReceiverMutabilityFact::MutatesReceiver,
        None,
        NullabilityState::Unknown,
        &[],
    ),
    semantic_typed_effect(
        "org.jgrapht.Graph",
        "vertexSet",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Set<Object>",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "Graph",
        "vertexSet",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Set<Object>",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "org.jgrapht.graph.SimpleGraph",
        "vertexSet",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Set<Object>",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "SimpleGraph",
        "vertexSet",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Set<Object>",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "org.jgrapht.graph.DefaultDirectedWeightedGraph",
        "vertexSet",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Set<Object>",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "DefaultDirectedWeightedGraph",
        "vertexSet",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Set<Object>",
        NullabilityState::DefinitelyNonNull,
    ),
    api_effect(
        "org.jgrapht.io.DOTExporter",
        "exportGraph",
        ExternalApiRole::SideEffect,
        ReceiverMutabilityFact::ReadOnly,
        None,
        NullabilityState::Unknown,
        &[],
    ),
    api_effect(
        "DOTExporter",
        "exportGraph",
        ExternalApiRole::SideEffect,
        ReceiverMutabilityFact::ReadOnly,
        None,
        NullabilityState::Unknown,
        &[],
    ),
];

const JGRAPHT_MODELED_APIS: &[ExternalPackModeledApi] = &[
    modeled_semantic("org.jgrapht.alg.ConnectivityInspector", "connectedSets"),
    modeled_semantic("ConnectivityInspector", "connectedSets"),
    modeled_semantic("org.jgrapht.alg.CycleDetector", "detectCycles"),
    modeled_semantic("CycleDetector", "detectCycles"),
    modeled_semantic("org.jgrapht.Graph", "addVertex"),
    modeled_semantic("Graph", "addVertex"),
    modeled_semantic("org.jgrapht.graph.SimpleGraph", "addVertex"),
    modeled_semantic("SimpleGraph", "addVertex"),
    modeled_semantic(
        "org.jgrapht.graph.DefaultDirectedWeightedGraph",
        "addVertex",
    ),
    modeled_semantic("DefaultDirectedWeightedGraph", "addVertex"),
    modeled_semantic("org.jgrapht.Graph", "addEdge"),
    modeled_semantic("Graph", "addEdge"),
    modeled_semantic("org.jgrapht.graph.SimpleGraph", "addEdge"),
    modeled_semantic("SimpleGraph", "addEdge"),
    modeled_semantic("org.jgrapht.graph.DefaultDirectedWeightedGraph", "addEdge"),
    modeled_semantic("DefaultDirectedWeightedGraph", "addEdge"),
    modeled_semantic(
        "org.jgrapht.graph.DefaultDirectedWeightedGraph",
        "setEdgeWeight",
    ),
    modeled_semantic("DefaultDirectedWeightedGraph", "setEdgeWeight"),
    modeled_semantic("org.jgrapht.Graph", "vertexSet"),
    modeled_semantic("Graph", "vertexSet"),
    modeled_semantic("org.jgrapht.graph.SimpleGraph", "vertexSet"),
    modeled_semantic("SimpleGraph", "vertexSet"),
    modeled_semantic(
        "org.jgrapht.graph.DefaultDirectedWeightedGraph",
        "vertexSet",
    ),
    modeled_semantic("DefaultDirectedWeightedGraph", "vertexSet"),
    modeled_semantic("org.jgrapht.io.DOTExporter", "exportGraph"),
    modeled_semantic("DOTExporter", "exportGraph"),
];

const FASTUTIL_API_EFFECTS: &[ExternalPackApiEffect] = &[
    semantic_typed_effect(
        "it.unimi.dsi.fastutil.ints.Int2ObjectArrayMap",
        "put",
        ExternalApiRole::Mutator,
        ReceiverMutabilityFact::MutatesReceiver,
        "Object",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "Int2ObjectArrayMap",
        "put",
        ExternalApiRole::Mutator,
        ReceiverMutabilityFact::MutatesReceiver,
        "Object",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "it.unimi.dsi.fastutil.ints.Int2ObjectArrayMap",
        "get",
        ExternalApiRole::IndexedGetter,
        ReceiverMutabilityFact::ReadOnly,
        "Object",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "Int2ObjectArrayMap",
        "get",
        ExternalApiRole::IndexedGetter,
        ReceiverMutabilityFact::ReadOnly,
        "Object",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "it.unimi.dsi.fastutil.ints.Int2ObjectOpenHashMap",
        "put",
        ExternalApiRole::Mutator,
        ReceiverMutabilityFact::MutatesReceiver,
        "Object",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "Int2ObjectOpenHashMap",
        "put",
        ExternalApiRole::Mutator,
        ReceiverMutabilityFact::MutatesReceiver,
        "Object",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "it.unimi.dsi.fastutil.ints.Int2ObjectOpenHashMap",
        "get",
        ExternalApiRole::IndexedGetter,
        ReceiverMutabilityFact::ReadOnly,
        "Object",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "Int2ObjectOpenHashMap",
        "get",
        ExternalApiRole::IndexedGetter,
        ReceiverMutabilityFact::ReadOnly,
        "Object",
        NullabilityState::MaybeNull,
    ),
    api_effect(
        "it.unimi.dsi.fastutil.ints.IntArrayList",
        "add",
        ExternalApiRole::Mutator,
        ReceiverMutabilityFact::MutatesReceiver,
        Some("boolean"),
        NullabilityState::DefinitelyNonNull,
        &[],
    ),
    api_effect(
        "IntArrayList",
        "add",
        ExternalApiRole::Mutator,
        ReceiverMutabilityFact::MutatesReceiver,
        Some("boolean"),
        NullabilityState::DefinitelyNonNull,
        &[],
    ),
    semantic_typed_effect(
        "it.unimi.dsi.fastutil.ints.IntArrayList",
        "getInt",
        ExternalApiRole::IndexedGetter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "IntArrayList",
        "getInt",
        ExternalApiRole::IndexedGetter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "it.unimi.dsi.fastutil.ints.Int2IntArrayMap",
        "put",
        ExternalApiRole::Mutator,
        ReceiverMutabilityFact::MutatesReceiver,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "Int2IntArrayMap",
        "put",
        ExternalApiRole::Mutator,
        ReceiverMutabilityFact::MutatesReceiver,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "it.unimi.dsi.fastutil.ints.Int2IntArrayMap",
        "get",
        ExternalApiRole::IndexedGetter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "Int2IntArrayMap",
        "get",
        ExternalApiRole::IndexedGetter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "it.unimi.dsi.fastutil.objects.Object2IntOpenHashMap",
        "put",
        ExternalApiRole::Mutator,
        ReceiverMutabilityFact::MutatesReceiver,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "Object2IntOpenHashMap",
        "put",
        ExternalApiRole::Mutator,
        ReceiverMutabilityFact::MutatesReceiver,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "it.unimi.dsi.fastutil.objects.Object2IntOpenHashMap",
        "getInt",
        ExternalApiRole::IndexedGetter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "Object2IntOpenHashMap",
        "getInt",
        ExternalApiRole::IndexedGetter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    api_effect_with_arity(
        "it.unimi.dsi.fastutil.objects.Object2IntOpenHashMap",
        "defaultReturnValue",
        1,
        ExternalApiRole::Setter,
        ReceiverMutabilityFact::MutatesReceiver,
        None,
        NullabilityState::Unknown,
        &[],
    ),
    api_effect_with_arity(
        "Object2IntOpenHashMap",
        "defaultReturnValue",
        1,
        ExternalApiRole::Setter,
        ReceiverMutabilityFact::MutatesReceiver,
        None,
        NullabilityState::Unknown,
        &[],
    ),
    semantic_typed_effect(
        "it.unimi.dsi.fastutil.ints.Int2ObjectArrayMap",
        "containsKey",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "Int2ObjectArrayMap",
        "containsKey",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "it.unimi.dsi.fastutil.ints.Int2ObjectOpenHashMap",
        "containsKey",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "Int2ObjectOpenHashMap",
        "containsKey",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "it.unimi.dsi.fastutil.ints.IntArrayList",
        "size",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "IntArrayList",
        "size",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
];

const FASTUTIL_MODELED_APIS: &[ExternalPackModeledApi] = &[
    modeled_semantic("it.unimi.dsi.fastutil.ints.Int2ObjectArrayMap", "put"),
    modeled_semantic("Int2ObjectArrayMap", "put"),
    modeled_semantic("it.unimi.dsi.fastutil.ints.Int2ObjectArrayMap", "get"),
    modeled_semantic("Int2ObjectArrayMap", "get"),
    modeled_semantic(
        "it.unimi.dsi.fastutil.ints.Int2ObjectArrayMap",
        "containsKey",
    ),
    modeled_semantic("Int2ObjectArrayMap", "containsKey"),
    modeled_semantic("it.unimi.dsi.fastutil.ints.Int2ObjectOpenHashMap", "put"),
    modeled_semantic("Int2ObjectOpenHashMap", "put"),
    modeled_semantic("it.unimi.dsi.fastutil.ints.Int2ObjectOpenHashMap", "get"),
    modeled_semantic("Int2ObjectOpenHashMap", "get"),
    modeled_semantic(
        "it.unimi.dsi.fastutil.ints.Int2ObjectOpenHashMap",
        "containsKey",
    ),
    modeled_semantic("Int2ObjectOpenHashMap", "containsKey"),
    modeled_semantic("it.unimi.dsi.fastutil.ints.IntArrayList", "add"),
    modeled_semantic("IntArrayList", "add"),
    modeled_semantic("it.unimi.dsi.fastutil.ints.IntArrayList", "getInt"),
    modeled_semantic("IntArrayList", "getInt"),
    modeled_semantic("it.unimi.dsi.fastutil.ints.IntArrayList", "size"),
    modeled_semantic("IntArrayList", "size"),
    modeled_semantic("it.unimi.dsi.fastutil.ints.Int2IntArrayMap", "put"),
    modeled_semantic("Int2IntArrayMap", "put"),
    modeled_semantic("it.unimi.dsi.fastutil.ints.Int2IntArrayMap", "get"),
    modeled_semantic("Int2IntArrayMap", "get"),
    modeled_semantic("it.unimi.dsi.fastutil.objects.Object2IntOpenHashMap", "put"),
    modeled_semantic("Object2IntOpenHashMap", "put"),
    modeled_semantic(
        "it.unimi.dsi.fastutil.objects.Object2IntOpenHashMap",
        "getInt",
    ),
    modeled_semantic("Object2IntOpenHashMap", "getInt"),
    modeled_semantic(
        "it.unimi.dsi.fastutil.objects.Object2IntOpenHashMap",
        "defaultReturnValue",
    ),
    modeled_semantic("Object2IntOpenHashMap", "defaultReturnValue"),
];

const HDF5_API_EFFECTS: &[ExternalPackApiEffect] = &[
    api_effect(
        "HDF5File",
        "isOpen",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        Some("boolean"),
        NullabilityState::DefinitelyNonNull,
        &[],
    ),
    api_effect(
        "HDF5File",
        "exists",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        Some("boolean"),
        NullabilityState::DefinitelyNonNull,
        &[],
    ),
    api_effect(
        "HDF5File",
        "contains",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        Some("boolean"),
        NullabilityState::DefinitelyNonNull,
        &[],
    ),
    api_effect(
        "HDF5File",
        "getFile",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        Some("File"),
        NullabilityState::Unknown,
        HDF5_THROWN,
    ),
    api_effect(
        "HDF5File",
        "getDataset",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        None,
        NullabilityState::Unknown,
        HDF5_THROWN,
    ),
    api_effect(
        "HDF5File",
        "read",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        None,
        NullabilityState::Unknown,
        HDF5_THROWN,
    ),
    api_effect(
        "HDF5File",
        "readDoubleArray",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        Some("double[]"),
        NullabilityState::Unknown,
        HDF5_THROWN,
    ),
    api_effect(
        "HDF5File",
        "readDoubleMatrix",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        Some("double[][]"),
        NullabilityState::Unknown,
        HDF5_THROWN,
    ),
    api_effect(
        "HDF5File",
        "readDouble",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        Some("double"),
        NullabilityState::DefinitelyNonNull,
        HDF5_THROWN,
    ),
    api_effect(
        "HDF5File",
        "readStringArray",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        Some("String[]"),
        NullabilityState::Unknown,
        HDF5_THROWN,
    ),
    api_effect(
        "HDF5File",
        "readIntArray",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        Some("int[]"),
        NullabilityState::Unknown,
        HDF5_THROWN,
    ),
    api_effect(
        "HDF5File",
        "makeDouble",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        Some("double"),
        NullabilityState::DefinitelyNonNull,
        &[],
    ),
    api_effect(
        "HDF5File",
        "makeDoubleArray",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        Some("double[]"),
        NullabilityState::DefinitelyNonNull,
        &[],
    ),
    api_effect(
        "HDF5File",
        "makeDoubleMatrix",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        Some("double[][]"),
        NullabilityState::DefinitelyNonNull,
        &[],
    ),
    api_effect(
        "HDF5File",
        "makeStringArray",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        Some("String[]"),
        NullabilityState::DefinitelyNonNull,
        &[],
    ),
    api_effect(
        "HDF5File",
        "isPresent",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        Some("boolean"),
        NullabilityState::DefinitelyNonNull,
        &[],
    ),
    api_effect(
        "HDF5File",
        "write",
        ExternalApiRole::Mutator,
        ReceiverMutabilityFact::MutatesReceiver,
        None,
        NullabilityState::Unknown,
        HDF5_THROWN,
    ),
    api_effect(
        "HDF5File",
        "writeDoubleArray",
        ExternalApiRole::Mutator,
        ReceiverMutabilityFact::MutatesReceiver,
        None,
        NullabilityState::Unknown,
        HDF5_THROWN,
    ),
    api_effect(
        "HDF5File",
        "writeIntArray",
        ExternalApiRole::Mutator,
        ReceiverMutabilityFact::MutatesReceiver,
        None,
        NullabilityState::Unknown,
        HDF5_THROWN,
    ),
    api_effect(
        "HDF5File",
        "flush",
        ExternalApiRole::Mutator,
        ReceiverMutabilityFact::MutatesReceiver,
        None,
        NullabilityState::Unknown,
        HDF5_THROWN,
    ),
    api_effect(
        "HDF5File",
        "close",
        ExternalApiRole::SideEffect,
        ReceiverMutabilityFact::ReadOnly,
        None,
        NullabilityState::Unknown,
        HDF5_THROWN,
    ),
    api_effect(
        "HDF5Library",
        "load",
        ExternalApiRole::SideEffect,
        ReceiverMutabilityFact::ReadOnly,
        None,
        NullabilityState::Unknown,
        HDF5_THROWN,
    ),
];

const SQLITE_API_EFFECTS: &[ExternalPackApiEffect] = &[
    api_effect(
        "org.sqlite.SQLiteConfig",
        "setReadOnly",
        ExternalApiRole::Setter,
        ReceiverMutabilityFact::MutatesReceiver,
        None,
        NullabilityState::Unknown,
        &[],
    ),
    api_effect(
        "SQLiteConfig",
        "setReadOnly",
        ExternalApiRole::Setter,
        ReceiverMutabilityFact::MutatesReceiver,
        None,
        NullabilityState::Unknown,
        &[],
    ),
    api_effect(
        "org.sqlite.SQLiteConfig",
        "setBusyTimeout",
        ExternalApiRole::Setter,
        ReceiverMutabilityFact::MutatesReceiver,
        None,
        NullabilityState::Unknown,
        &[],
    ),
    api_effect(
        "SQLiteConfig",
        "setBusyTimeout",
        ExternalApiRole::Setter,
        ReceiverMutabilityFact::MutatesReceiver,
        None,
        NullabilityState::Unknown,
        &[],
    ),
    api_effect(
        "org.sqlite.SQLiteConfig",
        "setSharedCache",
        ExternalApiRole::Setter,
        ReceiverMutabilityFact::MutatesReceiver,
        None,
        NullabilityState::Unknown,
        &[],
    ),
    api_effect(
        "SQLiteConfig",
        "setSharedCache",
        ExternalApiRole::Setter,
        ReceiverMutabilityFact::MutatesReceiver,
        None,
        NullabilityState::Unknown,
        &[],
    ),
    api_effect(
        "org.sqlite.SQLiteConfig",
        "setJournalMode",
        ExternalApiRole::Setter,
        ReceiverMutabilityFact::MutatesReceiver,
        None,
        NullabilityState::Unknown,
        &[],
    ),
    api_effect(
        "SQLiteConfig",
        "setJournalMode",
        ExternalApiRole::Setter,
        ReceiverMutabilityFact::MutatesReceiver,
        None,
        NullabilityState::Unknown,
        &[],
    ),
    api_effect(
        "org.sqlite.SQLiteConfig",
        "createConnection",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        Some("SQLiteConnection"),
        NullabilityState::DefinitelyNonNull,
        SQLITE_THROWN,
    ),
    api_effect(
        "SQLiteConfig",
        "createConnection",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        Some("SQLiteConnection"),
        NullabilityState::DefinitelyNonNull,
        SQLITE_THROWN,
    ),
    api_effect(
        "org.sqlite.SQLiteConfig",
        "toProperties",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        Some("Properties"),
        NullabilityState::DefinitelyNonNull,
        &[],
    ),
    api_effect(
        "SQLiteConfig",
        "toProperties",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        Some("Properties"),
        NullabilityState::DefinitelyNonNull,
        &[],
    ),
    api_effect(
        "org.sqlite.SQLiteConnection",
        "prepareStatement",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        None,
        NullabilityState::DefinitelyNonNull,
        SQLITE_THROWN,
    ),
    api_effect(
        "SQLiteConnection",
        "prepareStatement",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        None,
        NullabilityState::DefinitelyNonNull,
        SQLITE_THROWN,
    ),
    api_effect(
        "org.sqlite.SQLiteConnection",
        "createStatement",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        None,
        NullabilityState::DefinitelyNonNull,
        SQLITE_THROWN,
    ),
    api_effect(
        "SQLiteConnection",
        "createStatement",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        None,
        NullabilityState::DefinitelyNonNull,
        SQLITE_THROWN,
    ),
    api_effect(
        "org.sqlite.SQLiteConnection",
        "setAutoCommit",
        ExternalApiRole::Setter,
        ReceiverMutabilityFact::MutatesReceiver,
        None,
        NullabilityState::Unknown,
        SQLITE_THROWN,
    ),
    api_effect(
        "SQLiteConnection",
        "setAutoCommit",
        ExternalApiRole::Setter,
        ReceiverMutabilityFact::MutatesReceiver,
        None,
        NullabilityState::Unknown,
        SQLITE_THROWN,
    ),
    api_effect(
        "org.sqlite.SQLiteConnection",
        "commit",
        ExternalApiRole::SideEffect,
        ReceiverMutabilityFact::ReadOnly,
        None,
        NullabilityState::Unknown,
        SQLITE_THROWN,
    ),
    api_effect(
        "SQLiteConnection",
        "commit",
        ExternalApiRole::SideEffect,
        ReceiverMutabilityFact::ReadOnly,
        None,
        NullabilityState::Unknown,
        SQLITE_THROWN,
    ),
    api_effect(
        "org.sqlite.SQLiteConnection",
        "rollback",
        ExternalApiRole::SideEffect,
        ReceiverMutabilityFact::ReadOnly,
        None,
        NullabilityState::Unknown,
        SQLITE_THROWN,
    ),
    api_effect(
        "SQLiteConnection",
        "rollback",
        ExternalApiRole::SideEffect,
        ReceiverMutabilityFact::ReadOnly,
        None,
        NullabilityState::Unknown,
        SQLITE_THROWN,
    ),
    api_effect(
        "org.sqlite.SQLiteConnection",
        "close",
        ExternalApiRole::SideEffect,
        ReceiverMutabilityFact::ReadOnly,
        None,
        NullabilityState::Unknown,
        SQLITE_THROWN,
    ),
    api_effect(
        "SQLiteConnection",
        "close",
        ExternalApiRole::SideEffect,
        ReceiverMutabilityFact::ReadOnly,
        None,
        NullabilityState::Unknown,
        SQLITE_THROWN,
    ),
    api_effect(
        "org.sqlite.SQLiteStatement",
        "bindString",
        ExternalApiRole::Setter,
        ReceiverMutabilityFact::MutatesReceiver,
        None,
        NullabilityState::Unknown,
        SQLITE_THROWN,
    ),
    api_effect(
        "SQLiteStatement",
        "bindString",
        ExternalApiRole::Setter,
        ReceiverMutabilityFact::MutatesReceiver,
        None,
        NullabilityState::Unknown,
        SQLITE_THROWN,
    ),
    api_effect(
        "org.sqlite.SQLiteStatement",
        "bindInt",
        ExternalApiRole::Setter,
        ReceiverMutabilityFact::MutatesReceiver,
        None,
        NullabilityState::Unknown,
        SQLITE_THROWN,
    ),
    api_effect(
        "SQLiteStatement",
        "bindInt",
        ExternalApiRole::Setter,
        ReceiverMutabilityFact::MutatesReceiver,
        None,
        NullabilityState::Unknown,
        SQLITE_THROWN,
    ),
    api_effect(
        "org.sqlite.SQLiteStatement",
        "bindDouble",
        ExternalApiRole::Setter,
        ReceiverMutabilityFact::MutatesReceiver,
        None,
        NullabilityState::Unknown,
        SQLITE_THROWN,
    ),
    api_effect(
        "SQLiteStatement",
        "bindDouble",
        ExternalApiRole::Setter,
        ReceiverMutabilityFact::MutatesReceiver,
        None,
        NullabilityState::Unknown,
        SQLITE_THROWN,
    ),
    api_effect(
        "org.sqlite.SQLiteStatement",
        "clearParameters",
        ExternalApiRole::Setter,
        ReceiverMutabilityFact::MutatesReceiver,
        None,
        NullabilityState::Unknown,
        SQLITE_THROWN,
    ),
    api_effect(
        "SQLiteStatement",
        "clearParameters",
        ExternalApiRole::Setter,
        ReceiverMutabilityFact::MutatesReceiver,
        None,
        NullabilityState::Unknown,
        SQLITE_THROWN,
    ),
    api_effect(
        "org.sqlite.SQLiteStatement",
        "execute",
        ExternalApiRole::SideEffect,
        ReceiverMutabilityFact::ReadOnly,
        None,
        NullabilityState::Unknown,
        SQLITE_THROWN,
    ),
    api_effect(
        "SQLiteStatement",
        "execute",
        ExternalApiRole::SideEffect,
        ReceiverMutabilityFact::ReadOnly,
        None,
        NullabilityState::Unknown,
        SQLITE_THROWN,
    ),
    api_effect(
        "org.sqlite.SQLiteStatement",
        "executeUpdate",
        ExternalApiRole::SideEffect,
        ReceiverMutabilityFact::ReadOnly,
        None,
        NullabilityState::Unknown,
        SQLITE_THROWN,
    ),
    api_effect(
        "SQLiteStatement",
        "executeUpdate",
        ExternalApiRole::SideEffect,
        ReceiverMutabilityFact::ReadOnly,
        None,
        NullabilityState::Unknown,
        SQLITE_THROWN,
    ),
    api_effect(
        "org.sqlite.SQLiteStatement",
        "executeQuery",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        Some("SQLiteResultSet"),
        NullabilityState::DefinitelyNonNull,
        SQLITE_THROWN,
    ),
    api_effect(
        "SQLiteStatement",
        "executeQuery",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        Some("SQLiteResultSet"),
        NullabilityState::DefinitelyNonNull,
        SQLITE_THROWN,
    ),
    api_effect(
        "org.sqlite.SQLiteStatement",
        "close",
        ExternalApiRole::SideEffect,
        ReceiverMutabilityFact::ReadOnly,
        None,
        NullabilityState::Unknown,
        SQLITE_THROWN,
    ),
    api_effect(
        "SQLiteStatement",
        "close",
        ExternalApiRole::SideEffect,
        ReceiverMutabilityFact::ReadOnly,
        None,
        NullabilityState::Unknown,
        SQLITE_THROWN,
    ),
    api_effect(
        "org.sqlite.SQLiteResultSet",
        "next",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::MutatesReceiver,
        Some("boolean"),
        NullabilityState::DefinitelyNonNull,
        SQLITE_THROWN,
    ),
    api_effect(
        "SQLiteResultSet",
        "next",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::MutatesReceiver,
        Some("boolean"),
        NullabilityState::DefinitelyNonNull,
        SQLITE_THROWN,
    ),
    api_effect(
        "org.sqlite.SQLiteResultSet",
        "getString",
        ExternalApiRole::IndexedGetter,
        ReceiverMutabilityFact::ReadOnly,
        Some("String"),
        NullabilityState::MaybeNull,
        SQLITE_THROWN,
    ),
    api_effect(
        "SQLiteResultSet",
        "getString",
        ExternalApiRole::IndexedGetter,
        ReceiverMutabilityFact::ReadOnly,
        Some("String"),
        NullabilityState::MaybeNull,
        SQLITE_THROWN,
    ),
    api_effect(
        "org.sqlite.SQLiteResultSet",
        "getInt",
        ExternalApiRole::IndexedGetter,
        ReceiverMutabilityFact::ReadOnly,
        Some("int"),
        NullabilityState::DefinitelyNonNull,
        SQLITE_THROWN,
    ),
    api_effect(
        "SQLiteResultSet",
        "getInt",
        ExternalApiRole::IndexedGetter,
        ReceiverMutabilityFact::ReadOnly,
        Some("int"),
        NullabilityState::DefinitelyNonNull,
        SQLITE_THROWN,
    ),
    api_effect(
        "org.sqlite.SQLiteResultSet",
        "getLong",
        ExternalApiRole::IndexedGetter,
        ReceiverMutabilityFact::ReadOnly,
        Some("long"),
        NullabilityState::DefinitelyNonNull,
        SQLITE_THROWN,
    ),
    api_effect(
        "SQLiteResultSet",
        "getLong",
        ExternalApiRole::IndexedGetter,
        ReceiverMutabilityFact::ReadOnly,
        Some("long"),
        NullabilityState::DefinitelyNonNull,
        SQLITE_THROWN,
    ),
    api_effect(
        "org.sqlite.SQLiteResultSet",
        "getBoolean",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        Some("boolean"),
        NullabilityState::DefinitelyNonNull,
        SQLITE_THROWN,
    ),
    api_effect(
        "SQLiteResultSet",
        "getBoolean",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        Some("boolean"),
        NullabilityState::DefinitelyNonNull,
        SQLITE_THROWN,
    ),
    api_effect(
        "org.sqlite.SQLiteResultSet",
        "getDouble",
        ExternalApiRole::IndexedGetter,
        ReceiverMutabilityFact::ReadOnly,
        Some("double"),
        NullabilityState::DefinitelyNonNull,
        SQLITE_THROWN,
    ),
    api_effect(
        "SQLiteResultSet",
        "getDouble",
        ExternalApiRole::IndexedGetter,
        ReceiverMutabilityFact::ReadOnly,
        Some("double"),
        NullabilityState::DefinitelyNonNull,
        SQLITE_THROWN,
    ),
    api_effect(
        "org.sqlite.SQLiteResultSet",
        "close",
        ExternalApiRole::SideEffect,
        ReceiverMutabilityFact::ReadOnly,
        None,
        NullabilityState::Unknown,
        SQLITE_THROWN,
    ),
    api_effect(
        "SQLiteResultSet",
        "close",
        ExternalApiRole::SideEffect,
        ReceiverMutabilityFact::ReadOnly,
        None,
        NullabilityState::Unknown,
        SQLITE_THROWN,
    ),
];

const JAVA_SQL_API_EFFECTS: &[ExternalPackApiEffect] = &[
    api_effect(
        "driver_manager",
        "getConnection",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        Some("java.sql.Connection"),
        NullabilityState::DefinitelyNonNull,
        JAVA_SQL_THROWN,
    ),
    api_effect(
        "java.sql.DriverManager",
        "getConnection",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        Some("java.sql.Connection"),
        NullabilityState::DefinitelyNonNull,
        JAVA_SQL_THROWN,
    ),
    api_effect(
        "java.sql.Connection",
        "createStatement",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        Some("java.sql.Statement"),
        NullabilityState::DefinitelyNonNull,
        JAVA_SQL_THROWN,
    ),
    api_effect(
        "java.sql.Connection",
        "prepareStatement",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        Some("java.sql.PreparedStatement"),
        NullabilityState::DefinitelyNonNull,
        JAVA_SQL_THROWN,
    ),
    api_effect(
        "java.sql.Connection",
        "getMetaData",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        Some("java.sql.DatabaseMetaData"),
        NullabilityState::DefinitelyNonNull,
        JAVA_SQL_THROWN,
    ),
    api_effect(
        "java.sql.Connection",
        "commit",
        ExternalApiRole::SideEffect,
        ReceiverMutabilityFact::ReadOnly,
        None,
        NullabilityState::Unknown,
        JAVA_SQL_THROWN,
    ),
    api_effect(
        "java.sql.Connection",
        "rollback",
        ExternalApiRole::SideEffect,
        ReceiverMutabilityFact::ReadOnly,
        None,
        NullabilityState::Unknown,
        JAVA_SQL_THROWN,
    ),
    api_effect(
        "java.sql.Connection",
        "close",
        ExternalApiRole::SideEffect,
        ReceiverMutabilityFact::ReadOnly,
        None,
        NullabilityState::Unknown,
        JAVA_SQL_THROWN,
    ),
    api_effect(
        "java.sql.Statement",
        "executeQuery",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        Some("java.sql.ResultSet"),
        NullabilityState::DefinitelyNonNull,
        JAVA_SQL_THROWN,
    ),
    api_effect(
        "java.sql.PreparedStatement",
        "executeQuery",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        Some("java.sql.ResultSet"),
        NullabilityState::DefinitelyNonNull,
        JAVA_SQL_THROWN,
    ),
    api_effect(
        "java.sql.PreparedStatement",
        "executeUpdate",
        ExternalApiRole::SideEffect,
        ReceiverMutabilityFact::ReadOnly,
        Some("int"),
        NullabilityState::DefinitelyNonNull,
        JAVA_SQL_THROWN,
    ),
    api_effect(
        "java.sql.PreparedStatement",
        "setString",
        ExternalApiRole::SideEffect,
        ReceiverMutabilityFact::MutatesReceiver,
        None,
        NullabilityState::Unknown,
        JAVA_SQL_THROWN,
    ),
    api_effect(
        "java.sql.PreparedStatement",
        "setInt",
        ExternalApiRole::SideEffect,
        ReceiverMutabilityFact::MutatesReceiver,
        None,
        NullabilityState::Unknown,
        JAVA_SQL_THROWN,
    ),
    api_effect(
        "java.sql.PreparedStatement",
        "setDouble",
        ExternalApiRole::SideEffect,
        ReceiverMutabilityFact::MutatesReceiver,
        None,
        NullabilityState::Unknown,
        JAVA_SQL_THROWN,
    ),
    api_effect(
        "java.sql.PreparedStatement",
        "setLong",
        ExternalApiRole::SideEffect,
        ReceiverMutabilityFact::MutatesReceiver,
        None,
        NullabilityState::Unknown,
        JAVA_SQL_THROWN,
    ),
    api_effect(
        "java.sql.PreparedStatement",
        "setBoolean",
        ExternalApiRole::SideEffect,
        ReceiverMutabilityFact::MutatesReceiver,
        None,
        NullabilityState::Unknown,
        JAVA_SQL_THROWN,
    ),
    api_effect(
        "java.sql.PreparedStatement",
        "close",
        ExternalApiRole::SideEffect,
        ReceiverMutabilityFact::ReadOnly,
        None,
        NullabilityState::Unknown,
        JAVA_SQL_THROWN,
    ),
    api_effect(
        "java.sql.Statement",
        "close",
        ExternalApiRole::SideEffect,
        ReceiverMutabilityFact::ReadOnly,
        None,
        NullabilityState::Unknown,
        JAVA_SQL_THROWN,
    ),
    api_effect(
        "java.sql.ResultSet",
        "next",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::MutatesReceiver,
        Some("boolean"),
        NullabilityState::DefinitelyNonNull,
        JAVA_SQL_THROWN,
    ),
    api_effect(
        "java.sql.ResultSet",
        "getString",
        ExternalApiRole::IndexedGetter,
        ReceiverMutabilityFact::ReadOnly,
        Some("String"),
        NullabilityState::MaybeNull,
        JAVA_SQL_THROWN,
    ),
    api_effect(
        "java.sql.ResultSet",
        "getInt",
        ExternalApiRole::IndexedGetter,
        ReceiverMutabilityFact::ReadOnly,
        Some("int"),
        NullabilityState::DefinitelyNonNull,
        JAVA_SQL_THROWN,
    ),
    api_effect(
        "java.sql.ResultSet",
        "getLong",
        ExternalApiRole::IndexedGetter,
        ReceiverMutabilityFact::ReadOnly,
        Some("long"),
        NullabilityState::DefinitelyNonNull,
        JAVA_SQL_THROWN,
    ),
    api_effect(
        "java.sql.ResultSet",
        "getDouble",
        ExternalApiRole::IndexedGetter,
        ReceiverMutabilityFact::ReadOnly,
        Some("double"),
        NullabilityState::DefinitelyNonNull,
        JAVA_SQL_THROWN,
    ),
    api_effect(
        "java.sql.ResultSet",
        "getFloat",
        ExternalApiRole::IndexedGetter,
        ReceiverMutabilityFact::ReadOnly,
        Some("float"),
        NullabilityState::DefinitelyNonNull,
        JAVA_SQL_THROWN,
    ),
    api_effect(
        "java.sql.ResultSet",
        "getShort",
        ExternalApiRole::IndexedGetter,
        ReceiverMutabilityFact::ReadOnly,
        Some("short"),
        NullabilityState::DefinitelyNonNull,
        JAVA_SQL_THROWN,
    ),
    api_effect(
        "java.sql.ResultSet",
        "getByte",
        ExternalApiRole::IndexedGetter,
        ReceiverMutabilityFact::ReadOnly,
        Some("byte"),
        NullabilityState::DefinitelyNonNull,
        JAVA_SQL_THROWN,
    ),
    api_effect(
        "java.sql.ResultSet",
        "getBoolean",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        Some("boolean"),
        NullabilityState::DefinitelyNonNull,
        JAVA_SQL_THROWN,
    ),
    api_effect(
        "java.sql.ResultSet",
        "getObject",
        ExternalApiRole::IndexedGetter,
        ReceiverMutabilityFact::ReadOnly,
        Some("Object"),
        NullabilityState::MaybeNull,
        JAVA_SQL_THROWN,
    ),
    api_effect(
        "java.sql.ResultSet",
        "getBytes",
        ExternalApiRole::IndexedGetter,
        ReceiverMutabilityFact::ReadOnly,
        Some("byte[]"),
        NullabilityState::MaybeNull,
        JAVA_SQL_THROWN,
    ),
    api_effect(
        "java.sql.ResultSet",
        "getDate",
        ExternalApiRole::IndexedGetter,
        ReceiverMutabilityFact::ReadOnly,
        Some("java.sql.Date"),
        NullabilityState::MaybeNull,
        JAVA_SQL_THROWN,
    ),
    api_effect(
        "java.sql.ResultSet",
        "getTimestamp",
        ExternalApiRole::IndexedGetter,
        ReceiverMutabilityFact::ReadOnly,
        Some("java.sql.Timestamp"),
        NullabilityState::MaybeNull,
        JAVA_SQL_THROWN,
    ),
    api_effect(
        "java.sql.ResultSet",
        "wasNull",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        Some("boolean"),
        NullabilityState::DefinitelyNonNull,
        JAVA_SQL_THROWN,
    ),
    api_effect(
        "java.sql.ResultSet",
        "getMetaData",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        Some("java.sql.ResultSetMetaData"),
        NullabilityState::DefinitelyNonNull,
        JAVA_SQL_THROWN,
    ),
    api_effect(
        "java.sql.ResultSet",
        "close",
        ExternalApiRole::SideEffect,
        ReceiverMutabilityFact::ReadOnly,
        None,
        NullabilityState::Unknown,
        JAVA_SQL_THROWN,
    ),
    api_effect(
        "java.sql.ResultSetMetaData",
        "getColumnCount",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        Some("int"),
        NullabilityState::DefinitelyNonNull,
        JAVA_SQL_THROWN,
    ),
    api_effect(
        "java.sql.ResultSetMetaData",
        "getColumnName",
        ExternalApiRole::IndexedGetter,
        ReceiverMutabilityFact::ReadOnly,
        Some("String"),
        NullabilityState::DefinitelyNonNull,
        JAVA_SQL_THROWN,
    ),
    api_effect(
        "java.sql.ResultSetMetaData",
        "getColumnType",
        ExternalApiRole::IndexedGetter,
        ReceiverMutabilityFact::ReadOnly,
        Some("int"),
        NullabilityState::DefinitelyNonNull,
        JAVA_SQL_THROWN,
    ),
    api_effect(
        "java.sql.ResultSetMetaData",
        "getColumnTypeName",
        ExternalApiRole::IndexedGetter,
        ReceiverMutabilityFact::ReadOnly,
        Some("String"),
        NullabilityState::DefinitelyNonNull,
        JAVA_SQL_THROWN,
    ),
    api_effect(
        "java.sql.ResultSetMetaData",
        "getColumnClassName",
        ExternalApiRole::IndexedGetter,
        ReceiverMutabilityFact::ReadOnly,
        Some("String"),
        NullabilityState::DefinitelyNonNull,
        JAVA_SQL_THROWN,
    ),
    api_effect(
        "java.sql.ResultSetMetaData",
        "isNullable",
        ExternalApiRole::IndexedGetter,
        ReceiverMutabilityFact::ReadOnly,
        Some("int"),
        NullabilityState::DefinitelyNonNull,
        JAVA_SQL_THROWN,
    ),
    api_effect(
        "java.sql.ResultSetMetaData",
        "isSigned",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        Some("boolean"),
        NullabilityState::DefinitelyNonNull,
        JAVA_SQL_THROWN,
    ),
    api_effect(
        "java.sql.ResultSetMetaData",
        "isAutoIncrement",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        Some("boolean"),
        NullabilityState::DefinitelyNonNull,
        JAVA_SQL_THROWN,
    ),
    api_effect(
        "java.sql.ResultSetMetaData",
        "getPrecision",
        ExternalApiRole::IndexedGetter,
        ReceiverMutabilityFact::ReadOnly,
        Some("int"),
        NullabilityState::DefinitelyNonNull,
        JAVA_SQL_THROWN,
    ),
    api_effect(
        "java.sql.ResultSetMetaData",
        "getScale",
        ExternalApiRole::IndexedGetter,
        ReceiverMutabilityFact::ReadOnly,
        Some("int"),
        NullabilityState::DefinitelyNonNull,
        JAVA_SQL_THROWN,
    ),
    api_effect(
        "java.sql.ResultSetMetaData",
        "getColumnDisplaySize",
        ExternalApiRole::IndexedGetter,
        ReceiverMutabilityFact::ReadOnly,
        Some("int"),
        NullabilityState::DefinitelyNonNull,
        JAVA_SQL_THROWN,
    ),
    api_effect(
        "java.sql.DatabaseMetaData",
        "getDatabaseProductName",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        Some("String"),
        NullabilityState::DefinitelyNonNull,
        JAVA_SQL_THROWN,
    ),
    api_effect(
        "java.sql.DatabaseMetaData",
        "getTables",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        Some("java.sql.ResultSet"),
        NullabilityState::DefinitelyNonNull,
        JAVA_SQL_THROWN,
    ),
    api_effect(
        "java.sql.DatabaseMetaData",
        "getColumns",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        Some("java.sql.ResultSet"),
        NullabilityState::DefinitelyNonNull,
        JAVA_SQL_THROWN,
    ),
];

const TRIBBLE_FEATURE_API_EFFECTS: &[ExternalPackApiEffect] = &[
    semantic_getter_effect(
        "htsjdk.tribble.Feature",
        "getContig",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect("Feature", "getContig", NullabilityState::DefinitelyNonNull),
    semantic_getter_effect(
        "htsjdk.tribble.SimpleFeature",
        "getContig",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "SimpleFeature",
        "getContig",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "htsjdk.tribble.NamedFeature",
        "getContig",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "NamedFeature",
        "getContig",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "htsjdk.tribble.bed.BEDFeature",
        "getContig",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "BEDFeature",
        "getContig",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_boolean_getter_effect(
        "htsjdk.tribble.Feature",
        "contigsMatch",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_boolean_getter_effect(
        "Feature",
        "contigsMatch",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "htsjdk.tribble.Feature",
        "getStart",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect("Feature", "getStart", NullabilityState::DefinitelyNonNull),
    semantic_getter_effect(
        "htsjdk.tribble.SimpleFeature",
        "getStart",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "SimpleFeature",
        "getStart",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "htsjdk.tribble.NamedFeature",
        "getStart",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "NamedFeature",
        "getStart",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "htsjdk.tribble.bed.BEDFeature",
        "getStart",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "BEDFeature",
        "getStart",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "htsjdk.tribble.Feature",
        "getEnd",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect("Feature", "getEnd", NullabilityState::DefinitelyNonNull),
    semantic_getter_effect(
        "htsjdk.tribble.SimpleFeature",
        "getEnd",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "SimpleFeature",
        "getEnd",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "htsjdk.tribble.NamedFeature",
        "getEnd",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "NamedFeature",
        "getEnd",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "htsjdk.tribble.bed.BEDFeature",
        "getEnd",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect("BEDFeature", "getEnd", NullabilityState::DefinitelyNonNull),
    semantic_getter_effect(
        "htsjdk.tribble.SimpleFeature",
        "getName",
        NullabilityState::MaybeNull,
    ),
    semantic_getter_effect("SimpleFeature", "getName", NullabilityState::MaybeNull),
    semantic_getter_effect(
        "htsjdk.tribble.NamedFeature",
        "getName",
        NullabilityState::MaybeNull,
    ),
    semantic_getter_effect("NamedFeature", "getName", NullabilityState::MaybeNull),
    semantic_getter_effect(
        "htsjdk.tribble.bed.BEDFeature",
        "getName",
        NullabilityState::MaybeNull,
    ),
    semantic_getter_effect("BEDFeature", "getName", NullabilityState::MaybeNull),
    semantic_typed_effect(
        "htsjdk.tribble.bed.BEDFeature",
        "getScore",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "float",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "BEDFeature",
        "getScore",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "float",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.tribble.FeatureReader",
        "getHeader",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "VCFHeader",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "FeatureReader",
        "getHeader",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "VCFHeader",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect_with_arity(
        "htsjdk.tribble.AbstractFeatureReader",
        "getFeatureReader",
        6,
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "FeatureReader",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect_with_arity(
        "AbstractFeatureReader",
        "getFeatureReader",
        6,
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "FeatureReader",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect_with_arity(
        "htsjdk.tribble.FeatureReader",
        "query",
        1,
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "CloseableIterator<Object>",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect_with_arity(
        "FeatureReader",
        "query",
        1,
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "CloseableIterator<Object>",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect_with_arity(
        "htsjdk.tribble.FeatureReader",
        "query",
        3,
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "CloseableTribbleIterator",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect_with_arity(
        "FeatureReader",
        "query",
        3,
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "CloseableTribbleIterator",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.tribble.FeatureReader",
        "iterator",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "CloseableIterator<Object>",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "FeatureReader",
        "iterator",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "CloseableIterator<Object>",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.tribble.FeatureReader",
        "getSequenceNames",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "List<String>",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "FeatureReader",
        "getSequenceNames",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "List<String>",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_boolean_getter_effect(
        "htsjdk.tribble.FeatureReader",
        "isQueryable",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_boolean_getter_effect(
        "FeatureReader",
        "isQueryable",
        NullabilityState::DefinitelyNonNull,
    ),
    api_effect(
        "htsjdk.tribble.FeatureReader",
        "close",
        ExternalApiRole::SideEffect,
        ReceiverMutabilityFact::MutatesReceiver,
        None,
        NullabilityState::Unknown,
        &[],
    ),
    api_effect(
        "FeatureReader",
        "close",
        ExternalApiRole::SideEffect,
        ReceiverMutabilityFact::MutatesReceiver,
        None,
        NullabilityState::Unknown,
        &[],
    ),
];

const VCF_API_EFFECTS: &[ExternalPackApiEffect] = &[
    semantic_typed_effect(
        "htsjdk.variant.vcf.VCFFileReader",
        "getFileHeader",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "VCFHeader",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "VCFFileReader",
        "getFileHeader",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "VCFHeader",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.vcf.VCFFileReader",
        "iterator",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "CloseableIterator<VariantContext>",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "VCFFileReader",
        "iterator",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "CloseableIterator<VariantContext>",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.vcf.VCFHeader",
        "getGenotypeSamples",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "List<String>",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "VCFHeader",
        "getGenotypeSamples",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "List<String>",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.vcf.VCFHeader",
        "getSampleNamesInOrder",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "List<String>",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "VCFHeader",
        "getSampleNamesInOrder",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "List<String>",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.vcf.VCFHeader",
        "getMetaDataInSortedOrder",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Set<VCFHeaderLine>",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "VCFHeader",
        "getMetaDataInSortedOrder",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Set<VCFHeaderLine>",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.vcf.VCFHeader",
        "getNGenotypeSamples",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "VCFHeader",
        "getNGenotypeSamples",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_fluent_mutator_effect(
        "htsjdk.variant.variantcontext.writer.VariantContextWriterBuilder",
        "clearOptions",
        "VariantContextWriterBuilder",
    ),
    semantic_fluent_mutator_effect(
        "VariantContextWriterBuilder",
        "clearOptions",
        "VariantContextWriterBuilder",
    ),
    semantic_fluent_mutator_effect(
        "htsjdk.variant.variantcontext.writer.VariantContextWriterBuilder",
        "setCreateMD5",
        "VariantContextWriterBuilder",
    ),
    semantic_fluent_mutator_effect(
        "VariantContextWriterBuilder",
        "setCreateMD5",
        "VariantContextWriterBuilder",
    ),
    semantic_fluent_mutator_effect(
        "htsjdk.variant.variantcontext.writer.VariantContextWriterBuilder",
        "setOption",
        "VariantContextWriterBuilder",
    ),
    semantic_fluent_mutator_effect(
        "VariantContextWriterBuilder",
        "setOption",
        "VariantContextWriterBuilder",
    ),
    semantic_fluent_mutator_effect(
        "htsjdk.variant.variantcontext.writer.VariantContextWriterBuilder",
        "setOutputFileType",
        "VariantContextWriterBuilder",
    ),
    semantic_fluent_mutator_effect(
        "VariantContextWriterBuilder",
        "setOutputFileType",
        "VariantContextWriterBuilder",
    ),
    semantic_fluent_mutator_effect(
        "htsjdk.variant.variantcontext.writer.VariantContextWriterBuilder",
        "setReferenceDictionary",
        "VariantContextWriterBuilder",
    ),
    semantic_fluent_mutator_effect(
        "VariantContextWriterBuilder",
        "setReferenceDictionary",
        "VariantContextWriterBuilder",
    ),
    semantic_fluent_mutator_effect(
        "htsjdk.variant.variantcontext.writer.VariantContextWriterBuilder",
        "setOutputPath",
        "VariantContextWriterBuilder",
    ),
    semantic_fluent_mutator_effect(
        "VariantContextWriterBuilder",
        "setOutputPath",
        "VariantContextWriterBuilder",
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.writer.VariantContextWriterBuilder",
        "determineOutputTypeFromFile",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "OutputType",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "VariantContextWriterBuilder",
        "determineOutputTypeFromFile",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "OutputType",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.writer.VariantContextWriterBuilder",
        "build",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "VariantContextWriter",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "VariantContextWriterBuilder",
        "build",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "VariantContextWriter",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_mutator_effect(
        "htsjdk.variant.variantcontext.writer.VariantContextWriter",
        "setHeader",
        NullabilityState::Unknown,
    ),
    semantic_mutator_effect(
        "VariantContextWriter",
        "setHeader",
        NullabilityState::Unknown,
    ),
    semantic_mutator_effect(
        "htsjdk.variant.variantcontext.writer.VariantContextWriter",
        "writeHeader",
        NullabilityState::Unknown,
    ),
    semantic_mutator_effect(
        "VariantContextWriter",
        "writeHeader",
        NullabilityState::Unknown,
    ),
    semantic_mutator_effect(
        "htsjdk.variant.variantcontext.writer.VariantContextWriter",
        "add",
        NullabilityState::Unknown,
    ),
    semantic_mutator_effect("VariantContextWriter", "add", NullabilityState::Unknown),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.writer.VariantContextWriter",
        "checkError",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "VariantContextWriter",
        "checkError",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_mutator_effect(
        "htsjdk.variant.variantcontext.writer.VariantContextWriter",
        "close",
        NullabilityState::Unknown,
    ),
    semantic_mutator_effect("VariantContextWriter", "close", NullabilityState::Unknown),
    semantic_typed_effect(
        "htsjdk.variant.vcf.VCFHeader",
        "getMetaDataInInputOrder",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Set<VCFHeaderLine>",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "VCFHeader",
        "getMetaDataInInputOrder",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Set<VCFHeaderLine>",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "htsjdk.variant.vcf.VCFHeader",
        "getInfoHeaderLines",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "VCFHeader",
        "getInfoHeaderLines",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.vcf.VCFHeader",
        "getInfoHeaderLine",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "VCFInfoHeaderLine",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "VCFHeader",
        "getInfoHeaderLine",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "VCFInfoHeaderLine",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.vcf.VCFHeader",
        "getMetaDataLine",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "VCFHeaderLine",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "VCFHeader",
        "getMetaDataLine",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "VCFHeaderLine",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.vcf.VCFHeader",
        "getOtherHeaderLine",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "VCFHeaderLine",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "VCFHeader",
        "getOtherHeaderLine",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "VCFHeaderLine",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.vcf.VCFHeader",
        "hasInfoLine",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "VCFHeader",
        "hasInfoLine",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.vcf.VCFHeader",
        "hasFormatLine",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "VCFHeader",
        "hasFormatLine",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.vcf.VCFHeader",
        "hasGenotypingData",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "VCFHeader",
        "hasGenotypingData",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.vcf.VCFHeader",
        "getFormatHeaderLine",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "VCFFormatHeaderLine",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.vcf.VCFHeader",
        "getVCFRecordComparator",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Comparator<VariantContext>",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "VCFHeader",
        "getVCFRecordComparator",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Comparator<VariantContext>",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "VCFHeader",
        "getFormatHeaderLine",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "VCFFormatHeaderLine",
        NullabilityState::MaybeNull,
    ),
    semantic_getter_effect(
        "htsjdk.variant.vcf.VCFHeader",
        "getFormatHeaderLines",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "VCFHeader",
        "getFormatHeaderLines",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "htsjdk.variant.vcf.VCFHeader",
        "getFilterLines",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "VCFHeader",
        "getFilterLines",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "htsjdk.variant.vcf.VCFHeader",
        "getContigLines",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "VCFHeader",
        "getContigLines",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.vcf.VCFHeader",
        "getSequenceDictionary",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "SAMSequenceDictionary",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "VCFHeader",
        "getSequenceDictionary",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "SAMSequenceDictionary",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_mutator_effect(
        "htsjdk.variant.vcf.VCFHeader",
        "setSequenceDictionary",
        NullabilityState::Unknown,
    ),
    semantic_mutator_effect(
        "VCFHeader",
        "setSequenceDictionary",
        NullabilityState::Unknown,
    ),
    semantic_getter_effect(
        "htsjdk.variant.vcf.VCFHeader",
        "getVCFHeaderVersion",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "VCFHeader",
        "getVCFHeaderVersion",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.vcf.VCFHeaderVersion",
        "getFormatString",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "String",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.vcf.VCFHeaderVersion",
        "getVersionString",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "String",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "VCFHeaderVersion",
        "getVersionString",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "String",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "VCFHeaderVersion",
        "getFormatString",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "String",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "htsjdk.variant.vcf.VCFHeader",
        "getColumnCount",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "VCFHeader",
        "getColumnCount",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_mutator_effect(
        "htsjdk.variant.vcf.VCFHeader",
        "addMetaDataLine",
        NullabilityState::Unknown,
    ),
    semantic_mutator_effect("VCFHeader", "addMetaDataLine", NullabilityState::Unknown),
    semantic_getter_effect(
        "htsjdk.variant.vcf.VCFHeaderLine",
        "getKey",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "VCFHeaderLine",
        "getKey",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "htsjdk.variant.vcf.VCFInfoHeaderLine",
        "getKey",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "VCFInfoHeaderLine",
        "getKey",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "htsjdk.variant.vcf.VCFFormatHeaderLine",
        "getKey",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "VCFFormatHeaderLine",
        "getKey",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "htsjdk.variant.vcf.VCFFilterHeaderLine",
        "getKey",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "VCFFilterHeaderLine",
        "getKey",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "htsjdk.variant.vcf.VCFHeaderLine",
        "getValue",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "VCFHeaderLine",
        "getValue",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "htsjdk.variant.vcf.VCFInfoHeaderLine",
        "getValue",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "VCFInfoHeaderLine",
        "getValue",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "htsjdk.variant.vcf.VCFFormatHeaderLine",
        "getValue",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "VCFFormatHeaderLine",
        "getValue",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "htsjdk.variant.vcf.VCFFilterHeaderLine",
        "getValue",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "VCFFilterHeaderLine",
        "getValue",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "htsjdk.variant.vcf.VCFHeaderLine",
        "getID",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "VCFHeaderLine",
        "getID",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "htsjdk.variant.vcf.VCFInfoHeaderLine",
        "getID",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "VCFInfoHeaderLine",
        "getID",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "htsjdk.variant.vcf.VCFFormatHeaderLine",
        "getID",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "VCFFormatHeaderLine",
        "getID",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "htsjdk.variant.vcf.VCFFilterHeaderLine",
        "getID",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "VCFFilterHeaderLine",
        "getID",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "htsjdk.variant.vcf.VCFHeaderLine",
        "getDescription",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "VCFHeaderLine",
        "getDescription",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "htsjdk.variant.vcf.VCFInfoHeaderLine",
        "getDescription",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "VCFInfoHeaderLine",
        "getDescription",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "htsjdk.variant.vcf.VCFFormatHeaderLine",
        "getDescription",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "VCFFormatHeaderLine",
        "getDescription",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "htsjdk.variant.vcf.VCFFilterHeaderLine",
        "getDescription",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "VCFFilterHeaderLine",
        "getDescription",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "htsjdk.variant.vcf.VCFHeaderLine",
        "toString",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "VCFHeaderLine",
        "toString",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "htsjdk.variant.vcf.VCFInfoHeaderLine",
        "toString",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "VCFInfoHeaderLine",
        "toString",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "htsjdk.variant.vcf.VCFFormatHeaderLine",
        "toString",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "VCFFormatHeaderLine",
        "toString",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "htsjdk.variant.vcf.VCFFilterHeaderLine",
        "toString",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "VCFFilterHeaderLine",
        "toString",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "htsjdk.variant.vcf.VCFInfoHeaderLine",
        "getCount",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "VCFInfoHeaderLine",
        "getCount",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "htsjdk.variant.vcf.VCFFormatHeaderLine",
        "getCount",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "VCFFormatHeaderLine",
        "getCount",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "htsjdk.variant.vcf.VCFInfoHeaderLine",
        "getCountType",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "VCFInfoHeaderLine",
        "getCountType",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "htsjdk.variant.vcf.VCFFormatHeaderLine",
        "getCountType",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "VCFFormatHeaderLine",
        "getCountType",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.vcf.VCFHeaderLineType",
        "valueOf",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "VCFHeaderLineType",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "VCFHeaderLineType",
        "valueOf",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "VCFHeaderLineType",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.vcf.VCFInfoHeaderLine",
        "isFixedCount",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "VCFInfoHeaderLine",
        "isFixedCount",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.vcf.VCFFormatHeaderLine",
        "isFixedCount",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "VCFFormatHeaderLine",
        "isFixedCount",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "htsjdk.variant.vcf.VCFInfoHeaderLine",
        "getType",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "VCFInfoHeaderLine",
        "getType",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "htsjdk.variant.vcf.VCFFormatHeaderLine",
        "getType",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "VCFFormatHeaderLine",
        "getType",
        NullabilityState::DefinitelyNonNull,
    ),
];

const SAM_HEADER_API_EFFECTS: &[ExternalPackApiEffect] = &[
    semantic_indexed_getter_effect(
        "htsjdk.samtools.SAMSequenceDictionary",
        "getSequence",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_indexed_getter_effect(
        "SAMSequenceDictionary",
        "getSequence",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "htsjdk.samtools.SAMSequenceDictionary",
        "getSequenceIndex",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "SAMSequenceDictionary",
        "getSequenceIndex",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.SAMSequenceDictionary",
        "getReferenceLength",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "SAMSequenceDictionary",
        "getReferenceLength",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "htsjdk.samtools.SAMSequenceDictionary",
        "getSequences",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "SAMSequenceDictionary",
        "getSequences",
        NullabilityState::DefinitelyNonNull,
    ),
    api_effect(
        "htsjdk.samtools.SAMSequenceDictionary",
        "assertSameDictionary",
        ExternalApiRole::SideEffect,
        ReceiverMutabilityFact::ReadOnly,
        None,
        NullabilityState::Unknown,
        SAM_DICTIONARY_ASSERT_THROWN,
    ),
    api_effect(
        "SAMSequenceDictionary",
        "assertSameDictionary",
        ExternalApiRole::SideEffect,
        ReceiverMutabilityFact::ReadOnly,
        None,
        NullabilityState::Unknown,
        SAM_DICTIONARY_ASSERT_THROWN,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.SAMSequenceDictionary",
        "isSameDictionary",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "SAMSequenceDictionary",
        "isSameDictionary",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_mutator_effect(
        "htsjdk.samtools.SAMSequenceDictionary",
        "addSequence",
        NullabilityState::Unknown,
    ),
    semantic_mutator_effect(
        "SAMSequenceDictionary",
        "addSequence",
        NullabilityState::Unknown,
    ),
    semantic_getter_effect(
        "htsjdk.samtools.SAMSequenceDictionary",
        "size",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "SAMSequenceDictionary",
        "size",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "htsjdk.samtools.SAMSequenceDictionary",
        "isEmpty",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "SAMSequenceDictionary",
        "isEmpty",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.SAMFileHeader",
        "getComments",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "List<String>",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "SAMFileHeader",
        "getComments",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "List<String>",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "htsjdk.samtools.SAMSequenceRecord",
        "getSequenceName",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "SAMSequenceRecord",
        "getSequenceName",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "htsjdk.samtools.SAMSequenceRecord",
        "getSequenceLength",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "SAMSequenceRecord",
        "getSequenceLength",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_fluent_mutator_effect(
        "htsjdk.samtools.SAMSequenceRecord",
        "setSequenceLength",
        "SAMSequenceRecord",
    ),
    semantic_fluent_mutator_effect(
        "SAMSequenceRecord",
        "setSequenceLength",
        "SAMSequenceRecord",
    ),
    semantic_getter_effect(
        "htsjdk.samtools.SAMSequenceRecord",
        "getSequenceIndex",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "SAMSequenceRecord",
        "getSequenceIndex",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.SAMSequenceRecord",
        "getMd5",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "String",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "SAMSequenceRecord",
        "getMd5",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "String",
        NullabilityState::MaybeNull,
    ),
    semantic_getter_effect(
        "htsjdk.samtools.SAMSequenceRecord",
        "getAttribute",
        NullabilityState::MaybeNull,
    ),
    semantic_getter_effect(
        "SAMSequenceRecord",
        "getAttribute",
        NullabilityState::MaybeNull,
    ),
    semantic_getter_effect(
        "htsjdk.samtools.SAMSequenceRecord",
        "getAttributes",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "SAMSequenceRecord",
        "getAttributes",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.SAMSequenceRecord",
        "getAssembly",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "String",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "SAMSequenceRecord",
        "getAssembly",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "String",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.SAMSequenceRecord",
        "truncateSequenceName",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "String",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "SAMSequenceRecord",
        "truncateSequenceName",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "String",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "htsjdk.samtools.SAMFileHeader",
        "getSequenceDictionary",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "SAMFileHeader",
        "getSequenceDictionary",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.SAMFileHeader",
        "getSequence",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "SAMSequenceRecord",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "SAMFileHeader",
        "getSequence",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "SAMSequenceRecord",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.SAMFileHeader",
        "getSequenceIndex",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "SAMFileHeader",
        "getSequenceIndex",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "htsjdk.samtools.SAMFileHeader",
        "getSortOrder",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "SAMFileHeader",
        "getSortOrder",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "htsjdk.samtools.SAMFileHeader",
        "getGroupOrder",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "SAMFileHeader",
        "getGroupOrder",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.SAMFileHeader.SortOrder",
        "name",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "String",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "SAMFileHeader.SortOrder",
        "name",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "String",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "SortOrder",
        "name",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "String",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.SAMFileHeader.GroupOrder",
        "name",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "String",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "SAMFileHeader.GroupOrder",
        "name",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "String",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "GroupOrder",
        "name",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "String",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "htsjdk.samtools.SAMFileHeader",
        "getReadGroups",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "SAMFileHeader",
        "getReadGroups",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.SAMFileHeader",
        "getReadGroup",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "SAMReadGroupRecord",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "SAMFileHeader",
        "getReadGroup",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "SAMReadGroupRecord",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.SAMFileHeader",
        "clone",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "SAMFileHeader",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "SAMFileHeader",
        "clone",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "SAMFileHeader",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_mutator_effect(
        "htsjdk.samtools.SAMFileHeader",
        "addSequence",
        NullabilityState::Unknown,
    ),
    semantic_mutator_effect("SAMFileHeader", "addSequence", NullabilityState::Unknown),
    api_effect_with_arity(
        "htsjdk.samtools.SAMTextHeaderCodec",
        "encode",
        3,
        ExternalApiRole::SideEffect,
        ReceiverMutabilityFact::ReadOnly,
        None,
        NullabilityState::Unknown,
        &[],
    ),
    api_effect_with_arity(
        "SAMTextHeaderCodec",
        "encode",
        3,
        ExternalApiRole::SideEffect,
        ReceiverMutabilityFact::ReadOnly,
        None,
        NullabilityState::Unknown,
        &[],
    ),
    semantic_getter_effect(
        "htsjdk.samtools.SAMFileHeader",
        "getProgramRecords",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "SAMFileHeader",
        "getProgramRecords",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.SAMFileHeader",
        "getProgramRecord",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "SAMProgramRecord",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.SAMFileHeader",
        "getSAMString",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "String",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "SAMFileHeader",
        "getSAMString",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "String",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "SAMFileHeader",
        "getProgramRecord",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "SAMProgramRecord",
        NullabilityState::MaybeNull,
    ),
    semantic_mutator_effect(
        "htsjdk.samtools.SAMFileHeader",
        "setSequenceDictionary",
        NullabilityState::Unknown,
    ),
    semantic_mutator_effect(
        "SAMFileHeader",
        "setSequenceDictionary",
        NullabilityState::Unknown,
    ),
    semantic_mutator_effect(
        "htsjdk.samtools.SAMFileHeader",
        "setComments",
        NullabilityState::Unknown,
    ),
    semantic_mutator_effect("SAMFileHeader", "setComments", NullabilityState::Unknown),
    semantic_mutator_effect(
        "htsjdk.samtools.SAMFileHeader",
        "setReadGroups",
        NullabilityState::Unknown,
    ),
    semantic_mutator_effect("SAMFileHeader", "setReadGroups", NullabilityState::Unknown),
    semantic_mutator_effect(
        "htsjdk.samtools.SAMFileHeader",
        "setProgramRecords",
        NullabilityState::Unknown,
    ),
    semantic_mutator_effect(
        "SAMFileHeader",
        "setProgramRecords",
        NullabilityState::Unknown,
    ),
    semantic_mutator_effect(
        "htsjdk.samtools.SAMFileHeader",
        "setAttribute",
        NullabilityState::Unknown,
    ),
    semantic_mutator_effect("SAMFileHeader", "setAttribute", NullabilityState::Unknown),
    semantic_mutator_effect(
        "htsjdk.samtools.SAMFileHeader",
        "setSortOrder",
        NullabilityState::Unknown,
    ),
    semantic_mutator_effect("SAMFileHeader", "setSortOrder", NullabilityState::Unknown),
    semantic_mutator_effect(
        "htsjdk.samtools.SAMFileHeader",
        "setGroupOrder",
        NullabilityState::Unknown,
    ),
    semantic_mutator_effect("SAMFileHeader", "setGroupOrder", NullabilityState::Unknown),
    semantic_mutator_effect(
        "htsjdk.samtools.SAMFileHeader",
        "addReadGroup",
        NullabilityState::Unknown,
    ),
    semantic_mutator_effect("SAMFileHeader", "addReadGroup", NullabilityState::Unknown),
    semantic_mutator_effect(
        "htsjdk.samtools.SAMFileHeader",
        "addProgramRecord",
        NullabilityState::Unknown,
    ),
    semantic_mutator_effect(
        "SAMFileHeader",
        "addProgramRecord",
        NullabilityState::Unknown,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.SAMProgramRecord",
        "getId",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "String",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "SAMProgramRecord",
        "getId",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "String",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.SAMProgramRecord",
        "getProgramName",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "String",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "SAMProgramRecord",
        "getProgramName",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "String",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.SAMProgramRecord",
        "getProgramGroupId",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "String",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "SAMProgramRecord",
        "getProgramGroupId",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "String",
        NullabilityState::MaybeNull,
    ),
    semantic_mutator_effect(
        "htsjdk.samtools.SAMProgramRecord",
        "setCommandLine",
        NullabilityState::Unknown,
    ),
    semantic_mutator_effect(
        "SAMProgramRecord",
        "setCommandLine",
        NullabilityState::Unknown,
    ),
    semantic_mutator_effect(
        "htsjdk.samtools.SAMProgramRecord",
        "setPreviousProgramGroupId",
        NullabilityState::Unknown,
    ),
    semantic_mutator_effect(
        "SAMProgramRecord",
        "setPreviousProgramGroupId",
        NullabilityState::Unknown,
    ),
    semantic_mutator_effect(
        "htsjdk.samtools.SAMProgramRecord",
        "setProgramName",
        NullabilityState::Unknown,
    ),
    semantic_mutator_effect(
        "SAMProgramRecord",
        "setProgramName",
        NullabilityState::Unknown,
    ),
    semantic_mutator_effect(
        "htsjdk.samtools.SAMProgramRecord",
        "setProgramVersion",
        NullabilityState::Unknown,
    ),
    semantic_mutator_effect(
        "SAMProgramRecord",
        "setProgramVersion",
        NullabilityState::Unknown,
    ),
    semantic_getter_effect(
        "htsjdk.samtools.SAMReadGroupRecord",
        "getId",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "SAMReadGroupRecord",
        "getId",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "htsjdk.samtools.SAMReadGroupRecord",
        "getSample",
        NullabilityState::MaybeNull,
    ),
    semantic_getter_effect(
        "SAMReadGroupRecord",
        "getSample",
        NullabilityState::MaybeNull,
    ),
    semantic_getter_effect(
        "htsjdk.samtools.SAMReadGroupRecord",
        "getLibrary",
        NullabilityState::MaybeNull,
    ),
    semantic_getter_effect(
        "SAMReadGroupRecord",
        "getLibrary",
        NullabilityState::MaybeNull,
    ),
    semantic_getter_effect(
        "htsjdk.samtools.SAMReadGroupRecord",
        "getPlatform",
        NullabilityState::MaybeNull,
    ),
    semantic_getter_effect(
        "SAMReadGroupRecord",
        "getPlatform",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.SAMReadGroupRecord",
        "getPlatformUnit",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "String",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "SAMReadGroupRecord",
        "getPlatformUnit",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "String",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.SAMReadGroupRecord",
        "getFlowOrder",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "String",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "SAMReadGroupRecord",
        "getFlowOrder",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "String",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.SAMReadGroupRecord",
        "getSequencingCenter",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "String",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "SAMReadGroupRecord",
        "getSequencingCenter",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "String",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.SAMReadGroupRecord",
        "getAttribute",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Object",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "SAMReadGroupRecord",
        "getAttribute",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Object",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.SAMReadGroupRecord",
        "getReadGroupId",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "String",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "SAMReadGroupRecord",
        "getReadGroupId",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "String",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_setter_effect(
        "htsjdk.samtools.SAMReadGroupRecord",
        "setSample",
        NullabilityState::Unknown,
    ),
    semantic_setter_effect("SAMReadGroupRecord", "setSample", NullabilityState::Unknown),
    semantic_setter_effect(
        "htsjdk.samtools.SAMReadGroupRecord",
        "setLibrary",
        NullabilityState::Unknown,
    ),
    semantic_setter_effect(
        "SAMReadGroupRecord",
        "setLibrary",
        NullabilityState::Unknown,
    ),
    semantic_setter_effect(
        "htsjdk.samtools.SAMReadGroupRecord",
        "setPlatform",
        NullabilityState::Unknown,
    ),
    semantic_setter_effect(
        "SAMReadGroupRecord",
        "setPlatform",
        NullabilityState::Unknown,
    ),
    semantic_setter_effect(
        "htsjdk.samtools.SAMReadGroupRecord",
        "setPlatformUnit",
        NullabilityState::Unknown,
    ),
    semantic_setter_effect(
        "SAMReadGroupRecord",
        "setPlatformUnit",
        NullabilityState::Unknown,
    ),
    semantic_setter_effect(
        "htsjdk.samtools.SAMReadGroupRecord",
        "setFlowOrder",
        NullabilityState::Unknown,
    ),
    semantic_setter_effect(
        "SAMReadGroupRecord",
        "setFlowOrder",
        NullabilityState::Unknown,
    ),
    semantic_setter_effect(
        "htsjdk.samtools.SAMReadGroupRecord",
        "setSequencingCenter",
        NullabilityState::Unknown,
    ),
    semantic_setter_effect(
        "SAMReadGroupRecord",
        "setSequencingCenter",
        NullabilityState::Unknown,
    ),
    semantic_setter_effect(
        "htsjdk.samtools.SAMReadGroupRecord",
        "setAttribute",
        NullabilityState::Unknown,
    ),
    semantic_setter_effect(
        "SAMReadGroupRecord",
        "setAttribute",
        NullabilityState::Unknown,
    ),
];

const SAM_RECORD_API_EFFECTS: &[ExternalPackApiEffect] = &[
    semantic_typed_effect(
        "htsjdk.samtools.SAMRecord",
        "getContig",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "String",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "SAMRecord",
        "getContig",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "String",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.SAMRecord",
        "getStart",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "SAMRecord",
        "getStart",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.SAMRecord",
        "getEnd",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "SAMRecord",
        "getEnd",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.SAMRecord",
        "getReadName",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "String",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "SAMRecord",
        "getReadName",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "String",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.SAMRecord",
        "getHeader",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "SAMFileHeader",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "SAMRecord",
        "getHeader",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "SAMFileHeader",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.SAMRecordCoordinateComparator",
        "compare",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "SAMRecordCoordinateComparator",
        "compare",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.SAMRecordQueryNameComparator",
        "compare",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "SAMRecordQueryNameComparator",
        "compare",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_mutator_effect("htsjdk.samtools.Cigar", "add", NullabilityState::Unknown),
    semantic_mutator_effect("Cigar", "add", NullabilityState::Unknown),
    semantic_typed_effect(
        "htsjdk.samtools.Cigar",
        "getCigarElement",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "CigarElement",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "Cigar",
        "getCigarElement",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "CigarElement",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.Cigar",
        "getCigarElements",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "List<CigarElement>",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "Cigar",
        "getCigarElements",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "List<CigarElement>",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.Cigar",
        "getReadLength",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "Cigar",
        "getReadLength",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.Cigar",
        "getReferenceLength",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "Cigar",
        "getReferenceLength",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.Cigar",
        "getPaddedReferenceLength",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "Cigar",
        "getPaddedReferenceLength",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.Cigar",
        "isEmpty",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "Cigar",
        "isEmpty",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.Cigar",
        "getFirstCigarElement",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "CigarElement",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "Cigar",
        "getFirstCigarElement",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "CigarElement",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.Cigar",
        "getLastCigarElement",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "CigarElement",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "Cigar",
        "getLastCigarElement",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "CigarElement",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.Cigar",
        "isValid",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "Cigar",
        "isValid",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.Cigar",
        "numCigarElements",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "Cigar",
        "numCigarElements",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.CigarElement",
        "getOperator",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "CigarOperator",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "CigarElement",
        "getOperator",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "CigarOperator",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.CigarOperator",
        "consumesReadBases",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "CigarOperator",
        "consumesReadBases",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.CigarOperator",
        "consumesReferenceBases",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "CigarOperator",
        "consumesReferenceBases",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.CigarOperator",
        "isClipping",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "CigarOperator",
        "isClipping",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.CigarOperator",
        "isAlignment",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "CigarOperator",
        "isAlignment",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.CigarOperator",
        "isIndel",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "CigarOperator",
        "isIndel",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.CigarOperator",
        "characterToEnum",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "CigarOperator",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "CigarOperator",
        "characterToEnum",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "CigarOperator",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.SAMRecord",
        "getCigar",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Cigar",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "SAMRecord",
        "getCigar",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Cigar",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.SAMRecord",
        "getReadLength",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "SAMRecord",
        "getReadLength",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.SAMRecord",
        "getAlignmentStart",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "SAMRecord",
        "getAlignmentStart",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.SAMRecord",
        "getAlignmentEnd",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "SAMRecord",
        "getAlignmentEnd",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.SAMRecord",
        "getMappingQuality",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "SAMRecord",
        "getMappingQuality",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.SAMRecord",
        "getFlags",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "SAMRecord",
        "getFlags",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.SAMRecord",
        "getMateAlignmentStart",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "SAMRecord",
        "getMateAlignmentStart",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.SAMRecord",
        "getInferredInsertSize",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "SAMRecord",
        "getInferredInsertSize",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.SAMRecord",
        "getCigarLength",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "SAMRecord",
        "getCigarLength",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.SAMRecord",
        "getMateAlignmentEnd",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "SAMRecord",
        "getMateAlignmentEnd",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.SAMRecord",
        "getMateReferenceIndex",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "SAMRecord",
        "getMateReferenceIndex",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.SAMRecord",
        "getUnclippedStart",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "SAMRecord",
        "getUnclippedStart",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.SAMRecord",
        "getUnclippedEnd",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "SAMRecord",
        "getUnclippedEnd",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.SAMRecord",
        "getReadUnmappedFlag",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "SAMRecord",
        "getReadUnmappedFlag",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.SAMRecord",
        "getReadNegativeStrandFlag",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "SAMRecord",
        "getReadNegativeStrandFlag",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.SAMRecord",
        "getReadPairedFlag",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "SAMRecord",
        "getReadPairedFlag",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.SAMRecord",
        "getMateUnmappedFlag",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "SAMRecord",
        "getMateUnmappedFlag",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.SAMRecord",
        "getSecondOfPairFlag",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "SAMRecord",
        "getSecondOfPairFlag",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.SAMRecord",
        "getFirstOfPairFlag",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "SAMRecord",
        "getFirstOfPairFlag",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.SAMRecord",
        "getProperPairFlag",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "SAMRecord",
        "getProperPairFlag",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.SAMRecord",
        "getDuplicateReadFlag",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "SAMRecord",
        "getDuplicateReadFlag",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.SAMRecord",
        "isSecondaryAlignment",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "SAMRecord",
        "isSecondaryAlignment",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.SAMRecord",
        "isSecondaryOrSupplementary",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "SAMRecord",
        "isSecondaryOrSupplementary",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.SAMRecord",
        "getSupplementaryAlignmentFlag",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "SAMRecord",
        "getSupplementaryAlignmentFlag",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.SAMRecord",
        "getMateNegativeStrandFlag",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "SAMRecord",
        "getMateNegativeStrandFlag",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.SAMRecord",
        "getReadFailsVendorQualityCheckFlag",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "SAMRecord",
        "getReadFailsVendorQualityCheckFlag",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.SAMRecord",
        "getReferenceName",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "String",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "SAMRecord",
        "getReferenceName",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "String",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.SAMRecord",
        "getMateReferenceName",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "String",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "SAMRecord",
        "getMateReferenceName",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "String",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.SAMRecord",
        "getCigarString",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "String",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "SAMRecord",
        "getCigarString",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "String",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.SAMRecord",
        "getSAMString",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "String",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "SAMRecord",
        "getSAMString",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "String",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.SAMRecord",
        "getAttribute",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Object",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "SAMRecord",
        "getAttribute",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Object",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.SAMRecord",
        "getIntegerAttribute",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Integer",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "SAMRecord",
        "getIntegerAttribute",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Integer",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.SAMRecord",
        "getByteArrayAttribute",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "byte[]",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "SAMRecord",
        "getByteArrayAttribute",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "byte[]",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.SAMRecord",
        "getSignedByteArrayAttribute",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "byte[]",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "SAMRecord",
        "getSignedByteArrayAttribute",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "byte[]",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.SAMRecord",
        "getStringAttribute",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "String",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "SAMRecord",
        "getStringAttribute",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "String",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.SAMRecord",
        "getTransientAttribute",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Object",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "SAMRecord",
        "getTransientAttribute",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Object",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.SAMRecord",
        "getReadGroup",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "SAMReadGroupRecord",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "SAMRecord",
        "getReadGroup",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "SAMReadGroupRecord",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.SAMRecord",
        "getAttributes",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "List<SAMTagAndValue>",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "SAMRecord",
        "getAttributes",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "List<SAMTagAndValue>",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.SAMRecord",
        "getAlignmentBlocks",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "List<AlignmentBlock>",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "SAMRecord",
        "getAlignmentBlocks",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "List<AlignmentBlock>",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.SAMRecord",
        "clone",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "SAMRecord",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "SAMRecord",
        "clone",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "SAMRecord",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.SAMRecord",
        "deepCopy",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "SAMRecord",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "SAMRecord",
        "deepCopy",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "SAMRecord",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.SAMRecord",
        "hasAttribute",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "SAMRecord",
        "hasAttribute",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_mutator_effect(
        "htsjdk.samtools.SAMRecord",
        "setAttribute",
        NullabilityState::Unknown,
    ),
    semantic_mutator_effect("SAMRecord", "setAttribute", NullabilityState::Unknown),
    semantic_mutator_effect(
        "htsjdk.samtools.SAMRecord",
        "setBaseQualities",
        NullabilityState::Unknown,
    ),
    semantic_mutator_effect("SAMRecord", "setBaseQualities", NullabilityState::Unknown),
    semantic_mutator_effect(
        "htsjdk.samtools.SAMRecord",
        "setReadBases",
        NullabilityState::Unknown,
    ),
    semantic_mutator_effect("SAMRecord", "setReadBases", NullabilityState::Unknown),
    semantic_mutator_effect(
        "htsjdk.samtools.SAMRecord",
        "setCigar",
        NullabilityState::Unknown,
    ),
    semantic_mutator_effect("SAMRecord", "setCigar", NullabilityState::Unknown),
    semantic_mutator_effect(
        "htsjdk.samtools.SAMRecord",
        "setReadName",
        NullabilityState::Unknown,
    ),
    semantic_mutator_effect("SAMRecord", "setReadName", NullabilityState::Unknown),
    semantic_mutator_effect(
        "htsjdk.samtools.SAMRecord",
        "setReferenceName",
        NullabilityState::Unknown,
    ),
    semantic_mutator_effect("SAMRecord", "setReferenceName", NullabilityState::Unknown),
    semantic_mutator_effect(
        "htsjdk.samtools.SAMRecord",
        "setMappingQuality",
        NullabilityState::Unknown,
    ),
    semantic_mutator_effect("SAMRecord", "setMappingQuality", NullabilityState::Unknown),
    semantic_mutator_effect(
        "htsjdk.samtools.SAMRecord",
        "setMateUnmappedFlag",
        NullabilityState::Unknown,
    ),
    semantic_mutator_effect(
        "SAMRecord",
        "setMateUnmappedFlag",
        NullabilityState::Unknown,
    ),
    semantic_mutator_effect(
        "htsjdk.samtools.SAMRecord",
        "setProperPairFlag",
        NullabilityState::Unknown,
    ),
    semantic_mutator_effect("SAMRecord", "setProperPairFlag", NullabilityState::Unknown),
    semantic_mutator_effect(
        "htsjdk.samtools.SAMRecord",
        "setSecondaryAlignment",
        NullabilityState::Unknown,
    ),
    semantic_mutator_effect(
        "SAMRecord",
        "setSecondaryAlignment",
        NullabilityState::Unknown,
    ),
    semantic_mutator_effect(
        "htsjdk.samtools.SAMRecord",
        "setFirstOfPairFlag",
        NullabilityState::Unknown,
    ),
    semantic_mutator_effect("SAMRecord", "setFirstOfPairFlag", NullabilityState::Unknown),
    semantic_mutator_effect(
        "htsjdk.samtools.SAMRecord",
        "setSecondOfPairFlag",
        NullabilityState::Unknown,
    ),
    semantic_mutator_effect(
        "SAMRecord",
        "setSecondOfPairFlag",
        NullabilityState::Unknown,
    ),
    semantic_mutator_effect(
        "htsjdk.samtools.SAMRecord",
        "setCigarString",
        NullabilityState::Unknown,
    ),
    semantic_mutator_effect("SAMRecord", "setCigarString", NullabilityState::Unknown),
    semantic_mutator_effect(
        "htsjdk.samtools.SAMRecord",
        "setMateAlignmentStart",
        NullabilityState::Unknown,
    ),
    semantic_mutator_effect(
        "SAMRecord",
        "setMateAlignmentStart",
        NullabilityState::Unknown,
    ),
    semantic_mutator_effect(
        "htsjdk.samtools.SAMRecord",
        "setMateNegativeStrandFlag",
        NullabilityState::Unknown,
    ),
    semantic_mutator_effect(
        "SAMRecord",
        "setMateNegativeStrandFlag",
        NullabilityState::Unknown,
    ),
    semantic_mutator_effect(
        "htsjdk.samtools.SAMRecord",
        "setReferenceIndex",
        NullabilityState::Unknown,
    ),
    semantic_mutator_effect("SAMRecord", "setReferenceIndex", NullabilityState::Unknown),
    semantic_mutator_effect(
        "htsjdk.samtools.SAMRecord",
        "setMateReferenceIndex",
        NullabilityState::Unknown,
    ),
    semantic_mutator_effect(
        "SAMRecord",
        "setMateReferenceIndex",
        NullabilityState::Unknown,
    ),
    semantic_mutator_effect(
        "htsjdk.samtools.SAMRecord",
        "setMateReferenceName",
        NullabilityState::Unknown,
    ),
    semantic_mutator_effect(
        "SAMRecord",
        "setMateReferenceName",
        NullabilityState::Unknown,
    ),
    semantic_mutator_effect(
        "htsjdk.samtools.SAMRecord",
        "setReadUnmappedFlag",
        NullabilityState::Unknown,
    ),
    semantic_mutator_effect(
        "SAMRecord",
        "setReadUnmappedFlag",
        NullabilityState::Unknown,
    ),
    semantic_mutator_effect(
        "htsjdk.samtools.SAMRecord",
        "setAlignmentStart",
        NullabilityState::Unknown,
    ),
    semantic_mutator_effect("SAMRecord", "setAlignmentStart", NullabilityState::Unknown),
    semantic_mutator_effect(
        "htsjdk.samtools.SAMRecord",
        "setFlags",
        NullabilityState::Unknown,
    ),
    semantic_mutator_effect("SAMRecord", "setFlags", NullabilityState::Unknown),
    semantic_mutator_effect(
        "htsjdk.samtools.SAMRecord",
        "setHeaderStrict",
        NullabilityState::Unknown,
    ),
    semantic_mutator_effect("SAMRecord", "setHeaderStrict", NullabilityState::Unknown),
    semantic_mutator_effect(
        "htsjdk.samtools.SAMRecord",
        "clearAttributes",
        NullabilityState::Unknown,
    ),
    semantic_mutator_effect("SAMRecord", "clearAttributes", NullabilityState::Unknown),
    semantic_mutator_effect(
        "htsjdk.samtools.SAMRecord",
        "removeTransientAttribute",
        NullabilityState::Unknown,
    ),
    semantic_mutator_effect(
        "SAMRecord",
        "removeTransientAttribute",
        NullabilityState::Unknown,
    ),
    semantic_mutator_effect(
        "htsjdk.samtools.SAMRecord",
        "setReadNegativeStrandFlag",
        NullabilityState::Unknown,
    ),
    semantic_mutator_effect(
        "SAMRecord",
        "setReadNegativeStrandFlag",
        NullabilityState::Unknown,
    ),
    semantic_mutator_effect(
        "htsjdk.samtools.SAMRecord",
        "setReadPairedFlag",
        NullabilityState::Unknown,
    ),
    semantic_mutator_effect("SAMRecord", "setReadPairedFlag", NullabilityState::Unknown),
    semantic_mutator_effect(
        "htsjdk.samtools.SAMRecord",
        "setSupplementaryAlignmentFlag",
        NullabilityState::Unknown,
    ),
    semantic_mutator_effect(
        "SAMRecord",
        "setSupplementaryAlignmentFlag",
        NullabilityState::Unknown,
    ),
    semantic_mutator_effect(
        "htsjdk.samtools.SAMRecord",
        "setReadFailsVendorQualityCheckFlag",
        NullabilityState::Unknown,
    ),
    semantic_mutator_effect(
        "SAMRecord",
        "setReadFailsVendorQualityCheckFlag",
        NullabilityState::Unknown,
    ),
    semantic_mutator_effect(
        "htsjdk.samtools.SAMRecord",
        "setDuplicateReadFlag",
        NullabilityState::Unknown,
    ),
    semantic_mutator_effect(
        "SAMRecord",
        "setDuplicateReadFlag",
        NullabilityState::Unknown,
    ),
    semantic_mutator_effect(
        "htsjdk.samtools.SAMRecord",
        "setTransientAttribute",
        NullabilityState::Unknown,
    ),
    semantic_mutator_effect(
        "SAMRecord",
        "setTransientAttribute",
        NullabilityState::Unknown,
    ),
    semantic_mutator_effect(
        "htsjdk.samtools.SAMRecord",
        "setInferredInsertSize",
        NullabilityState::Unknown,
    ),
    semantic_mutator_effect(
        "SAMRecord",
        "setInferredInsertSize",
        NullabilityState::Unknown,
    ),
    semantic_mutator_effect(
        "htsjdk.samtools.SAMRecord",
        "reverseComplement",
        NullabilityState::Unknown,
    ),
    semantic_mutator_effect("SAMRecord", "reverseComplement", NullabilityState::Unknown),
];

const SAM_READER_API_EFFECTS: &[ExternalPackApiEffect] = &[
    semantic_typed_effect(
        "htsjdk.samtools.SamReaderFactory",
        "makeDefault",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "SamReaderFactory",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "SamReaderFactory",
        "makeDefault",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "SamReaderFactory",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.SamReaderFactory",
        "open",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "SamReader",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "SamReaderFactory",
        "open",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "SamReader",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.SamReaderFactory",
        "referenceSequence",
        ExternalApiRole::FluentMutator,
        ReceiverMutabilityFact::MutatesReceiver,
        "SamReaderFactory",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "SamReaderFactory",
        "referenceSequence",
        ExternalApiRole::FluentMutator,
        ReceiverMutabilityFact::MutatesReceiver,
        "SamReaderFactory",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.SamFileHeaderMerger",
        "getMergedHeader",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "SAMFileHeader",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "SamFileHeaderMerger",
        "getMergedHeader",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "SAMFileHeader",
        NullabilityState::DefinitelyNonNull,
    ),
];

const SAM_WRITER_API_EFFECTS: &[ExternalPackApiEffect] = &[
    semantic_typed_effect(
        "htsjdk.samtools.SAMFileWriterFactory",
        "makeWriter",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "SAMFileWriter",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "SAMFileWriterFactory",
        "makeWriter",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "SAMFileWriter",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.SAMFileWriterFactory",
        "setCreateIndex",
        ExternalApiRole::FluentMutator,
        ReceiverMutabilityFact::MutatesReceiver,
        "SAMFileWriterFactory",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "SAMFileWriterFactory",
        "setCreateIndex",
        ExternalApiRole::FluentMutator,
        ReceiverMutabilityFact::MutatesReceiver,
        "SAMFileWriterFactory",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.SAMFileWriterFactory",
        "setCreateMd5File",
        ExternalApiRole::FluentMutator,
        ReceiverMutabilityFact::MutatesReceiver,
        "SAMFileWriterFactory",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "SAMFileWriterFactory",
        "setCreateMd5File",
        ExternalApiRole::FluentMutator,
        ReceiverMutabilityFact::MutatesReceiver,
        "SAMFileWriterFactory",
        NullabilityState::DefinitelyNonNull,
    ),
];

const SAM_REFERENCE_API_EFFECTS: &[ExternalPackApiEffect] = &[
    semantic_typed_effect(
        "htsjdk.samtools.reference.ReferenceSequence",
        "getName",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "String",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "ReferenceSequence",
        "getName",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "String",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.reference.ReferenceSequence",
        "getContigIndex",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "ReferenceSequence",
        "getContigIndex",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.reference.ReferenceSequence",
        "getBases",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "byte[]",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "ReferenceSequence",
        "getBases",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "byte[]",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.reference.FastaSequenceFile",
        "nextSequence",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::MutatesReceiver,
        "ReferenceSequence",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "FastaSequenceFile",
        "nextSequence",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::MutatesReceiver,
        "ReferenceSequence",
        NullabilityState::MaybeNull,
    ),
    api_effect(
        "htsjdk.samtools.reference.FastaSequenceFile",
        "close",
        ExternalApiRole::SideEffect,
        ReceiverMutabilityFact::MutatesReceiver,
        None,
        NullabilityState::Unknown,
        &[],
    ),
    api_effect(
        "FastaSequenceFile",
        "close",
        ExternalApiRole::SideEffect,
        ReceiverMutabilityFact::MutatesReceiver,
        None,
        NullabilityState::Unknown,
        &[],
    ),
    semantic_typed_effect(
        "htsjdk.samtools.reference.ReferenceSequenceFileWalker",
        "getSequenceDictionary",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "SAMSequenceDictionary",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "ReferenceSequenceFileWalker",
        "getSequenceDictionary",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "SAMSequenceDictionary",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect_with_arity(
        "htsjdk.samtools.reference.ReferenceSequenceFileWalker",
        "get",
        1,
        ExternalApiRole::IndexedGetter,
        ReceiverMutabilityFact::ReadOnly,
        "ReferenceSequence",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect_with_arity(
        "ReferenceSequenceFileWalker",
        "get",
        1,
        ExternalApiRole::IndexedGetter,
        ReceiverMutabilityFact::ReadOnly,
        "ReferenceSequence",
        NullabilityState::DefinitelyNonNull,
    ),
];

const SAMTOOLS_UTIL_API_EFFECTS: &[ExternalPackApiEffect] = &[
    semantic_typed_effect(
        "htsjdk.samtools.util.Locatable",
        "getContig",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "String",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "Locatable",
        "getContig",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "String",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.util.Interval",
        "getContig",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "String",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "Interval",
        "getContig",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "String",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.util.IntervalList",
        "getContig",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "String",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "IntervalList",
        "getContig",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "String",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.util.Locatable",
        "getStart",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "Locatable",
        "getStart",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.util.Interval",
        "getStart",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "Interval",
        "getStart",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.util.IntervalList",
        "getStart",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "IntervalList",
        "getStart",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.util.Locatable",
        "getEnd",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "Locatable",
        "getEnd",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_boolean_getter_effect(
        "htsjdk.samtools.util.Locatable",
        "contains",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_boolean_getter_effect("Locatable", "contains", NullabilityState::DefinitelyNonNull),
    semantic_boolean_getter_effect(
        "htsjdk.samtools.util.Locatable",
        "overlaps",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_boolean_getter_effect("Locatable", "overlaps", NullabilityState::DefinitelyNonNull),
    semantic_boolean_getter_effect(
        "htsjdk.samtools.util.Locatable",
        "contigsMatch",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_boolean_getter_effect(
        "Locatable",
        "contigsMatch",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.util.Locatable",
        "getLengthOnReference",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "Locatable",
        "getLengthOnReference",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.util.Interval",
        "getEnd",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "Interval",
        "getEnd",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.util.IntervalList",
        "getEnd",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "IntervalList",
        "getEnd",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "htsjdk.samtools.util.Interval",
        "getName",
        NullabilityState::MaybeNull,
    ),
    semantic_getter_effect("Interval", "getName", NullabilityState::MaybeNull),
    semantic_getter_effect(
        "htsjdk.samtools.util.Interval",
        "getInterval",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "Interval",
        "getInterval",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "htsjdk.samtools.util.Interval",
        "toString",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect("Interval", "toString", NullabilityState::DefinitelyNonNull),
    semantic_boolean_getter_effect(
        "htsjdk.samtools.util.Interval",
        "isNegativeStrand",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_boolean_getter_effect(
        "Interval",
        "isNegativeStrand",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_boolean_getter_effect(
        "htsjdk.samtools.util.IntervalList",
        "isNegativeStrand",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_boolean_getter_effect(
        "IntervalList",
        "isNegativeStrand",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_boolean_getter_effect(
        "htsjdk.samtools.util.Interval",
        "intersects",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_boolean_getter_effect(
        "Interval",
        "intersects",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_boolean_getter_effect(
        "htsjdk.samtools.util.IntervalList",
        "intersects",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_boolean_getter_effect(
        "IntervalList",
        "intersects",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_boolean_getter_effect(
        "htsjdk.samtools.util.Interval",
        "contains",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_boolean_getter_effect("Interval", "contains", NullabilityState::DefinitelyNonNull),
    semantic_boolean_getter_effect(
        "htsjdk.samtools.util.IntervalList",
        "contains",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_boolean_getter_effect(
        "IntervalList",
        "contains",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_boolean_getter_effect(
        "htsjdk.samtools.util.Interval",
        "overlaps",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_boolean_getter_effect("Interval", "overlaps", NullabilityState::DefinitelyNonNull),
    semantic_boolean_getter_effect(
        "htsjdk.samtools.util.IntervalList",
        "overlaps",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_boolean_getter_effect(
        "IntervalList",
        "overlaps",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "htsjdk.samtools.util.Interval",
        "size",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect("Interval", "size", NullabilityState::DefinitelyNonNull),
    semantic_getter_effect(
        "htsjdk.samtools.util.IntervalList",
        "size",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect("IntervalList", "size", NullabilityState::DefinitelyNonNull),
    semantic_mutator_effect(
        "htsjdk.samtools.util.IntervalList",
        "add",
        NullabilityState::Unknown,
    ),
    semantic_mutator_effect("IntervalList", "add", NullabilityState::Unknown),
    semantic_mutator_effect(
        "htsjdk.samtools.util.IntervalList",
        "write",
        NullabilityState::Unknown,
    ),
    semantic_mutator_effect("IntervalList", "write", NullabilityState::Unknown),
    semantic_typed_effect(
        "htsjdk.samtools.util.IntervalList",
        "fromFile",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "IntervalList",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "IntervalList",
        "fromFile",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "IntervalList",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "htsjdk.samtools.util.IntervalTree",
        "size",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect("IntervalTree", "size", NullabilityState::DefinitelyNonNull),
    semantic_typed_effect(
        "htsjdk.samtools.util.IntervalTree",
        "put",
        ExternalApiRole::Mutator,
        ReceiverMutabilityFact::MutatesReceiver,
        "Object",
        NullabilityState::Unknown,
    ),
    semantic_typed_effect(
        "IntervalTree",
        "put",
        ExternalApiRole::Mutator,
        ReceiverMutabilityFact::MutatesReceiver,
        "Object",
        NullabilityState::Unknown,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.util.IntervalTree",
        "minOverlapper",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Object",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "IntervalTree",
        "minOverlapper",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Object",
        NullabilityState::MaybeNull,
    ),
    semantic_getter_effect(
        "htsjdk.samtools.util.Histogram",
        "size",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect("Histogram", "size", NullabilityState::DefinitelyNonNull),
    semantic_typed_effect(
        "htsjdk.samtools.util.Histogram",
        "keySet",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Set<Object>",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "Histogram",
        "keySet",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Set<Object>",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.util.Histogram",
        "values",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Collection<Object>",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "Histogram",
        "values",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Collection<Object>",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.util.Histogram",
        "getCount",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "double",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "Histogram",
        "getCount",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "double",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.util.Histogram",
        "getSumOfValues",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "double",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "Histogram",
        "getSumOfValues",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "double",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.util.Histogram",
        "getMin",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "double",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "Histogram",
        "getMin",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "double",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.util.Histogram",
        "getMax",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "double",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "Histogram",
        "getMax",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "double",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.util.Histogram",
        "getMean",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "double",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "Histogram",
        "getMean",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "double",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.util.Histogram",
        "getStandardDeviation",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "double",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "Histogram",
        "getStandardDeviation",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "double",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.util.Histogram",
        "getMedian",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "double",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.util.Histogram",
        "getMedianAbsoluteDeviation",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "double",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "Histogram",
        "getMedianAbsoluteDeviation",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "double",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "Histogram",
        "getMedian",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "double",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_mutator_effect(
        "htsjdk.samtools.util.Histogram",
        "increment",
        NullabilityState::Unknown,
    ),
    semantic_mutator_effect("Histogram", "increment", NullabilityState::Unknown),
    semantic_mutator_effect(
        "htsjdk.samtools.util.Histogram",
        "prefillBins",
        NullabilityState::Unknown,
    ),
    semantic_mutator_effect("Histogram", "prefillBins", NullabilityState::Unknown),
    semantic_mutator_effect(
        "htsjdk.samtools.util.Histogram",
        "trimByWidth",
        NullabilityState::Unknown,
    ),
    semantic_mutator_effect("Histogram", "trimByWidth", NullabilityState::Unknown),
    semantic_getter_effect(
        "htsjdk.samtools.util.IntervalList",
        "isEmpty",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "IntervalList",
        "isEmpty",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "htsjdk.samtools.util.IntervalTree",
        "isEmpty",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "IntervalTree",
        "isEmpty",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "htsjdk.samtools.util.Histogram",
        "isEmpty",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect("Histogram", "isEmpty", NullabilityState::DefinitelyNonNull),
    semantic_getter_effect(
        "htsjdk.samtools.util.IntervalList",
        "iterator",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "IntervalList",
        "iterator",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "htsjdk.samtools.util.IntervalTree",
        "iterator",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "IntervalTree",
        "iterator",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "htsjdk.samtools.util.Histogram",
        "iterator",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect("Histogram", "iterator", NullabilityState::DefinitelyNonNull),
    semantic_typed_effect(
        "htsjdk.samtools.util.Histogram",
        "getValueLabel",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "String",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "Histogram",
        "getValueLabel",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "String",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.util.Histogram",
        "get",
        ExternalApiRole::IndexedGetter,
        ReceiverMutabilityFact::ReadOnly,
        "Object",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "Histogram",
        "get",
        ExternalApiRole::IndexedGetter,
        ReceiverMutabilityFact::ReadOnly,
        "Object",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.SAMUtils",
        "phredToFastq",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "String",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "SAMUtils",
        "phredToFastq",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "String",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.SAMUtils",
        "cigarMapsNoBasesToRef",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "SAMUtils",
        "cigarMapsNoBasesToRef",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.SAMUtils",
        "combineMapqs",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "SAMUtils",
        "combineMapqs",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.SAMUtils",
        "compareMapqs",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "SAMUtils",
        "compareMapqs",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.SAMUtils",
        "getMateAlignmentEnd",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "SAMUtils",
        "getMateAlignmentEnd",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.SAMUtils",
        "hasMateCigar",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "SAMUtils",
        "hasMateCigar",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.SAMUtils",
        "recordMapsEntirelyBeyondEndOfReference",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "SAMUtils",
        "recordMapsEntirelyBeyondEndOfReference",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.SAMUtils",
        "getMateCigar",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Cigar",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "SAMUtils",
        "getMateCigar",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Cigar",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.SamPairUtil",
        "setMateInfo",
        ExternalApiRole::SideEffect,
        ReceiverMutabilityFact::ReadOnly,
        "void",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "SamPairUtil",
        "setMateInfo",
        ExternalApiRole::SideEffect,
        ReceiverMutabilityFact::ReadOnly,
        "void",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.SamPairUtil",
        "setMateInformationOnSupplementalAlignment",
        ExternalApiRole::SideEffect,
        ReceiverMutabilityFact::ReadOnly,
        "void",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "SamPairUtil",
        "setMateInformationOnSupplementalAlignment",
        ExternalApiRole::SideEffect,
        ReceiverMutabilityFact::ReadOnly,
        "void",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.SamPairUtil",
        "setProperPairFlags",
        ExternalApiRole::SideEffect,
        ReceiverMutabilityFact::ReadOnly,
        "void",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "SamPairUtil",
        "setProperPairFlags",
        ExternalApiRole::SideEffect,
        ReceiverMutabilityFact::ReadOnly,
        "void",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.SAMUtils",
        "chainSAMProgramRecord",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "SAMProgramRecord",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "SAMUtils",
        "chainSAMProgramRecord",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "SAMProgramRecord",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.SAMUtils",
        "makeReadUnmapped",
        ExternalApiRole::SideEffect,
        ReceiverMutabilityFact::ReadOnly,
        "void",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "SAMUtils",
        "makeReadUnmapped",
        ExternalApiRole::SideEffect,
        ReceiverMutabilityFact::ReadOnly,
        "void",
        NullabilityState::DefinitelyNonNull,
    ),
    api_effect(
        "htsjdk.samtools.SAMUtils",
        "fastqToPhred",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        None,
        NullabilityState::DefinitelyNonNull,
        &[],
    ),
    api_effect(
        "SAMUtils",
        "fastqToPhred",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        None,
        NullabilityState::DefinitelyNonNull,
        &[],
    ),
    semantic_typed_effect(
        "htsjdk.samtools.util.SequenceUtil",
        "isNoCall",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "SequenceUtil",
        "isNoCall",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.util.SequenceUtil",
        "reverseComplement",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Object",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "SequenceUtil",
        "reverseComplement",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Object",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.util.SequenceUtil",
        "calculateSamNmTag",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "SequenceUtil",
        "calculateSamNmTag",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.samtools.util.SequenceUtil",
        "sumQualitiesOfMismatches",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "SequenceUtil",
        "sumQualitiesOfMismatches",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_boolean_getter_effect(
        "htsjdk.samtools.util.PeekableIterator",
        "hasNext",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_boolean_getter_effect(
        "PeekableIterator",
        "hasNext",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_boolean_getter_effect(
        "htsjdk.samtools.util.DelegatingIterator",
        "hasNext",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_boolean_getter_effect(
        "DelegatingIterator",
        "hasNext",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_boolean_getter_effect(
        "htsjdk.samtools.util.MergingIterator",
        "hasNext",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_boolean_getter_effect(
        "MergingIterator",
        "hasNext",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_boolean_getter_effect(
        "htsjdk.samtools.util.IntervalMergerIterator",
        "hasNext",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_boolean_getter_effect(
        "IntervalMergerIterator",
        "hasNext",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_boolean_getter_effect(
        "htsjdk.samtools.util.CloseableIterator",
        "hasNext",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_boolean_getter_effect(
        "CloseableIterator",
        "hasNext",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "htsjdk.samtools.util.PeekableIterator",
        "next",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "PeekableIterator",
        "next",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "htsjdk.samtools.util.DelegatingIterator",
        "next",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "DelegatingIterator",
        "next",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "htsjdk.samtools.util.MergingIterator",
        "next",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "MergingIterator",
        "next",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "htsjdk.samtools.util.IntervalMergerIterator",
        "next",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "IntervalMergerIterator",
        "next",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "htsjdk.samtools.util.CloseableIterator",
        "next",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "CloseableIterator",
        "next",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_side_effect(
        "htsjdk.samtools.util.PeekableIterator",
        "close",
        NullabilityState::Unknown,
    ),
    semantic_side_effect("PeekableIterator", "close", NullabilityState::Unknown),
    semantic_side_effect(
        "htsjdk.samtools.util.DelegatingIterator",
        "close",
        NullabilityState::Unknown,
    ),
    semantic_side_effect("DelegatingIterator", "close", NullabilityState::Unknown),
    semantic_side_effect(
        "htsjdk.samtools.util.CloseableIterator",
        "close",
        NullabilityState::Unknown,
    ),
    semantic_side_effect("CloseableIterator", "close", NullabilityState::Unknown),
    semantic_side_effect(
        "htsjdk.samtools.util.CloseableIterator",
        "remove",
        NullabilityState::Unknown,
    ),
    semantic_side_effect("CloseableIterator", "remove", NullabilityState::Unknown),
    api_effect(
        "htsjdk.samtools.util.BlockCompressedOutputStream",
        "flush",
        ExternalApiRole::SideEffect,
        ReceiverMutabilityFact::MutatesReceiver,
        None,
        NullabilityState::Unknown,
        &[],
    ),
    api_effect(
        "BlockCompressedOutputStream",
        "flush",
        ExternalApiRole::SideEffect,
        ReceiverMutabilityFact::MutatesReceiver,
        None,
        NullabilityState::Unknown,
        &[],
    ),
    api_effect(
        "htsjdk.samtools.util.BlockCompressedOutputStream",
        "close",
        ExternalApiRole::SideEffect,
        ReceiverMutabilityFact::MutatesReceiver,
        None,
        NullabilityState::Unknown,
        &[],
    ),
    api_effect(
        "BlockCompressedOutputStream",
        "close",
        ExternalApiRole::SideEffect,
        ReceiverMutabilityFact::MutatesReceiver,
        None,
        NullabilityState::Unknown,
        &[],
    ),
    semantic_getter_effect(
        "htsjdk.samtools.util.PeekableIterator",
        "peek",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_getter_effect(
        "PeekableIterator",
        "peek",
        NullabilityState::DefinitelyNonNull,
    ),
    api_effect_with_arity(
        "htsjdk.samtools.util.BinaryCodec",
        "writeBytes",
        1,
        ExternalApiRole::Mutator,
        ReceiverMutabilityFact::MutatesReceiver,
        None,
        NullabilityState::Unknown,
        &[],
    ),
    api_effect_with_arity(
        "BinaryCodec",
        "writeBytes",
        1,
        ExternalApiRole::Mutator,
        ReceiverMutabilityFact::MutatesReceiver,
        None,
        NullabilityState::Unknown,
        &[],
    ),
    api_effect_with_arity(
        "htsjdk.samtools.util.BinaryCodec",
        "writeInt",
        1,
        ExternalApiRole::Mutator,
        ReceiverMutabilityFact::MutatesReceiver,
        None,
        NullabilityState::Unknown,
        &[],
    ),
    api_effect_with_arity(
        "BinaryCodec",
        "writeInt",
        1,
        ExternalApiRole::Mutator,
        ReceiverMutabilityFact::MutatesReceiver,
        None,
        NullabilityState::Unknown,
        &[],
    ),
    api_effect_with_arity(
        "htsjdk.samtools.util.BinaryCodec",
        "writeString",
        3,
        ExternalApiRole::Mutator,
        ReceiverMutabilityFact::MutatesReceiver,
        None,
        NullabilityState::Unknown,
        &[],
    ),
    api_effect_with_arity(
        "BinaryCodec",
        "writeString",
        3,
        ExternalApiRole::Mutator,
        ReceiverMutabilityFact::MutatesReceiver,
        None,
        NullabilityState::Unknown,
        &[],
    ),
];

const VARIANTCONTEXT_API_EFFECTS: &[ExternalPackApiEffect] = &[
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.Allele",
        "create",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Allele",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "Allele",
        "create",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Allele",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.Allele",
        "getBaseString",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "String",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "Allele",
        "getBaseString",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "String",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.Allele",
        "getBases",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "byte[]",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "Allele",
        "getBases",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "byte[]",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.Allele",
        "isReference",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "Allele",
        "isReference",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.Allele",
        "isNoCall",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "Allele",
        "isNoCall",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.Allele",
        "isNonReference",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "Allele",
        "isNonReference",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.Allele",
        "isSymbolic",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "Allele",
        "isSymbolic",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.Allele",
        "isCalled",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "Allele",
        "isCalled",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.Allele",
        "isBreakpoint",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "Allele",
        "isBreakpoint",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.Allele",
        "isSingleBreakend",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "Allele",
        "isSingleBreakend",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.Allele",
        "isNonRefAllele",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect_with_arity(
        "htsjdk.variant.variantcontext.Allele",
        "equals",
        2,
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect_with_arity(
        "Allele",
        "equals",
        2,
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.Allele",
        "oneIsPrefixOfOther",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "Allele",
        "oneIsPrefixOfOther",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "Allele",
        "isNonRefAllele",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.Allele",
        "basesMatch",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "Allele",
        "basesMatch",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.Allele",
        "extend",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Allele",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "Allele",
        "extend",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Allele",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.Allele",
        "length",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "Allele",
        "length",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.Allele",
        "create",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Allele",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "Allele",
        "create",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Allele",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.Allele",
        "getDisplayString",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "String",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "Allele",
        "getDisplayString",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "String",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.VariantContext",
        "getContig",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "String",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "VariantContext",
        "getContig",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "String",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.VariantContext",
        "getStart",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "VariantContext",
        "getStart",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.VariantContext",
        "getEnd",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "VariantContext",
        "getEnd",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.VariantContext",
        "getID",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "String",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "VariantContext",
        "getID",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "String",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.VariantContext",
        "getAlleles",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "List<Allele>",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "VariantContext",
        "getAlleles",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "List<Allele>",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.VariantContext",
        "getAlternateAlleles",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "List<Allele>",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "VariantContext",
        "getAlternateAlleles",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "List<Allele>",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.VariantContext",
        "getReference",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Allele",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "VariantContext",
        "getReference",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Allele",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.VariantContext",
        "getGenotypes",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "GenotypesContext",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "VariantContext",
        "getGenotypes",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "GenotypesContext",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.VariantContext",
        "hasGenotypes",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "VariantContext",
        "hasGenotypes",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    api_effect(
        "htsjdk.variant.variantcontext.VariantContext",
        "extraStrictValidation",
        ExternalApiRole::SideEffect,
        ReceiverMutabilityFact::ReadOnly,
        None,
        NullabilityState::Unknown,
        VARIANT_VALIDATION_THROWN,
    ),
    api_effect(
        "VariantContext",
        "extraStrictValidation",
        ExternalApiRole::SideEffect,
        ReceiverMutabilityFact::ReadOnly,
        None,
        NullabilityState::Unknown,
        VARIANT_VALIDATION_THROWN,
    ),
    api_effect(
        "htsjdk.variant.variantcontext.VariantContext",
        "validateReferenceBases",
        ExternalApiRole::SideEffect,
        ReceiverMutabilityFact::ReadOnly,
        None,
        NullabilityState::Unknown,
        VARIANT_VALIDATION_THROWN,
    ),
    api_effect(
        "VariantContext",
        "validateReferenceBases",
        ExternalApiRole::SideEffect,
        ReceiverMutabilityFact::ReadOnly,
        None,
        NullabilityState::Unknown,
        VARIANT_VALIDATION_THROWN,
    ),
    api_effect(
        "htsjdk.variant.variantcontext.VariantContext",
        "validateRSIDs",
        ExternalApiRole::SideEffect,
        ReceiverMutabilityFact::ReadOnly,
        None,
        NullabilityState::Unknown,
        VARIANT_VALIDATION_THROWN,
    ),
    api_effect(
        "VariantContext",
        "validateRSIDs",
        ExternalApiRole::SideEffect,
        ReceiverMutabilityFact::ReadOnly,
        None,
        NullabilityState::Unknown,
        VARIANT_VALIDATION_THROWN,
    ),
    api_effect(
        "htsjdk.variant.variantcontext.VariantContext",
        "validateAlternateAlleles",
        ExternalApiRole::SideEffect,
        ReceiverMutabilityFact::ReadOnly,
        None,
        NullabilityState::Unknown,
        VARIANT_VALIDATION_THROWN,
    ),
    api_effect(
        "VariantContext",
        "validateAlternateAlleles",
        ExternalApiRole::SideEffect,
        ReceiverMutabilityFact::ReadOnly,
        None,
        NullabilityState::Unknown,
        VARIANT_VALIDATION_THROWN,
    ),
    api_effect(
        "htsjdk.variant.variantcontext.VariantContext",
        "validateChromosomeCounts",
        ExternalApiRole::SideEffect,
        ReceiverMutabilityFact::ReadOnly,
        None,
        NullabilityState::Unknown,
        VARIANT_VALIDATION_THROWN,
    ),
    api_effect(
        "VariantContext",
        "validateChromosomeCounts",
        ExternalApiRole::SideEffect,
        ReceiverMutabilityFact::ReadOnly,
        None,
        NullabilityState::Unknown,
        VARIANT_VALIDATION_THROWN,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.VariantContext",
        "getAttributes",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Map<String,Object>",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "VariantContext",
        "getAttributes",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Map<String,Object>",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.VariantContext",
        "getType",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Type",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "VariantContext",
        "getType",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Type",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.VariantContext.Type",
        "name",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "String",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "Type",
        "name",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "String",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.VariantContext.Type",
        "values",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Type[]",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "Type",
        "values",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Type[]",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.VariantContext",
        "hasID",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "VariantContext",
        "hasID",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.VariantContext",
        "hasAttribute",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "VariantContext",
        "hasAttribute",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.VariantContext",
        "isBiallelic",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "VariantContext",
        "isBiallelic",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.VariantContext",
        "isSNP",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "VariantContext",
        "isSNP",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.VariantContext",
        "isIndel",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "VariantContext",
        "isIndel",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.VariantContext",
        "isNotFiltered",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "VariantContext",
        "isNotFiltered",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.VariantContext",
        "isMixed",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "VariantContext",
        "isMixed",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.VariantContext",
        "isMNP",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "VariantContext",
        "isMNP",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.VariantContext",
        "isSimpleInsertion",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "VariantContext",
        "isSimpleInsertion",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.VariantContext",
        "isSimpleDeletion",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "VariantContext",
        "isSimpleDeletion",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.VariantContext",
        "isComplexIndel",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "VariantContext",
        "isComplexIndel",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.VariantContext",
        "isStructuralIndel",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "VariantContext",
        "isStructuralIndel",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.VariantContext",
        "isMonomorphicInSamples",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "VariantContext",
        "isMonomorphicInSamples",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.VariantContext",
        "isPolymorphicInSamples",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "VariantContext",
        "isPolymorphicInSamples",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.VariantContext",
        "hasSymbolicAlleles",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "VariantContext",
        "hasSymbolicAlleles",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.VariantContext",
        "overlaps",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "VariantContext",
        "overlaps",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.VariantContext",
        "getAttribute",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Object",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "VariantContext",
        "getAttribute",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Object",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.VariantContext",
        "getAttributeAsString",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "String",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "VariantContext",
        "getAttributeAsString",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "String",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.VariantContext",
        "getGenotype",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Genotype",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "VariantContext",
        "getGenotype",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Genotype",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.VariantContext",
        "getPloidy",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "VariantContext",
        "getPloidy",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.VariantContext",
        "getNSamples",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "VariantContext",
        "getNSamples",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.VariantContext",
        "getCalledChrCount",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "VariantContext",
        "getCalledChrCount",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.VariantContext",
        "getHetCount",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "VariantContext",
        "getHetCount",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.VariantContext",
        "getHomVarCount",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "VariantContext",
        "getHomVarCount",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.VariantContext",
        "getNAlleles",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "VariantContext",
        "getNAlleles",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.VariantContext",
        "getAttributeAsInt",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "VariantContext",
        "getAttributeAsInt",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.VariantContext",
        "getAttributeAsDouble",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "double",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "VariantContext",
        "getAttributeAsDouble",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "double",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.VariantContext",
        "getAttributeAsBoolean",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "VariantContext",
        "getAttributeAsBoolean",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.VariantContext",
        "getAlternateAllele",
        ExternalApiRole::IndexedGetter,
        ReceiverMutabilityFact::ReadOnly,
        "Allele",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "VariantContext",
        "getAlternateAllele",
        ExternalApiRole::IndexedGetter,
        ReceiverMutabilityFact::ReadOnly,
        "Allele",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.VariantContext",
        "getAllele",
        ExternalApiRole::IndexedGetter,
        ReceiverMutabilityFact::ReadOnly,
        "Allele",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "VariantContext",
        "getAllele",
        ExternalApiRole::IndexedGetter,
        ReceiverMutabilityFact::ReadOnly,
        "Allele",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.VariantContext",
        "getAlleleIndex",
        ExternalApiRole::IndexedGetter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "VariantContext",
        "getAlleleIndex",
        ExternalApiRole::IndexedGetter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.VariantContext",
        "hasGenotype",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "VariantContext",
        "hasGenotype",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.VariantContext",
        "hasAllele",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "VariantContext",
        "hasAllele",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.VariantContext",
        "hasAlternateAllele",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "VariantContext",
        "hasAlternateAllele",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.VariantContext",
        "isSymbolic",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "VariantContext",
        "isSymbolic",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.VariantContext",
        "isVariant",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "VariantContext",
        "isVariant",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.VariantContext",
        "hasExtendedAttribute",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "VariantContext",
        "hasExtendedAttribute",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.VariantContext",
        "getFilters",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Set<String>",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "VariantContext",
        "getFilters",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Set<String>",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.VariantContext",
        "getSampleNames",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Set<String>",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "VariantContext",
        "getSampleNames",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Set<String>",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.VariantContext",
        "getGenotypesOrderedByName",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Iterable<Genotype>",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "VariantContext",
        "getGenotypesOrderedByName",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Iterable<Genotype>",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.VariantContext",
        "getAnyAttribute",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Object",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "VariantContext",
        "getAnyAttribute",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Object",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.VariantContext",
        "getAttributeAsList",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "List<Object>",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "VariantContext",
        "getAttributeAsList",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "List<Object>",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.CommonInfo",
        "getAttributeAsList",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "List<Object>",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "CommonInfo",
        "getAttributeAsList",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "List<Object>",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.VariantContext",
        "getAttributeAsDoubleList",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "List<Double>",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "VariantContext",
        "getAttributeAsDoubleList",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "List<Double>",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.VariantContext",
        "getAttributeAsIntList",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "List<Integer>",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "VariantContext",
        "getAttributeAsIntList",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "List<Integer>",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.VariantContext",
        "getIndelLengths",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "List<Integer>",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "VariantContext",
        "getIndelLengths",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "List<Integer>",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.VariantContext",
        "getAttributeAsStringList",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "List<String>",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.CommonInfo",
        "getAttributeAsStringList",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "List<String>",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "CommonInfo",
        "getAttributeAsStringList",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "List<String>",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.VariantContext",
        "getCommonInfo",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "CommonInfo",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "VariantContext",
        "getCommonInfo",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "CommonInfo",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.VariantContext",
        "getMaxPloidy",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "VariantContext",
        "getMaxPloidy",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.VariantContext",
        "getNoCallCount",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "VariantContext",
        "getNoCallCount",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "VariantContext",
        "getAttributeAsStringList",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "List<String>",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.VariantContext",
        "getSource",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "String",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "VariantContext",
        "getSource",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "String",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.VariantContext",
        "getDisplayString",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "String",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "VariantContext",
        "getDisplayString",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "String",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.VariantContext",
        "isFiltered",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "VariantContext",
        "isFiltered",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.VariantContext",
        "filtersWereApplied",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "VariantContext",
        "filtersWereApplied",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.VariantContext",
        "contigsMatch",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "VariantContext",
        "contigsMatch",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.VariantContext",
        "getPhredScaledQual",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "double",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "VariantContext",
        "getPhredScaledQual",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "double",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.VariantContext",
        "getLog10PError",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "double",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "VariantContext",
        "getLog10PError",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "double",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.VariantContext",
        "hasLog10PError",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "VariantContext",
        "hasLog10PError",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.VariantContext",
        "getStructuralVariantType",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "StructuralVariantType",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "VariantContext",
        "getStructuralVariantType",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "StructuralVariantType",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.VariantContext",
        "toStringWithoutGenotypes",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "String",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "VariantContext",
        "toStringWithoutGenotypes",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "String",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.VariantContext",
        "toStringDecodeGenotypes",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "String",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "VariantContext",
        "toStringDecodeGenotypes",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "String",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.VariantContext",
        "getGLIndicesOfAlternateAllele",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int[]",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "VariantContext",
        "getGLIndicesOfAlternateAllele",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int[]",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.VariantContext",
        "getAlleleIndices",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "List<Integer>",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "VariantContext",
        "getAlleleIndices",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "List<Integer>",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.VariantContext",
        "contains",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "VariantContext",
        "contains",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.VariantContext",
        "withinDistanceOf",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "VariantContext",
        "withinDistanceOf",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.VariantContext",
        "getLengthOnReference",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "VariantContext",
        "getLengthOnReference",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.VariantContext",
        "isSymbolicOrSV",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "VariantContext",
        "isSymbolicOrSV",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.VariantContext",
        "subContextFromSamples",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "VariantContext",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "VariantContext",
        "subContextFromSamples",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "VariantContext",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_fluent_mutator_effect(
        "htsjdk.variant.variantcontext.VariantContextBuilder",
        "rmAttribute",
        "VariantContextBuilder",
    ),
    semantic_fluent_mutator_effect(
        "VariantContextBuilder",
        "rmAttribute",
        "VariantContextBuilder",
    ),
    semantic_fluent_mutator_effect(
        "htsjdk.variant.variantcontext.VariantContextBuilder",
        "chr",
        "VariantContextBuilder",
    ),
    semantic_fluent_mutator_effect("VariantContextBuilder", "chr", "VariantContextBuilder"),
    semantic_fluent_mutator_effect(
        "htsjdk.variant.variantcontext.VariantContextBuilder",
        "start",
        "VariantContextBuilder",
    ),
    semantic_fluent_mutator_effect("VariantContextBuilder", "start", "VariantContextBuilder"),
    semantic_fluent_mutator_effect(
        "htsjdk.variant.variantcontext.VariantContextBuilder",
        "stop",
        "VariantContextBuilder",
    ),
    semantic_fluent_mutator_effect("VariantContextBuilder", "stop", "VariantContextBuilder"),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.VariantContextBuilder",
        "getStop",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "VariantContextBuilder",
        "getStop",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_fluent_mutator_effect(
        "htsjdk.variant.variantcontext.VariantContextBuilder",
        "alleles",
        "VariantContextBuilder",
    ),
    semantic_fluent_mutator_effect("VariantContextBuilder", "alleles", "VariantContextBuilder"),
    semantic_fluent_mutator_effect(
        "htsjdk.variant.variantcontext.VariantContextBuilder",
        "genotypes",
        "VariantContextBuilder",
    ),
    semantic_fluent_mutator_effect(
        "VariantContextBuilder",
        "genotypes",
        "VariantContextBuilder",
    ),
    semantic_fluent_mutator_effect(
        "htsjdk.variant.variantcontext.VariantContextBuilder",
        "attribute",
        "VariantContextBuilder",
    ),
    semantic_fluent_mutator_effect(
        "VariantContextBuilder",
        "attribute",
        "VariantContextBuilder",
    ),
    semantic_fluent_mutator_effect(
        "htsjdk.variant.variantcontext.VariantContextBuilder",
        "source",
        "VariantContextBuilder",
    ),
    semantic_fluent_mutator_effect("VariantContextBuilder", "source", "VariantContextBuilder"),
    semantic_fluent_mutator_effect(
        "htsjdk.variant.variantcontext.VariantContextBuilder",
        "rmAttributes",
        "VariantContextBuilder",
    ),
    semantic_fluent_mutator_effect(
        "VariantContextBuilder",
        "rmAttributes",
        "VariantContextBuilder",
    ),
    semantic_fluent_mutator_effect(
        "htsjdk.variant.variantcontext.VariantContextBuilder",
        "unfiltered",
        "VariantContextBuilder",
    ),
    semantic_fluent_mutator_effect(
        "VariantContextBuilder",
        "unfiltered",
        "VariantContextBuilder",
    ),
    semantic_fluent_mutator_effect(
        "htsjdk.variant.variantcontext.VariantContextBuilder",
        "passFilters",
        "VariantContextBuilder",
    ),
    semantic_fluent_mutator_effect(
        "VariantContextBuilder",
        "passFilters",
        "VariantContextBuilder",
    ),
    semantic_fluent_mutator_effect(
        "htsjdk.variant.variantcontext.VariantContextBuilder",
        "noGenotypes",
        "VariantContextBuilder",
    ),
    semantic_fluent_mutator_effect(
        "VariantContextBuilder",
        "noGenotypes",
        "VariantContextBuilder",
    ),
    semantic_fluent_mutator_effect(
        "htsjdk.variant.variantcontext.VariantContextBuilder",
        "computeEndFromAlleles",
        "VariantContextBuilder",
    ),
    semantic_fluent_mutator_effect(
        "VariantContextBuilder",
        "computeEndFromAlleles",
        "VariantContextBuilder",
    ),
    semantic_fluent_mutator_effect(
        "htsjdk.variant.variantcontext.VariantContextBuilder",
        "loc",
        "VariantContextBuilder",
    ),
    semantic_fluent_mutator_effect("VariantContextBuilder", "loc", "VariantContextBuilder"),
    semantic_fluent_mutator_effect(
        "htsjdk.variant.variantcontext.VariantContextBuilder",
        "noID",
        "VariantContextBuilder",
    ),
    semantic_fluent_mutator_effect("VariantContextBuilder", "noID", "VariantContextBuilder"),
    semantic_fluent_mutator_effect(
        "htsjdk.variant.variantcontext.VariantContextBuilder",
        "attributes",
        "VariantContextBuilder",
    ),
    semantic_fluent_mutator_effect(
        "VariantContextBuilder",
        "attributes",
        "VariantContextBuilder",
    ),
    semantic_fluent_mutator_effect(
        "htsjdk.variant.variantcontext.VariantContextBuilder",
        "putAttributes",
        "VariantContextBuilder",
    ),
    semantic_fluent_mutator_effect(
        "VariantContextBuilder",
        "putAttributes",
        "VariantContextBuilder",
    ),
    semantic_fluent_mutator_effect(
        "htsjdk.variant.variantcontext.VariantContextBuilder",
        "id",
        "VariantContextBuilder",
    ),
    semantic_fluent_mutator_effect("VariantContextBuilder", "id", "VariantContextBuilder"),
    semantic_fluent_mutator_effect(
        "htsjdk.variant.variantcontext.VariantContextBuilder",
        "log10PError",
        "VariantContextBuilder",
    ),
    semantic_fluent_mutator_effect(
        "VariantContextBuilder",
        "log10PError",
        "VariantContextBuilder",
    ),
    semantic_fluent_mutator_effect(
        "htsjdk.variant.variantcontext.VariantContextBuilder",
        "filter",
        "VariantContextBuilder",
    ),
    semantic_fluent_mutator_effect("VariantContextBuilder", "filter", "VariantContextBuilder"),
    semantic_fluent_mutator_effect(
        "htsjdk.variant.variantcontext.VariantContextBuilder",
        "filters",
        "VariantContextBuilder",
    ),
    semantic_fluent_mutator_effect("VariantContextBuilder", "filters", "VariantContextBuilder"),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.VariantContextBuilder",
        "getFilters",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Set<String>",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "VariantContextBuilder",
        "getFilters",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Set<String>",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.VariantContextBuilder",
        "getAttributes",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Map<String,Object>",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "VariantContextBuilder",
        "getAttributes",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Map<String,Object>",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.VariantContextBuilder",
        "getGenotypes",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "GenotypesContext",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "VariantContextBuilder",
        "getGenotypes",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "GenotypesContext",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.VariantContextBuilder",
        "make",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "VariantContext",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "VariantContextBuilder",
        "make",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "VariantContext",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_fluent_mutator_effect(
        "htsjdk.variant.variantcontext.GenotypeBuilder",
        "name",
        "GenotypeBuilder",
    ),
    semantic_fluent_mutator_effect("GenotypeBuilder", "name", "GenotypeBuilder"),
    semantic_fluent_mutator_effect(
        "htsjdk.variant.variantcontext.GenotypeBuilder",
        "alleles",
        "GenotypeBuilder",
    ),
    semantic_fluent_mutator_effect("GenotypeBuilder", "alleles", "GenotypeBuilder"),
    semantic_fluent_mutator_effect(
        "htsjdk.variant.variantcontext.GenotypeBuilder",
        "attribute",
        "GenotypeBuilder",
    ),
    semantic_fluent_mutator_effect("GenotypeBuilder", "attribute", "GenotypeBuilder"),
    semantic_fluent_mutator_effect(
        "htsjdk.variant.variantcontext.GenotypeBuilder",
        "attributes",
        "GenotypeBuilder",
    ),
    semantic_fluent_mutator_effect("GenotypeBuilder", "attributes", "GenotypeBuilder"),
    semantic_fluent_mutator_effect(
        "htsjdk.variant.variantcontext.GenotypeBuilder",
        "DP",
        "GenotypeBuilder",
    ),
    semantic_fluent_mutator_effect("GenotypeBuilder", "DP", "GenotypeBuilder"),
    semantic_fluent_mutator_effect(
        "htsjdk.variant.variantcontext.GenotypeBuilder",
        "GQ",
        "GenotypeBuilder",
    ),
    semantic_fluent_mutator_effect("GenotypeBuilder", "GQ", "GenotypeBuilder"),
    semantic_fluent_mutator_effect(
        "htsjdk.variant.variantcontext.GenotypeBuilder",
        "noGQ",
        "GenotypeBuilder",
    ),
    semantic_fluent_mutator_effect("GenotypeBuilder", "noGQ", "GenotypeBuilder"),
    semantic_fluent_mutator_effect(
        "htsjdk.variant.variantcontext.GenotypeBuilder",
        "noDP",
        "GenotypeBuilder",
    ),
    semantic_fluent_mutator_effect("GenotypeBuilder", "noDP", "GenotypeBuilder"),
    semantic_fluent_mutator_effect(
        "htsjdk.variant.variantcontext.GenotypeBuilder",
        "noAttributes",
        "GenotypeBuilder",
    ),
    semantic_fluent_mutator_effect("GenotypeBuilder", "noAttributes", "GenotypeBuilder"),
    semantic_fluent_mutator_effect(
        "htsjdk.variant.variantcontext.GenotypeBuilder",
        "log10PError",
        "GenotypeBuilder",
    ),
    semantic_fluent_mutator_effect("GenotypeBuilder", "log10PError", "GenotypeBuilder"),
    semantic_fluent_mutator_effect(
        "htsjdk.variant.variantcontext.GenotypeBuilder",
        "phased",
        "GenotypeBuilder",
    ),
    semantic_fluent_mutator_effect("GenotypeBuilder", "phased", "GenotypeBuilder"),
    semantic_fluent_mutator_effect(
        "htsjdk.variant.variantcontext.GenotypeBuilder",
        "PL",
        "GenotypeBuilder",
    ),
    semantic_fluent_mutator_effect("GenotypeBuilder", "PL", "GenotypeBuilder"),
    semantic_fluent_mutator_effect(
        "htsjdk.variant.variantcontext.GenotypeBuilder",
        "filters",
        "GenotypeBuilder",
    ),
    semantic_fluent_mutator_effect("GenotypeBuilder", "filters", "GenotypeBuilder"),
    semantic_fluent_mutator_effect(
        "htsjdk.variant.variantcontext.GenotypeBuilder",
        "AD",
        "GenotypeBuilder",
    ),
    semantic_fluent_mutator_effect("GenotypeBuilder", "AD", "GenotypeBuilder"),
    semantic_fluent_mutator_effect(
        "htsjdk.variant.variantcontext.GenotypeBuilder",
        "noAD",
        "GenotypeBuilder",
    ),
    semantic_fluent_mutator_effect("GenotypeBuilder", "noAD", "GenotypeBuilder"),
    semantic_fluent_mutator_effect(
        "htsjdk.variant.variantcontext.GenotypeBuilder",
        "noPL",
        "GenotypeBuilder",
    ),
    semantic_fluent_mutator_effect("GenotypeBuilder", "noPL", "GenotypeBuilder"),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.GenotypeBuilder",
        "make",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Genotype",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "GenotypeBuilder",
        "make",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Genotype",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.GenotypeBuilder",
        "makeWithShallowCopy",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Genotype",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "GenotypeBuilder",
        "makeWithShallowCopy",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Genotype",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.Genotype",
        "getSampleName",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "String",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "Genotype",
        "getSampleName",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "String",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.Genotype",
        "getAlleles",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "List<Allele>",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "Genotype",
        "getAlleles",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "List<Allele>",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.Genotype",
        "getExtendedAttributes",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Map<String,Object>",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "Genotype",
        "getExtendedAttributes",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Map<String,Object>",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.Genotype",
        "getPloidy",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "Genotype",
        "getPloidy",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.Genotype",
        "getDP",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "Genotype",
        "getDP",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.Genotype",
        "getAllele",
        ExternalApiRole::IndexedGetter,
        ReceiverMutabilityFact::ReadOnly,
        "Allele",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "Genotype",
        "getAllele",
        ExternalApiRole::IndexedGetter,
        ReceiverMutabilityFact::ReadOnly,
        "Allele",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.Genotype",
        "getType",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "GenotypeType",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "Genotype",
        "getType",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "GenotypeType",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.Genotype",
        "getAnyAttribute",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Object",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "Genotype",
        "getAnyAttribute",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Object",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.Genotype",
        "hasAnyAttribute",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "Genotype",
        "hasAnyAttribute",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.Genotype",
        "getExtendedAttribute",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Object",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "Genotype",
        "getExtendedAttribute",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Object",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.Genotype",
        "getLikelihoods",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "GenotypeLikelihoods",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "Genotype",
        "getLikelihoods",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "GenotypeLikelihoods",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.GenotypeLikelihoods",
        "fromLog10Likelihoods",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "GenotypeLikelihoods",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "GenotypeLikelihoods",
        "fromLog10Likelihoods",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "GenotypeLikelihoods",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.GenotypeLikelihoods",
        "fromPLs",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "GenotypeLikelihoods",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "GenotypeLikelihoods",
        "fromPLs",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "GenotypeLikelihoods",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.GenotypeLikelihoods",
        "calculatePLindex",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "GenotypeLikelihoods",
        "calculatePLindex",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.GenotypeLikelihoods",
        "getAllelePair",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "GenotypeLikelihoodsAllelePair",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "GenotypeLikelihoods",
        "getAllelePair",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "GenotypeLikelihoodsAllelePair",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.GenotypeLikelihoods",
        "numLikelihoods",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "GenotypeLikelihoods",
        "numLikelihoods",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.GenotypeLikelihoods",
        "getAsPLs",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int[]",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "GenotypeLikelihoods",
        "getAsPLs",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int[]",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.GenotypeLikelihoods",
        "getAsVector",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "double[]",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "GenotypeLikelihoods",
        "getAsVector",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "double[]",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.GenotypeLikelihoods",
        "getGQLog10FromLikelihoods",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "double",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "GenotypeLikelihoods",
        "getGQLog10FromLikelihoods",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "double",
        NullabilityState::DefinitelyNonNull,
    ),
    api_effect(
        "htsjdk.variant.variantcontext.GenotypeLikelihoods",
        "forEach",
        ExternalApiRole::SideEffect,
        ReceiverMutabilityFact::ReadOnly,
        None,
        NullabilityState::Unknown,
        &[],
    ),
    api_effect(
        "GenotypeLikelihoods",
        "forEach",
        ExternalApiRole::SideEffect,
        ReceiverMutabilityFact::ReadOnly,
        None,
        NullabilityState::Unknown,
        &[],
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.Genotype",
        "getGenotypeString",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "String",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "Genotype",
        "getGenotypeString",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "String",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.Genotype",
        "hasAD",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "Genotype",
        "hasAD",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.Genotype",
        "hasDP",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "Genotype",
        "hasDP",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.Genotype",
        "hasLikelihoods",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "Genotype",
        "hasLikelihoods",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.Genotype",
        "hasPL",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "Genotype",
        "hasPL",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.Genotype",
        "hasGQ",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "Genotype",
        "hasGQ",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.Genotype",
        "hasExtendedAttribute",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "Genotype",
        "hasExtendedAttribute",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.Genotype",
        "isCalled",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "Genotype",
        "isCalled",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.Genotype",
        "isNoCall",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "Genotype",
        "isNoCall",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.Genotype",
        "isHomRef",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "Genotype",
        "isHomRef",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.Genotype",
        "isHomVar",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "Genotype",
        "isHomVar",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.Genotype",
        "isHet",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "Genotype",
        "isHet",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.Genotype",
        "isHetNonRef",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "Genotype",
        "isHetNonRef",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.Genotype",
        "isMixed",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "Genotype",
        "isMixed",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.Genotype",
        "isPhased",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "Genotype",
        "isPhased",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.Genotype",
        "isFiltered",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "Genotype",
        "isFiltered",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.Genotype",
        "isNonInformative",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "Genotype",
        "isNonInformative",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.Genotype",
        "sameGenotype",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "Genotype",
        "sameGenotype",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.Genotype",
        "getFilters",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Set<String>",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "Genotype",
        "getFilters",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Set<String>",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.Genotype",
        "countAllele",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "Genotype",
        "countAllele",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.GenotypesContext",
        "size",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "GenotypesContext",
        "size",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.GenotypesContext",
        "iterator",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Iterator<Genotype>",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "GenotypesContext",
        "iterator",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Iterator<Genotype>",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.GenotypesContext",
        "stream",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Stream<Genotype>",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "GenotypesContext",
        "stream",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Stream<Genotype>",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.GenotypesContext",
        "iterateInSampleNameOrder",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Iterable<Genotype>",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "GenotypesContext",
        "iterateInSampleNameOrder",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Iterable<Genotype>",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.GenotypesContext",
        "spliterator",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Spliterator<Genotype>",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "GenotypesContext",
        "spliterator",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Spliterator<Genotype>",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.GenotypesContext",
        "get",
        ExternalApiRole::IndexedGetter,
        ReceiverMutabilityFact::ReadOnly,
        "Genotype",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "GenotypesContext",
        "get",
        ExternalApiRole::IndexedGetter,
        ReceiverMutabilityFact::ReadOnly,
        "Genotype",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.GenotypesContext",
        "isEmpty",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "GenotypesContext",
        "isEmpty",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.GenotypesContext",
        "add",
        ExternalApiRole::Mutator,
        ReceiverMutabilityFact::MutatesReceiver,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "GenotypesContext",
        "add",
        ExternalApiRole::Mutator,
        ReceiverMutabilityFact::MutatesReceiver,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.GenotypesContext",
        "addAll",
        ExternalApiRole::Mutator,
        ReceiverMutabilityFact::MutatesReceiver,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "GenotypesContext",
        "addAll",
        ExternalApiRole::Mutator,
        ReceiverMutabilityFact::MutatesReceiver,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.GenotypesContext",
        "containsSample",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "GenotypesContext",
        "containsSample",
        ExternalApiRole::BooleanGetter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.GenotypesContext",
        "replace",
        ExternalApiRole::Mutator,
        ReceiverMutabilityFact::MutatesReceiver,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "GenotypesContext",
        "replace",
        ExternalApiRole::Mutator,
        ReceiverMutabilityFact::MutatesReceiver,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.GenotypesContext",
        "create",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "GenotypesContext",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "GenotypesContext",
        "create",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "GenotypesContext",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.GenotypesContext",
        "copy",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "GenotypesContext",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "GenotypesContext",
        "copy",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "GenotypesContext",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.GenotypesContext",
        "getSampleNames",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Set<String>",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "GenotypesContext",
        "getSampleNames",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Set<String>",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "htsjdk.variant.variantcontext.GenotypeType",
        "values",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "GenotypeType[]",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "GenotypeType",
        "values",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "GenotypeType[]",
        NullabilityState::DefinitelyNonNull,
    ),
];

const HDF5_EXCEPTIONS: &[ExternalPackException] = &[ExternalPackException {
    type_name: "HDF5Error",
    kind: ExceptionKind::Checked,
}];

const SQLITE_EXCEPTIONS: &[ExternalPackException] = &[ExternalPackException {
    type_name: "SQLException",
    kind: ExceptionKind::Checked,
}];

const JAVA_SQL_EXCEPTIONS: &[ExternalPackException] = &[
    ExternalPackException {
        type_name: "java.sql.SQLException",
        kind: ExceptionKind::Checked,
    },
    ExternalPackException {
        type_name: "java.lang.ClassNotFoundException",
        kind: ExceptionKind::Checked,
    },
    ExternalPackException {
        type_name: "ClassNotFoundException",
        kind: ExceptionKind::Checked,
    },
];

const VARIANTCONTEXT_EXCEPTIONS: &[ExternalPackException] = &[
    ExternalPackException {
        type_name: "htsjdk.variant.variantcontext.VariantContextException",
        kind: ExceptionKind::Unchecked,
    },
    ExternalPackException {
        type_name: "VariantContextException",
        kind: ExceptionKind::Unchecked,
    },
    ExternalPackException {
        type_name: "htsjdk.tribble.TribbleException.InternalCodecException",
        kind: ExceptionKind::Unchecked,
    },
    ExternalPackException {
        type_name: "InternalCodecException",
        kind: ExceptionKind::Unchecked,
    },
];

const SAM_HEADER_EXCEPTIONS: &[ExternalPackException] = &[ExternalPackException {
    type_name: "AssertionError",
    kind: ExceptionKind::Unchecked,
}];

const SAMTOOLS_UTIL_EXCEPTIONS: &[ExternalPackException] = &[
    ExternalPackException {
        type_name: "htsjdk.samtools.util.RuntimeIOException",
        kind: ExceptionKind::Unchecked,
    },
    ExternalPackException {
        type_name: "RuntimeIOException",
        kind: ExceptionKind::Unchecked,
    },
];

const BARCLAY_HELP_EXCEPTIONS: &[ExternalPackException] = &[
    ExternalPackException {
        type_name: "org.broadinstitute.barclay.help.DocException",
        kind: ExceptionKind::Unchecked,
    },
    ExternalPackException {
        type_name: "DocException",
        kind: ExceptionKind::Unchecked,
    },
];

const APACHE_COMMONS_LANG_EXCEPTIONS: &[ExternalPackException] = &[
    ExternalPackException {
        type_name: "org.apache.commons.lang3.NotImplementedException",
        kind: ExceptionKind::Unchecked,
    },
    ExternalPackException {
        type_name: "NotImplementedException",
        kind: ExceptionKind::Unchecked,
    },
];

const APACHE_COMMONS_LANG_API_EFFECTS: &[ExternalPackApiEffect] = &[
    semantic_typed_effect_with_arity(
        "org.apache.commons.lang3.tuple.Pair",
        "of",
        2,
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Pair",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect_with_arity(
        "Pair",
        "of",
        2,
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Pair",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect_with_arity(
        "org.apache.commons.lang3.tuple.ImmutablePair",
        "of",
        2,
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Pair",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect_with_arity(
        "ImmutablePair",
        "of",
        2,
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Pair",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "Pair",
        "getLeft",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Object",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "Pair",
        "getRight",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Object",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "ImmutablePair",
        "getLeft",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Object",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "ImmutablePair",
        "getRight",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Object",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "MutablePair",
        "getLeft",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Object",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "MutablePair",
        "getRight",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Object",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "Triple",
        "getLeft",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Object",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "Triple",
        "getMiddle",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Object",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "Triple",
        "getRight",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Object",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "ImmutableTriple",
        "getLeft",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Object",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "ImmutableTriple",
        "getMiddle",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Object",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "ImmutableTriple",
        "getRight",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Object",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "MutableTriple",
        "getLeft",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Object",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "MutableTriple",
        "getMiddle",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Object",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "MutableTriple",
        "getRight",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Object",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "org.apache.commons.lang3.mutable.MutableInt",
        "doubleValue",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "double",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "MutableInt",
        "doubleValue",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "double",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "org.apache.commons.lang3.mutable.MutableInt",
        "getValue",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "MutableInt",
        "getValue",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_mutator_effect(
        "org.apache.commons.lang3.mutable.MutableInt",
        "increment",
        NullabilityState::Unknown,
    ),
    semantic_mutator_effect("MutableInt", "increment", NullabilityState::Unknown),
    semantic_mutator_effect(
        "org.apache.commons.lang3.mutable.MutableInt",
        "decrement",
        NullabilityState::Unknown,
    ),
    semantic_mutator_effect("MutableInt", "decrement", NullabilityState::Unknown),
];

const APACHE_COMMONS_LANG_MODELED_APIS: &[ExternalPackModeledApi] = &[
    modeled_semantic("org.apache.commons.lang3.tuple.Pair", "of"),
    modeled_semantic("Pair", "of"),
    modeled_semantic("org.apache.commons.lang3.tuple.ImmutablePair", "of"),
    modeled_semantic("ImmutablePair", "of"),
    modeled_semantic("Pair", "getLeft"),
    modeled_semantic("Pair", "getRight"),
    modeled_semantic("ImmutablePair", "getLeft"),
    modeled_semantic("ImmutablePair", "getRight"),
    modeled_semantic("MutablePair", "getLeft"),
    modeled_semantic("MutablePair", "getRight"),
    modeled_semantic("Triple", "getLeft"),
    modeled_semantic("Triple", "getMiddle"),
    modeled_semantic("Triple", "getRight"),
    modeled_semantic("ImmutableTriple", "getLeft"),
    modeled_semantic("ImmutableTriple", "getMiddle"),
    modeled_semantic("ImmutableTriple", "getRight"),
    modeled_semantic("MutableTriple", "getLeft"),
    modeled_semantic("MutableTriple", "getMiddle"),
    modeled_semantic("MutableTriple", "getRight"),
    modeled_semantic("org.apache.commons.lang3.mutable.MutableInt", "doubleValue"),
    modeled_semantic("MutableInt", "doubleValue"),
    modeled_semantic("org.apache.commons.lang3.mutable.MutableInt", "getValue"),
    modeled_semantic("MutableInt", "getValue"),
    modeled_semantic("org.apache.commons.lang3.mutable.MutableInt", "increment"),
    modeled_semantic("MutableInt", "increment"),
    modeled_semantic("org.apache.commons.lang3.mutable.MutableInt", "decrement"),
    modeled_semantic("MutableInt", "decrement"),
];

const HDF5_FIELD_NULLABILITY: &[ExternalPackFieldNullability] = &[
    non_null_field("HDF5File", "OpenMode"),
    non_null_field("HDF5File.OpenMode", "READ_ONLY"),
    non_null_field("HDF5File.OpenMode", "CREATE"),
    non_null_field("OpenMode", "READ_ONLY"),
    non_null_field("OpenMode", "CREATE"),
];

const SAM_HEADER_FIELD_NULLABILITY: &[ExternalPackFieldNullability] = &[
    non_null_field(
        "htsjdk.samtools.SAMSequenceRecord",
        "UNKNOWN_SEQUENCE_LENGTH",
    ),
    non_null_field("SAMSequenceRecord", "UNKNOWN_SEQUENCE_LENGTH"),
    non_null_field("htsjdk.samtools.SAMFileHeader", "SortOrder"),
    non_null_field("SAMFileHeader", "SortOrder"),
    non_null_field("htsjdk.samtools.SAMFileHeader", "GroupOrder"),
    non_null_field("SAMFileHeader", "GroupOrder"),
];

const VARIANTCONTEXT_ARGUMENT_EFFECTS: &[ExternalPackArgumentEffect] = &[
    readonly_borrowed_argument(
        "htsjdk.variant.variantcontext.VariantContextBuilder",
        "chr",
        0,
        "String",
    ),
    readonly_borrowed_argument("VariantContextBuilder", "chr", 0, "String"),
    readonly_borrowed_argument(
        "htsjdk.variant.variantcontext.VariantContextBuilder",
        "id",
        0,
        "String",
    ),
    readonly_borrowed_argument("VariantContextBuilder", "id", 0, "String"),
    readonly_borrowed_argument(
        "htsjdk.variant.variantcontext.VariantContextBuilder",
        "alleles",
        0,
        "List<Allele>",
    ),
    readonly_borrowed_argument("VariantContextBuilder", "alleles", 0, "List<Allele>"),
    readonly_borrowed_argument(
        "htsjdk.variant.variantcontext.VariantContextBuilder",
        "genotypes",
        0,
        "GenotypesContext",
    ),
    readonly_borrowed_argument("VariantContextBuilder", "genotypes", 0, "GenotypesContext"),
    readonly_borrowed_argument(
        "htsjdk.variant.variantcontext.GenotypeBuilder",
        "name",
        0,
        "String",
    ),
    readonly_borrowed_argument("GenotypeBuilder", "name", 0, "String"),
    readonly_borrowed_argument(
        "htsjdk.variant.variantcontext.VariantContext",
        "extraStrictValidation",
        0,
        "Allele",
    ),
    readonly_borrowed_argument("VariantContext", "extraStrictValidation", 0, "Allele"),
    readonly_borrowed_argument(
        "htsjdk.variant.variantcontext.VariantContext",
        "extraStrictValidation",
        1,
        "Allele",
    ),
    readonly_borrowed_argument("VariantContext", "extraStrictValidation", 1, "Allele"),
    readonly_borrowed_argument(
        "htsjdk.variant.variantcontext.VariantContext",
        "extraStrictValidation",
        2,
        "Set<String>",
    ),
    readonly_borrowed_argument("VariantContext", "extraStrictValidation", 2, "Set<String>"),
    readonly_borrowed_argument(
        "htsjdk.variant.variantcontext.VariantContext",
        "validateReferenceBases",
        0,
        "Allele",
    ),
    readonly_borrowed_argument("VariantContext", "validateReferenceBases", 0, "Allele"),
    readonly_borrowed_argument(
        "htsjdk.variant.variantcontext.VariantContext",
        "validateReferenceBases",
        1,
        "Allele",
    ),
    readonly_borrowed_argument("VariantContext", "validateReferenceBases", 1, "Allele"),
    readonly_borrowed_argument(
        "htsjdk.variant.variantcontext.VariantContext",
        "validateRSIDs",
        0,
        "Set<String>",
    ),
    readonly_borrowed_argument("VariantContext", "validateRSIDs", 0, "Set<String>"),
];

const HDF5_OBJECT_STRATEGIES: &[ExternalPackObjectStrategy] = &[
    erased_object_strategy("HDF5Library"),
    erased_object_strategy("HDF5File"),
    erased_object_strategy("HDF5File.OpenMode"),
    erased_object_strategy("OpenMode"),
];

const SQLITE_OBJECT_STRATEGIES: &[ExternalPackObjectStrategy] = &[
    erased_object_strategy("org.sqlite.SQLiteConfig"),
    erased_object_strategy("SQLiteConfig"),
    erased_object_strategy("org.sqlite.SQLiteConnection"),
    erased_object_strategy("SQLiteConnection"),
    erased_object_strategy("org.sqlite.SQLiteStatement"),
    erased_object_strategy("SQLiteStatement"),
    erased_object_strategy("org.sqlite.SQLiteResultSet"),
    erased_object_strategy("SQLiteResultSet"),
    erased_object_strategy("org.sqlite.SQLException"),
    erased_object_strategy("SQLException"),
];

const JAVA_SQL_OBJECT_STRATEGIES: &[ExternalPackObjectStrategy] = &[
    erased_object_strategy("java.sql.DriverManager"),
    erased_object_strategy("driver_manager"),
    erased_object_strategy("java.sql.Connection"),
    erased_object_strategy("java.sql.Statement"),
    erased_object_strategy("java.sql.PreparedStatement"),
    erased_object_strategy("java.sql.ResultSet"),
    erased_object_strategy("java.sql.ResultSetMetaData"),
    erased_object_strategy("java.sql.DatabaseMetaData"),
    erased_object_strategy("java.sql.Date"),
    erased_object_strategy("java.sql.Timestamp"),
    erased_object_strategy("java.sql.SQLException"),
];

const SQLITE_ARGUMENT_EFFECTS: &[ExternalPackArgumentEffect] = &[
    readonly_borrowed_argument("org.sqlite.SQLiteConnection", "setAutoCommit", 0, "boolean"),
    readonly_borrowed_argument("SQLiteConnection", "setAutoCommit", 0, "boolean"),
    readonly_borrowed_argument("org.sqlite.SQLiteStatement", "bindString", 0, "int"),
    readonly_borrowed_argument("SQLiteStatement", "bindString", 0, "int"),
    readonly_borrowed_argument("org.sqlite.SQLiteStatement", "bindString", 1, "String"),
    readonly_borrowed_argument("SQLiteStatement", "bindString", 1, "String"),
    readonly_borrowed_argument("org.sqlite.SQLiteStatement", "bindInt", 0, "int"),
    readonly_borrowed_argument("SQLiteStatement", "bindInt", 0, "int"),
    readonly_borrowed_argument("org.sqlite.SQLiteStatement", "bindInt", 1, "int"),
    readonly_borrowed_argument("SQLiteStatement", "bindInt", 1, "int"),
    readonly_borrowed_argument("org.sqlite.SQLiteStatement", "bindDouble", 0, "int"),
    readonly_borrowed_argument("SQLiteStatement", "bindDouble", 0, "int"),
    readonly_borrowed_argument("org.sqlite.SQLiteStatement", "bindDouble", 1, "double"),
    readonly_borrowed_argument("SQLiteStatement", "bindDouble", 1, "double"),
    readonly_borrowed_argument("org.sqlite.SQLiteResultSet", "getString", 0, "String"),
    readonly_borrowed_argument("SQLiteResultSet", "getString", 0, "String"),
    readonly_borrowed_argument("org.sqlite.SQLiteResultSet", "getInt", 0, "int"),
    readonly_borrowed_argument("SQLiteResultSet", "getInt", 0, "int"),
    readonly_borrowed_argument("org.sqlite.SQLiteResultSet", "getLong", 0, "int"),
    readonly_borrowed_argument("SQLiteResultSet", "getLong", 0, "int"),
    readonly_borrowed_argument("org.sqlite.SQLiteResultSet", "getBoolean", 0, "String"),
    readonly_borrowed_argument("SQLiteResultSet", "getBoolean", 0, "String"),
    readonly_borrowed_argument("org.sqlite.SQLiteResultSet", "getDouble", 0, "String"),
    readonly_borrowed_argument("SQLiteResultSet", "getDouble", 0, "String"),
];

const JAVA_SQL_ARGUMENT_EFFECTS: &[ExternalPackArgumentEffect] = &[
    readonly_borrowed_argument("driver_manager", "getConnection", 0, "String"),
    readonly_borrowed_argument("java.sql.DriverManager", "getConnection", 0, "String"),
    readonly_borrowed_argument("driver_manager", "getConnection", 1, "Properties"),
    readonly_borrowed_argument("java.sql.DriverManager", "getConnection", 1, "Properties"),
    readonly_borrowed_argument("java.sql.Connection", "prepareStatement", 0, "String"),
    readonly_borrowed_argument("java.sql.Statement", "executeQuery", 0, "String"),
    readonly_borrowed_argument("java.sql.PreparedStatement", "setString", 0, "int"),
    readonly_borrowed_argument("java.sql.PreparedStatement", "setString", 1, "String"),
    readonly_borrowed_argument("java.sql.PreparedStatement", "setInt", 0, "int"),
    readonly_borrowed_argument("java.sql.PreparedStatement", "setInt", 1, "int"),
    readonly_borrowed_argument("java.sql.PreparedStatement", "setDouble", 0, "int"),
    readonly_borrowed_argument("java.sql.PreparedStatement", "setDouble", 1, "double"),
    readonly_borrowed_argument("java.sql.PreparedStatement", "setLong", 0, "int"),
    readonly_borrowed_argument("java.sql.PreparedStatement", "setLong", 1, "long"),
    readonly_borrowed_argument("java.sql.PreparedStatement", "setBoolean", 0, "int"),
    readonly_borrowed_argument("java.sql.PreparedStatement", "setBoolean", 1, "boolean"),
    readonly_borrowed_argument("java.sql.ResultSet", "getString", 0, "int"),
    readonly_borrowed_argument("java.sql.ResultSet", "getString", 0, "String"),
    readonly_borrowed_argument("java.sql.ResultSet", "getInt", 0, "int"),
    readonly_borrowed_argument("java.sql.ResultSet", "getInt", 0, "String"),
    readonly_borrowed_argument("java.sql.ResultSet", "getLong", 0, "int"),
    readonly_borrowed_argument("java.sql.ResultSet", "getLong", 0, "String"),
    readonly_borrowed_argument("java.sql.ResultSet", "getDouble", 0, "int"),
    readonly_borrowed_argument("java.sql.ResultSet", "getDouble", 0, "String"),
    readonly_borrowed_argument("java.sql.ResultSet", "getFloat", 0, "int"),
    readonly_borrowed_argument("java.sql.ResultSet", "getFloat", 0, "String"),
    readonly_borrowed_argument("java.sql.ResultSet", "getShort", 0, "int"),
    readonly_borrowed_argument("java.sql.ResultSet", "getShort", 0, "String"),
    readonly_borrowed_argument("java.sql.ResultSet", "getByte", 0, "int"),
    readonly_borrowed_argument("java.sql.ResultSet", "getByte", 0, "String"),
    readonly_borrowed_argument("java.sql.ResultSet", "getBoolean", 0, "int"),
    readonly_borrowed_argument("java.sql.ResultSet", "getBoolean", 0, "String"),
    readonly_borrowed_argument("java.sql.ResultSet", "getObject", 0, "int"),
    readonly_borrowed_argument("java.sql.ResultSet", "getObject", 0, "String"),
    readonly_borrowed_argument("java.sql.ResultSet", "getBytes", 0, "int"),
    readonly_borrowed_argument("java.sql.ResultSet", "getBytes", 0, "String"),
    readonly_borrowed_argument("java.sql.ResultSet", "getDate", 0, "int"),
    readonly_borrowed_argument("java.sql.ResultSet", "getDate", 0, "String"),
    readonly_borrowed_argument("java.sql.ResultSet", "getTimestamp", 0, "int"),
    readonly_borrowed_argument("java.sql.ResultSet", "getTimestamp", 0, "String"),
    readonly_borrowed_argument("java.sql.ResultSetMetaData", "getColumnName", 0, "int"),
    readonly_borrowed_argument("java.sql.ResultSetMetaData", "getColumnType", 0, "int"),
    readonly_borrowed_argument("java.sql.ResultSetMetaData", "getColumnTypeName", 0, "int"),
    readonly_borrowed_argument("java.sql.ResultSetMetaData", "getColumnClassName", 0, "int"),
    readonly_borrowed_argument("java.sql.ResultSetMetaData", "isNullable", 0, "int"),
    readonly_borrowed_argument("java.sql.ResultSetMetaData", "isSigned", 0, "int"),
    readonly_borrowed_argument("java.sql.ResultSetMetaData", "isAutoIncrement", 0, "int"),
    readonly_borrowed_argument("java.sql.ResultSetMetaData", "getPrecision", 0, "int"),
    readonly_borrowed_argument("java.sql.ResultSetMetaData", "getScale", 0, "int"),
    readonly_borrowed_argument(
        "java.sql.ResultSetMetaData",
        "getColumnDisplaySize",
        0,
        "int",
    ),
    readonly_borrowed_argument("java.sql.DatabaseMetaData", "getTables", 0, "String"),
    readonly_borrowed_argument("java.sql.DatabaseMetaData", "getTables", 1, "String"),
    readonly_borrowed_argument("java.sql.DatabaseMetaData", "getTables", 2, "String"),
    readonly_borrowed_argument("java.sql.DatabaseMetaData", "getTables", 3, "String[]"),
    readonly_borrowed_argument("java.sql.DatabaseMetaData", "getColumns", 0, "String"),
    readonly_borrowed_argument("java.sql.DatabaseMetaData", "getColumns", 1, "String"),
    readonly_borrowed_argument("java.sql.DatabaseMetaData", "getColumns", 2, "String"),
    readonly_borrowed_argument("java.sql.DatabaseMetaData", "getColumns", 3, "String"),
];

const VARIANTCONTEXT_OBJECT_STRATEGIES: &[ExternalPackObjectStrategy] = &[
    erased_object_strategy("htsjdk.variant.variantcontext.Allele"),
    erased_object_strategy("Allele"),
    erased_object_strategy("htsjdk.variant.variantcontext.VariantContext"),
    erased_object_strategy("VariantContext"),
    erased_object_strategy("htsjdk.variant.variantcontext.VariantContextBuilder"),
    erased_object_strategy("VariantContextBuilder"),
    erased_object_strategy("htsjdk.variant.variantcontext.VariantContextComparator"),
    erased_object_strategy("VariantContextComparator"),
    erased_object_strategy("htsjdk.variant.variantcontext.Genotype"),
    erased_object_strategy("Genotype"),
    erased_object_strategy("htsjdk.variant.variantcontext.GenotypeBuilder"),
    erased_object_strategy("GenotypeBuilder"),
    erased_object_strategy("htsjdk.variant.variantcontext.GenotypeLikelihoods"),
    erased_object_strategy("GenotypeLikelihoods"),
    erased_object_strategy("htsjdk.variant.variantcontext.GenotypeLikelihoodsAllelePair"),
    erased_object_strategy("GenotypeLikelihoodsAllelePair"),
    erased_object_strategy("htsjdk.variant.variantcontext.JexlVCMatchExp"),
    erased_object_strategy("JexlVCMatchExp"),
    erased_object_strategy("htsjdk.variant.variantcontext.JexlMissingValueTreatment"),
    erased_object_strategy("JexlMissingValueTreatment"),
    erased_object_strategy("htsjdk.variant.variantcontext.GenotypesContext"),
    erased_object_strategy("GenotypesContext"),
    erased_object_strategy("htsjdk.variant.variantcontext.VariantContext.Type"),
    erased_object_strategy("VariantContext.Type"),
    erased_object_strategy("htsjdk.variant.variantcontext.GenotypeType"),
    erased_object_strategy("GenotypeType"),
];

const VCF_OBJECT_STRATEGIES: &[ExternalPackObjectStrategy] = &[
    erased_object_strategy("htsjdk.variant.vcf.VCFFileReader"),
    erased_object_strategy("VCFFileReader"),
    erased_object_strategy("htsjdk.variant.vcf.VCFSimpleHeaderLine"),
    erased_object_strategy("VCFSimpleHeaderLine"),
    erased_object_strategy("htsjdk.variant.vcf.VCFHeader"),
    erased_object_strategy("VCFHeader"),
    erased_object_strategy("htsjdk.variant.vcf.VCFHeaderLine"),
    erased_object_strategy("VCFHeaderLine"),
    erased_object_strategy("htsjdk.variant.vcf.VCFInfoHeaderLine"),
    erased_object_strategy("VCFInfoHeaderLine"),
    erased_object_strategy("htsjdk.variant.vcf.VCFFormatHeaderLine"),
    erased_object_strategy("VCFFormatHeaderLine"),
    erased_object_strategy("htsjdk.variant.vcf.VCFFilterHeaderLine"),
    erased_object_strategy("VCFFilterHeaderLine"),
    erased_object_strategy("htsjdk.variant.vcf.VCFHeaderLineType"),
    erased_object_strategy("VCFHeaderLineType"),
    erased_object_strategy("htsjdk.variant.vcf.VCFHeaderLineCount"),
    erased_object_strategy("VCFHeaderLineCount"),
    erased_object_strategy("htsjdk.variant.vcf.VCFHeaderVersion"),
    erased_object_strategy("VCFHeaderVersion"),
    erased_object_strategy("htsjdk.variant.vcf.VCFConstants"),
    erased_object_strategy("VCFConstants"),
    erased_object_strategy("htsjdk.variant.vcf.VCFCompoundHeaderLine"),
    erased_object_strategy("VCFCompoundHeaderLine"),
    erased_object_strategy("htsjdk.variant.vcf.VCFCompoundHeaderLine.SupportedHeaderLineType"),
    erased_object_strategy("VCFCompoundHeaderLine.SupportedHeaderLineType"),
    erased_object_strategy("SupportedHeaderLineType"),
    erased_object_strategy("htsjdk.variant.variantcontext.writer.VariantContextWriterBuilder"),
    erased_object_strategy("VariantContextWriterBuilder"),
    erased_object_strategy("htsjdk.variant.variantcontext.writer.VariantContextWriter"),
    erased_object_strategy("VariantContextWriter"),
    erased_object_strategy(
        "htsjdk.variant.variantcontext.writer.VariantContextWriterBuilder.OutputType",
    ),
    erased_object_strategy("VariantContextWriterBuilder.OutputType"),
    erased_object_strategy("OutputType"),
    erased_object_strategy("htsjdk.variant.variantcontext.writer.Options"),
    erased_object_strategy("Options"),
];

const APACHE_COMMONS_MATH_OPTIM_OBJECT_STRATEGIES: &[ExternalPackObjectStrategy] = &[
    erased_object_strategy("org.apache.commons.math3.optim.univariate.SearchInterval"),
    erased_object_strategy("SearchInterval"),
    erased_object_strategy("org.apache.commons.math3.optim.univariate.UnivariateObjectiveFunction"),
    erased_object_strategy("UnivariateObjectiveFunction"),
    erased_object_strategy("org.apache.commons.math3.optim.univariate.BrentOptimizer"),
    erased_object_strategy("BrentOptimizer"),
    erased_object_strategy("org.apache.commons.math3.optim.nonlinear.scalar.GoalType"),
    erased_object_strategy("GoalType"),
];

const APACHE_COMMONS_MATH_OBJECT_STRATEGIES: &[ExternalPackObjectStrategy] = &[
    erased_object_strategy("org.apache.commons.math3.linear.Array2DRowRealMatrix"),
    erased_object_strategy("Array2DRowRealMatrix"),
    erased_object_strategy("org.apache.commons.math3.linear.ArrayRealVector"),
    erased_object_strategy("ArrayRealVector"),
    erased_object_strategy("org.apache.commons.math3.linear.DefaultRealMatrixChangingVisitor"),
    erased_object_strategy("DefaultRealMatrixChangingVisitor"),
    erased_object_strategy("org.apache.commons.math3.stat.descriptive.moment.Mean"),
    erased_object_strategy("Mean"),
    erased_object_strategy("org.apache.commons.math3.distribution.BinomialDistribution"),
    erased_object_strategy("BinomialDistribution"),
    erased_object_strategy("org.apache.commons.math3.distribution.BetaDistribution"),
    erased_object_strategy("BetaDistribution"),
    erased_object_strategy("org.apache.commons.math3.distribution.AbstractIntegerDistribution"),
    erased_object_strategy("AbstractIntegerDistribution"),
    erased_object_strategy("org.apache.commons.math3.stat.descriptive.DescriptiveStatistics"),
    erased_object_strategy("DescriptiveStatistics"),
    erased_object_strategy("org.apache.commons.math3.exception.DimensionMismatchException"),
    erased_object_strategy("DimensionMismatchException"),
    erased_object_strategy("org.apache.commons.math3.distribution.EnumeratedDistribution"),
    erased_object_strategy("EnumeratedDistribution"),
    erased_object_strategy("org.apache.commons.math3.distribution.ExponentialDistribution"),
    erased_object_strategy("ExponentialDistribution"),
    erased_object_strategy("org.apache.commons.math3.distribution.HypergeometricDistribution"),
    erased_object_strategy("HypergeometricDistribution"),
    erased_object_strategy("org.apache.commons.math3.random.JDKRandomGenerator"),
    erased_object_strategy("JDKRandomGenerator"),
    erased_object_strategy("org.apache.commons.math3.optim.MaxEval"),
    erased_object_strategy("MaxEval"),
    erased_object_strategy("org.apache.commons.math3.stat.descriptive.rank.Median"),
    erased_object_strategy("Median"),
    erased_object_strategy("org.apache.commons.math3.distribution.NormalDistribution"),
    erased_object_strategy("NormalDistribution"),
    erased_object_strategy("org.apache.commons.math3.distribution.ChiSquaredDistribution"),
    erased_object_strategy("ChiSquaredDistribution"),
    erased_object_strategy("org.apache.commons.math3.distribution.PoissonDistribution"),
    erased_object_strategy("PoissonDistribution"),
    erased_object_strategy("org.apache.commons.math3.linear.SingularValueDecomposition"),
    erased_object_strategy("SingularValueDecomposition"),
    erased_object_strategy("org.apache.commons.math3.stat.descriptive.moment.StandardDeviation"),
    erased_object_strategy("StandardDeviation"),
    erased_object_strategy("org.apache.commons.math3.distribution.TDistribution"),
    erased_object_strategy("TDistribution"),
];

const APACHE_COMMONS_MATH_STATS_OBJECT_STRATEGIES: &[ExternalPackObjectStrategy] = &[
    erased_object_strategy("org.apache.commons.math3.stat.descriptive.rank.Percentile"),
    erased_object_strategy("Percentile"),
    erased_object_strategy(
        "org.apache.commons.math3.stat.descriptive.rank.Percentile.EstimationType",
    ),
    erased_object_strategy("Percentile.EstimationType"),
    erased_object_strategy("EstimationType"),
];

const APACHE_COMMONS_COLLECTIONS_OBJECT_STRATEGIES: &[ExternalPackObjectStrategy] = &[
    erased_object_strategy("org.apache.commons.collections4.map.DefaultedMap"),
    erased_object_strategy("DefaultedMap"),
    erased_object_strategy("org.apache.commons.collections4.iterators.FilterIterator"),
    erased_object_strategy("FilterIterator"),
    erased_object_strategy("org.apache.commons.collections4.iterators.SingletonIterator"),
    erased_object_strategy("SingletonIterator"),
    erased_object_strategy("org.apache.commons.collections4.MultiMap"),
    erased_object_strategy("MultiMap"),
];

const APACHE_COMMONS_COMPRESS_OBJECT_STRATEGIES: &[ExternalPackObjectStrategy] = &[
    erased_object_strategy(
        "org.apache.commons.compress.compressors.gzip.GzipCompressorInputStream",
    ),
    erased_object_strategy("GzipCompressorInputStream"),
    erased_object_strategy(
        "org.apache.commons.compress.compressors.gzip.GzipCompressorOutputStream",
    ),
    erased_object_strategy("GzipCompressorOutputStream"),
    erased_object_strategy("org.apache.commons.io.input.NullInputStream"),
    erased_object_strategy("NullInputStream"),
    erased_object_strategy("org.apache.commons.io.output.NullOutputStream"),
    erased_object_strategy("NullOutputStream"),
    erased_object_strategy("org.apache.commons.io.output.NullWriter"),
    erased_object_strategy("NullWriter"),
    erased_object_strategy("org.apache.commons.compress.archivers.tar.TarArchiveInputStream"),
    erased_object_strategy("TarArchiveInputStream"),
    erased_object_strategy("org.apache.commons.compress.archivers.tar.TarArchiveOutputStream"),
    erased_object_strategy("TarArchiveOutputStream"),
    erased_object_strategy("org.apache.commons.compress.archivers.tar.TarArchiveEntry"),
    erased_object_strategy("TarArchiveEntry"),
];

const APACHE_COMMONS_LANG_OBJECT_STRATEGIES: &[ExternalPackObjectStrategy] = &[
    erased_object_strategy("org.apache.commons.lang3.builder.HashCodeBuilder"),
    erased_object_strategy("HashCodeBuilder"),
    erased_object_strategy("org.apache.commons.lang3.SystemUtils"),
    erased_object_strategy("SystemUtils"),
    erased_object_strategy("org.apache.commons.lang3.mutable.MutableInt"),
    erased_object_strategy("MutableInt"),
    erased_object_strategy("org.apache.commons.lang3.mutable.MutableLong"),
    erased_object_strategy("MutableLong"),
    erased_object_strategy("org.apache.commons.lang3.mutable.MutableDouble"),
    erased_object_strategy("MutableDouble"),
    erased_object_strategy("org.apache.commons.lang3.NotImplementedException"),
    erased_object_strategy("NotImplementedException"),
    erased_object_strategy("org.apache.commons.lang3.tuple.Pair"),
    erased_object_strategy("Pair"),
    erased_object_strategy("org.apache.commons.lang3.tuple.ImmutablePair"),
    erased_object_strategy("ImmutablePair"),
    erased_object_strategy("org.apache.commons.lang3.tuple.MutablePair"),
    erased_object_strategy("MutablePair"),
    erased_object_strategy("org.apache.commons.lang3.tuple.Triple"),
    erased_object_strategy("Triple"),
    erased_object_strategy("org.apache.commons.lang3.tuple.ImmutableTriple"),
    erased_object_strategy("ImmutableTriple"),
    erased_object_strategy("org.apache.commons.lang3.tuple.MutableTriple"),
    erased_object_strategy("MutableTriple"),
];

const APACHE_COMMONS_CODEC_OBJECT_STRATEGIES: &[ExternalPackObjectStrategy] = &[
    erased_object_strategy("org.apache.commons.codec.digest.MessageDigestAlgorithms"),
    erased_object_strategy("MessageDigestAlgorithms"),
];

const JAMA_OBJECT_STRATEGIES: &[ExternalPackObjectStrategy] = &[
    erased_object_strategy("Jama.Matrix"),
    erased_object_strategy("Matrix"),
];

const HIPPARCHUS_DISTRIBUTION_OBJECT_STRATEGIES: &[ExternalPackObjectStrategy] = &[
    erased_object_strategy("org.hipparchus.random.EmpiricalDistribution"),
    erased_object_strategy("EmpiricalDistribution"),
];

const APACHE_HTTP_OBJECT_STRATEGIES: &[ExternalPackObjectStrategy] = &[
    erased_object_strategy("org.apache.http.client.methods.HttpGet"),
    erased_object_strategy("HttpGet"),
    erased_object_strategy("org.apache.http.impl.conn.PoolingHttpClientConnectionManager"),
    erased_object_strategy("PoolingHttpClientConnectionManager"),
];

const GOOGLE_BIGQUERY_OBJECT_STRATEGIES: &[ExternalPackObjectStrategy] = &[
    erased_object_strategy("com.google.cloud.bigquery.FieldValue"),
    erased_object_strategy("FieldValue"),
    erased_object_strategy("com.google.cloud.bigquery.FieldValue.Attribute"),
    erased_object_strategy("FieldValue.Attribute"),
    erased_object_strategy("com.google.cloud.bigquery.QueryJobConfiguration"),
    erased_object_strategy("QueryJobConfiguration"),
    erased_object_strategy("com.google.cloud.bigquery.QueryJobConfiguration.Priority"),
    erased_object_strategy("QueryJobConfiguration.Priority"),
];

const GOOGLE_BIGQUERY_STORAGE_OBJECT_STRATEGIES: &[ExternalPackObjectStrategy] = &[
    erased_object_strategy("com.google.cloud.bigquery.storage.v1.DataFormat"),
    erased_object_strategy("DataFormat"),
];

const GOOGLE_STORAGE_OBJECT_STRATEGIES: &[ExternalPackObjectStrategy] = &[
    erased_object_strategy("com.google.cloud.storage.Storage"),
    erased_object_strategy("Storage"),
    erased_object_strategy("com.google.cloud.storage.Storage.SignUrlOption"),
    erased_object_strategy("Storage.SignUrlOption"),
    erased_object_strategy("SignUrlOption"),
];

const GOOGLE_CLOUD_NIO_OBJECT_STRATEGIES: &[ExternalPackObjectStrategy] = &[
    erased_object_strategy("com.google.cloud.storage.contrib.nio.CloudStorageFileSystem"),
    erased_object_strategy("CloudStorageFileSystem"),
    erased_object_strategy("com.google.cloud.storage.contrib.nio.CloudStorageConfiguration"),
    erased_object_strategy("CloudStorageConfiguration"),
];

const GENOMICSDB_OBJECT_STRATEGIES: &[ExternalPackObjectStrategy] = &[
    erased_object_strategy("com.intel.genomicsdb.GenomicsDBFeatureReader"),
    erased_object_strategy("GenomicsDBFeatureReader"),
    erased_object_strategy("com.intel.genomicsdb.GenomicsDBImporter"),
    erased_object_strategy("GenomicsDBImporter"),
    erased_object_strategy("com.intel.genomicsdb.importer.model.ImportConfig"),
    erased_object_strategy("ImportConfig"),
    erased_object_strategy("com.intel.genomicsdb.model.Coordinates"),
    erased_object_strategy("Coordinates"),
    erased_object_strategy("com.intel.genomicsdb.model.Coordinates.GenomicsDBColumn"),
    erased_object_strategy("Coordinates.GenomicsDBColumn"),
    erased_object_strategy("GenomicsDBColumn"),
    erased_object_strategy("com.intel.genomicsdb.model.Coordinates.ContigPosition"),
    erased_object_strategy("Coordinates.ContigPosition"),
    erased_object_strategy("ContigPosition"),
    erased_object_strategy("com.intel.genomicsdb.model.GenomicsDBExportConfiguration"),
    erased_object_strategy("GenomicsDBExportConfiguration"),
    erased_object_strategy(
        "com.intel.genomicsdb.model.GenomicsDBExportConfiguration.ExportConfiguration",
    ),
    erased_object_strategy("GenomicsDBExportConfiguration.ExportConfiguration"),
    erased_object_strategy("ExportConfiguration"),
    erased_object_strategy("com.intel.genomicsdb.model.GenomicsDBImportConfiguration"),
    erased_object_strategy("GenomicsDBImportConfiguration"),
    erased_object_strategy(
        "com.intel.genomicsdb.model.GenomicsDBImportConfiguration.ImportConfiguration",
    ),
    erased_object_strategy("GenomicsDBImportConfiguration.ImportConfiguration"),
    erased_object_strategy("ImportConfiguration"),
    erased_object_strategy("com.intel.genomicsdb.model.GenomicsDBImportConfiguration.Partition"),
    erased_object_strategy("GenomicsDBImportConfiguration.Partition"),
    erased_object_strategy("Partition"),
    erased_object_strategy("com.intel.genomicsdb.model.GenomicsDBVidMapProto"),
    erased_object_strategy("GenomicsDBVidMapProto"),
    erased_object_strategy("com.intel.genomicsdb.model.GenomicsDBVidMapProto.VidMappingPB"),
    erased_object_strategy("GenomicsDBVidMapProto.VidMappingPB"),
    erased_object_strategy("VidMappingPB"),
    erased_object_strategy("com.intel.genomicsdb.model.GenomicsDBVidMapProto.Chromosome"),
    erased_object_strategy("org.genomicsdb.model.GenomicsDBVidMapProto.Chromosome"),
    erased_object_strategy("GenomicsDBVidMapProto.Chromosome"),
    erased_object_strategy("Chromosome"),
    erased_object_strategy("com.intel.genomicsdb.model.GenomicsDBVidMapProto.FieldInfo"),
    erased_object_strategy("GenomicsDBVidMapProto.FieldInfo"),
    erased_object_strategy("FieldInfo"),
    erased_object_strategy(
        "com.intel.genomicsdb.model.GenomicsDBVidMapProto.FieldLengthDescriptorComponentPB",
    ),
    erased_object_strategy("GenomicsDBVidMapProto.FieldLengthDescriptorComponentPB"),
    erased_object_strategy("FieldLengthDescriptorComponentPB"),
    erased_object_strategy("com.intel.genomicsdb.model.ReadSession"),
    erased_object_strategy("ReadSession"),
    erased_object_strategy("com.intel.genomicsdb.model.ReadSession.TableReadOptions"),
    erased_object_strategy("ReadSession.TableReadOptions"),
    erased_object_strategy("TableReadOptions"),
];

const PICARD_PROGRAMGROUPS_OBJECT_STRATEGIES: &[ExternalPackObjectStrategy] = &[
    erased_object_strategy("picard.cmdline.programgroups.DiagnosticsAndQCProgramGroup"),
    erased_object_strategy("DiagnosticsAndQCProgramGroup"),
    erased_object_strategy("picard.cmdline.programgroups.IntervalsManipulationProgramGroup"),
    erased_object_strategy("IntervalsManipulationProgramGroup"),
    erased_object_strategy("picard.cmdline.programgroups.OtherProgramGroup"),
    erased_object_strategy("OtherProgramGroup"),
    erased_object_strategy("picard.cmdline.programgroups.ReadDataManipulationProgramGroup"),
    erased_object_strategy("ReadDataManipulationProgramGroup"),
    erased_object_strategy("picard.cmdline.programgroups.ReferenceProgramGroup"),
    erased_object_strategy("ReferenceProgramGroup"),
    erased_object_strategy("picard.cmdline.programgroups.VariantEvaluationProgramGroup"),
    erased_object_strategy("VariantEvaluationProgramGroup"),
    erased_object_strategy("picard.cmdline.programgroups.VariantFilteringProgramGroup"),
    erased_object_strategy("VariantFilteringProgramGroup"),
    erased_object_strategy("picard.cmdline.programgroups.VariantManipulationProgramGroup"),
    erased_object_strategy("VariantManipulationProgramGroup"),
];

const PICARD_MARKDUPLICATES_OBJECT_STRATEGIES: &[ExternalPackObjectStrategy] = &[
    erased_object_strategy("picard.sam.markduplicates.MarkDuplicates"),
    erased_object_strategy("MarkDuplicates"),
    erased_object_strategy("picard.sam.markduplicates.util.OpticalDuplicateFinder"),
    erased_object_strategy("OpticalDuplicateFinder"),
    erased_object_strategy("picard.sam.markduplicates.util.ReadEnds"),
    erased_object_strategy("ReadEnds"),
    erased_object_strategy("picard.sam.markduplicates.MarkDuplicates.DuplicateTaggingPolicy"),
    erased_object_strategy("MarkDuplicates.DuplicateTaggingPolicy"),
    erased_object_strategy("DuplicateTaggingPolicy"),
];

const PICARD_METRICS_OBJECT_STRATEGIES: &[ExternalPackObjectStrategy] = &[
    erased_object_strategy("picard.sam.DuplicationMetrics"),
    erased_object_strategy("DuplicationMetrics"),
];

const AVRO_OBJECT_STRATEGIES: &[ExternalPackObjectStrategy] = &[
    erased_object_strategy("org.apache.avro.Schema"),
    erased_object_strategy("org.apache.avro.Schema.Parser"),
    erased_object_strategy("Schema.Parser"),
    erased_object_strategy("Parser"),
    erased_object_strategy("org.apache.avro.generic.GenericDatumReader"),
    erased_object_strategy("GenericDatumReader"),
];

const BARCLAY_HELP_OBJECT_STRATEGIES: &[ExternalPackObjectStrategy] = &[
    erased_object_strategy("org.broadinstitute.barclay.help.DocException"),
    erased_object_strategy("DocException"),
    erased_object_strategy("org.broadinstitute.barclay.help.TemplateProperties"),
    erased_object_strategy("TemplateProperties"),
];

const BARCLAY_ARGPARSER_OBJECT_STRATEGIES: &[ExternalPackObjectStrategy] = &[
    erased_object_strategy("org.broadinstitute.barclay.argparser.CommandLineException"),
    erased_object_strategy("CommandLineException"),
    erased_object_strategy(
        "org.broadinstitute.barclay.argparser.CommandLineException.BadArgumentValue",
    ),
    erased_object_strategy("CommandLineException.BadArgumentValue"),
    erased_object_strategy("BadArgumentValue"),
    erased_object_strategy(
        "org.broadinstitute.barclay.argparser.CommandLineException.MissingArgument",
    ),
    erased_object_strategy("CommandLineException.MissingArgument"),
    erased_object_strategy("MissingArgument"),
    erased_object_strategy("org.broadinstitute.barclay.argparser.CommandLineArgumentParser"),
    erased_object_strategy("CommandLineArgumentParser"),
    erased_object_strategy("org.broadinstitute.barclay.argparser.SpecialArgumentsCollection"),
    erased_object_strategy("SpecialArgumentsCollection"),
];

const BARCLAY_UTILS_OBJECT_STRATEGIES: &[ExternalPackObjectStrategy] = &[
    erased_object_strategy("org.broadinstitute.barclay.utils.ClassFinder"),
    erased_object_strategy("ClassFinder"),
];

const SCALA_TUPLE_OBJECT_STRATEGIES: &[ExternalPackObjectStrategy] = &[
    erased_object_strategy("scala.Tuple2"),
    erased_object_strategy("Tuple2"),
    erased_object_strategy("scala.Tuple3"),
    erased_object_strategy("Tuple3"),
    erased_object_strategy("scala.Tuple4"),
    erased_object_strategy("Tuple4"),
];

const KRYO_OBJECT_STRATEGIES: &[ExternalPackObjectStrategy] = &[
    erased_object_strategy("com.esotericsoftware.kryo.Kryo"),
    erased_object_strategy("Kryo"),
    erased_object_strategy("com.esotericsoftware.kryo.Serializer"),
    erased_object_strategy("com.esotericsoftware.kryo.io.Input"),
    erased_object_strategy("Input"),
    erased_object_strategy("com.esotericsoftware.kryo.io.Output"),
    erased_object_strategy("Output"),
    erased_object_strategy("com.esotericsoftware.kryo.serializers.FieldSerializer"),
    erased_object_strategy("FieldSerializer"),
    erased_object_strategy("com.esotericsoftware.kryo.serializers.ImmutableMapSerializer"),
    erased_object_strategy("ImmutableMapSerializer"),
];

const OPENCSV_OBJECT_STRATEGIES: &[ExternalPackObjectStrategy] = &[
    erased_object_strategy("com.opencsv.CSVReader"),
    erased_object_strategy("CSVReader"),
    erased_object_strategy("com.opencsv.CSVWriter"),
    erased_object_strategy("CSVWriter"),
];

const ORG_JSON_OBJECT_STRATEGIES: &[ExternalPackObjectStrategy] = &[
    erased_object_strategy("org.json.JSONObject"),
    erased_object_strategy("JSONObject"),
    erased_object_strategy("org.json.JSONArray"),
    erased_object_strategy("JSONArray"),
    erased_object_strategy("org.json.JSONTokener"),
    erased_object_strategy("JSONTokener"),
];

const REFLECTIONS_OBJECT_STRATEGIES: &[ExternalPackObjectStrategy] = &[
    erased_object_strategy("org.reflections.Reflections"),
    erased_object_strategy("Reflections"),
];

const HADOOP_CONF_OBJECT_STRATEGIES: &[ExternalPackObjectStrategy] = &[
    erased_object_strategy("org.apache.hadoop.conf.Configuration"),
    erased_object_strategy("Configuration"),
];

const APACHE_COMMONS_CONFIG_OBJECT_STRATEGIES: &[ExternalPackObjectStrategy] = &[
    erased_object_strategy("org.apache.commons.configuration2.builder.fluent.Configurations"),
    erased_object_strategy("Configurations"),
    erased_object_strategy("org.apache.commons.configuration2.PropertiesConfiguration"),
    erased_object_strategy("PropertiesConfiguration"),
    erased_object_strategy("org.apache.commons.configuration2.XMLConfiguration"),
    erased_object_strategy("XMLConfiguration"),
    erased_object_strategy("org.apache.commons.configuration2.INIConfiguration"),
    erased_object_strategy("INIConfiguration"),
];

const SPARK_OBJECT_STRATEGIES: &[ExternalPackObjectStrategy] = &[
    erased_object_strategy("org.apache.spark.SparkConf"),
    erased_object_strategy("SparkConf"),
    erased_object_strategy("org.apache.spark.HashPartitioner"),
    erased_object_strategy("HashPartitioner"),
    erased_object_strategy("org.apache.spark.api.java.JavaSparkContext"),
    erased_object_strategy("JavaSparkContext"),
    erased_object_strategy("org.apache.spark.api.java.JavaRDD"),
    erased_object_strategy("JavaRDD"),
    erased_object_strategy("org.apache.spark.api.java.JavaPairRDD"),
    erased_object_strategy("JavaPairRDD"),
    erased_object_strategy("org.apache.spark.rdd.PartitionGroup"),
    erased_object_strategy("PartitionGroup"),
    erased_object_strategy("org.apache.spark.mllib.linalg.distributed.RowMatrix"),
    erased_object_strategy("RowMatrix"),
    erased_object_strategy("org.apache.spark.broadcast.Broadcast"),
    erased_object_strategy("Broadcast"),
];

const BWA_OBJECT_STRATEGIES: &[ExternalPackObjectStrategy] = &[
    erased_object_strategy("org.broadinstitute.hellbender.utils.bwa.BwaMemIndex"),
    erased_object_strategy("BwaMemIndex"),
    erased_object_strategy("org.broadinstitute.hellbender.utils.bwa.BwaMemAligner"),
    erased_object_strategy("BwaMemAligner"),
];

const JGRAPHT_OBJECT_STRATEGIES: &[ExternalPackObjectStrategy] = &[
    erased_object_strategy("org.jgrapht.Graph"),
    erased_object_strategy("Graph"),
    erased_object_strategy("org.jgrapht.alg.ConnectivityInspector"),
    erased_object_strategy("ConnectivityInspector"),
    erased_object_strategy("org.jgrapht.alg.CycleDetector"),
    erased_object_strategy("CycleDetector"),
    erased_object_strategy("org.jgrapht.graph.SimpleGraph"),
    erased_object_strategy("SimpleGraph"),
    erased_object_strategy("org.jgrapht.graph.DefaultDirectedWeightedGraph"),
    erased_object_strategy("DefaultDirectedWeightedGraph"),
    erased_object_strategy("org.jgrapht.io.IntegerComponentNameProvider"),
    erased_object_strategy("IntegerComponentNameProvider"),
    erased_object_strategy("org.jgrapht.io.DOTExporter"),
    erased_object_strategy("DOTExporter"),
];

const FASTUTIL_OBJECT_STRATEGIES: &[ExternalPackObjectStrategy] = &[
    erased_object_strategy("it.unimi.dsi.fastutil.ints.Int2ObjectArrayMap"),
    erased_object_strategy("Int2ObjectArrayMap"),
    erased_object_strategy("it.unimi.dsi.fastutil.ints.Int2ObjectOpenHashMap"),
    erased_object_strategy("Int2ObjectOpenHashMap"),
    erased_object_strategy("it.unimi.dsi.fastutil.ints.IntArrayList"),
    erased_object_strategy("IntArrayList"),
    erased_object_strategy("it.unimi.dsi.fastutil.ints.Int2IntArrayMap"),
    erased_object_strategy("Int2IntArrayMap"),
    erased_object_strategy("it.unimi.dsi.fastutil.objects.Object2IntOpenHashMap"),
    erased_object_strategy("Object2IntOpenHashMap"),
];

const KRYO_SERIALIZERS_OBJECT_STRATEGIES: &[ExternalPackObjectStrategy] = &[
    erased_object_strategy("de.javakaffee.kryoserializers.UnmodifiableCollectionsSerializer"),
    erased_object_strategy("UnmodifiableCollectionsSerializer"),
];

const DISQ_WRITE_OBJECT_STRATEGIES: &[ExternalPackObjectStrategy] = &[
    erased_object_strategy("org.disq_bio.disq.HtsjdkReadsRdd"),
    erased_object_strategy("HtsjdkReadsRdd"),
    erased_object_strategy("org.disq_bio.disq.HtsjdkVariantsRdd"),
    erased_object_strategy("HtsjdkVariantsRdd"),
    erased_object_strategy("org.disq_bio.disq.HtsjdkReadsTraversalParameters"),
    erased_object_strategy("HtsjdkReadsTraversalParameters"),
    erased_object_strategy("org.disq_bio.disq.TempPartsDirectoryWriteOption"),
    erased_object_strategy("TempPartsDirectoryWriteOption"),
    erased_object_strategy("org.disq_bio.disq.ReadsFormatWriteOption"),
    erased_object_strategy("ReadsFormatWriteOption"),
    erased_object_strategy("org.disq_bio.disq.FileCardinalityWriteOption"),
    erased_object_strategy("FileCardinalityWriteOption"),
];

const INTEL_GKL_OBJECT_STRATEGIES: &[ExternalPackObjectStrategy] = &[
    erased_object_strategy("com.intel.gkl.compression.IntelDeflaterFactory"),
    erased_object_strategy("IntelDeflaterFactory"),
    erased_object_strategy("com.intel.gkl.compression.IntelInflaterFactory"),
    erased_object_strategy("IntelInflaterFactory"),
    erased_object_strategy("com.intel.gkl.pdhmm.IntelPDHMM"),
    erased_object_strategy("IntelPDHMM"),
    erased_object_strategy("com.intel.gkl.pairhmm.IntelPairHmm"),
    erased_object_strategy("IntelPairHmm"),
    erased_object_strategy("com.intel.gkl.pairhmm.IntelPairHmmOMP"),
    erased_object_strategy("IntelPairHmmOMP"),
    erased_object_strategy("com.intel.gkl.pdhmm.PDHMMNativeArguments"),
    erased_object_strategy("PDHMMNativeArguments"),
    erased_object_strategy("com.intel.gkl.pairhmm.PairHMMNativeArguments"),
    erased_object_strategy("PairHMMNativeArguments"),
    erased_object_strategy("com.intel.gkl.pairhmm.IntelPairHmm.ReadDataHolder"),
    erased_object_strategy("ReadDataHolder"),
    erased_object_strategy("com.intel.gkl.pairhmm.IntelPairHmm.HaplotypeDataHolder"),
    erased_object_strategy("HaplotypeDataHolder"),
    erased_object_strategy(
        "com.intel.gkl.pairhmm.IntelPairHmm.BatchCompletionCallbackFunctionArgument",
    ),
    erased_object_strategy("BatchCompletionCallbackFunctionArgument"),
    erased_object_strategy("com.intel.gkl.smithwaterman.SWNativeAlignerResult"),
    erased_object_strategy("SWNativeAlignerResult"),
    erased_object_strategy("com.intel.gkl.pdhmm.AVXLevel"),
    erased_object_strategy("AVXLevel"),
    erased_object_strategy("com.intel.gkl.pdhmm.OpenMPSetting"),
    erased_object_strategy("OpenMPSetting"),
];

const GUAVA_IMMUTABLE_COLLECT_OBJECT_STRATEGIES: &[ExternalPackObjectStrategy] = &[
    erased_object_strategy("com.google.common.collect.ImmutableMap.Builder"),
    erased_object_strategy("ImmutableMap.Builder"),
    erased_object_strategy("com.google.common.collect.AbstractIterator"),
    erased_object_strategy("AbstractIterator"),
];

const GUAVA_CONCURRENT_OBJECT_STRATEGIES: &[ExternalPackObjectStrategy] = &[
    erased_object_strategy("com.google.common.util.concurrent.ThreadFactoryBuilder"),
    erased_object_strategy("ThreadFactoryBuilder"),
];

const BROAD_HTTP_OBJECT_STRATEGIES: &[ExternalPackObjectStrategy] = &[
    erased_object_strategy("org.broadinstitute.http.nio.HttpFileSystemProviderSettings"),
    erased_object_strategy("HttpFileSystemProviderSettings"),
    erased_object_strategy(
        "org.broadinstitute.http.nio.HttpFileSystemProviderSettings.RetrySettings",
    ),
    erased_object_strategy("HttpFileSystemProviderSettings.RetrySettings"),
    erased_object_strategy("RetrySettings"),
    erased_object_strategy("org.broadinstitute.http.nio.RetryHandler"),
    erased_object_strategy("RetryHandler"),
    erased_object_strategy("org.broadinstitute.http.nio.HttpFileSystemProvider"),
    erased_object_strategy("HttpFileSystemProvider"),
    erased_object_strategy("org.broadinstitute.http.nio.HttpsFileSystemProvider"),
    erased_object_strategy("HttpsFileSystemProvider"),
];

const PICARD_VCF_OBJECT_STRATEGIES: &[ExternalPackObjectStrategy] = &[
    erased_object_strategy("picard.vcf.GenotypeConcordanceStates"),
    erased_object_strategy("GenotypeConcordanceStates"),
    erased_object_strategy("picard.vcf.GenotypeConcordance"),
    erased_object_strategy("GenotypeConcordance"),
    erased_object_strategy("picard.vcf.GenotypeConcordanceCounts"),
    erased_object_strategy("GenotypeConcordanceCounts"),
    erased_object_strategy("picard.vcf.GenotypeConcordanceSummaryMetrics"),
    erased_object_strategy("GenotypeConcordanceSummaryMetrics"),
    erased_object_strategy("picard.vcf.GenotypeConcordanceStates.TruthAndCallStates"),
    erased_object_strategy("GenotypeConcordanceStates.TruthAndCallStates"),
    erased_object_strategy("TruthAndCallStates"),
    erased_object_strategy("picard.vcf.GenotypeConcordanceStates.TruthState"),
    erased_object_strategy("GenotypeConcordanceStates.TruthState"),
    erased_object_strategy("TruthState"),
    erased_object_strategy("picard.vcf.GenotypeConcordanceStates.CallState"),
    erased_object_strategy("GenotypeConcordanceStates.CallState"),
    erased_object_strategy("CallState"),
];
const JACKSON_DATABIND_OBJECT_STRATEGIES: &[ExternalPackObjectStrategy] = &[
    erased_object_strategy("com.fasterxml.jackson.databind.MapperFeature"),
    erased_object_strategy("MapperFeature"),
    erased_object_strategy("com.fasterxml.jackson.databind.DeserializationFeature"),
    erased_object_strategy("DeserializationFeature"),
];

const BCF_OBJECT_STRATEGIES: &[ExternalPackObjectStrategy] = &[
    erased_object_strategy("htsjdk.variant.bcf2.BCF2Codec"),
    erased_object_strategy("BCF2Codec"),
];

const SAM_HEADER_OBJECT_STRATEGIES: &[ExternalPackObjectStrategy] = &[
    erased_object_strategy("htsjdk.samtools.SAMSequenceDictionary"),
    erased_object_strategy("SAMSequenceDictionary"),
    erased_object_strategy("htsjdk.samtools.SAMSequenceRecord"),
    erased_object_strategy("SAMSequenceRecord"),
    erased_object_strategy("htsjdk.samtools.SAMFileHeader"),
    erased_object_strategy("SAMFileHeader"),
    erased_object_strategy("htsjdk.samtools.SAMReadGroupRecord"),
    erased_object_strategy("SAMReadGroupRecord"),
    erased_object_strategy("htsjdk.samtools.SAMProgramRecord"),
    erased_object_strategy("SAMProgramRecord"),
    erased_object_strategy("htsjdk.samtools.SAMTextHeaderCodec"),
    erased_object_strategy("SAMTextHeaderCodec"),
    erased_object_strategy("htsjdk.samtools.SAMFormatException"),
    erased_object_strategy("SAMFormatException"),
    erased_object_strategy("htsjdk.samtools.SAMFileHeader.SortOrder"),
    erased_object_strategy("SAMFileHeader.SortOrder"),
    erased_object_strategy("SortOrder"),
    erased_object_strategy("htsjdk.samtools.SAMFileHeader.GroupOrder"),
    erased_object_strategy("SAMFileHeader.GroupOrder"),
    erased_object_strategy("GroupOrder"),
];

const SAM_RECORD_OBJECT_STRATEGIES: &[ExternalPackObjectStrategy] = &[
    erased_object_strategy("htsjdk.samtools.SAMRecord"),
    erased_object_strategy("SAMRecord"),
    erased_object_strategy("htsjdk.samtools.SAMTagAndValue"),
    erased_object_strategy("SAMTagAndValue"),
    erased_object_strategy("htsjdk.samtools.SAMBinaryTagAndValue"),
    erased_object_strategy("SAMBinaryTagAndValue"),
    erased_object_strategy("htsjdk.samtools.Cigar"),
    erased_object_strategy("Cigar"),
    erased_object_strategy("htsjdk.samtools.CigarElement"),
    erased_object_strategy("CigarElement"),
    erased_object_strategy("htsjdk.samtools.CigarOperator"),
    erased_object_strategy("CigarOperator"),
    erased_object_strategy("htsjdk.samtools.DefaultSAMRecordFactory"),
    erased_object_strategy("DefaultSAMRecordFactory"),
    erased_object_strategy("htsjdk.samtools.SAMRecordCoordinateComparator"),
    erased_object_strategy("SAMRecordCoordinateComparator"),
    erased_object_strategy("htsjdk.samtools.SAMRecordQueryNameComparator"),
    erased_object_strategy("SAMRecordQueryNameComparator"),
    erased_object_strategy("htsjdk.samtools.filter.SamRecordFilter"),
    erased_object_strategy("SamRecordFilter"),
    erased_object_strategy("htsjdk.samtools.filter.FilteringSamIterator"),
    erased_object_strategy("FilteringSamIterator"),
];

const SAM_REFERENCE_OBJECT_STRATEGIES: &[ExternalPackObjectStrategy] = &[
    erased_object_strategy("htsjdk.samtools.reference.FastaSequenceFile"),
    erased_object_strategy("FastaSequenceFile"),
    erased_object_strategy("htsjdk.samtools.reference.IndexedFastaSequenceFile"),
    erased_object_strategy("IndexedFastaSequenceFile"),
    erased_object_strategy("htsjdk.samtools.reference.FastaReferenceWriterBuilder"),
    erased_object_strategy("FastaReferenceWriterBuilder"),
    erased_object_strategy("htsjdk.samtools.reference.ReferenceSequence"),
    erased_object_strategy("ReferenceSequence"),
    erased_object_strategy("htsjdk.samtools.reference.ReferenceSequenceFileWalker"),
    erased_object_strategy("ReferenceSequenceFileWalker"),
];

const SAM_READER_OBJECT_STRATEGIES: &[ExternalPackObjectStrategy] = &[
    erased_object_strategy("htsjdk.samtools.SamReaderFactory"),
    erased_object_strategy("SamReaderFactory"),
    erased_object_strategy("htsjdk.samtools.SamReaderFactory.Option"),
    erased_object_strategy("SamReaderFactory.Option"),
    erased_object_strategy("htsjdk.samtools.ValidationStringency"),
    erased_object_strategy("ValidationStringency"),
    erased_object_strategy("htsjdk.samtools.SamFileHeaderMerger"),
    erased_object_strategy("SamFileHeaderMerger"),
    erased_object_strategy("htsjdk.samtools.MergingSamRecordIterator"),
    erased_object_strategy("MergingSamRecordIterator"),
];

const SAM_WRITER_OBJECT_STRATEGIES: &[ExternalPackObjectStrategy] = &[
    erased_object_strategy("htsjdk.samtools.SAMFileWriterFactory"),
    erased_object_strategy("SAMFileWriterFactory"),
];

const SAM_BAM_OBJECT_STRATEGIES: &[ExternalPackObjectStrategy] = &[
    erased_object_strategy("htsjdk.samtools.BAMRecordCodec"),
    erased_object_strategy("BAMRecordCodec"),
    erased_object_strategy("htsjdk.samtools.BAMIndexer"),
    erased_object_strategy("BAMIndexer"),
    erased_object_strategy("htsjdk.samtools.SBIIndexWriter"),
    erased_object_strategy("SBIIndexWriter"),
    erased_object_strategy("htsjdk.samtools.BAMFileConstants"),
    erased_object_strategy("BAMFileConstants"),
];

const SAM_CRAM_OBJECT_STRATEGIES: &[ExternalPackObjectStrategy] = &[
    erased_object_strategy("htsjdk.samtools.cram.build.CramContainerIterator"),
    erased_object_strategy("CramContainerIterator"),
    erased_object_strategy("htsjdk.samtools.cram.structure.Container"),
    erased_object_strategy("Container"),
    erased_object_strategy("htsjdk.samtools.cram.structure.DataSeriesType"),
    erased_object_strategy("DataSeriesType"),
    erased_object_strategy("htsjdk.samtools.cram.ref.CRAMReferenceRegion"),
    erased_object_strategy("CRAMReferenceRegion"),
];

const SAM_METRICS_OBJECT_STRATEGIES: &[ExternalPackObjectStrategy] = &[
    erased_object_strategy("htsjdk.samtools.metrics.MetricsFile"),
    erased_object_strategy("MetricsFile"),
];

const HTSJDK_NESTED_HELPER_OBJECT_STRATEGIES: &[ExternalPackObjectStrategy] =
    &[erased_object_strategy("ComponentMethods")];

const TRIBBLE_FEATURE_OBJECT_STRATEGIES: &[ExternalPackObjectStrategy] = &[
    erased_object_strategy("htsjdk.tribble.Feature"),
    erased_object_strategy("Feature"),
    erased_object_strategy("htsjdk.tribble.SimpleFeature"),
    erased_object_strategy("SimpleFeature"),
    erased_object_strategy("htsjdk.tribble.NamedFeature"),
    erased_object_strategy("NamedFeature"),
    erased_object_strategy("htsjdk.tribble.FeatureReader"),
    erased_object_strategy("FeatureReader"),
    erased_object_strategy("htsjdk.tribble.bed.BEDFeature"),
    erased_object_strategy("BEDFeature"),
    erased_object_strategy("htsjdk.tribble.FeatureCodecHeader"),
    erased_object_strategy("FeatureCodecHeader"),
    erased_object_strategy("htsjdk.tribble.readers.CodecLineParsingException"),
    erased_object_strategy("CodecLineParsingException"),
    erased_object_strategy("htsjdk.tribble.gff.Gff3BaseData"),
    erased_object_strategy("Gff3BaseData"),
];

const TRIBBLE_INDEX_OBJECT_STRATEGIES: &[ExternalPackObjectStrategy] = &[
    erased_object_strategy("htsjdk.tribble.index.IndexFactory"),
    erased_object_strategy("IndexFactory"),
    erased_object_strategy("htsjdk.tribble.index.IndexFactory.IndexType"),
    erased_object_strategy("IndexFactory.IndexType"),
    erased_object_strategy("IndexType"),
    erased_object_strategy("htsjdk.tribble.index.IndexFactory.IndexBalanceApproach"),
    erased_object_strategy("IndexFactory.IndexBalanceApproach"),
    erased_object_strategy("IndexBalanceApproach"),
    erased_object_strategy("htsjdk.tribble.index.tabix.TabixFormat"),
    erased_object_strategy("TabixFormat"),
];

const SAMTOOLS_UTIL_OBJECT_STRATEGIES: &[ExternalPackObjectStrategy] = &[
    concrete_object_strategy("htsjdk.samtools.util.Locatable"),
    concrete_object_strategy("Locatable"),
    erased_object_strategy("htsjdk.samtools.util.Interval"),
    erased_object_strategy("Interval"),
    erased_object_strategy("htsjdk.samtools.util.IntervalList"),
    erased_object_strategy("IntervalList"),
    erased_object_strategy("htsjdk.samtools.util.IntervalTree"),
    erased_object_strategy("IntervalTree"),
    erased_object_strategy("htsjdk.samtools.util.IntervalMergerIterator"),
    erased_object_strategy("IntervalMergerIterator"),
    erased_object_strategy("htsjdk.samtools.util.Histogram"),
    erased_object_strategy("Histogram"),
    erased_object_strategy("htsjdk.samtools.util.PeekableIterator"),
    erased_object_strategy("PeekableIterator"),
    erased_object_strategy("htsjdk.samtools.util.DelegatingIterator"),
    erased_object_strategy("DelegatingIterator"),
    erased_object_strategy("htsjdk.samtools.util.MergingIterator"),
    erased_object_strategy("MergingIterator"),
    erased_object_strategy("htsjdk.samtools.util.CloseableIterator"),
    erased_object_strategy("CloseableIterator"),
    erased_object_strategy("htsjdk.samtools.util.RuntimeIOException"),
    erased_object_strategy("RuntimeIOException"),
    erased_object_strategy("htsjdk.samtools.util.DefaultingMap"),
    erased_object_strategy("DefaultingMap"),
    erased_object_strategy("htsjdk.samtools.util.Lazy"),
    erased_object_strategy("Lazy"),
    erased_object_strategy("htsjdk.samtools.seekablestream.SeekableBufferedStream"),
    erased_object_strategy("SeekableBufferedStream"),
    erased_object_strategy("htsjdk.samtools.seekablestream.SeekablePathStream"),
    erased_object_strategy("SeekablePathStream"),
    erased_object_strategy("htsjdk.samtools.util.BinaryCodec"),
    erased_object_strategy("BinaryCodec"),
    erased_object_strategy("htsjdk.samtools.util.BufferedLineReader"),
    erased_object_strategy("BufferedLineReader"),
    erased_object_strategy("htsjdk.samtools.util.AsciiLineReaderIterator"),
    erased_object_strategy("AsciiLineReaderIterator"),
    erased_object_strategy("htsjdk.samtools.util.LineIteratorImpl"),
    erased_object_strategy("LineIteratorImpl"),
    erased_object_strategy("htsjdk.samtools.util.SynchronousLineReader"),
    erased_object_strategy("SynchronousLineReader"),
    erased_object_strategy("htsjdk.samtools.Defaults"),
    erased_object_strategy("Defaults"),
    erased_object_strategy("htsjdk.samtools.SAMUtils"),
    erased_object_strategy("SAMUtils"),
    erased_object_strategy("htsjdk.samtools.ReservedTagConstants"),
    erased_object_strategy("ReservedTagConstants"),
    erased_object_strategy("htsjdk.samtools.fastq.FastqConstants"),
    erased_object_strategy("FastqConstants"),
    erased_object_strategy("htsjdk.samtools.util.SequenceUtil"),
    erased_object_strategy("SequenceUtil"),
    erased_object_strategy("htsjdk.samtools.util.BlockCompressedStreamConstants"),
    erased_object_strategy("BlockCompressedStreamConstants"),
    erased_object_strategy("htsjdk.samtools.util.BlockCompressedInputStream"),
    erased_object_strategy("BlockCompressedInputStream"),
    erased_object_strategy("htsjdk.samtools.util.BlockCompressedInputStream.FileTermination"),
    erased_object_strategy("BlockCompressedInputStream.FileTermination"),
    erased_object_strategy("FileTermination"),
    erased_object_strategy("htsjdk.samtools.util.BlockCompressedOutputStream"),
    erased_object_strategy("BlockCompressedOutputStream"),
    erased_object_strategy("htsjdk.samtools.util.Log"),
    erased_object_strategy("Log"),
    erased_object_strategy("htsjdk.samtools.util.Log.LogLevel"),
    erased_object_strategy("Log.LogLevel"),
    erased_object_strategy("LogLevel"),
    erased_object_strategy("htsjdk.samtools.SamPairUtil"),
    erased_object_strategy("SamPairUtil"),
    erased_object_strategy("htsjdk.samtools.SamPairUtil.PairOrientation"),
    erased_object_strategy("SamPairUtil.PairOrientation"),
    erased_object_strategy("PairOrientation"),
];

const VARIANTCONTEXT_MODELED_CONSTRUCTORS: &[ExternalPackModeledConstructor] = &[
    modeled_semantic_constructor("htsjdk.variant.variantcontext.VariantContextComparator", 1),
    modeled_semantic_constructor("VariantContextComparator", 1),
    modeled_semantic_constructor("htsjdk.variant.variantcontext.VariantContextBuilder", 0),
    modeled_semantic_constructor("htsjdk.variant.variantcontext.VariantContextBuilder", 1),
    modeled_semantic_constructor("htsjdk.variant.variantcontext.VariantContextBuilder", 5),
    modeled_semantic_constructor("htsjdk.variant.variantcontext.GenotypeBuilder", 0),
    modeled_semantic_constructor("htsjdk.variant.variantcontext.GenotypeBuilder", 1),
    modeled_semantic_constructor("htsjdk.variant.variantcontext.GenotypeBuilder", 2),
    modeled_semantic_constructor("VariantContextBuilder", 0),
    modeled_semantic_constructor("VariantContextBuilder", 1),
    modeled_semantic_constructor("VariantContextBuilder", 5),
    modeled_semantic_constructor("GenotypeBuilder", 0),
    modeled_semantic_constructor("GenotypeBuilder", 1),
    modeled_semantic_constructor("GenotypeBuilder", 2),
];

const HDF5_MODELED_CONSTRUCTORS: &[ExternalPackModeledConstructor] = &[
    modeled_mapped_constructor("HDF5Library", 0),
    modeled_mapped_constructor("HDF5File", 1),
    modeled_mapped_constructor("HDF5File", 2),
];

const SQLITE_MODELED_CONSTRUCTORS: &[ExternalPackModeledConstructor] = &[
    modeled_mapped_constructor("org.sqlite.SQLiteConfig", 0),
    modeled_mapped_constructor("SQLiteConfig", 0),
];

const VCF_MODELED_CONSTRUCTORS: &[ExternalPackModeledConstructor] = &[
    modeled_semantic_constructor("htsjdk.variant.vcf.VCFFileReader", 1),
    modeled_semantic_constructor("VCFFileReader", 1),
    modeled_semantic_constructor("htsjdk.variant.vcf.VCFSimpleHeaderLine", 2),
    modeled_semantic_constructor("VCFSimpleHeaderLine", 2),
    modeled_semantic_constructor("htsjdk.variant.vcf.VCFSimpleHeaderLine", 3),
    modeled_semantic_constructor("VCFSimpleHeaderLine", 3),
    modeled_semantic_constructor("htsjdk.variant.vcf.VCFInfoHeaderLine", 3),
    modeled_semantic_constructor("htsjdk.variant.vcf.VCFInfoHeaderLine", 4),
    modeled_semantic_constructor("VCFInfoHeaderLine", 3),
    modeled_semantic_constructor("VCFInfoHeaderLine", 4),
    modeled_semantic_constructor("htsjdk.variant.vcf.VCFFormatHeaderLine", 3),
    modeled_semantic_constructor("htsjdk.variant.vcf.VCFFormatHeaderLine", 4),
    modeled_semantic_constructor("VCFFormatHeaderLine", 3),
    modeled_semantic_constructor("VCFFormatHeaderLine", 4),
    modeled_semantic_constructor("htsjdk.variant.vcf.VCFFilterHeaderLine", 2),
    modeled_semantic_constructor("VCFFilterHeaderLine", 2),
    modeled_semantic_constructor("htsjdk.variant.vcf.VCFHeaderLine", 2),
    modeled_semantic_constructor("VCFHeaderLine", 2),
    modeled_semantic_constructor("htsjdk.variant.vcf.VCFHeader", 0),
    modeled_semantic_constructor("htsjdk.variant.vcf.VCFHeader", 1),
    modeled_semantic_constructor("htsjdk.variant.vcf.VCFHeader", 2),
    modeled_semantic_constructor("VCFHeader", 0),
    modeled_semantic_constructor("VCFHeader", 1),
    modeled_semantic_constructor("VCFHeader", 2),
    modeled_semantic_constructor("htsjdk.variant.vcf.VCFCodec", 0),
    modeled_semantic_constructor("VCFCodec", 0),
    modeled_semantic_constructor("htsjdk.variant.vcf.VCFRecordCodec", 2),
    modeled_semantic_constructor("VCFRecordCodec", 2),
    modeled_semantic_constructor(
        "htsjdk.variant.variantcontext.writer.VariantContextWriterBuilder",
        0,
    ),
    modeled_semantic_constructor("VariantContextWriterBuilder", 0),
];

const BCF_MODELED_CONSTRUCTORS: &[ExternalPackModeledConstructor] = &[
    modeled_semantic_constructor("htsjdk.variant.bcf2.BCF2Codec", 0),
    modeled_semantic_constructor("BCF2Codec", 0),
];

const VCF_MODELED_FIELDS: &[ExternalPackModeledField] = &[
    modeled_semantic_field("htsjdk.variant.vcf.VCFConstants", "PASSES_FILTERS_v4"),
    modeled_semantic_field("VCFConstants", "PASSES_FILTERS_v4"),
    modeled_semantic_field("htsjdk.variant.vcf.VCFConstants", "MISSING_VALUE_v4"),
    modeled_semantic_field("VCFConstants", "MISSING_VALUE_v4"),
    modeled_semantic_field("htsjdk.variant.vcf.VCFConstants", "EMPTY_ID_FIELD"),
    modeled_semantic_field("VCFConstants", "EMPTY_ID_FIELD"),
    modeled_semantic_field("htsjdk.variant.vcf.VCFConstants", "EMPTY_INFO_FIELD"),
    modeled_semantic_field("VCFConstants", "EMPTY_INFO_FIELD"),
    modeled_semantic_field("htsjdk.variant.vcf.VCFConstants", "GENOTYPE_ALLELE_DEPTHS"),
    modeled_semantic_field("VCFConstants", "GENOTYPE_ALLELE_DEPTHS"),
    modeled_semantic_field("htsjdk.variant.vcf.VCFConstants", "SVTYPE"),
    modeled_semantic_field("VCFConstants", "SVTYPE"),
    modeled_semantic_field("htsjdk.variant.vcf.VCFConstants", "NULL_ALLELE"),
    modeled_semantic_field("VCFConstants", "NULL_ALLELE"),
    modeled_semantic_field("htsjdk.variant.vcf.VCFConstants", "UNFILTERED"),
    modeled_semantic_field("VCFConstants", "UNFILTERED"),
    modeled_semantic_field(
        "htsjdk.variant.vcf.VCFConstants",
        "INFO_FIELD_ARRAY_SEPARATOR_CHAR",
    ),
    modeled_semantic_field("VCFConstants", "INFO_FIELD_ARRAY_SEPARATOR_CHAR"),
    modeled_semantic_field("htsjdk.variant.vcf.VCFConstants", "MAX_GENOTYPE_QUAL"),
    modeled_semantic_field("VCFConstants", "MAX_GENOTYPE_QUAL"),
    modeled_semantic_field("htsjdk.variant.vcf.VCFConstants", "DEPTH_KEY"),
    modeled_semantic_field("VCFConstants", "DEPTH_KEY"),
    modeled_semantic_field("htsjdk.variant.vcf.VCFConstants", "ALLELE_NUMBER_KEY"),
    modeled_semantic_field("VCFConstants", "ALLELE_NUMBER_KEY"),
    modeled_semantic_field("htsjdk.variant.vcf.VCFConstants", "ALLELE_COUNT_KEY"),
    modeled_semantic_field("VCFConstants", "ALLELE_COUNT_KEY"),
    modeled_semantic_field("htsjdk.variant.vcf.VCFConstants", "ALLELE_FREQUENCY_KEY"),
    modeled_semantic_field("VCFConstants", "ALLELE_FREQUENCY_KEY"),
    modeled_semantic_field("htsjdk.variant.vcf.VCFConstants", "END_KEY"),
    modeled_semantic_field("VCFConstants", "END_KEY"),
    modeled_semantic_field("htsjdk.variant.vcf.VCFConstants", "GENOTYPE_KEY"),
    modeled_semantic_field("VCFConstants", "GENOTYPE_KEY"),
    modeled_semantic_field("htsjdk.variant.vcf.VCFConstants", "RMS_MAPPING_QUALITY_KEY"),
    modeled_semantic_field("VCFConstants", "RMS_MAPPING_QUALITY_KEY"),
    modeled_semantic_field("htsjdk.variant.vcf.VCFConstants", "DBSNP_KEY"),
    modeled_semantic_field("VCFConstants", "DBSNP_KEY"),
    modeled_semantic_field("htsjdk.variant.vcf.VCFConstants", "GENOTYPE_FILTER_KEY"),
    modeled_semantic_field("VCFConstants", "GENOTYPE_FILTER_KEY"),
    modeled_semantic_field("htsjdk.variant.vcf.VCFConstants", "GENOTYPE_PL_KEY"),
    modeled_semantic_field("VCFConstants", "GENOTYPE_PL_KEY"),
    modeled_semantic_field("htsjdk.variant.vcf.VCFConstants", "GENOTYPE_POSTERIORS_KEY"),
    modeled_semantic_field("VCFConstants", "GENOTYPE_POSTERIORS_KEY"),
    modeled_semantic_field("htsjdk.variant.vcf.VCFConstants", "ID_FIELD_SEPARATOR"),
    modeled_semantic_field("VCFConstants", "ID_FIELD_SEPARATOR"),
    modeled_semantic_field(
        "htsjdk.variant.vcf.VCFConstants",
        "INFO_FIELD_ARRAY_SEPARATOR",
    ),
    modeled_semantic_field("VCFConstants", "INFO_FIELD_ARRAY_SEPARATOR"),
    modeled_semantic_field("htsjdk.variant.vcf.VCFHeader", "HEADER_FIELDS"),
    modeled_semantic_field("VCFHeader", "HEADER_FIELDS"),
    modeled_semantic_field("htsjdk.variant.vcf.VCFHeader", "REFERENCE_KEY"),
    modeled_semantic_field("VCFHeader", "REFERENCE_KEY"),
    modeled_semantic_field(
        "htsjdk.variant.vcf.VCFCompoundHeaderLine",
        "SupportedHeaderLineType",
    ),
    modeled_semantic_field("VCFCompoundHeaderLine", "SupportedHeaderLineType"),
    modeled_semantic_field(
        "htsjdk.variant.vcf.VCFCompoundHeaderLine.SupportedHeaderLineType",
        "FORMAT",
    ),
    modeled_semantic_field("VCFCompoundHeaderLine.SupportedHeaderLineType", "FORMAT"),
    modeled_semantic_field("SupportedHeaderLineType", "FORMAT"),
    modeled_semantic_field(
        "htsjdk.variant.vcf.VCFCompoundHeaderLine.SupportedHeaderLineType",
        "INFO",
    ),
    modeled_semantic_field("VCFCompoundHeaderLine.SupportedHeaderLineType", "INFO"),
    modeled_semantic_field("SupportedHeaderLineType", "INFO"),
    modeled_semantic_field(
        "htsjdk.variant.variantcontext.writer.VariantContextWriterBuilder",
        "OutputType",
    ),
    modeled_semantic_field("VariantContextWriterBuilder", "OutputType"),
    modeled_semantic_field(
        "htsjdk.variant.variantcontext.writer.VariantContextWriterBuilder",
        "DEFAULT_OPTIONS",
    ),
    modeled_semantic_field("VariantContextWriterBuilder", "DEFAULT_OPTIONS"),
    modeled_semantic_field(
        "htsjdk.variant.variantcontext.writer.Options",
        "INDEX_ON_THE_FLY",
    ),
    modeled_semantic_field("Options", "INDEX_ON_THE_FLY"),
    modeled_semantic_field(
        "htsjdk.variant.variantcontext.writer.Options",
        "DO_NOT_WRITE_GENOTYPES",
    ),
    modeled_semantic_field("Options", "DO_NOT_WRITE_GENOTYPES"),
    modeled_semantic_field(
        "htsjdk.variant.variantcontext.writer.Options",
        "ALLOW_MISSING_FIELDS_IN_HEADER",
    ),
    modeled_semantic_field("Options", "ALLOW_MISSING_FIELDS_IN_HEADER"),
];

const VARIANTCONTEXT_MODELED_FIELDS: &[ExternalPackModeledField] = &[
    modeled_semantic_field("htsjdk.variant.variantcontext.Allele", "NO_CALL"),
    modeled_semantic_field("Allele", "NO_CALL"),
    modeled_semantic_field("htsjdk.variant.variantcontext.Allele", "NON_REF_ALLELE"),
    modeled_semantic_field("Allele", "NON_REF_ALLELE"),
    modeled_semantic_field("htsjdk.variant.variantcontext.Allele", "SPAN_DEL"),
    modeled_semantic_field("Allele", "SPAN_DEL"),
    modeled_semantic_field("htsjdk.variant.variantcontext.Allele", "SV_SIMPLE_DEL"),
    modeled_semantic_field("Allele", "SV_SIMPLE_DEL"),
    modeled_semantic_field("htsjdk.variant.variantcontext.Allele", "SV_SIMPLE_DUP"),
    modeled_semantic_field("Allele", "SV_SIMPLE_DUP"),
    modeled_semantic_field(
        "htsjdk.variant.variantcontext.Allele",
        "UNSPECIFIED_ALTERNATE_ALLELE",
    ),
    modeled_semantic_field("Allele", "UNSPECIFIED_ALTERNATE_ALLELE"),
    modeled_semantic_field("htsjdk.variant.variantcontext.Allele", "NON_REF_STRING"),
    modeled_semantic_field("Allele", "NON_REF_STRING"),
    modeled_semantic_field(
        "htsjdk.variant.variantcontext.Allele",
        "UNSPECIFIED_ALTERNATE_ALLELE_STRING",
    ),
    modeled_semantic_field("Allele", "UNSPECIFIED_ALTERNATE_ALLELE_STRING"),
    modeled_semantic_field(
        "htsjdk.variant.variantcontext.GenotypeLikelihoods",
        "MAX_DIPLOID_ALT_ALLELES_THAT_CAN_BE_GENOTYPED",
    ),
    modeled_semantic_field(
        "GenotypeLikelihoods",
        "MAX_DIPLOID_ALT_ALLELES_THAT_CAN_BE_GENOTYPED",
    ),
    modeled_semantic_field(
        "htsjdk.variant.variantcontext.GenotypesContext",
        "NO_GENOTYPES",
    ),
    modeled_semantic_field("GenotypesContext", "NO_GENOTYPES"),
    modeled_semantic_field("htsjdk.variant.variantcontext.VariantContext", "Type"),
    modeled_semantic_field("VariantContext", "Type"),
    modeled_semantic_field(
        "htsjdk.variant.variantcontext.CommonInfo",
        "NO_LOG10_PERROR",
    ),
    modeled_semantic_field("CommonInfo", "NO_LOG10_PERROR"),
    modeled_semantic_field(
        "htsjdk.variant.variantcontext.VariantContext",
        "NO_LOG10_PERROR",
    ),
    modeled_semantic_field("VariantContext", "NO_LOG10_PERROR"),
    modeled_semantic_field(
        "htsjdk.variant.variantcontext.Genotype",
        "UNPHASED_ALLELE_SEPARATOR",
    ),
    modeled_semantic_field("Genotype", "UNPHASED_ALLELE_SEPARATOR"),
    modeled_semantic_field("htsjdk.variant.variantcontext.VariantContext.Type", "INDEL"),
    modeled_semantic_field("VariantContext.Type", "INDEL"),
    modeled_semantic_field("Type", "INDEL"),
    modeled_semantic_field("htsjdk.variant.variantcontext.VariantContext.Type", "SNP"),
    modeled_semantic_field("VariantContext.Type", "SNP"),
    modeled_semantic_field("Type", "SNP"),
    modeled_semantic_field("htsjdk.variant.variantcontext.VariantContext.Type", "MNP"),
    modeled_semantic_field("VariantContext.Type", "MNP"),
    modeled_semantic_field("Type", "MNP"),
    modeled_semantic_field(
        "htsjdk.variant.variantcontext.VariantContext.Type",
        "SYMBOLIC",
    ),
    modeled_semantic_field("VariantContext.Type", "SYMBOLIC"),
    modeled_semantic_field("Type", "SYMBOLIC"),
    modeled_semantic_field(
        "htsjdk.variant.variantcontext.VariantContext.Type",
        "NO_VARIATION",
    ),
    modeled_semantic_field("VariantContext.Type", "NO_VARIATION"),
    modeled_semantic_field("Type", "NO_VARIATION"),
    modeled_semantic_field(
        "htsjdk.variant.variantcontext.GenotypeLikelihoodsAllelePair",
        "alleleIndex1",
    ),
    modeled_semantic_field("GenotypeLikelihoodsAllelePair", "alleleIndex1"),
    modeled_semantic_field(
        "htsjdk.variant.variantcontext.GenotypeLikelihoodsAllelePair",
        "alleleIndex2",
    ),
    modeled_semantic_field("GenotypeLikelihoodsAllelePair", "alleleIndex2"),
    modeled_semantic_field("htsjdk.variant.variantcontext.JexlVCMatchExp", "name"),
    modeled_semantic_field("JexlVCMatchExp", "name"),
    modeled_semantic_field("htsjdk.variant.variantcontext.JexlVCMatchExp", "exp"),
    modeled_semantic_field("JexlVCMatchExp", "exp"),
    modeled_semantic_field(
        "htsjdk.variant.variantcontext.JexlMissingValueTreatment",
        "TREAT_AS_MATCH",
    ),
    modeled_semantic_field("JexlMissingValueTreatment", "TREAT_AS_MATCH"),
    modeled_semantic_field(
        "htsjdk.variant.variantcontext.JexlMissingValueTreatment",
        "TREAT_AS_MISMATCH",
    ),
    modeled_semantic_field("JexlMissingValueTreatment", "TREAT_AS_MISMATCH"),
];

const SAM_HEADER_MODELED_CONSTRUCTORS: &[ExternalPackModeledConstructor] = &[
    modeled_semantic_constructor("htsjdk.samtools.SAMSequenceDictionary", 0),
    modeled_semantic_constructor("htsjdk.samtools.SAMSequenceDictionary", 1),
    modeled_semantic_constructor("SAMSequenceDictionary", 0),
    modeled_semantic_constructor("SAMSequenceDictionary", 1),
    modeled_semantic_constructor("htsjdk.samtools.SAMSequenceRecord", 2),
    modeled_semantic_constructor("SAMSequenceRecord", 2),
    modeled_semantic_constructor("htsjdk.samtools.SAMFileHeader", 0),
    modeled_semantic_constructor("htsjdk.samtools.SAMFileHeader", 1),
    modeled_semantic_constructor("SAMFileHeader", 0),
    modeled_semantic_constructor("SAMFileHeader", 1),
    modeled_semantic_constructor("htsjdk.samtools.SAMReadGroupRecord", 1),
    modeled_semantic_constructor("SAMReadGroupRecord", 1),
    modeled_semantic_constructor("htsjdk.samtools.SAMProgramRecord", 1),
    modeled_semantic_constructor("SAMProgramRecord", 1),
    modeled_semantic_constructor("htsjdk.samtools.SAMTextHeaderCodec", 0),
    modeled_semantic_constructor("SAMTextHeaderCodec", 0),
    modeled_semantic_constructor("htsjdk.samtools.SAMFormatException", 1),
    modeled_semantic_constructor("SAMFormatException", 1),
];

const SAM_RECORD_MODELED_CONSTRUCTORS: &[ExternalPackModeledConstructor] = &[
    modeled_semantic_constructor("htsjdk.samtools.SAMRecord", 1),
    modeled_semantic_constructor("SAMRecord", 1),
    modeled_semantic_constructor("htsjdk.samtools.CigarElement", 2),
    modeled_semantic_constructor("CigarElement", 2),
    modeled_semantic_constructor("htsjdk.samtools.Cigar", 0),
    modeled_semantic_constructor("htsjdk.samtools.Cigar", 1),
    modeled_semantic_constructor("Cigar", 0),
    modeled_semantic_constructor("Cigar", 1),
    modeled_semantic_constructor("htsjdk.samtools.DefaultSAMRecordFactory", 0),
    modeled_semantic_constructor("DefaultSAMRecordFactory", 0),
    modeled_semantic_constructor("htsjdk.samtools.SAMRecordCoordinateComparator", 0),
    modeled_semantic_constructor("SAMRecordCoordinateComparator", 0),
    modeled_semantic_constructor("htsjdk.samtools.SAMRecordQueryNameComparator", 0),
    modeled_semantic_constructor("SAMRecordQueryNameComparator", 0),
    modeled_semantic_constructor("htsjdk.samtools.filter.FilteringSamIterator", 2),
    modeled_semantic_constructor("FilteringSamIterator", 2),
];

const SAM_RECORD_MODELED_FIELDS: &[ExternalPackModeledField] = &[
    modeled_semantic_field("htsjdk.samtools.SAMRecord", "NO_ALIGNMENT_CIGAR"),
    modeled_semantic_field("SAMRecord", "NO_ALIGNMENT_CIGAR"),
    modeled_semantic_field("htsjdk.samtools.SAMRecord", "NO_ALIGNMENT_START"),
    modeled_semantic_field("SAMRecord", "NO_ALIGNMENT_START"),
    modeled_semantic_field("htsjdk.samtools.SAMRecord", "NO_ALIGNMENT_REFERENCE_INDEX"),
    modeled_semantic_field("SAMRecord", "NO_ALIGNMENT_REFERENCE_INDEX"),
    modeled_semantic_field("htsjdk.samtools.SAMRecord", "NO_ALIGNMENT_REFERENCE_NAME"),
    modeled_semantic_field("SAMRecord", "NO_ALIGNMENT_REFERENCE_NAME"),
    modeled_semantic_field("htsjdk.samtools.SAMRecord", "UNKNOWN_MAPPING_QUALITY"),
    modeled_semantic_field("SAMRecord", "UNKNOWN_MAPPING_QUALITY"),
    modeled_semantic_field("htsjdk.samtools.SAMRecord", "NO_MAPPING_QUALITY"),
    modeled_semantic_field("SAMRecord", "NO_MAPPING_QUALITY"),
    modeled_semantic_field("htsjdk.samtools.SAMRecord", "NULL_QUALS"),
    modeled_semantic_field("SAMRecord", "NULL_QUALS"),
    modeled_semantic_field("htsjdk.samtools.SAMRecord", "NULL_QUALS_STRING"),
    modeled_semantic_field("SAMRecord", "NULL_QUALS_STRING"),
    modeled_semantic_field("htsjdk.samtools.SAMRecord", "NULL_SEQUENCE"),
    modeled_semantic_field("SAMRecord", "NULL_SEQUENCE"),
    modeled_semantic_field("htsjdk.samtools.SAMTagAndValue", "tag"),
    modeled_semantic_field("SAMTagAndValue", "tag"),
    modeled_semantic_field("htsjdk.samtools.SAMTagAndValue", "value"),
    modeled_semantic_field("SAMTagAndValue", "value"),
    modeled_semantic_field("htsjdk.samtools.SAMBinaryTagAndValue", "tag"),
    modeled_semantic_field("SAMBinaryTagAndValue", "tag"),
    modeled_semantic_field("htsjdk.samtools.SAMBinaryTagAndValue", "value"),
    modeled_semantic_field("SAMBinaryTagAndValue", "value"),
    modeled_semantic_field("htsjdk.samtools.CigarOperator", "M"),
    modeled_semantic_field("CigarOperator", "M"),
    modeled_semantic_field("htsjdk.samtools.CigarOperator", "I"),
    modeled_semantic_field("CigarOperator", "I"),
    modeled_semantic_field("htsjdk.samtools.CigarOperator", "D"),
    modeled_semantic_field("CigarOperator", "D"),
    modeled_semantic_field("htsjdk.samtools.CigarOperator", "N"),
    modeled_semantic_field("CigarOperator", "N"),
    modeled_semantic_field("htsjdk.samtools.CigarOperator", "S"),
    modeled_semantic_field("CigarOperator", "S"),
    modeled_semantic_field("htsjdk.samtools.CigarOperator", "H"),
    modeled_semantic_field("CigarOperator", "H"),
    modeled_semantic_field("htsjdk.samtools.CigarOperator", "P"),
    modeled_semantic_field("CigarOperator", "P"),
    modeled_semantic_field("htsjdk.samtools.CigarOperator", "EQ"),
    modeled_semantic_field("CigarOperator", "EQ"),
    modeled_semantic_field("htsjdk.samtools.CigarOperator", "X"),
    modeled_semantic_field("CigarOperator", "X"),
    modeled_semantic_field("htsjdk.samtools.CigarOperator", "MATCH"),
    modeled_semantic_field("CigarOperator", "MATCH"),
    modeled_semantic_field("htsjdk.samtools.CigarOperator", "MATCH_OR_MISMATCH"),
    modeled_semantic_field("CigarOperator", "MATCH_OR_MISMATCH"),
    modeled_semantic_field("htsjdk.samtools.CigarOperator", "INSERTION"),
    modeled_semantic_field("CigarOperator", "INSERTION"),
    modeled_semantic_field("htsjdk.samtools.CigarOperator", "DELETION"),
    modeled_semantic_field("CigarOperator", "DELETION"),
    modeled_semantic_field("htsjdk.samtools.CigarOperator", "SKIPPED_REGION"),
    modeled_semantic_field("CigarOperator", "SKIPPED_REGION"),
    modeled_semantic_field("htsjdk.samtools.CigarOperator", "SOFT_CLIP"),
    modeled_semantic_field("CigarOperator", "SOFT_CLIP"),
    modeled_semantic_field("htsjdk.samtools.CigarOperator", "HARD_CLIP"),
    modeled_semantic_field("CigarOperator", "HARD_CLIP"),
    modeled_semantic_field("htsjdk.samtools.CigarOperator", "PADDING"),
    modeled_semantic_field("CigarOperator", "PADDING"),
];

const SAM_REFERENCE_MODELED_CONSTRUCTORS: &[ExternalPackModeledConstructor] = &[
    modeled_semantic_constructor("htsjdk.samtools.reference.FastaSequenceFile", 2),
    modeled_semantic_constructor("FastaSequenceFile", 2),
    modeled_semantic_constructor("htsjdk.samtools.reference.IndexedFastaSequenceFile", 1),
    modeled_semantic_constructor("IndexedFastaSequenceFile", 1),
    modeled_semantic_constructor("htsjdk.samtools.reference.FastaReferenceWriterBuilder", 0),
    modeled_semantic_constructor("FastaReferenceWriterBuilder", 0),
    modeled_semantic_constructor("htsjdk.samtools.reference.ReferenceSequence", 3),
    modeled_semantic_constructor("ReferenceSequence", 3),
    modeled_semantic_constructor("htsjdk.samtools.reference.ReferenceSequenceFileWalker", 1),
    modeled_semantic_constructor("ReferenceSequenceFileWalker", 1),
];

const TRIBBLE_FEATURE_MODELED_CONSTRUCTORS: &[ExternalPackModeledConstructor] = &[
    modeled_semantic_constructor("htsjdk.tribble.SimpleFeature", 3),
    modeled_semantic_constructor("SimpleFeature", 3),
    modeled_semantic_constructor("htsjdk.tribble.FeatureCodecHeader", 2),
    modeled_semantic_constructor("FeatureCodecHeader", 2),
    modeled_semantic_constructor("htsjdk.tribble.readers.CodecLineParsingException", 1),
    modeled_semantic_constructor("CodecLineParsingException", 1),
    modeled_semantic_constructor("htsjdk.tribble.gff.Gff3BaseData", 9),
    modeled_semantic_constructor("Gff3BaseData", 9),
];

const TRIBBLE_FEATURE_MODELED_FIELDS: &[ExternalPackModeledField] = &[
    modeled_semantic_field("htsjdk.tribble.FeatureCodecHeader", "NO_HEADER_END"),
    modeled_semantic_field("FeatureCodecHeader", "NO_HEADER_END"),
];

const SAMTOOLS_UTIL_MODELED_CONSTRUCTORS: &[ExternalPackModeledConstructor] = &[
    modeled_semantic_constructor("htsjdk.samtools.util.BlockCompressedInputStream", 1),
    modeled_semantic_constructor("htsjdk.samtools.util.BlockCompressedInputStream", 2),
    modeled_semantic_constructor("BlockCompressedInputStream", 1),
    modeled_semantic_constructor("BlockCompressedInputStream", 2),
    modeled_semantic_constructor("htsjdk.samtools.util.BlockCompressedOutputStream", 2),
    modeled_semantic_constructor("htsjdk.samtools.util.BlockCompressedOutputStream", 3),
    modeled_semantic_constructor("BlockCompressedOutputStream", 2),
    modeled_semantic_constructor("BlockCompressedOutputStream", 3),
    modeled_semantic_constructor("htsjdk.samtools.util.Interval", 1),
    modeled_semantic_constructor("htsjdk.samtools.util.Interval", 2),
    modeled_semantic_constructor("htsjdk.samtools.util.Interval", 3),
    modeled_semantic_constructor("htsjdk.samtools.util.Interval", 5),
    modeled_semantic_constructor("Interval", 1),
    modeled_semantic_constructor("Interval", 2),
    modeled_semantic_constructor("Interval", 3),
    modeled_semantic_constructor("Interval", 5),
    modeled_semantic_constructor("htsjdk.samtools.util.IntervalList", 1),
    modeled_semantic_constructor("IntervalList", 1),
    modeled_semantic_constructor("htsjdk.samtools.util.IntervalTree", 0),
    modeled_semantic_constructor("IntervalTree", 0),
    modeled_semantic_constructor("htsjdk.samtools.util.IntervalMergerIterator", 4),
    modeled_semantic_constructor("IntervalMergerIterator", 4),
    modeled_semantic_constructor("htsjdk.samtools.util.Histogram", 0),
    modeled_semantic_constructor("htsjdk.samtools.util.Histogram", 2),
    modeled_semantic_constructor("Histogram", 0),
    modeled_semantic_constructor("Histogram", 2),
    modeled_semantic_constructor("htsjdk.samtools.util.PeekableIterator", 1),
    modeled_semantic_constructor("PeekableIterator", 1),
    modeled_semantic_constructor("htsjdk.samtools.util.DelegatingIterator", 1),
    modeled_semantic_constructor("DelegatingIterator", 1),
    modeled_semantic_constructor("htsjdk.samtools.util.MergingIterator", 2),
    modeled_semantic_constructor("MergingIterator", 2),
    modeled_semantic_constructor("htsjdk.samtools.util.RuntimeIOException", 1),
    modeled_semantic_constructor("htsjdk.samtools.util.RuntimeIOException", 2),
    modeled_semantic_constructor("RuntimeIOException", 1),
    modeled_semantic_constructor("RuntimeIOException", 2),
    modeled_semantic_constructor("htsjdk.samtools.util.DefaultingMap", 2),
    modeled_semantic_constructor("DefaultingMap", 2),
    modeled_semantic_constructor("htsjdk.samtools.util.Lazy", 1),
    modeled_semantic_constructor("Lazy", 1),
    modeled_semantic_constructor("htsjdk.samtools.seekablestream.SeekableBufferedStream", 1),
    modeled_semantic_constructor("SeekableBufferedStream", 1),
    modeled_semantic_constructor("htsjdk.samtools.seekablestream.SeekablePathStream", 1),
    modeled_semantic_constructor("SeekablePathStream", 1),
    modeled_semantic_constructor("htsjdk.samtools.util.BinaryCodec", 0),
    modeled_semantic_constructor("htsjdk.samtools.util.BinaryCodec", 1),
    modeled_semantic_constructor("BinaryCodec", 0),
    modeled_semantic_constructor("BinaryCodec", 1),
    modeled_semantic_constructor("htsjdk.samtools.util.BufferedLineReader", 1),
    modeled_semantic_constructor("BufferedLineReader", 1),
    modeled_semantic_constructor("htsjdk.samtools.util.AsciiLineReaderIterator", 1),
    modeled_semantic_constructor("AsciiLineReaderIterator", 1),
    modeled_semantic_constructor("htsjdk.samtools.util.LineIteratorImpl", 1),
    modeled_semantic_constructor("LineIteratorImpl", 1),
    modeled_semantic_constructor("htsjdk.samtools.util.SynchronousLineReader", 1),
    modeled_semantic_constructor("SynchronousLineReader", 1),
];

const SAMTOOLS_UTIL_MODELED_FIELDS: &[ExternalPackModeledField] = &[
    modeled_semantic_field("htsjdk.samtools.Defaults", "COMPRESSION_LEVEL"),
    modeled_semantic_field("Defaults", "COMPRESSION_LEVEL"),
    modeled_semantic_field("htsjdk.samtools.Defaults", "USE_ASYNC_IO_READ_FOR_SAMTOOLS"),
    modeled_semantic_field("Defaults", "USE_ASYNC_IO_READ_FOR_SAMTOOLS"),
    modeled_semantic_field(
        "htsjdk.samtools.Defaults",
        "USE_ASYNC_IO_WRITE_FOR_SAMTOOLS",
    ),
    modeled_semantic_field("Defaults", "USE_ASYNC_IO_WRITE_FOR_SAMTOOLS"),
    modeled_semantic_field("htsjdk.samtools.Defaults", "USE_ASYNC_IO_WRITE_FOR_TRIBBLE"),
    modeled_semantic_field("Defaults", "USE_ASYNC_IO_WRITE_FOR_TRIBBLE"),
    modeled_semantic_field("htsjdk.samtools.SAMUtils", "MAX_PHRED_SCORE"),
    modeled_semantic_field("SAMUtils", "MAX_PHRED_SCORE"),
    modeled_semantic_field("htsjdk.samtools.ReservedTagConstants", "XT"),
    modeled_semantic_field("ReservedTagConstants", "XT"),
    modeled_semantic_field("htsjdk.samtools.ReservedTagConstants", "PROGRAM_GROUP_ID"),
    modeled_semantic_field("ReservedTagConstants", "PROGRAM_GROUP_ID"),
    modeled_semantic_field("htsjdk.samtools.ReservedTagConstants", "READ_GROUP_ID"),
    modeled_semantic_field("ReservedTagConstants", "READ_GROUP_ID"),
    modeled_semantic_field(
        "htsjdk.samtools.util.BlockCompressedStreamConstants",
        "BLOCK_HEADER_LENGTH",
    ),
    modeled_semantic_field("BlockCompressedStreamConstants", "BLOCK_HEADER_LENGTH"),
    modeled_semantic_field(
        "htsjdk.samtools.util.BlockCompressedStreamConstants",
        "BLOCK_LENGTH_OFFSET",
    ),
    modeled_semantic_field("BlockCompressedStreamConstants", "BLOCK_LENGTH_OFFSET"),
    modeled_semantic_field(
        "htsjdk.samtools.util.BlockCompressedStreamConstants",
        "DEFAULT_COMPRESSION_LEVEL",
    ),
    modeled_semantic_field(
        "BlockCompressedStreamConstants",
        "DEFAULT_COMPRESSION_LEVEL",
    ),
    modeled_semantic_field(
        "htsjdk.samtools.util.BlockCompressedStreamConstants",
        "EMPTY_GZIP_BLOCK",
    ),
    modeled_semantic_field("BlockCompressedStreamConstants", "EMPTY_GZIP_BLOCK"),
    modeled_semantic_field("htsjdk.samtools.fastq.FastqConstants", "FIRST_OF_PAIR"),
    modeled_semantic_field("FastqConstants", "FIRST_OF_PAIR"),
    modeled_semantic_field("htsjdk.samtools.fastq.FastqConstants", "SECOND_OF_PAIR"),
    modeled_semantic_field("FastqConstants", "SECOND_OF_PAIR"),
    modeled_semantic_field("htsjdk.samtools.util.SequenceUtil", "VALID_BASES_UPPER"),
    modeled_semantic_field("SequenceUtil", "VALID_BASES_UPPER"),
    modeled_semantic_field(
        "htsjdk.samtools.util.BlockCompressedInputStream",
        "FileTermination",
    ),
    modeled_semantic_field("BlockCompressedInputStream", "FileTermination"),
    modeled_semantic_field("htsjdk.samtools.util.Log", "LogLevel"),
    modeled_semantic_field("Log", "LogLevel"),
    modeled_semantic_field("htsjdk.samtools.util.Log.LogLevel", "ERROR"),
    modeled_semantic_field("Log.LogLevel", "ERROR"),
    modeled_semantic_field("LogLevel", "ERROR"),
    modeled_semantic_field("htsjdk.samtools.util.Log.LogLevel", "WARNING"),
    modeled_semantic_field("Log.LogLevel", "WARNING"),
    modeled_semantic_field("LogLevel", "WARNING"),
    modeled_semantic_field("htsjdk.samtools.util.Log.LogLevel", "INFO"),
    modeled_semantic_field("Log.LogLevel", "INFO"),
    modeled_semantic_field("LogLevel", "INFO"),
    modeled_semantic_field("htsjdk.samtools.util.Log.LogLevel", "DEBUG"),
    modeled_semantic_field("Log.LogLevel", "DEBUG"),
    modeled_semantic_field("LogLevel", "DEBUG"),
    modeled_semantic_field("htsjdk.samtools.SamPairUtil", "PairOrientation"),
    modeled_semantic_field("SamPairUtil", "PairOrientation"),
    modeled_semantic_field("htsjdk.samtools.SamPairUtil.PairOrientation", "FR"),
    modeled_semantic_field("SamPairUtil.PairOrientation", "FR"),
    modeled_semantic_field("PairOrientation", "FR"),
    modeled_semantic_field("htsjdk.samtools.SamPairUtil.PairOrientation", "RF"),
    modeled_semantic_field("SamPairUtil.PairOrientation", "RF"),
    modeled_semantic_field("PairOrientation", "RF"),
    modeled_semantic_field("htsjdk.samtools.SamPairUtil.PairOrientation", "TANDEM"),
    modeled_semantic_field("SamPairUtil.PairOrientation", "TANDEM"),
    modeled_semantic_field("PairOrientation", "TANDEM"),
    modeled_semantic_field(
        "htsjdk.samtools.util.BlockCompressedInputStream.FileTermination",
        "DEFECTIVE",
    ),
    modeled_semantic_field("BlockCompressedInputStream.FileTermination", "DEFECTIVE"),
    modeled_semantic_field("FileTermination", "DEFECTIVE"),
    modeled_semantic_field(
        "htsjdk.samtools.util.BlockCompressedInputStream.FileTermination",
        "HAS_TERMINATOR_BLOCK",
    ),
    modeled_semantic_field(
        "BlockCompressedInputStream.FileTermination",
        "HAS_TERMINATOR_BLOCK",
    ),
    modeled_semantic_field("FileTermination", "HAS_TERMINATOR_BLOCK"),
];

const SAM_READER_MODELED_CONSTRUCTORS: &[ExternalPackModeledConstructor] = &[
    modeled_semantic_constructor("htsjdk.samtools.SamFileHeaderMerger", 3),
    modeled_semantic_constructor("SamFileHeaderMerger", 3),
    modeled_semantic_constructor("htsjdk.samtools.MergingSamRecordIterator", 3),
    modeled_semantic_constructor("MergingSamRecordIterator", 3),
];

const SAM_WRITER_MODELED_CONSTRUCTORS: &[ExternalPackModeledConstructor] = &[
    modeled_semantic_constructor("htsjdk.samtools.SAMFileWriterFactory", 0),
    modeled_semantic_constructor("SAMFileWriterFactory", 0),
];

const SAM_BAM_MODELED_CONSTRUCTORS: &[ExternalPackModeledConstructor] = &[
    modeled_semantic_constructor("htsjdk.samtools.BAMRecordCodec", 1),
    modeled_semantic_constructor("BAMRecordCodec", 1),
    modeled_semantic_constructor("htsjdk.samtools.BAMIndexer", 2),
    modeled_semantic_constructor("BAMIndexer", 2),
    modeled_semantic_constructor("htsjdk.samtools.SBIIndexWriter", 2),
    modeled_semantic_constructor("SBIIndexWriter", 2),
];

const SAM_BAM_MODELED_FIELDS: &[ExternalPackModeledField] = &[
    modeled_semantic_field("htsjdk.samtools.SBIIndexWriter", "DEFAULT_GRANULARITY"),
    modeled_semantic_field("SBIIndexWriter", "DEFAULT_GRANULARITY"),
    modeled_semantic_field("htsjdk.samtools.BAMFileConstants", "FIXED_BLOCK_SIZE"),
    modeled_semantic_field("BAMFileConstants", "FIXED_BLOCK_SIZE"),
];

const SAM_CRAM_MODELED_CONSTRUCTORS: &[ExternalPackModeledConstructor] = &[
    modeled_semantic_constructor("htsjdk.samtools.cram.build.CramContainerIterator", 1),
    modeled_semantic_constructor("CramContainerIterator", 1),
    modeled_semantic_constructor("htsjdk.samtools.cram.structure.Container", 3),
    modeled_semantic_constructor("Container", 3),
    modeled_semantic_constructor("htsjdk.samtools.cram.ref.CRAMReferenceRegion", 2),
    modeled_semantic_constructor("CRAMReferenceRegion", 2),
];

const SAM_CRAM_MODELED_FIELDS: &[ExternalPackModeledField] = &[
    modeled_semantic_field(
        "htsjdk.samtools.cram.structure.DataSeriesType",
        "BYTE_ARRAY",
    ),
    modeled_semantic_field("DataSeriesType", "BYTE_ARRAY"),
];

const SAM_METRICS_MODELED_CONSTRUCTORS: &[ExternalPackModeledConstructor] = &[
    modeled_semantic_constructor("htsjdk.samtools.metrics.MetricsFile", 0),
    modeled_semantic_constructor("MetricsFile", 0),
];

const HTSJDK_NESTED_HELPER_MODELED_CONSTRUCTORS: &[ExternalPackModeledConstructor] =
    &[modeled_semantic_constructor("ComponentMethods", 0)];

const SAM_READER_MODELED_FIELDS: &[ExternalPackModeledField] = &[
    modeled_semantic_field("htsjdk.samtools.SamReaderFactory", "Option"),
    modeled_semantic_field("SamReaderFactory", "Option"),
    modeled_semantic_field(
        "htsjdk.samtools.SamReaderFactory.Option",
        "CACHE_FILE_BASED_INDEXES",
    ),
    modeled_semantic_field("SamReaderFactory.Option", "CACHE_FILE_BASED_INDEXES"),
    modeled_semantic_field(
        "htsjdk.samtools.SamReaderFactory.Option",
        "INCLUDE_SOURCE_IN_RECORDS",
    ),
    modeled_semantic_field("SamReaderFactory.Option", "INCLUDE_SOURCE_IN_RECORDS"),
    modeled_semantic_field("htsjdk.samtools.ValidationStringency", "LENIENT"),
    modeled_semantic_field("ValidationStringency", "LENIENT"),
    modeled_semantic_field("htsjdk.samtools.ValidationStringency", "SILENT"),
    modeled_semantic_field("ValidationStringency", "SILENT"),
    modeled_semantic_field("htsjdk.samtools.ValidationStringency", "STRICT"),
    modeled_semantic_field("ValidationStringency", "STRICT"),
];

const TRIBBLE_INDEX_MODELED_FIELDS: &[ExternalPackModeledField] = &[
    modeled_semantic_field("htsjdk.tribble.index.IndexFactory", "IndexType"),
    modeled_semantic_field("IndexFactory", "IndexType"),
    modeled_semantic_field("htsjdk.tribble.index.IndexFactory", "IndexBalanceApproach"),
    modeled_semantic_field("IndexFactory", "IndexBalanceApproach"),
    modeled_semantic_field("htsjdk.tribble.index.IndexFactory.IndexType", "TABIX"),
    modeled_semantic_field("IndexFactory.IndexType", "TABIX"),
    modeled_semantic_field("IndexType", "TABIX"),
    modeled_semantic_field(
        "htsjdk.tribble.index.IndexFactory.IndexBalanceApproach",
        "FOR_SEEK_TIME",
    ),
    modeled_semantic_field("IndexFactory.IndexBalanceApproach", "FOR_SEEK_TIME"),
    modeled_semantic_field("IndexBalanceApproach", "FOR_SEEK_TIME"),
    modeled_semantic_field("htsjdk.tribble.index.tabix.TabixFormat", "GENERIC_FLAGS"),
    modeled_semantic_field("TabixFormat", "GENERIC_FLAGS"),
    modeled_semantic_field("htsjdk.tribble.index.tabix.TabixFormat", "ZERO_BASED"),
    modeled_semantic_field("TabixFormat", "ZERO_BASED"),
];

const APACHE_COMMONS_MATH_OPTIM_MODELED_CONSTRUCTORS: &[ExternalPackModeledConstructor] = &[
    modeled_semantic_constructor(
        "org.apache.commons.math3.optim.univariate.SearchInterval",
        3,
    ),
    modeled_semantic_constructor("SearchInterval", 3),
    modeled_semantic_constructor(
        "org.apache.commons.math3.optim.univariate.UnivariateObjectiveFunction",
        1,
    ),
    modeled_semantic_constructor("UnivariateObjectiveFunction", 1),
    modeled_semantic_constructor(
        "org.apache.commons.math3.optim.univariate.BrentOptimizer",
        2,
    ),
    modeled_semantic_constructor("BrentOptimizer", 2),
];

const APACHE_COMMONS_MATH_OPTIM_MODELED_FIELDS: &[ExternalPackModeledField] = &[
    modeled_semantic_field(
        "org.apache.commons.math3.optim.nonlinear.scalar.GoalType",
        "MAXIMIZE",
    ),
    modeled_semantic_field("GoalType", "MAXIMIZE"),
    modeled_semantic_field(
        "org.apache.commons.math3.optim.nonlinear.scalar.GoalType",
        "MINIMIZE",
    ),
    modeled_semantic_field("GoalType", "MINIMIZE"),
];

const APACHE_COMMONS_MATH_MODELED_CONSTRUCTORS: &[ExternalPackModeledConstructor] = &[
    modeled_semantic_constructor("org.apache.commons.math3.linear.Array2DRowRealMatrix", 1),
    modeled_semantic_constructor("Array2DRowRealMatrix", 1),
    modeled_semantic_constructor("org.apache.commons.math3.linear.Array2DRowRealMatrix", 2),
    modeled_semantic_constructor("Array2DRowRealMatrix", 2),
    modeled_semantic_constructor("org.apache.commons.math3.linear.ArrayRealVector", 0),
    modeled_semantic_constructor("ArrayRealVector", 0),
    modeled_semantic_constructor("org.apache.commons.math3.linear.ArrayRealVector", 1),
    modeled_semantic_constructor("ArrayRealVector", 1),
    modeled_semantic_constructor("org.apache.commons.math3.linear.ArrayRealVector", 2),
    modeled_semantic_constructor("ArrayRealVector", 2),
    modeled_semantic_constructor(
        "org.apache.commons.math3.linear.DefaultRealMatrixChangingVisitor",
        0,
    ),
    modeled_semantic_constructor("DefaultRealMatrixChangingVisitor", 0),
    modeled_semantic_constructor("org.apache.commons.math3.stat.descriptive.moment.Mean", 0),
    modeled_semantic_constructor("Mean", 0),
    modeled_semantic_constructor(
        "org.apache.commons.math3.distribution.BinomialDistribution",
        2,
    ),
    modeled_semantic_constructor("BinomialDistribution", 2),
    modeled_semantic_constructor(
        "org.apache.commons.math3.distribution.BinomialDistribution",
        3,
    ),
    modeled_semantic_constructor("BinomialDistribution", 3),
    modeled_semantic_constructor("org.apache.commons.math3.distribution.BetaDistribution", 3),
    modeled_semantic_constructor("BetaDistribution", 3),
    modeled_semantic_constructor(
        "org.apache.commons.math3.distribution.AbstractIntegerDistribution",
        1,
    ),
    modeled_semantic_constructor("AbstractIntegerDistribution", 1),
    modeled_semantic_constructor(
        "org.apache.commons.math3.stat.descriptive.DescriptiveStatistics",
        0,
    ),
    modeled_semantic_constructor("DescriptiveStatistics", 0),
    modeled_semantic_constructor(
        "org.apache.commons.math3.exception.DimensionMismatchException",
        2,
    ),
    modeled_semantic_constructor("DimensionMismatchException", 2),
    modeled_semantic_constructor(
        "org.apache.commons.math3.distribution.EnumeratedDistribution",
        2,
    ),
    modeled_semantic_constructor("EnumeratedDistribution", 2),
    modeled_semantic_constructor(
        "org.apache.commons.math3.distribution.ExponentialDistribution",
        2,
    ),
    modeled_semantic_constructor("ExponentialDistribution", 2),
    modeled_semantic_constructor(
        "org.apache.commons.math3.distribution.HypergeometricDistribution",
        4,
    ),
    modeled_semantic_constructor("HypergeometricDistribution", 4),
    modeled_semantic_constructor("org.apache.commons.math3.random.JDKRandomGenerator", 0),
    modeled_semantic_constructor("JDKRandomGenerator", 0),
    modeled_semantic_constructor("org.apache.commons.math3.optim.MaxEval", 1),
    modeled_semantic_constructor("MaxEval", 1),
    modeled_semantic_constructor("org.apache.commons.math3.stat.descriptive.rank.Median", 0),
    modeled_semantic_constructor("Median", 0),
    modeled_semantic_constructor(
        "org.apache.commons.math3.distribution.NormalDistribution",
        2,
    ),
    modeled_semantic_constructor("NormalDistribution", 2),
    modeled_semantic_constructor(
        "org.apache.commons.math3.distribution.NormalDistribution",
        3,
    ),
    modeled_semantic_constructor("NormalDistribution", 3),
    modeled_semantic_constructor(
        "org.apache.commons.math3.distribution.ChiSquaredDistribution",
        1,
    ),
    modeled_semantic_constructor("ChiSquaredDistribution", 1),
    modeled_semantic_constructor(
        "org.apache.commons.math3.distribution.ChiSquaredDistribution",
        2,
    ),
    modeled_semantic_constructor("ChiSquaredDistribution", 2),
    modeled_semantic_constructor(
        "org.apache.commons.math3.distribution.PoissonDistribution",
        1,
    ),
    modeled_semantic_constructor("PoissonDistribution", 1),
    modeled_semantic_constructor(
        "org.apache.commons.math3.linear.SingularValueDecomposition",
        1,
    ),
    modeled_semantic_constructor("SingularValueDecomposition", 1),
    modeled_semantic_constructor(
        "org.apache.commons.math3.stat.descriptive.moment.StandardDeviation",
        0,
    ),
    modeled_semantic_constructor("StandardDeviation", 0),
    modeled_semantic_constructor("org.apache.commons.math3.distribution.TDistribution", 1),
    modeled_semantic_constructor("TDistribution", 1),
    modeled_semantic_constructor("org.apache.commons.math3.distribution.TDistribution", 2),
    modeled_semantic_constructor("TDistribution", 2),
];

const APACHE_COMMONS_MATH_STATS_MODELED_CONSTRUCTORS: &[ExternalPackModeledConstructor] = &[
    modeled_semantic_constructor(
        "org.apache.commons.math3.stat.descriptive.rank.Percentile",
        0,
    ),
    modeled_semantic_constructor(
        "org.apache.commons.math3.stat.descriptive.rank.Percentile",
        1,
    ),
    modeled_semantic_constructor("Percentile", 0),
    modeled_semantic_constructor("Percentile", 1),
];

const APACHE_COMMONS_COLLECTIONS_MODELED_CONSTRUCTORS: &[ExternalPackModeledConstructor] = &[
    modeled_semantic_constructor("org.apache.commons.collections4.map.DefaultedMap", 1),
    modeled_semantic_constructor("DefaultedMap", 1),
    modeled_semantic_constructor(
        "org.apache.commons.collections4.iterators.FilterIterator",
        2,
    ),
    modeled_semantic_constructor("FilterIterator", 2),
    modeled_semantic_constructor(
        "org.apache.commons.collections4.iterators.SingletonIterator",
        1,
    ),
    modeled_semantic_constructor("SingletonIterator", 1),
    modeled_semantic_constructor("org.apache.commons.collections4.MultiMap", 0),
    modeled_semantic_constructor("MultiMap", 0),
];

const APACHE_COMMONS_COMPRESS_MODELED_CONSTRUCTORS: &[ExternalPackModeledConstructor] = &[
    modeled_semantic_constructor(
        "org.apache.commons.compress.compressors.gzip.GzipCompressorInputStream",
        1,
    ),
    modeled_semantic_constructor("GzipCompressorInputStream", 1),
    modeled_semantic_constructor(
        "org.apache.commons.compress.compressors.gzip.GzipCompressorOutputStream",
        1,
    ),
    modeled_semantic_constructor("GzipCompressorOutputStream", 1),
    modeled_semantic_constructor("org.apache.commons.io.input.NullInputStream", 0),
    modeled_semantic_constructor("NullInputStream", 0),
    modeled_semantic_constructor("org.apache.commons.io.input.NullInputStream", 1),
    modeled_semantic_constructor("NullInputStream", 1),
    modeled_semantic_constructor("org.apache.commons.io.output.NullOutputStream", 0),
    modeled_semantic_constructor("NullOutputStream", 0),
    modeled_semantic_constructor("org.apache.commons.io.output.NullOutputStream", 1),
    modeled_semantic_constructor("NullOutputStream", 1),
    modeled_semantic_constructor(
        "org.apache.commons.compress.archivers.tar.TarArchiveInputStream",
        1,
    ),
    modeled_semantic_constructor("TarArchiveInputStream", 1),
    modeled_semantic_constructor(
        "org.apache.commons.compress.archivers.tar.TarArchiveOutputStream",
        1,
    ),
    modeled_semantic_constructor("TarArchiveOutputStream", 1),
    modeled_semantic_constructor(
        "org.apache.commons.compress.archivers.tar.TarArchiveEntry",
        1,
    ),
    modeled_semantic_constructor("TarArchiveEntry", 1),
    modeled_semantic_constructor(
        "org.apache.commons.compress.archivers.tar.TarArchiveEntry",
        2,
    ),
    modeled_semantic_constructor("TarArchiveEntry", 2),
];

const APACHE_COMMONS_COMPRESS_MODELED_FIELDS: &[ExternalPackModeledField] = &[
    modeled_semantic_field(
        "org.apache.commons.compress.archivers.tar.TarArchiveOutputStream",
        "BIGNUMBER_STAR",
    ),
    modeled_semantic_field("TarArchiveOutputStream", "BIGNUMBER_STAR"),
    modeled_semantic_field(
        "org.apache.commons.compress.archivers.tar.TarArchiveOutputStream",
        "LONGFILE_GNU",
    ),
    modeled_semantic_field("TarArchiveOutputStream", "LONGFILE_GNU"),
    modeled_semantic_field("org.apache.commons.io.output.NullOutputStream", "INSTANCE"),
    modeled_semantic_field("NullOutputStream", "INSTANCE"),
    modeled_semantic_field(
        "org.apache.commons.io.output.NullOutputStream",
        "NULL_OUTPUT_STREAM",
    ),
    modeled_semantic_field("NullOutputStream", "NULL_OUTPUT_STREAM"),
    modeled_semantic_field("org.apache.commons.io.output.NullWriter", "INSTANCE"),
    modeled_semantic_field("NullWriter", "INSTANCE"),
];

const APACHE_COMMONS_LANG_MODELED_CONSTRUCTORS: &[ExternalPackModeledConstructor] = &[
    modeled_semantic_constructor("org.apache.commons.lang3.builder.HashCodeBuilder", 0),
    modeled_semantic_constructor("HashCodeBuilder", 0),
    modeled_semantic_constructor("org.apache.commons.lang3.mutable.MutableInt", 0),
    modeled_semantic_constructor("MutableInt", 0),
    modeled_semantic_constructor("org.apache.commons.lang3.mutable.MutableInt", 1),
    modeled_semantic_constructor("MutableInt", 1),
    modeled_semantic_constructor("org.apache.commons.lang3.mutable.MutableLong", 0),
    modeled_semantic_constructor("MutableLong", 0),
    modeled_semantic_constructor("org.apache.commons.lang3.mutable.MutableLong", 1),
    modeled_semantic_constructor("MutableLong", 1),
    modeled_semantic_constructor("org.apache.commons.lang3.mutable.MutableDouble", 0),
    modeled_semantic_constructor("MutableDouble", 0),
    modeled_semantic_constructor("org.apache.commons.lang3.mutable.MutableDouble", 1),
    modeled_semantic_constructor("MutableDouble", 1),
    modeled_semantic_constructor("org.apache.commons.lang3.NotImplementedException", 1),
    modeled_semantic_constructor("NotImplementedException", 1),
    modeled_semantic_constructor("org.apache.commons.lang3.NotImplementedException", 2),
    modeled_semantic_constructor("NotImplementedException", 2),
    modeled_semantic_constructor("org.apache.commons.lang3.tuple.Pair", 2),
    modeled_semantic_constructor("Pair", 2),
    modeled_semantic_constructor("org.apache.commons.lang3.tuple.ImmutablePair", 2),
    modeled_semantic_constructor("ImmutablePair", 2),
    modeled_semantic_constructor("org.apache.commons.lang3.tuple.MutablePair", 2),
    modeled_semantic_constructor("MutablePair", 2),
    modeled_semantic_constructor("org.apache.commons.lang3.tuple.Triple", 3),
    modeled_semantic_constructor("Triple", 3),
    modeled_semantic_constructor("org.apache.commons.lang3.tuple.ImmutableTriple", 3),
    modeled_semantic_constructor("ImmutableTriple", 3),
    modeled_semantic_constructor("org.apache.commons.lang3.tuple.MutableTriple", 3),
    modeled_semantic_constructor("MutableTriple", 3),
];

const APACHE_COMMONS_LANG_MODELED_FIELDS: &[ExternalPackModeledField] = &[
    modeled_semantic_field("org.apache.commons.lang3.SystemUtils", "OS_ARCH"),
    modeled_semantic_field("SystemUtils", "OS_ARCH"),
    modeled_semantic_field("org.apache.commons.lang3.SystemUtils", "OS_NAME"),
    modeled_semantic_field("SystemUtils", "OS_NAME"),
    modeled_semantic_field("org.apache.commons.lang3.SystemUtils", "IS_OS_MAC"),
    modeled_semantic_field("SystemUtils", "IS_OS_MAC"),
    modeled_semantic_field("org.apache.commons.lang3.SystemUtils", "IS_OS_LINUX"),
    modeled_semantic_field("SystemUtils", "IS_OS_LINUX"),
    modeled_semantic_field("org.apache.commons.lang3.SystemUtils", "IS_OS_WINDOWS"),
    modeled_semantic_field("SystemUtils", "IS_OS_WINDOWS"),
];

const APACHE_COMMONS_CODEC_MODELED_FIELDS: &[ExternalPackModeledField] = &[
    modeled_semantic_field(
        "org.apache.commons.codec.digest.MessageDigestAlgorithms",
        "SHA_256",
    ),
    modeled_semantic_field("MessageDigestAlgorithms", "SHA_256"),
];

const JAMA_MODELED_CONSTRUCTORS: &[ExternalPackModeledConstructor] = &[
    modeled_semantic_constructor("Jama.Matrix", 1),
    modeled_semantic_constructor("Matrix", 1),
    modeled_semantic_constructor("Jama.Matrix", 2),
    modeled_semantic_constructor("Matrix", 2),
];

const JAMA_API_EFFECTS: &[ExternalPackApiEffect] = &[
    semantic_typed_effect_with_arity(
        "Jama.Matrix",
        "numRows",
        0,
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect_with_arity(
        "Matrix",
        "numRows",
        0,
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect_with_arity(
        "Jama.Matrix",
        "numCols",
        0,
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect_with_arity(
        "Matrix",
        "numCols",
        0,
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect_with_arity(
        "Jama.Matrix",
        "toArray",
        0,
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "double[][]",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect_with_arity(
        "Matrix",
        "toArray",
        0,
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "double[][]",
        NullabilityState::DefinitelyNonNull,
    ),
];

const JAMA_MODELED_APIS: &[ExternalPackModeledApi] = &[
    modeled_semantic("Jama.Matrix", "numRows"),
    modeled_semantic("Matrix", "numRows"),
    modeled_semantic("Jama.Matrix", "numCols"),
    modeled_semantic("Matrix", "numCols"),
    modeled_semantic("Jama.Matrix", "toArray"),
    modeled_semantic("Matrix", "toArray"),
];

const HIPPARCHUS_DISTRIBUTION_MODELED_CONSTRUCTORS: &[ExternalPackModeledConstructor] = &[
    modeled_semantic_constructor("org.hipparchus.random.EmpiricalDistribution", 0),
    modeled_semantic_constructor("EmpiricalDistribution", 0),
];

const APACHE_HTTP_API_EFFECTS: &[ExternalPackApiEffect] = &[
    semantic_mutator_effect(
        "org.apache.http.client.methods.HttpGet",
        "addHeader",
        NullabilityState::Unknown,
    ),
    semantic_mutator_effect("HttpGet", "addHeader", NullabilityState::Unknown),
];

const APACHE_HTTP_MODELED_APIS: &[ExternalPackModeledApi] = &[
    modeled_semantic("org.apache.http.client.methods.HttpGet", "addHeader"),
    modeled_semantic("HttpGet", "addHeader"),
];

const APACHE_HTTP_MODELED_CONSTRUCTORS: &[ExternalPackModeledConstructor] = &[
    modeled_semantic_constructor("org.apache.http.client.methods.HttpGet", 1),
    modeled_semantic_constructor("HttpGet", 1),
    modeled_semantic_constructor(
        "org.apache.http.impl.conn.PoolingHttpClientConnectionManager",
        0,
    ),
    modeled_semantic_constructor("PoolingHttpClientConnectionManager", 0),
];

const APACHE_COMMONS_MATH_STATS_MODELED_FIELDS: &[ExternalPackModeledField] = &[
    modeled_semantic_field(
        "org.apache.commons.math3.stat.descriptive.rank.Percentile",
        "EstimationType",
    ),
    modeled_semantic_field("Percentile", "EstimationType"),
    modeled_semantic_field(
        "org.apache.commons.math3.stat.descriptive.rank.Percentile.EstimationType",
        "R_1",
    ),
    modeled_semantic_field("Percentile.EstimationType", "R_1"),
    modeled_semantic_field("EstimationType", "R_1"),
];

const GOOGLE_BIGQUERY_MODELED_FIELDS: &[ExternalPackModeledField] = &[
    modeled_semantic_field("com.google.cloud.bigquery.FieldValue", "Attribute"),
    modeled_semantic_field("FieldValue", "Attribute"),
    modeled_semantic_field(
        "com.google.cloud.bigquery.FieldValue.Attribute",
        "PRIMITIVE",
    ),
    modeled_semantic_field("FieldValue.Attribute", "PRIMITIVE"),
    modeled_semantic_field(
        "com.google.cloud.bigquery.QueryJobConfiguration",
        "Priority",
    ),
    modeled_semantic_field("QueryJobConfiguration", "Priority"),
    modeled_semantic_field(
        "com.google.cloud.bigquery.QueryJobConfiguration.Priority",
        "BATCH",
    ),
    modeled_semantic_field("QueryJobConfiguration.Priority", "BATCH"),
    modeled_semantic_field(
        "com.google.cloud.bigquery.QueryJobConfiguration.Priority",
        "INTERACTIVE",
    ),
    modeled_semantic_field("QueryJobConfiguration.Priority", "INTERACTIVE"),
];

const GOOGLE_BIGQUERY_STORAGE_MODELED_FIELDS: &[ExternalPackModeledField] = &[
    modeled_semantic_field("com.google.cloud.bigquery.storage.v1.DataFormat", "AVRO"),
    modeled_semantic_field("DataFormat", "AVRO"),
    modeled_semantic_field("com.google.cloud.bigquery.storage.v1.DataFormat", "ARROW"),
    modeled_semantic_field("DataFormat", "ARROW"),
];

const GOOGLE_STORAGE_MODELED_FIELDS: &[ExternalPackModeledField] = &[
    modeled_semantic_field("com.google.cloud.storage.Storage", "SignUrlOption"),
    modeled_semantic_field("Storage", "SignUrlOption"),
];

const GOOGLE_CLOUD_NIO_MODELED_FIELDS: &[ExternalPackModeledField] = &[
    modeled_semantic_field(
        "com.google.cloud.storage.contrib.nio.CloudStorageFileSystem",
        "URI_SCHEME",
    ),
    modeled_semantic_field("CloudStorageFileSystem", "URI_SCHEME"),
    modeled_semantic_field(
        "com.google.cloud.storage.contrib.nio.CloudStorageConfiguration",
        "DEFAULT",
    ),
    modeled_semantic_field("CloudStorageConfiguration", "DEFAULT"),
];

const GENOMICSDB_MODELED_CONSTRUCTORS: &[ExternalPackModeledConstructor] = &[
    modeled_semantic_constructor("com.intel.genomicsdb.GenomicsDBFeatureReader", 3),
    modeled_semantic_constructor("GenomicsDBFeatureReader", 3),
    modeled_semantic_constructor("com.intel.genomicsdb.GenomicsDBImporter", 1),
    modeled_semantic_constructor("GenomicsDBImporter", 1),
    modeled_semantic_constructor("com.intel.genomicsdb.importer.model.ImportConfig", 8),
    modeled_semantic_constructor("ImportConfig", 8),
];

const GENOMICSDB_MODELED_FIELDS: &[ExternalPackModeledField] = &[
    modeled_semantic_field("com.intel.genomicsdb.model.Coordinates", "GenomicsDBColumn"),
    modeled_semantic_field("Coordinates", "GenomicsDBColumn"),
    modeled_semantic_field("com.intel.genomicsdb.model.Coordinates", "ContigPosition"),
    modeled_semantic_field("Coordinates", "ContigPosition"),
    modeled_semantic_field(
        "com.intel.genomicsdb.model.GenomicsDBExportConfiguration",
        "ExportConfiguration",
    ),
    modeled_semantic_field("GenomicsDBExportConfiguration", "ExportConfiguration"),
    modeled_semantic_field(
        "com.intel.genomicsdb.model.GenomicsDBImportConfiguration",
        "ImportConfiguration",
    ),
    modeled_semantic_field("GenomicsDBImportConfiguration", "ImportConfiguration"),
    modeled_semantic_field(
        "com.intel.genomicsdb.model.GenomicsDBImportConfiguration",
        "Partition",
    ),
    modeled_semantic_field("GenomicsDBImportConfiguration", "Partition"),
    modeled_semantic_field(
        "com.intel.genomicsdb.model.GenomicsDBVidMapProto",
        "VidMappingPB",
    ),
    modeled_semantic_field("GenomicsDBVidMapProto", "VidMappingPB"),
    modeled_semantic_field(
        "com.intel.genomicsdb.model.GenomicsDBVidMapProto",
        "FieldLengthDescriptorComponentPB",
    ),
    modeled_semantic_field("GenomicsDBVidMapProto", "FieldLengthDescriptorComponentPB"),
    modeled_semantic_field("com.intel.genomicsdb.model.ReadSession", "TableReadOptions"),
    modeled_semantic_field("ReadSession", "TableReadOptions"),
];

const GENOMICSDB_ARGUMENT_EFFECTS: &[ExternalPackArgumentEffect] = &[
    readonly_borrowed_argument(
        "org.genomicsdb.GenomicsDBUtils",
        "isGenomicsDBArray",
        0,
        "String",
    ),
    readonly_borrowed_argument("GenomicsDBUtils", "isGenomicsDBArray", 0, "String"),
    readonly_borrowed_argument(
        "org.genomicsdb.GenomicsDBUtils",
        "isGenomicsDBArray",
        1,
        "String",
    ),
    readonly_borrowed_argument("GenomicsDBUtils", "isGenomicsDBArray", 1, "String"),
    readonly_borrowed_argument(
        "org.genomicsdb.GenomicsDBUtils",
        "readEntireFile",
        0,
        "String",
    ),
    readonly_borrowed_argument("GenomicsDBUtils", "readEntireFile", 0, "String"),
    readonly_borrowed_argument(
        "org.genomicsdb.GenomicsDBUtils",
        "useGcsHdfsConnector",
        0,
        "boolean",
    ),
    readonly_borrowed_argument("GenomicsDBUtils", "useGcsHdfsConnector", 0, "boolean"),
    ExternalPackArgumentEffect {
        receiver_type: "com.intel.genomicsdb.importer.model.ImportConfig",
        method_name: "new",
        parameter_index: 4,
        parameter_type: Some("Set<VCFHeaderLine>"),
        parameter_nullability: NullabilityState::DefinitelyNonNull,
        argument_mutability: ReceiverMutabilityFact::ReadOnly,
        alias_policy: ExternalArgumentAliasPolicy::Escapes,
    },
    ExternalPackArgumentEffect {
        receiver_type: "ImportConfig",
        method_name: "new",
        parameter_index: 4,
        parameter_type: Some("Set<VCFHeaderLine>"),
        parameter_nullability: NullabilityState::DefinitelyNonNull,
        argument_mutability: ReceiverMutabilityFact::ReadOnly,
        alias_policy: ExternalArgumentAliasPolicy::Escapes,
    },
    ExternalPackArgumentEffect {
        receiver_type: "com.intel.genomicsdb.importer.model.ImportConfig",
        method_name: "new",
        parameter_index: 6,
        parameter_type: Some("Function"),
        parameter_nullability: NullabilityState::MaybeNull,
        argument_mutability: ReceiverMutabilityFact::ReadOnly,
        alias_policy: ExternalArgumentAliasPolicy::Escapes,
    },
    ExternalPackArgumentEffect {
        receiver_type: "ImportConfig",
        method_name: "new",
        parameter_index: 6,
        parameter_type: Some("Function"),
        parameter_nullability: NullabilityState::MaybeNull,
        argument_mutability: ReceiverMutabilityFact::ReadOnly,
        alias_policy: ExternalArgumentAliasPolicy::Escapes,
    },
];

const GENOMICSDB_API_EFFECTS: &[ExternalPackApiEffect] = &[
    semantic_typed_effect_with_arity(
        "org.genomicsdb.GenomicsDBUtils",
        "isGenomicsDBArray",
        2,
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect_with_arity(
        "GenomicsDBUtils",
        "isGenomicsDBArray",
        2,
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "boolean",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect_with_arity(
        "org.genomicsdb.GenomicsDBUtils",
        "getArrayColumnBounds",
        2,
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "long[]",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect_with_arity(
        "GenomicsDBUtils",
        "getArrayColumnBounds",
        2,
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "long[]",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect_with_arity(
        "org.genomicsdb.GenomicsDBUtils",
        "readEntireFile",
        1,
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "String",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect_with_arity(
        "GenomicsDBUtils",
        "readEntireFile",
        1,
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "String",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_side_effect_with_arity(
        "org.genomicsdb.GenomicsDBUtils",
        "useGcsHdfsConnector",
        1,
        NullabilityState::Unknown,
    ),
    semantic_side_effect_with_arity(
        "GenomicsDBUtils",
        "useGcsHdfsConnector",
        1,
        NullabilityState::Unknown,
    ),
    semantic_typed_effect(
        "com.intel.genomicsdb.model.Coordinates.GenomicsDBColumn",
        "newBuilder",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "GenomicsDBColumn.Builder",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "GenomicsDBColumn",
        "newBuilder",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "GenomicsDBColumn.Builder",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "Coordinates.GenomicsDBColumn.Builder",
        "build",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Coordinates.GenomicsDBColumn",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "GenomicsDBColumn.Builder",
        "build",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "GenomicsDBColumn",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "com.intel.genomicsdb.model.Coordinates.ContigPosition",
        "newBuilder",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "ContigPosition.Builder",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "Coordinates.ContigPosition.Builder",
        "build",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Coordinates.ContigPosition",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "ContigPosition.Builder",
        "build",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "ContigPosition",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "ContigPosition",
        "newBuilder",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "ContigPosition.Builder",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "com.intel.genomicsdb.model.GenomicsDBExportConfiguration.ExportConfiguration",
        "newBuilder",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "ExportConfiguration.Builder",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "ExportConfiguration",
        "newBuilder",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "ExportConfiguration.Builder",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "GenomicsDBExportConfiguration.ExportConfiguration.Builder",
        "build",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "GenomicsDBExportConfiguration.ExportConfiguration",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "ExportConfiguration.Builder",
        "build",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "ExportConfiguration",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "com.intel.genomicsdb.model.GenomicsDBImportConfiguration.ImportConfiguration",
        "newBuilder",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "ImportConfiguration.Builder",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "ImportConfiguration",
        "newBuilder",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "ImportConfiguration.Builder",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "GenomicsDBImportConfiguration.ImportConfiguration.Builder",
        "build",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "GenomicsDBImportConfiguration.ImportConfiguration",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "ImportConfiguration.Builder",
        "build",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "ImportConfiguration",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "com.intel.genomicsdb.model.GenomicsDBImportConfiguration.Partition",
        "newBuilder",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Partition.Builder",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "Partition",
        "newBuilder",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Partition.Builder",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "GenomicsDBImportConfiguration.Partition.Builder",
        "build",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "GenomicsDBImportConfiguration.Partition",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "Partition.Builder",
        "build",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Partition",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "com.intel.genomicsdb.model.GenomicsDBVidMapProto.VidMappingPB",
        "newBuilder",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "VidMappingPB.Builder",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "VidMappingPB",
        "newBuilder",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "VidMappingPB.Builder",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "GenomicsDBVidMapProto.VidMappingPB",
        "newBuilder",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "GenomicsDBVidMapProto.VidMappingPB.Builder",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "GenomicsDBVidMapProto.VidMappingPB.Builder",
        "build",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "GenomicsDBVidMapProto.VidMappingPB",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "VidMappingPB.Builder",
        "build",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "VidMappingPB",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "com.intel.genomicsdb.model.GenomicsDBVidMapProto.VidMappingPB",
        "toBuilder",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "VidMappingPB.Builder",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "VidMappingPB",
        "toBuilder",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "VidMappingPB.Builder",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "GenomicsDBVidMapProto.VidMappingPB",
        "toBuilder",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "GenomicsDBVidMapProto.VidMappingPB.Builder",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "com.intel.genomicsdb.model.GenomicsDBVidMapProto.VidMappingPB",
        "getContigsList",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "List<GenomicsDBVidMapProto.Chromosome>",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "VidMappingPB",
        "getContigsList",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "List<GenomicsDBVidMapProto.Chromosome>",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "GenomicsDBVidMapProto.VidMappingPB",
        "getContigsList",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "List<GenomicsDBVidMapProto.Chromosome>",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "com.intel.genomicsdb.model.GenomicsDBVidMapProto.Chromosome",
        "getName",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "String",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "org.genomicsdb.model.GenomicsDBVidMapProto.Chromosome",
        "getName",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "String",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "GenomicsDBVidMapProto.Chromosome",
        "getName",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "String",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "Chromosome",
        "getName",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "String",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "com.intel.genomicsdb.model.GenomicsDBVidMapProto.Chromosome",
        "getTiledbColumnOffset",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "long",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "org.genomicsdb.model.GenomicsDBVidMapProto.Chromosome",
        "getTiledbColumnOffset",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "long",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "GenomicsDBVidMapProto.Chromosome",
        "getTiledbColumnOffset",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "long",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "Chromosome",
        "getTiledbColumnOffset",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "long",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "com.intel.genomicsdb.model.GenomicsDBVidMapProto.Chromosome",
        "getLength",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "org.genomicsdb.model.GenomicsDBVidMapProto.Chromosome",
        "getLength",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "GenomicsDBVidMapProto.Chromosome",
        "getLength",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "Chromosome",
        "getLength",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect_with_arity(
        "com.intel.genomicsdb.model.GenomicsDBVidMapProto.VidMappingPB",
        "getFields",
        1,
        ExternalApiRole::IndexedGetter,
        ReceiverMutabilityFact::ReadOnly,
        "FieldInfo",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect_with_arity(
        "VidMappingPB",
        "getFields",
        1,
        ExternalApiRole::IndexedGetter,
        ReceiverMutabilityFact::ReadOnly,
        "FieldInfo",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect_with_arity(
        "GenomicsDBVidMapProto.VidMappingPB",
        "getFields",
        1,
        ExternalApiRole::IndexedGetter,
        ReceiverMutabilityFact::ReadOnly,
        "GenomicsDBVidMapProto.FieldInfo",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "com.intel.genomicsdb.model.GenomicsDBVidMapProto.VidMappingPB",
        "getFieldsCount",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "VidMappingPB",
        "getFieldsCount",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "GenomicsDBVidMapProto.VidMappingPB",
        "getFieldsCount",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "com.intel.genomicsdb.model.GenomicsDBVidMapProto.FieldInfo",
        "getName",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "String",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "FieldInfo",
        "getName",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "String",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "GenomicsDBVidMapProto.FieldInfo",
        "getName",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "String",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "com.intel.genomicsdb.model.GenomicsDBVidMapProto.FieldLengthDescriptorComponentPB",
        "newBuilder",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "FieldLengthDescriptorComponentPB.Builder",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "FieldLengthDescriptorComponentPB",
        "newBuilder",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "FieldLengthDescriptorComponentPB.Builder",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "GenomicsDBVidMapProto.FieldLengthDescriptorComponentPB.Builder",
        "build",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "GenomicsDBVidMapProto.FieldLengthDescriptorComponentPB",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "FieldLengthDescriptorComponentPB.Builder",
        "build",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "FieldLengthDescriptorComponentPB",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "com.intel.genomicsdb.GenomicsDBImporter",
        "getProtobufVidMapping",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "VidMappingPB",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "GenomicsDBImporter",
        "getProtobufVidMapping",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "VidMappingPB",
        NullabilityState::MaybeNull,
    ),
    semantic_mutator_effect(
        "com.intel.genomicsdb.GenomicsDBImporter",
        "updateProtobufVidMapping",
        NullabilityState::Unknown,
    ),
    semantic_mutator_effect(
        "GenomicsDBImporter",
        "updateProtobufVidMapping",
        NullabilityState::Unknown,
    ),
    semantic_typed_effect(
        "com.intel.genomicsdb.GenomicsDBImporter",
        "coalesceContigsIntoNumPartitions",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "List<Locatable>",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "GenomicsDBImporter",
        "coalesceContigsIntoNumPartitions",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "List<Locatable>",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_side_effect(
        "com.intel.genomicsdb.GenomicsDBImporter",
        "executeImport",
        NullabilityState::Unknown,
    ),
    semantic_side_effect(
        "GenomicsDBImporter",
        "executeImport",
        NullabilityState::Unknown,
    ),
    semantic_mutator_effect(
        "com.intel.genomicsdb.importer.model.ImportConfig",
        "setFunctionToCallOnBatchCompletion",
        NullabilityState::Unknown,
    ),
    semantic_mutator_effect(
        "ImportConfig",
        "setFunctionToCallOnBatchCompletion",
        NullabilityState::Unknown,
    ),
    semantic_mutator_effect(
        "com.intel.genomicsdb.importer.model.ImportConfig",
        "setOutputCallsetmapJsonFile",
        NullabilityState::Unknown,
    ),
    semantic_mutator_effect(
        "ImportConfig",
        "setOutputCallsetmapJsonFile",
        NullabilityState::Unknown,
    ),
    semantic_mutator_effect(
        "com.intel.genomicsdb.importer.model.ImportConfig",
        "setOutputVcfHeaderFile",
        NullabilityState::Unknown,
    ),
    semantic_mutator_effect(
        "ImportConfig",
        "setOutputVcfHeaderFile",
        NullabilityState::Unknown,
    ),
    semantic_mutator_effect(
        "com.intel.genomicsdb.importer.model.ImportConfig",
        "setOutputVidmapJsonFile",
        NullabilityState::Unknown,
    ),
    semantic_mutator_effect(
        "ImportConfig",
        "setOutputVidmapJsonFile",
        NullabilityState::Unknown,
    ),
    semantic_mutator_effect(
        "com.intel.genomicsdb.importer.model.ImportConfig",
        "setUseSamplesInOrder",
        NullabilityState::Unknown,
    ),
    semantic_mutator_effect(
        "ImportConfig",
        "setUseSamplesInOrder",
        NullabilityState::Unknown,
    ),
];

const GENOMICSDB_MODELED_APIS: &[ExternalPackModeledApi] = &[
    modeled_semantic("org.genomicsdb.GenomicsDBUtils", "isGenomicsDBArray"),
    modeled_semantic("GenomicsDBUtils", "isGenomicsDBArray"),
    modeled_semantic("org.genomicsdb.GenomicsDBUtils", "readEntireFile"),
    modeled_semantic("GenomicsDBUtils", "readEntireFile"),
    modeled_semantic("org.genomicsdb.GenomicsDBUtils", "useGcsHdfsConnector"),
    modeled_semantic("GenomicsDBUtils", "useGcsHdfsConnector"),
    modeled_semantic(
        "com.intel.genomicsdb.model.Coordinates.GenomicsDBColumn",
        "newBuilder",
    ),
    modeled_semantic("GenomicsDBColumn", "newBuilder"),
    modeled_semantic("Coordinates.GenomicsDBColumn.Builder", "build"),
    modeled_semantic("GenomicsDBColumn.Builder", "build"),
    modeled_semantic(
        "com.intel.genomicsdb.model.Coordinates.ContigPosition",
        "newBuilder",
    ),
    modeled_semantic("ContigPosition", "newBuilder"),
    modeled_semantic("Coordinates.ContigPosition.Builder", "build"),
    modeled_semantic("ContigPosition.Builder", "build"),
    modeled_semantic(
        "com.intel.genomicsdb.model.GenomicsDBExportConfiguration.ExportConfiguration",
        "newBuilder",
    ),
    modeled_semantic("ExportConfiguration", "newBuilder"),
    modeled_semantic(
        "GenomicsDBExportConfiguration.ExportConfiguration.Builder",
        "build",
    ),
    modeled_semantic("ExportConfiguration.Builder", "build"),
    modeled_semantic(
        "com.intel.genomicsdb.model.GenomicsDBImportConfiguration.ImportConfiguration",
        "newBuilder",
    ),
    modeled_semantic("ImportConfiguration", "newBuilder"),
    modeled_semantic(
        "GenomicsDBImportConfiguration.ImportConfiguration.Builder",
        "build",
    ),
    modeled_semantic("ImportConfiguration.Builder", "build"),
    modeled_semantic(
        "com.intel.genomicsdb.model.GenomicsDBImportConfiguration.Partition",
        "newBuilder",
    ),
    modeled_semantic("Partition", "newBuilder"),
    modeled_semantic("GenomicsDBImportConfiguration.Partition.Builder", "build"),
    modeled_semantic("Partition.Builder", "build"),
    modeled_semantic(
        "com.intel.genomicsdb.model.GenomicsDBVidMapProto.VidMappingPB",
        "newBuilder",
    ),
    modeled_semantic("VidMappingPB", "newBuilder"),
    modeled_semantic("GenomicsDBVidMapProto.VidMappingPB", "newBuilder"),
    modeled_semantic("GenomicsDBVidMapProto.VidMappingPB.Builder", "build"),
    modeled_semantic("VidMappingPB.Builder", "build"),
    modeled_semantic(
        "com.intel.genomicsdb.model.GenomicsDBVidMapProto.VidMappingPB",
        "toBuilder",
    ),
    modeled_semantic("VidMappingPB", "toBuilder"),
    modeled_semantic("GenomicsDBVidMapProto.VidMappingPB", "toBuilder"),
    modeled_semantic(
        "com.intel.genomicsdb.model.GenomicsDBVidMapProto.VidMappingPB",
        "getContigsList",
    ),
    modeled_semantic("VidMappingPB", "getContigsList"),
    modeled_semantic("GenomicsDBVidMapProto.VidMappingPB", "getContigsList"),
    modeled_semantic(
        "com.intel.genomicsdb.model.GenomicsDBVidMapProto.Chromosome",
        "getName",
    ),
    modeled_semantic(
        "org.genomicsdb.model.GenomicsDBVidMapProto.Chromosome",
        "getName",
    ),
    modeled_semantic("GenomicsDBVidMapProto.Chromosome", "getName"),
    modeled_semantic("Chromosome", "getName"),
    modeled_semantic(
        "com.intel.genomicsdb.model.GenomicsDBVidMapProto.Chromosome",
        "getTiledbColumnOffset",
    ),
    modeled_semantic(
        "org.genomicsdb.model.GenomicsDBVidMapProto.Chromosome",
        "getTiledbColumnOffset",
    ),
    modeled_semantic("GenomicsDBVidMapProto.Chromosome", "getTiledbColumnOffset"),
    modeled_semantic("Chromosome", "getTiledbColumnOffset"),
    modeled_semantic(
        "com.intel.genomicsdb.model.GenomicsDBVidMapProto.Chromosome",
        "getLength",
    ),
    modeled_semantic(
        "org.genomicsdb.model.GenomicsDBVidMapProto.Chromosome",
        "getLength",
    ),
    modeled_semantic("GenomicsDBVidMapProto.Chromosome", "getLength"),
    modeled_semantic("Chromosome", "getLength"),
    modeled_semantic(
        "com.intel.genomicsdb.model.GenomicsDBVidMapProto.VidMappingPB",
        "getFields",
    ),
    modeled_semantic("VidMappingPB", "getFields"),
    modeled_semantic("GenomicsDBVidMapProto.VidMappingPB", "getFields"),
    modeled_semantic(
        "com.intel.genomicsdb.model.GenomicsDBVidMapProto.VidMappingPB",
        "getFieldsCount",
    ),
    modeled_semantic("VidMappingPB", "getFieldsCount"),
    modeled_semantic("GenomicsDBVidMapProto.VidMappingPB", "getFieldsCount"),
    modeled_semantic(
        "com.intel.genomicsdb.model.GenomicsDBVidMapProto.FieldInfo",
        "getName",
    ),
    modeled_semantic("FieldInfo", "getName"),
    modeled_semantic("GenomicsDBVidMapProto.FieldInfo", "getName"),
    modeled_semantic(
        "com.intel.genomicsdb.model.GenomicsDBVidMapProto.FieldLengthDescriptorComponentPB",
        "newBuilder",
    ),
    modeled_semantic("FieldLengthDescriptorComponentPB", "newBuilder"),
    modeled_semantic(
        "GenomicsDBVidMapProto.FieldLengthDescriptorComponentPB.Builder",
        "build",
    ),
    modeled_semantic("FieldLengthDescriptorComponentPB.Builder", "build"),
    modeled_semantic(
        "com.intel.genomicsdb.GenomicsDBImporter",
        "getProtobufVidMapping",
    ),
    modeled_semantic("GenomicsDBImporter", "getProtobufVidMapping"),
    modeled_semantic(
        "com.intel.genomicsdb.GenomicsDBImporter",
        "updateProtobufVidMapping",
    ),
    modeled_semantic("GenomicsDBImporter", "updateProtobufVidMapping"),
    modeled_semantic(
        "com.intel.genomicsdb.GenomicsDBImporter",
        "coalesceContigsIntoNumPartitions",
    ),
    modeled_semantic("GenomicsDBImporter", "coalesceContigsIntoNumPartitions"),
    modeled_semantic("com.intel.genomicsdb.GenomicsDBImporter", "executeImport"),
    modeled_semantic("GenomicsDBImporter", "executeImport"),
    modeled_semantic(
        "com.intel.genomicsdb.importer.model.ImportConfig",
        "setFunctionToCallOnBatchCompletion",
    ),
    modeled_semantic("ImportConfig", "setFunctionToCallOnBatchCompletion"),
    modeled_semantic(
        "com.intel.genomicsdb.importer.model.ImportConfig",
        "setOutputCallsetmapJsonFile",
    ),
    modeled_semantic("ImportConfig", "setOutputCallsetmapJsonFile"),
    modeled_semantic(
        "com.intel.genomicsdb.importer.model.ImportConfig",
        "setOutputVcfHeaderFile",
    ),
    modeled_semantic("ImportConfig", "setOutputVcfHeaderFile"),
    modeled_semantic(
        "com.intel.genomicsdb.importer.model.ImportConfig",
        "setOutputVidmapJsonFile",
    ),
    modeled_semantic("ImportConfig", "setOutputVidmapJsonFile"),
    modeled_semantic(
        "com.intel.genomicsdb.importer.model.ImportConfig",
        "setUseSamplesInOrder",
    ),
    modeled_semantic("ImportConfig", "setUseSamplesInOrder"),
];

const PICARD_PROGRAMGROUPS_MODELED_CONSTRUCTORS: &[ExternalPackModeledConstructor] = &[
    modeled_semantic_constructor(
        "picard.cmdline.programgroups.DiagnosticsAndQCProgramGroup",
        0,
    ),
    modeled_semantic_constructor("DiagnosticsAndQCProgramGroup", 0),
    modeled_semantic_constructor(
        "picard.cmdline.programgroups.IntervalsManipulationProgramGroup",
        0,
    ),
    modeled_semantic_constructor("IntervalsManipulationProgramGroup", 0),
    modeled_semantic_constructor("picard.cmdline.programgroups.OtherProgramGroup", 0),
    modeled_semantic_constructor("OtherProgramGroup", 0),
    modeled_semantic_constructor(
        "picard.cmdline.programgroups.ReadDataManipulationProgramGroup",
        0,
    ),
    modeled_semantic_constructor("ReadDataManipulationProgramGroup", 0),
    modeled_semantic_constructor("picard.cmdline.programgroups.ReferenceProgramGroup", 0),
    modeled_semantic_constructor("ReferenceProgramGroup", 0),
    modeled_semantic_constructor(
        "picard.cmdline.programgroups.VariantEvaluationProgramGroup",
        0,
    ),
    modeled_semantic_constructor("VariantEvaluationProgramGroup", 0),
    modeled_semantic_constructor(
        "picard.cmdline.programgroups.VariantFilteringProgramGroup",
        0,
    ),
    modeled_semantic_constructor("VariantFilteringProgramGroup", 0),
    modeled_semantic_constructor(
        "picard.cmdline.programgroups.VariantManipulationProgramGroup",
        0,
    ),
    modeled_semantic_constructor("VariantManipulationProgramGroup", 0),
];

const PICARD_MARKDUPLICATES_MODELED_FIELDS: &[ExternalPackModeledField] = &[
    modeled_semantic_field(
        "picard.sam.markduplicates.MarkDuplicates",
        "DuplicateTaggingPolicy",
    ),
    modeled_semantic_field("MarkDuplicates", "DuplicateTaggingPolicy"),
    modeled_semantic_field(
        "picard.sam.markduplicates.MarkDuplicates.DuplicateTaggingPolicy",
        "DontTag",
    ),
    modeled_semantic_field("MarkDuplicates.DuplicateTaggingPolicy", "DontTag"),
    modeled_semantic_field("DuplicateTaggingPolicy", "DontTag"),
    modeled_semantic_field(
        "picard.sam.markduplicates.MarkDuplicates.DuplicateTaggingPolicy",
        "OpticalOnly",
    ),
    modeled_semantic_field("MarkDuplicates.DuplicateTaggingPolicy", "OpticalOnly"),
    modeled_semantic_field("DuplicateTaggingPolicy", "OpticalOnly"),
    modeled_semantic_field(
        "picard.sam.markduplicates.MarkDuplicates.DuplicateTaggingPolicy",
        "All",
    ),
    modeled_semantic_field("MarkDuplicates.DuplicateTaggingPolicy", "All"),
    modeled_semantic_field("DuplicateTaggingPolicy", "All"),
    modeled_semantic_field("picard.sam.markduplicates.util.ReadEnds", "FF"),
    modeled_semantic_field("ReadEnds", "FF"),
    modeled_semantic_field("picard.sam.markduplicates.util.ReadEnds", "FR"),
    modeled_semantic_field("ReadEnds", "FR"),
    modeled_semantic_field("picard.sam.markduplicates.util.ReadEnds", "RF"),
    modeled_semantic_field("ReadEnds", "RF"),
    modeled_semantic_field("picard.sam.markduplicates.util.ReadEnds", "RR"),
    modeled_semantic_field("ReadEnds", "RR"),
    modeled_semantic_field("picard.sam.markduplicates.util.ReadEnds", "F"),
    modeled_semantic_field("ReadEnds", "F"),
    modeled_semantic_field("picard.sam.markduplicates.util.ReadEnds", "R"),
    modeled_semantic_field("ReadEnds", "R"),
];

const PICARD_METRICS_MODELED_FIELDS: &[ExternalPackModeledField] = &[
    modeled_semantic_field("picard.sam.DuplicationMetrics", "LIBRARY"),
    modeled_semantic_field("DuplicationMetrics", "LIBRARY"),
    modeled_semantic_field("picard.sam.DuplicationMetrics", "UNPAIRED_READS_EXAMINED"),
    modeled_semantic_field("DuplicationMetrics", "UNPAIRED_READS_EXAMINED"),
    modeled_semantic_field("picard.sam.DuplicationMetrics", "READ_PAIRS_EXAMINED"),
    modeled_semantic_field("DuplicationMetrics", "READ_PAIRS_EXAMINED"),
    modeled_semantic_field(
        "picard.sam.DuplicationMetrics",
        "SECONDARY_OR_SUPPLEMENTARY_RDS",
    ),
    modeled_semantic_field("DuplicationMetrics", "SECONDARY_OR_SUPPLEMENTARY_RDS"),
    modeled_semantic_field("picard.sam.DuplicationMetrics", "UNMAPPED_READS"),
    modeled_semantic_field("DuplicationMetrics", "UNMAPPED_READS"),
    modeled_semantic_field("picard.sam.DuplicationMetrics", "UNPAIRED_READ_DUPLICATES"),
    modeled_semantic_field("DuplicationMetrics", "UNPAIRED_READ_DUPLICATES"),
    modeled_semantic_field("picard.sam.DuplicationMetrics", "READ_PAIR_DUPLICATES"),
    modeled_semantic_field("DuplicationMetrics", "READ_PAIR_DUPLICATES"),
    modeled_semantic_field(
        "picard.sam.DuplicationMetrics",
        "READ_PAIR_OPTICAL_DUPLICATES",
    ),
    modeled_semantic_field("DuplicationMetrics", "READ_PAIR_OPTICAL_DUPLICATES"),
    modeled_semantic_field("picard.sam.DuplicationMetrics", "ESTIMATED_LIBRARY_SIZE"),
    modeled_semantic_field("DuplicationMetrics", "ESTIMATED_LIBRARY_SIZE"),
    modeled_semantic_field("picard.sam.DuplicationMetrics", "PERCENT_DUPLICATION"),
    modeled_semantic_field("DuplicationMetrics", "PERCENT_DUPLICATION"),
];

const PICARD_MARKDUPLICATES_MODELED_CONSTRUCTORS: &[ExternalPackModeledConstructor] = &[
    modeled_semantic_constructor("picard.sam.markduplicates.util.OpticalDuplicateFinder", 0),
    modeled_semantic_constructor("OpticalDuplicateFinder", 0),
    modeled_semantic_constructor("picard.sam.markduplicates.util.OpticalDuplicateFinder", 3),
    modeled_semantic_constructor("OpticalDuplicateFinder", 3),
];

const AVRO_MODELED_CONSTRUCTORS: &[ExternalPackModeledConstructor] = &[
    modeled_semantic_constructor("org.apache.avro.Schema.Parser", 0),
    modeled_semantic_constructor("Schema.Parser", 0),
    modeled_semantic_constructor("Parser", 0),
    modeled_semantic_constructor("org.apache.avro.generic.GenericDatumReader", 1),
    modeled_semantic_constructor("GenericDatumReader", 1),
];

const BARCLAY_HELP_MODELED_CONSTRUCTORS: &[ExternalPackModeledConstructor] = &[
    modeled_semantic_constructor("org.broadinstitute.barclay.help.DocException", 2),
    modeled_semantic_constructor("DocException", 2),
];

const BARCLAY_HELP_MODELED_FIELDS: &[ExternalPackModeledField] = &[
    modeled_semantic_field(
        "org.broadinstitute.barclay.help.TemplateProperties",
        "ARGUMENTS_POSITIONAL",
    ),
    modeled_semantic_field("TemplateProperties", "ARGUMENTS_POSITIONAL"),
    modeled_semantic_field(
        "org.broadinstitute.barclay.help.TemplateProperties",
        "ARGUMENT_DEFAULT_VALUE",
    ),
    modeled_semantic_field("TemplateProperties", "ARGUMENT_DEFAULT_VALUE"),
    modeled_semantic_field(
        "org.broadinstitute.barclay.help.TemplateProperties",
        "ARGUMENT_TYPE",
    ),
    modeled_semantic_field("TemplateProperties", "ARGUMENT_TYPE"),
];

const BARCLAY_ARGPARSER_MODELED_CONSTRUCTORS: &[ExternalPackModeledConstructor] = &[
    modeled_semantic_constructor(
        "org.broadinstitute.barclay.argparser.CommandLineException",
        1,
    ),
    modeled_semantic_constructor(
        "org.broadinstitute.barclay.argparser.CommandLineException",
        2,
    ),
    modeled_semantic_constructor("CommandLineException", 1),
    modeled_semantic_constructor("CommandLineException", 2),
    modeled_semantic_constructor(
        "org.broadinstitute.barclay.argparser.CommandLineException.BadArgumentValue",
        1,
    ),
    modeled_semantic_constructor(
        "org.broadinstitute.barclay.argparser.CommandLineException.BadArgumentValue",
        2,
    ),
    modeled_semantic_constructor(
        "org.broadinstitute.barclay.argparser.CommandLineException.BadArgumentValue",
        3,
    ),
    modeled_semantic_constructor("CommandLineException.BadArgumentValue", 1),
    modeled_semantic_constructor("CommandLineException.BadArgumentValue", 2),
    modeled_semantic_constructor("CommandLineException.BadArgumentValue", 3),
    modeled_semantic_constructor("BadArgumentValue", 1),
    modeled_semantic_constructor("BadArgumentValue", 2),
    modeled_semantic_constructor("BadArgumentValue", 3),
    modeled_semantic_constructor(
        "org.broadinstitute.barclay.argparser.CommandLineException.MissingArgument",
        1,
    ),
    modeled_semantic_constructor(
        "org.broadinstitute.barclay.argparser.CommandLineException.MissingArgument",
        2,
    ),
    modeled_semantic_constructor("CommandLineException.MissingArgument", 1),
    modeled_semantic_constructor("CommandLineException.MissingArgument", 2),
    modeled_semantic_constructor("MissingArgument", 1),
    modeled_semantic_constructor("MissingArgument", 2),
    modeled_semantic_constructor(
        "org.broadinstitute.barclay.argparser.CommandLineArgumentParser",
        3,
    ),
    modeled_semantic_constructor("CommandLineArgumentParser", 3),
];

const BARCLAY_ARGPARSER_MODELED_FIELDS: &[ExternalPackModeledField] = &[
    modeled_semantic_field(
        "org.broadinstitute.barclay.argparser.SpecialArgumentsCollection",
        "SHOW_HIDDEN",
    ),
    modeled_semantic_field("SpecialArgumentsCollection", "SHOW_HIDDEN"),
];

const BARCLAY_UTILS_MODELED_CONSTRUCTORS: &[ExternalPackModeledConstructor] = &[
    modeled_semantic_constructor("org.broadinstitute.barclay.utils.ClassFinder", 0),
    modeled_semantic_constructor("ClassFinder", 0),
];

const SCALA_TUPLE_MODELED_CONSTRUCTORS: &[ExternalPackModeledConstructor] = &[
    modeled_semantic_constructor("scala.Tuple2", 2),
    modeled_semantic_constructor("Tuple2", 2),
    modeled_semantic_constructor("scala.Tuple3", 3),
    modeled_semantic_constructor("Tuple3", 3),
    modeled_semantic_constructor("scala.Tuple4", 4),
    modeled_semantic_constructor("Tuple4", 4),
];

const KRYO_MODELED_CONSTRUCTORS: &[ExternalPackModeledConstructor] = &[
    modeled_semantic_constructor("com.esotericsoftware.kryo.Kryo", 0),
    modeled_semantic_constructor("Kryo", 0),
    modeled_semantic_constructor("com.esotericsoftware.kryo.io.Input", 1),
    modeled_semantic_constructor("Input", 1),
    modeled_semantic_constructor("com.esotericsoftware.kryo.io.Output", 1),
    modeled_semantic_constructor("Output", 1),
    modeled_semantic_constructor("com.esotericsoftware.kryo.serializers.FieldSerializer", 2),
    modeled_semantic_constructor("FieldSerializer", 2),
    modeled_semantic_constructor(
        "com.esotericsoftware.kryo.serializers.ImmutableMapSerializer",
        0,
    ),
    modeled_semantic_constructor("ImmutableMapSerializer", 0),
];

const OPENCSV_MODELED_CONSTRUCTORS: &[ExternalPackModeledConstructor] = &[
    modeled_semantic_constructor("com.opencsv.CSVReader", 1),
    modeled_semantic_constructor("CSVReader", 1),
    modeled_semantic_constructor("com.opencsv.CSVReader", 4),
    modeled_semantic_constructor("CSVReader", 4),
    modeled_semantic_constructor("com.opencsv.CSVWriter", 2),
    modeled_semantic_constructor("CSVWriter", 2),
    modeled_semantic_constructor("com.opencsv.CSVWriter", 4),
    modeled_semantic_constructor("CSVWriter", 4),
];

const ORG_JSON_MODELED_CONSTRUCTORS: &[ExternalPackModeledConstructor] = &[
    modeled_semantic_constructor("org.json.JSONObject", 0),
    modeled_semantic_constructor("JSONObject", 0),
    modeled_semantic_constructor("org.json.JSONObject", 1),
    modeled_semantic_constructor("JSONObject", 1),
    modeled_semantic_constructor("org.json.JSONArray", 0),
    modeled_semantic_constructor("JSONArray", 0),
    modeled_semantic_constructor("org.json.JSONArray", 1),
    modeled_semantic_constructor("JSONArray", 1),
    modeled_semantic_constructor("org.json.JSONTokener", 1),
    modeled_semantic_constructor("JSONTokener", 1),
];

const REFLECTIONS_MODELED_CONSTRUCTORS: &[ExternalPackModeledConstructor] = &[
    modeled_semantic_constructor("org.reflections.Reflections", 1),
    modeled_semantic_constructor("Reflections", 1),
];

const HADOOP_CONF_MODELED_CONSTRUCTORS: &[ExternalPackModeledConstructor] = &[
    modeled_semantic_constructor("org.apache.hadoop.conf.Configuration", 0),
    modeled_semantic_constructor("Configuration", 0),
];

const APACHE_COMMONS_CONFIG_MODELED_CONSTRUCTORS: &[ExternalPackModeledConstructor] = &[
    modeled_semantic_constructor(
        "org.apache.commons.configuration2.builder.fluent.Configurations",
        0,
    ),
    modeled_semantic_constructor("Configurations", 0),
];

const SPARK_MODELED_CONSTRUCTORS: &[ExternalPackModeledConstructor] = &[
    modeled_semantic_constructor("org.apache.spark.SparkConf", 0),
    modeled_semantic_constructor("SparkConf", 0),
    modeled_semantic_constructor("org.apache.spark.HashPartitioner", 1),
    modeled_semantic_constructor("HashPartitioner", 1),
    modeled_semantic_constructor("org.apache.spark.api.java.JavaSparkContext", 1),
    modeled_semantic_constructor("JavaSparkContext", 1),
    modeled_semantic_constructor("org.apache.spark.api.java.JavaRDD", 2),
    modeled_semantic_constructor("JavaRDD", 2),
    modeled_semantic_constructor("org.apache.spark.rdd.PartitionGroup", 1),
    modeled_semantic_constructor("PartitionGroup", 1),
    modeled_semantic_constructor("org.apache.spark.mllib.linalg.distributed.RowMatrix", 1),
    modeled_semantic_constructor("RowMatrix", 1),
];

const BWA_MODELED_CONSTRUCTORS: &[ExternalPackModeledConstructor] = &[
    modeled_semantic_constructor("org.broadinstitute.hellbender.utils.bwa.BwaMemIndex", 1),
    modeled_semantic_constructor("BwaMemIndex", 1),
    modeled_semantic_constructor("org.broadinstitute.hellbender.utils.bwa.BwaMemAligner", 1),
    modeled_semantic_constructor("BwaMemAligner", 1),
    modeled_semantic_constructor(
        "org.broadinstitute.hellbender.utils.bwa.BwaMemAlignment",
        16,
    ),
    modeled_semantic_constructor("BwaMemAlignment", 16),
];

const BWA_MODELED_FIELDS: &[ExternalPackModeledField] = &[
    modeled_semantic_field(
        "org.broadinstitute.hellbender.utils.bwa.BwaMemAligner",
        "MEM_F_ALL",
    ),
    modeled_semantic_field("BwaMemAligner", "MEM_F_ALL"),
];

const BWA_API_EFFECTS: &[ExternalPackApiEffect] = &[
    semantic_typed_effect(
        "org.broadinstitute.hellbender.utils.bwa.BwaMemAlignment",
        "getSamFlag",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "BwaMemAlignment",
        "getSamFlag",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "org.broadinstitute.hellbender.utils.bwa.BwaMemAlignment",
        "getRefId",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "BwaMemAlignment",
        "getRefId",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "org.broadinstitute.hellbender.utils.bwa.BwaMemAlignment",
        "getRefStart",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "BwaMemAlignment",
        "getRefStart",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "org.broadinstitute.hellbender.utils.bwa.BwaMemAlignment",
        "getRefEnd",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "BwaMemAlignment",
        "getRefEnd",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "org.broadinstitute.hellbender.utils.bwa.BwaMemAlignment",
        "getSeqStart",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "BwaMemAlignment",
        "getSeqStart",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "org.broadinstitute.hellbender.utils.bwa.BwaMemAlignment",
        "getSeqEnd",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "BwaMemAlignment",
        "getSeqEnd",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "org.broadinstitute.hellbender.utils.bwa.BwaMemAlignment",
        "getMapQual",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "BwaMemAlignment",
        "getMapQual",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "org.broadinstitute.hellbender.utils.bwa.BwaMemAlignment",
        "getNMismatches",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "BwaMemAlignment",
        "getNMismatches",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "org.broadinstitute.hellbender.utils.bwa.BwaMemAlignment",
        "getAlignerScore",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "BwaMemAlignment",
        "getAlignerScore",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "org.broadinstitute.hellbender.utils.bwa.BwaMemAlignment",
        "getSuboptimalScore",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "BwaMemAlignment",
        "getSuboptimalScore",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "org.broadinstitute.hellbender.utils.bwa.BwaMemAlignment",
        "getCigar",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "String",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "BwaMemAlignment",
        "getCigar",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "String",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "org.broadinstitute.hellbender.utils.bwa.BwaMemAlignment",
        "getMDTag",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "String",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "BwaMemAlignment",
        "getMDTag",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "String",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "org.broadinstitute.hellbender.utils.bwa.BwaMemAlignment",
        "getXATag",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "String",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "BwaMemAlignment",
        "getXATag",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "String",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "org.broadinstitute.hellbender.utils.bwa.BwaMemAlignment",
        "getMateRefId",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "BwaMemAlignment",
        "getMateRefId",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "org.broadinstitute.hellbender.utils.bwa.BwaMemAlignment",
        "getMateRefStart",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "BwaMemAlignment",
        "getMateRefStart",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "org.broadinstitute.hellbender.utils.bwa.BwaMemAlignment",
        "getTemplateLen",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "BwaMemAlignment",
        "getTemplateLen",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "int",
        NullabilityState::DefinitelyNonNull,
    ),
    api_effect(
        "org.broadinstitute.hellbender.utils.bwa.BwaMemAligner",
        "setIntraCtgOptions",
        ExternalApiRole::Mutator,
        ReceiverMutabilityFact::MutatesReceiver,
        None,
        NullabilityState::Unknown,
        &[],
    ),
    api_effect(
        "BwaMemAligner",
        "setIntraCtgOptions",
        ExternalApiRole::Mutator,
        ReceiverMutabilityFact::MutatesReceiver,
        None,
        NullabilityState::Unknown,
        &[],
    ),
    api_effect(
        "org.broadinstitute.hellbender.utils.bwa.BwaMemAligner",
        "setZDropOption",
        ExternalApiRole::Mutator,
        ReceiverMutabilityFact::MutatesReceiver,
        None,
        NullabilityState::Unknown,
        &[],
    ),
    api_effect(
        "BwaMemAligner",
        "setZDropOption",
        ExternalApiRole::Mutator,
        ReceiverMutabilityFact::MutatesReceiver,
        None,
        NullabilityState::Unknown,
        &[],
    ),
    semantic_typed_effect(
        "org.broadinstitute.hellbender.utils.bwa.BwaMemAligner",
        "alignSeqs",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "List<List<BwaMemAlignment>>",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "BwaMemAligner",
        "alignSeqs",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "List<List<BwaMemAlignment>>",
        NullabilityState::DefinitelyNonNull,
    ),
];

const BWA_MODELED_APIS: &[ExternalPackModeledApi] = &[
    modeled_semantic(
        "org.broadinstitute.hellbender.utils.bwa.BwaMemAlignment",
        "getSamFlag",
    ),
    modeled_semantic("BwaMemAlignment", "getSamFlag"),
    modeled_semantic(
        "org.broadinstitute.hellbender.utils.bwa.BwaMemAlignment",
        "getRefId",
    ),
    modeled_semantic("BwaMemAlignment", "getRefId"),
    modeled_semantic(
        "org.broadinstitute.hellbender.utils.bwa.BwaMemAlignment",
        "getRefStart",
    ),
    modeled_semantic("BwaMemAlignment", "getRefStart"),
    modeled_semantic(
        "org.broadinstitute.hellbender.utils.bwa.BwaMemAlignment",
        "getRefEnd",
    ),
    modeled_semantic("BwaMemAlignment", "getRefEnd"),
    modeled_semantic(
        "org.broadinstitute.hellbender.utils.bwa.BwaMemAlignment",
        "getSeqStart",
    ),
    modeled_semantic("BwaMemAlignment", "getSeqStart"),
    modeled_semantic(
        "org.broadinstitute.hellbender.utils.bwa.BwaMemAlignment",
        "getSeqEnd",
    ),
    modeled_semantic("BwaMemAlignment", "getSeqEnd"),
    modeled_semantic(
        "org.broadinstitute.hellbender.utils.bwa.BwaMemAlignment",
        "getMapQual",
    ),
    modeled_semantic("BwaMemAlignment", "getMapQual"),
    modeled_semantic(
        "org.broadinstitute.hellbender.utils.bwa.BwaMemAlignment",
        "getNMismatches",
    ),
    modeled_semantic("BwaMemAlignment", "getNMismatches"),
    modeled_semantic(
        "org.broadinstitute.hellbender.utils.bwa.BwaMemAlignment",
        "getAlignerScore",
    ),
    modeled_semantic("BwaMemAlignment", "getAlignerScore"),
    modeled_semantic(
        "org.broadinstitute.hellbender.utils.bwa.BwaMemAlignment",
        "getSuboptimalScore",
    ),
    modeled_semantic("BwaMemAlignment", "getSuboptimalScore"),
    modeled_semantic(
        "org.broadinstitute.hellbender.utils.bwa.BwaMemAlignment",
        "getCigar",
    ),
    modeled_semantic("BwaMemAlignment", "getCigar"),
    modeled_semantic(
        "org.broadinstitute.hellbender.utils.bwa.BwaMemAlignment",
        "getMDTag",
    ),
    modeled_semantic("BwaMemAlignment", "getMDTag"),
    modeled_semantic(
        "org.broadinstitute.hellbender.utils.bwa.BwaMemAlignment",
        "getXATag",
    ),
    modeled_semantic("BwaMemAlignment", "getXATag"),
    modeled_semantic(
        "org.broadinstitute.hellbender.utils.bwa.BwaMemAlignment",
        "getMateRefId",
    ),
    modeled_semantic("BwaMemAlignment", "getMateRefId"),
    modeled_semantic(
        "org.broadinstitute.hellbender.utils.bwa.BwaMemAlignment",
        "getMateRefStart",
    ),
    modeled_semantic("BwaMemAlignment", "getMateRefStart"),
    modeled_semantic(
        "org.broadinstitute.hellbender.utils.bwa.BwaMemAlignment",
        "getTemplateLen",
    ),
    modeled_semantic("BwaMemAlignment", "getTemplateLen"),
    modeled_semantic(
        "org.broadinstitute.hellbender.utils.bwa.BwaMemAligner",
        "setIntraCtgOptions",
    ),
    modeled_semantic("BwaMemAligner", "setIntraCtgOptions"),
    modeled_semantic(
        "org.broadinstitute.hellbender.utils.bwa.BwaMemAligner",
        "setZDropOption",
    ),
    modeled_semantic("BwaMemAligner", "setZDropOption"),
    modeled_semantic(
        "org.broadinstitute.hellbender.utils.bwa.BwaMemAligner",
        "alignSeqs",
    ),
    modeled_semantic("BwaMemAligner", "alignSeqs"),
];

const JGRAPHT_MODELED_CONSTRUCTORS: &[ExternalPackModeledConstructor] = &[
    modeled_semantic_constructor("org.jgrapht.io.IntegerComponentNameProvider", 0),
    modeled_semantic_constructor("IntegerComponentNameProvider", 0),
    modeled_semantic_constructor("org.jgrapht.graph.SimpleGraph", 1),
    modeled_semantic_constructor("SimpleGraph", 1),
    modeled_semantic_constructor("org.jgrapht.alg.ConnectivityInspector", 1),
    modeled_semantic_constructor("ConnectivityInspector", 1),
    modeled_semantic_constructor("org.jgrapht.alg.CycleDetector", 1),
    modeled_semantic_constructor("CycleDetector", 1),
    modeled_semantic_constructor("org.jgrapht.graph.DefaultDirectedWeightedGraph", 1),
    modeled_semantic_constructor("DefaultDirectedWeightedGraph", 1),
    modeled_semantic_constructor("org.jgrapht.io.DOTExporter", 5),
    modeled_semantic_constructor("DOTExporter", 5),
];

const FASTUTIL_MODELED_CONSTRUCTORS: &[ExternalPackModeledConstructor] = &[
    modeled_semantic_constructor("it.unimi.dsi.fastutil.ints.Int2ObjectArrayMap", 0),
    modeled_semantic_constructor("it.unimi.dsi.fastutil.ints.Int2ObjectArrayMap", 1),
    modeled_semantic_constructor("Int2ObjectArrayMap", 0),
    modeled_semantic_constructor("Int2ObjectArrayMap", 1),
    modeled_semantic_constructor("it.unimi.dsi.fastutil.ints.Int2ObjectOpenHashMap", 0),
    modeled_semantic_constructor("it.unimi.dsi.fastutil.ints.Int2ObjectOpenHashMap", 1),
    modeled_semantic_constructor("Int2ObjectOpenHashMap", 0),
    modeled_semantic_constructor("Int2ObjectOpenHashMap", 1),
    modeled_semantic_constructor("it.unimi.dsi.fastutil.ints.IntArrayList", 0),
    modeled_semantic_constructor("it.unimi.dsi.fastutil.ints.IntArrayList", 1),
    modeled_semantic_constructor("IntArrayList", 0),
    modeled_semantic_constructor("IntArrayList", 1),
    modeled_semantic_constructor("it.unimi.dsi.fastutil.ints.Int2IntArrayMap", 2),
    modeled_semantic_constructor("Int2IntArrayMap", 2),
    modeled_semantic_constructor("it.unimi.dsi.fastutil.objects.Object2IntOpenHashMap", 0),
    modeled_semantic_constructor("it.unimi.dsi.fastutil.objects.Object2IntOpenHashMap", 1),
    modeled_semantic_constructor("Object2IntOpenHashMap", 0),
    modeled_semantic_constructor("Object2IntOpenHashMap", 1),
];

const KRYO_SERIALIZERS_MODELED_CONSTRUCTORS: &[ExternalPackModeledConstructor] = &[
    modeled_semantic_constructor(
        "de.javakaffee.kryoserializers.UnmodifiableCollectionsSerializer",
        0,
    ),
    modeled_semantic_constructor("UnmodifiableCollectionsSerializer", 0),
];

const DISQ_WRITE_MODELED_CONSTRUCTORS: &[ExternalPackModeledConstructor] = &[
    modeled_semantic_constructor("org.disq_bio.disq.HtsjdkReadsRdd", 2),
    modeled_semantic_constructor("HtsjdkReadsRdd", 2),
    modeled_semantic_constructor("org.disq_bio.disq.HtsjdkVariantsRdd", 2),
    modeled_semantic_constructor("HtsjdkVariantsRdd", 2),
    modeled_semantic_constructor("org.disq_bio.disq.HtsjdkReadsTraversalParameters", 2),
    modeled_semantic_constructor("HtsjdkReadsTraversalParameters", 2),
    modeled_semantic_constructor("org.disq_bio.disq.TempPartsDirectoryWriteOption", 1),
    modeled_semantic_constructor("TempPartsDirectoryWriteOption", 1),
];

const DISQ_WRITE_FIELD_NULLABILITY: &[ExternalPackFieldNullability] = &[
    non_null_field("org.disq_bio.disq.ReadsFormatWriteOption", "BAM"),
    non_null_field("ReadsFormatWriteOption", "BAM"),
    non_null_field("org.disq_bio.disq.FileCardinalityWriteOption", "MULTIPLE"),
    non_null_field("FileCardinalityWriteOption", "MULTIPLE"),
    non_null_field("org.disq_bio.disq.FileCardinalityWriteOption", "SINGLE"),
    non_null_field("FileCardinalityWriteOption", "SINGLE"),
];

const DISQ_WRITE_MODELED_FIELDS: &[ExternalPackModeledField] = &[
    modeled_semantic_field("org.disq_bio.disq.ReadsFormatWriteOption", "BAM"),
    modeled_semantic_field("ReadsFormatWriteOption", "BAM"),
    modeled_semantic_field("org.disq_bio.disq.FileCardinalityWriteOption", "MULTIPLE"),
    modeled_semantic_field("FileCardinalityWriteOption", "MULTIPLE"),
    modeled_semantic_field("org.disq_bio.disq.FileCardinalityWriteOption", "SINGLE"),
    modeled_semantic_field("FileCardinalityWriteOption", "SINGLE"),
];

const INTEL_GKL_MODELED_CONSTRUCTORS: &[ExternalPackModeledConstructor] = &[
    modeled_semantic_constructor("com.intel.gkl.compression.IntelDeflaterFactory", 0),
    modeled_semantic_constructor("IntelDeflaterFactory", 0),
    modeled_semantic_constructor("com.intel.gkl.compression.IntelInflaterFactory", 0),
    modeled_semantic_constructor("IntelInflaterFactory", 0),
    modeled_semantic_constructor("com.intel.gkl.pdhmm.IntelPDHMM", 0),
    modeled_semantic_constructor("IntelPDHMM", 0),
    modeled_semantic_constructor("com.intel.gkl.pairhmm.IntelPairHmm", 0),
    modeled_semantic_constructor("IntelPairHmm", 0),
    modeled_semantic_constructor("com.intel.gkl.pairhmm.IntelPairHmmOMP", 0),
    modeled_semantic_constructor("IntelPairHmmOMP", 0),
];

const INTEL_GKL_MODELED_FIELDS: &[ExternalPackModeledField] = &[
    modeled_semantic_field("com.intel.gkl.pdhmm.PDHMMNativeArguments", "avxLevel"),
    modeled_semantic_field("PDHMMNativeArguments", "avxLevel"),
    modeled_semantic_field("com.intel.gkl.pdhmm.PDHMMNativeArguments", "openMPSetting"),
    modeled_semantic_field("PDHMMNativeArguments", "openMPSetting"),
    modeled_semantic_field(
        "com.intel.gkl.pdhmm.PDHMMNativeArguments",
        "maxNumberOfThreads",
    ),
    modeled_semantic_field("PDHMMNativeArguments", "maxNumberOfThreads"),
    modeled_semantic_field(
        "com.intel.gkl.pairhmm.PairHMMNativeArguments",
        "maxNumberOfThreads",
    ),
    modeled_semantic_field("PairHMMNativeArguments", "maxNumberOfThreads"),
    modeled_semantic_field(
        "com.intel.gkl.pairhmm.PairHMMNativeArguments",
        "useDoublePrecision",
    ),
    modeled_semantic_field("PairHMMNativeArguments", "useDoublePrecision"),
    modeled_semantic_field(
        "com.intel.gkl.pairhmm.IntelPairHmm.ReadDataHolder",
        "readBases",
    ),
    modeled_semantic_field("ReadDataHolder", "readBases"),
    modeled_semantic_field(
        "com.intel.gkl.pairhmm.IntelPairHmm.ReadDataHolder",
        "readQuals",
    ),
    modeled_semantic_field("ReadDataHolder", "readQuals"),
    modeled_semantic_field(
        "com.intel.gkl.pairhmm.IntelPairHmm.ReadDataHolder",
        "insertionGOP",
    ),
    modeled_semantic_field("ReadDataHolder", "insertionGOP"),
    modeled_semantic_field(
        "com.intel.gkl.pairhmm.IntelPairHmm.ReadDataHolder",
        "deletionGOP",
    ),
    modeled_semantic_field("ReadDataHolder", "deletionGOP"),
    modeled_semantic_field(
        "com.intel.gkl.pairhmm.IntelPairHmm.ReadDataHolder",
        "overallGCP",
    ),
    modeled_semantic_field("ReadDataHolder", "overallGCP"),
    modeled_semantic_field(
        "com.intel.gkl.pairhmm.IntelPairHmm.HaplotypeDataHolder",
        "haplotypeBases",
    ),
    modeled_semantic_field("HaplotypeDataHolder", "haplotypeBases"),
    modeled_semantic_field(
        "com.intel.gkl.pairhmm.IntelPairHmm.HaplotypeDataHolder",
        "haplotypePDBases",
    ),
    modeled_semantic_field("HaplotypeDataHolder", "haplotypePDBases"),
    modeled_semantic_field(
        "com.intel.gkl.pairhmm.IntelPairHmm.BatchCompletionCallbackFunctionArgument",
        "batchCount",
    ),
    modeled_semantic_field("BatchCompletionCallbackFunctionArgument", "batchCount"),
    modeled_semantic_field(
        "com.intel.gkl.pairhmm.IntelPairHmm.BatchCompletionCallbackFunctionArgument",
        "totalBatchCount",
    ),
    modeled_semantic_field("BatchCompletionCallbackFunctionArgument", "totalBatchCount"),
    modeled_semantic_field(
        "com.intel.gkl.smithwaterman.SWNativeAlignerResult",
        "alignment_offset",
    ),
    modeled_semantic_field("SWNativeAlignerResult", "alignment_offset"),
    modeled_semantic_field("com.intel.gkl.smithwaterman.SWNativeAlignerResult", "cigar"),
    modeled_semantic_field("SWNativeAlignerResult", "cigar"),
];

const GUAVA_IMMUTABLE_COLLECT_MODELED_CONSTRUCTORS: &[ExternalPackModeledConstructor] = &[
    modeled_semantic_constructor("com.google.common.collect.ImmutableMap.Builder", 0),
    modeled_semantic_constructor("ImmutableMap.Builder", 0),
    modeled_semantic_constructor("Builder", 0),
];

const GUAVA_CONCURRENT_MODELED_CONSTRUCTORS: &[ExternalPackModeledConstructor] = &[
    modeled_semantic_constructor("com.google.common.util.concurrent.ThreadFactoryBuilder", 0),
    modeled_semantic_constructor("ThreadFactoryBuilder", 0),
];

const GUAVA_CONCURRENT_API_EFFECTS: &[ExternalPackApiEffect] = &[
    semantic_fluent_mutator_effect(
        "com.google.common.util.concurrent.ThreadFactoryBuilder",
        "setNameFormat",
        "ThreadFactoryBuilder",
    ),
    semantic_fluent_mutator_effect(
        "ThreadFactoryBuilder",
        "setNameFormat",
        "ThreadFactoryBuilder",
    ),
    semantic_fluent_mutator_effect(
        "com.google.common.util.concurrent.ThreadFactoryBuilder",
        "setDaemon",
        "ThreadFactoryBuilder",
    ),
    semantic_fluent_mutator_effect("ThreadFactoryBuilder", "setDaemon", "ThreadFactoryBuilder"),
    semantic_typed_effect(
        "com.google.common.util.concurrent.ThreadFactoryBuilder",
        "build",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "ThreadFactory",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "ThreadFactoryBuilder",
        "build",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "ThreadFactory",
        NullabilityState::DefinitelyNonNull,
    ),
];

const GUAVA_CONCURRENT_MODELED_APIS: &[ExternalPackModeledApi] = &[
    modeled_semantic(
        "com.google.common.util.concurrent.ThreadFactoryBuilder",
        "setNameFormat",
    ),
    modeled_semantic("ThreadFactoryBuilder", "setNameFormat"),
    modeled_semantic(
        "com.google.common.util.concurrent.ThreadFactoryBuilder",
        "setDaemon",
    ),
    modeled_semantic("ThreadFactoryBuilder", "setDaemon"),
    modeled_semantic(
        "com.google.common.util.concurrent.ThreadFactoryBuilder",
        "build",
    ),
    modeled_semantic("ThreadFactoryBuilder", "build"),
];

const GUAVA_IMMUTABLE_COLLECT_API_EFFECTS: &[ExternalPackApiEffect] = &[
    semantic_fluent_mutator_effect(
        "com.google.common.collect.ImmutableMap.Builder",
        "put",
        "com.google.common.collect.ImmutableMap.Builder",
    ),
    semantic_fluent_mutator_effect("ImmutableMap.Builder", "put", "ImmutableMap.Builder"),
    semantic_typed_effect(
        "com.google.common.collect.ImmutableMap.Builder",
        "build",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "com.google.common.collect.ImmutableMap",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "ImmutableMap.Builder",
        "build",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "ImmutableMap",
        NullabilityState::DefinitelyNonNull,
    ),
];

const GUAVA_IMMUTABLE_COLLECT_MODELED_APIS: &[ExternalPackModeledApi] = &[
    modeled_semantic("com.google.common.collect.ImmutableMap.Builder", "put"),
    modeled_semantic("ImmutableMap.Builder", "put"),
    modeled_semantic("com.google.common.collect.ImmutableMap.Builder", "build"),
    modeled_semantic("ImmutableMap.Builder", "build"),
];

const BROAD_HELLBENDER_UTILS_ARGUMENT_EFFECTS: &[ExternalPackArgumentEffect] = &[
    readonly_borrowed_argument(
        "org.broadinstitute.hellbender.utils.Utils",
        "validate",
        0,
        "boolean",
    ),
    readonly_borrowed_argument("Utils", "validate", 0, "boolean"),
    readonly_borrowed_argument(
        "org.broadinstitute.hellbender.utils.Utils",
        "validateArg",
        0,
        "boolean",
    ),
    readonly_borrowed_argument("Utils", "validateArg", 0, "boolean"),
];

const BROAD_HELLBENDER_UTILS_API_EFFECTS: &[ExternalPackApiEffect] = &[
    semantic_side_effect_with_arity(
        "org.broadinstitute.hellbender.utils.Utils",
        "validate",
        2,
        NullabilityState::Unknown,
    ),
    semantic_side_effect_with_arity("Utils", "validate", 2, NullabilityState::Unknown),
    semantic_side_effect_with_arity(
        "org.broadinstitute.hellbender.utils.Utils",
        "validateArg",
        2,
        NullabilityState::Unknown,
    ),
    semantic_side_effect_with_arity("Utils", "validateArg", 2, NullabilityState::Unknown),
];

const BROAD_HELLBENDER_UTILS_MODELED_APIS: &[ExternalPackModeledApi] = &[
    modeled_semantic("org.broadinstitute.hellbender.utils.Utils", "validate"),
    modeled_semantic("Utils", "validate"),
    modeled_semantic("org.broadinstitute.hellbender.utils.Utils", "validateArg"),
    modeled_semantic("Utils", "validateArg"),
];

const BROAD_HTTP_MODELED_CONSTRUCTORS: &[ExternalPackModeledConstructor] = &[
    modeled_semantic_constructor(
        "org.broadinstitute.http.nio.HttpFileSystemProviderSettings",
        3,
    ),
    modeled_semantic_constructor("HttpFileSystemProviderSettings", 3),
    modeled_semantic_constructor(
        "org.broadinstitute.http.nio.HttpFileSystemProviderSettings.RetrySettings",
        5,
    ),
    modeled_semantic_constructor("HttpFileSystemProviderSettings.RetrySettings", 5),
    modeled_semantic_constructor("RetrySettings", 5),
];

const BROAD_HTTP_API_EFFECTS: &[ExternalPackApiEffect] = &[
    semantic_typed_effect_with_arity(
        "org.broadinstitute.hellbender.utils.gcs.BucketUtils",
        "getPrefetchingWrapper",
        1,
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Function<SeekableByteChannel, SeekableByteChannel>",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect_with_arity(
        "BucketUtils",
        "getPrefetchingWrapper",
        1,
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Function<SeekableByteChannel, SeekableByteChannel>",
        NullabilityState::DefinitelyNonNull,
    ),
    semantic_typed_effect(
        "org.broadinstitute.http.nio.HttpFileSystemProviderSettings",
        "redirect",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Object",
        NullabilityState::MaybeNull,
    ),
    semantic_typed_effect(
        "HttpFileSystemProviderSettings",
        "redirect",
        ExternalApiRole::Getter,
        ReceiverMutabilityFact::ReadOnly,
        "Object",
        NullabilityState::MaybeNull,
    ),
];

const BROAD_HTTP_MODELED_APIS: &[ExternalPackModeledApi] = &[
    modeled_semantic(
        "org.broadinstitute.hellbender.utils.gcs.BucketUtils",
        "getPrefetchingWrapper",
    ),
    modeled_semantic("BucketUtils", "getPrefetchingWrapper"),
    modeled_semantic(
        "org.broadinstitute.http.nio.HttpFileSystemProviderSettings",
        "redirect",
    ),
    modeled_semantic("HttpFileSystemProviderSettings", "redirect"),
];

const BROAD_HTTP_MODELED_FIELDS: &[ExternalPackModeledField] = &[
    modeled_semantic_field(
        "org.broadinstitute.http.nio.HttpFileSystemProviderSettings",
        "DEFAULT_SETTINGS",
    ),
    modeled_semantic_field("HttpFileSystemProviderSettings", "DEFAULT_SETTINGS"),
    modeled_semantic_field(
        "org.broadinstitute.http.nio.HttpFileSystemProvider",
        "SCHEME",
    ),
    modeled_semantic_field("HttpFileSystemProvider", "SCHEME"),
    modeled_semantic_field(
        "org.broadinstitute.http.nio.HttpsFileSystemProvider",
        "SCHEME",
    ),
    modeled_semantic_field("HttpsFileSystemProvider", "SCHEME"),
    modeled_semantic_field(
        "org.broadinstitute.http.nio.RetryHandler",
        "DEFAULT_RETRYABLE_HTTP_CODES",
    ),
    modeled_semantic_field("RetryHandler", "DEFAULT_RETRYABLE_HTTP_CODES"),
    modeled_semantic_field(
        "org.broadinstitute.http.nio.RetryHandler",
        "DEFAULT_RETRYABLE_EXCEPTIONS",
    ),
    modeled_semantic_field("RetryHandler", "DEFAULT_RETRYABLE_EXCEPTIONS"),
    modeled_semantic_field(
        "org.broadinstitute.http.nio.RetryHandler",
        "DEFALT_RETRYABLE_MESSAGES",
    ),
    modeled_semantic_field("RetryHandler", "DEFALT_RETRYABLE_MESSAGES"),
];

const PICARD_VCF_MODELED_CONSTRUCTORS: &[ExternalPackModeledConstructor] = &[
    modeled_semantic_constructor("picard.vcf.GenotypeConcordanceCounts", 0),
    modeled_semantic_constructor("GenotypeConcordanceCounts", 0),
    modeled_semantic_constructor("picard.vcf.GenotypeConcordanceSummaryMetrics", 5),
    modeled_semantic_constructor("GenotypeConcordanceSummaryMetrics", 5),
    modeled_semantic_constructor("picard.vcf.GenotypeConcordanceStates.TruthAndCallStates", 2),
    modeled_semantic_constructor("GenotypeConcordanceStates.TruthAndCallStates", 2),
    modeled_semantic_constructor("TruthAndCallStates", 2),
];

const PICARD_VCF_MODELED_FIELDS: &[ExternalPackModeledField] = &[
    modeled_semantic_field("picard.vcf.GenotypeConcordance", "CONTINGENCY_STATE_TAG"),
    modeled_semantic_field("GenotypeConcordance", "CONTINGENCY_STATE_TAG"),
    modeled_semantic_field("picard.vcf.GenotypeConcordanceStates", "CallState"),
    modeled_semantic_field("GenotypeConcordanceStates", "CallState"),
    modeled_semantic_field("picard.vcf.GenotypeConcordanceStates", "TruthState"),
    modeled_semantic_field("GenotypeConcordanceStates", "TruthState"),
    modeled_semantic_field("picard.vcf.GenotypeConcordanceStates.CallState", "HOM_REF"),
    modeled_semantic_field("GenotypeConcordanceStates.CallState", "HOM_REF"),
    modeled_semantic_field("CallState", "HOM_REF"),
    modeled_semantic_field(
        "picard.vcf.GenotypeConcordanceStates.CallState",
        "HET_REF_VAR1",
    ),
    modeled_semantic_field("GenotypeConcordanceStates.CallState", "HET_REF_VAR1"),
    modeled_semantic_field("CallState", "HET_REF_VAR1"),
    modeled_semantic_field("picard.vcf.GenotypeConcordanceStates.TruthState", "HOM_REF"),
    modeled_semantic_field("GenotypeConcordanceStates.TruthState", "HOM_REF"),
    modeled_semantic_field("TruthState", "HOM_REF"),
    modeled_semantic_field("picard.vcf.GenotypeConcordanceStates.TruthState", "NO_CALL"),
    modeled_semantic_field("GenotypeConcordanceStates.TruthState", "NO_CALL"),
    modeled_semantic_field("TruthState", "NO_CALL"),
    modeled_semantic_field(
        "picard.vcf.GenotypeConcordanceStates.TruthAndCallStates",
        "truthState",
    ),
    modeled_semantic_field("GenotypeConcordanceStates.TruthAndCallStates", "truthState"),
    modeled_semantic_field("TruthAndCallStates", "truthState"),
    modeled_semantic_field(
        "picard.vcf.GenotypeConcordanceStates.TruthAndCallStates",
        "callState",
    ),
    modeled_semantic_field("GenotypeConcordanceStates.TruthAndCallStates", "callState"),
    modeled_semantic_field("TruthAndCallStates", "callState"),
    modeled_semantic_field(
        "picard.vcf.GenotypeConcordanceSummaryMetrics",
        "GENOTYPE_CONCORDANCE",
    ),
    modeled_semantic_field("GenotypeConcordanceSummaryMetrics", "GENOTYPE_CONCORDANCE"),
    modeled_semantic_field(
        "picard.vcf.GenotypeConcordanceSummaryMetrics",
        "NON_REF_GENOTYPE_CONCORDANCE",
    ),
    modeled_semantic_field(
        "GenotypeConcordanceSummaryMetrics",
        "NON_REF_GENOTYPE_CONCORDANCE",
    ),
    modeled_semantic_field("picard.vcf.GenotypeConcordanceSummaryMetrics", "HET_PPV"),
    modeled_semantic_field("GenotypeConcordanceSummaryMetrics", "HET_PPV"),
    modeled_semantic_field(
        "picard.vcf.GenotypeConcordanceSummaryMetrics",
        "HET_SENSITIVITY",
    ),
    modeled_semantic_field("GenotypeConcordanceSummaryMetrics", "HET_SENSITIVITY"),
    modeled_semantic_field("picard.vcf.GenotypeConcordanceSummaryMetrics", "HOMVAR_PPV"),
    modeled_semantic_field("GenotypeConcordanceSummaryMetrics", "HOMVAR_PPV"),
    modeled_semantic_field(
        "picard.vcf.GenotypeConcordanceSummaryMetrics",
        "HOMVAR_SENSITIVITY",
    ),
    modeled_semantic_field("GenotypeConcordanceSummaryMetrics", "HOMVAR_SENSITIVITY"),
    modeled_semantic_field("picard.vcf.GenotypeConcordanceSummaryMetrics", "VAR_PPV"),
    modeled_semantic_field("GenotypeConcordanceSummaryMetrics", "VAR_PPV"),
    modeled_semantic_field(
        "picard.vcf.GenotypeConcordanceSummaryMetrics",
        "VAR_SENSITIVITY",
    ),
    modeled_semantic_field("GenotypeConcordanceSummaryMetrics", "VAR_SENSITIVITY"),
    modeled_semantic_field(
        "picard.vcf.GenotypeConcordanceSummaryMetrics",
        "VAR_SPECIFICITY",
    ),
    modeled_semantic_field("GenotypeConcordanceSummaryMetrics", "VAR_SPECIFICITY"),
];

const JACKSON_DATABIND_MODELED_FIELDS: &[ExternalPackModeledField] = &[
    modeled_semantic_field(
        "com.fasterxml.jackson.databind.MapperFeature",
        "ACCEPT_CASE_INSENSITIVE_PROPERTIES",
    ),
    modeled_semantic_field("MapperFeature", "ACCEPT_CASE_INSENSITIVE_PROPERTIES"),
    modeled_semantic_field(
        "com.fasterxml.jackson.databind.DeserializationFeature",
        "UNWRAP_ROOT_VALUE",
    ),
    modeled_semantic_field("DeserializationFeature", "UNWRAP_ROOT_VALUE"),
];

const HDF5_MODELED_FIELDS: &[ExternalPackModeledField] = &[
    modeled_mapped_field("HDF5File", "OpenMode"),
    modeled_mapped_field("HDF5File.OpenMode", "READ_ONLY"),
    modeled_mapped_field("HDF5File.OpenMode", "CREATE"),
    modeled_mapped_field("OpenMode", "READ_ONLY"),
    modeled_mapped_field("OpenMode", "CREATE"),
];

const SAM_HEADER_MODELED_FIELDS: &[ExternalPackModeledField] = &[
    modeled_semantic_field(
        "htsjdk.samtools.SAMSequenceRecord",
        "UNKNOWN_SEQUENCE_LENGTH",
    ),
    modeled_semantic_field("SAMSequenceRecord", "UNKNOWN_SEQUENCE_LENGTH"),
    modeled_semantic_field("htsjdk.samtools.SAMReadGroupRecord", "READ_GROUP_ID_TAG"),
    modeled_semantic_field("SAMReadGroupRecord", "READ_GROUP_ID_TAG"),
    modeled_semantic_field(
        "htsjdk.samtools.SAMReadGroupRecord",
        "READ_GROUP_SAMPLE_TAG",
    ),
    modeled_semantic_field("SAMReadGroupRecord", "READ_GROUP_SAMPLE_TAG"),
    modeled_semantic_field("htsjdk.samtools.SAMFileHeader", "SortOrder"),
    modeled_semantic_field("SAMFileHeader", "SortOrder"),
    modeled_semantic_field("htsjdk.samtools.SAMFileHeader", "GroupOrder"),
    modeled_semantic_field("SAMFileHeader", "GroupOrder"),
    modeled_semantic_field("htsjdk.samtools.SAMFileHeader.SortOrder", "coordinate"),
    modeled_semantic_field("SAMFileHeader.SortOrder", "coordinate"),
    modeled_semantic_field("SortOrder", "coordinate"),
    modeled_semantic_field("htsjdk.samtools.SAMFileHeader.SortOrder", "queryname"),
    modeled_semantic_field("SAMFileHeader.SortOrder", "queryname"),
    modeled_semantic_field("SortOrder", "queryname"),
    modeled_semantic_field("htsjdk.samtools.SAMFileHeader.SortOrder", "unsorted"),
    modeled_semantic_field("SAMFileHeader.SortOrder", "unsorted"),
    modeled_semantic_field("SortOrder", "unsorted"),
    modeled_semantic_field("htsjdk.samtools.SAMFileHeader.SortOrder", "unknown"),
    modeled_semantic_field("SAMFileHeader.SortOrder", "unknown"),
    modeled_semantic_field("SortOrder", "unknown"),
    modeled_semantic_field("htsjdk.samtools.SAMFileHeader.SortOrder", "duplicate"),
    modeled_semantic_field("SAMFileHeader.SortOrder", "duplicate"),
    modeled_semantic_field("SortOrder", "duplicate"),
    modeled_semantic_field("htsjdk.samtools.SAMFileHeader.GroupOrder", "none"),
    modeled_semantic_field("SAMFileHeader.GroupOrder", "none"),
    modeled_semantic_field("GroupOrder", "none"),
    modeled_semantic_field("htsjdk.samtools.SAMFileHeader.GroupOrder", "query"),
    modeled_semantic_field("SAMFileHeader.GroupOrder", "query"),
    modeled_semantic_field("GroupOrder", "query"),
];

const NO_MODELED_FIELDS: &[ExternalPackModeledField] = &[];

const VARIANTCONTEXT_MODELED_APIS: &[ExternalPackModeledApi] = &[
    modeled_semantic("htsjdk.variant.variantcontext.Allele", "create"),
    modeled_semantic("Allele", "create"),
    modeled_semantic("htsjdk.variant.variantcontext.Allele", "getBaseString"),
    modeled_semantic("Allele", "getBaseString"),
    modeled_semantic("htsjdk.variant.variantcontext.Allele", "getBases"),
    modeled_semantic("Allele", "getBases"),
    modeled_semantic("htsjdk.variant.variantcontext.Allele", "isReference"),
    modeled_semantic("Allele", "isReference"),
    modeled_semantic("htsjdk.variant.variantcontext.Allele", "isNoCall"),
    modeled_semantic("Allele", "isNoCall"),
    modeled_semantic("htsjdk.variant.variantcontext.Allele", "isNonReference"),
    modeled_semantic("Allele", "isNonReference"),
    modeled_semantic("htsjdk.variant.variantcontext.Allele", "isSymbolic"),
    modeled_semantic("Allele", "isSymbolic"),
    modeled_semantic("htsjdk.variant.variantcontext.Allele", "isCalled"),
    modeled_semantic("Allele", "isCalled"),
    modeled_semantic("htsjdk.variant.variantcontext.Allele", "isBreakpoint"),
    modeled_semantic("Allele", "isBreakpoint"),
    modeled_semantic("htsjdk.variant.variantcontext.Allele", "isSingleBreakend"),
    modeled_semantic("Allele", "isSingleBreakend"),
    modeled_semantic("htsjdk.variant.variantcontext.Allele", "isNonRefAllele"),
    modeled_semantic("Allele", "isNonRefAllele"),
    modeled_semantic("htsjdk.variant.variantcontext.Allele", "equals"),
    modeled_semantic("Allele", "equals"),
    modeled_semantic("htsjdk.variant.variantcontext.Allele", "oneIsPrefixOfOther"),
    modeled_semantic("Allele", "oneIsPrefixOfOther"),
    modeled_semantic("htsjdk.variant.variantcontext.Allele", "basesMatch"),
    modeled_semantic("Allele", "basesMatch"),
    modeled_semantic("htsjdk.variant.variantcontext.Allele", "extend"),
    modeled_semantic("Allele", "extend"),
    modeled_semantic("htsjdk.variant.variantcontext.Allele", "length"),
    modeled_semantic("Allele", "length"),
    modeled_semantic("htsjdk.variant.variantcontext.Allele", "getDisplayString"),
    modeled_semantic("Allele", "getDisplayString"),
    modeled_semantic("htsjdk.variant.variantcontext.VariantContext", "getContig"),
    modeled_semantic("VariantContext", "getContig"),
    modeled_semantic("htsjdk.variant.variantcontext.VariantContext", "getStart"),
    modeled_semantic("VariantContext", "getStart"),
    modeled_semantic("htsjdk.variant.variantcontext.VariantContext", "getEnd"),
    modeled_semantic("VariantContext", "getEnd"),
    modeled_semantic("htsjdk.variant.variantcontext.VariantContext", "getID"),
    modeled_semantic("VariantContext", "getID"),
    modeled_semantic("htsjdk.variant.variantcontext.VariantContext", "getAlleles"),
    modeled_semantic("VariantContext", "getAlleles"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.VariantContext",
        "getAlternateAlleles",
    ),
    modeled_semantic("VariantContext", "getAlternateAlleles"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.VariantContext",
        "getReference",
    ),
    modeled_semantic("VariantContext", "getReference"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.VariantContext",
        "getGenotypes",
    ),
    modeled_semantic("VariantContext", "getGenotypes"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.VariantContext",
        "hasGenotypes",
    ),
    modeled_semantic("VariantContext", "hasGenotypes"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.VariantContext",
        "extraStrictValidation",
    ),
    modeled_semantic("VariantContext", "extraStrictValidation"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.VariantContext",
        "validateReferenceBases",
    ),
    modeled_semantic("VariantContext", "validateReferenceBases"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.VariantContext",
        "validateRSIDs",
    ),
    modeled_semantic("VariantContext", "validateRSIDs"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.VariantContext",
        "validateAlternateAlleles",
    ),
    modeled_semantic("VariantContext", "validateAlternateAlleles"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.VariantContext",
        "validateChromosomeCounts",
    ),
    modeled_semantic("VariantContext", "validateChromosomeCounts"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.VariantContext",
        "getAttributes",
    ),
    modeled_semantic("VariantContext", "getAttributes"),
    modeled_semantic("htsjdk.variant.variantcontext.VariantContext", "getType"),
    modeled_semantic("VariantContext", "getType"),
    modeled_semantic("htsjdk.variant.variantcontext.VariantContext.Type", "name"),
    modeled_semantic("Type", "name"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.VariantContext.Type",
        "values",
    ),
    modeled_semantic("Type", "values"),
    modeled_semantic("htsjdk.variant.variantcontext.VariantContext", "hasID"),
    modeled_semantic("VariantContext", "hasID"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.VariantContext",
        "hasAttribute",
    ),
    modeled_semantic("VariantContext", "hasAttribute"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.VariantContext",
        "isBiallelic",
    ),
    modeled_semantic("VariantContext", "isBiallelic"),
    modeled_semantic("htsjdk.variant.variantcontext.VariantContext", "isSNP"),
    modeled_semantic("VariantContext", "isSNP"),
    modeled_semantic("htsjdk.variant.variantcontext.VariantContext", "isIndel"),
    modeled_semantic("VariantContext", "isIndel"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.VariantContext",
        "isNotFiltered",
    ),
    modeled_semantic("VariantContext", "isNotFiltered"),
    modeled_semantic("htsjdk.variant.variantcontext.VariantContext", "isMixed"),
    modeled_semantic("VariantContext", "isMixed"),
    modeled_semantic("htsjdk.variant.variantcontext.VariantContext", "isMNP"),
    modeled_semantic("VariantContext", "isMNP"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.VariantContext",
        "isSimpleInsertion",
    ),
    modeled_semantic("VariantContext", "isSimpleInsertion"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.VariantContext",
        "isSimpleDeletion",
    ),
    modeled_semantic("VariantContext", "isSimpleDeletion"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.VariantContext",
        "isComplexIndel",
    ),
    modeled_semantic("VariantContext", "isComplexIndel"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.VariantContext",
        "isStructuralIndel",
    ),
    modeled_semantic("VariantContext", "isStructuralIndel"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.VariantContext",
        "isMonomorphicInSamples",
    ),
    modeled_semantic("VariantContext", "isMonomorphicInSamples"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.VariantContext",
        "isPolymorphicInSamples",
    ),
    modeled_semantic("VariantContext", "isPolymorphicInSamples"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.VariantContext",
        "hasSymbolicAlleles",
    ),
    modeled_semantic("VariantContext", "hasSymbolicAlleles"),
    modeled_semantic("htsjdk.variant.variantcontext.VariantContext", "overlaps"),
    modeled_semantic("VariantContext", "overlaps"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.VariantContext",
        "getAttribute",
    ),
    modeled_semantic("VariantContext", "getAttribute"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.VariantContext",
        "getAttributeAsString",
    ),
    modeled_semantic("VariantContext", "getAttributeAsString"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.VariantContext",
        "getGenotype",
    ),
    modeled_semantic("VariantContext", "getGenotype"),
    modeled_semantic("htsjdk.variant.variantcontext.VariantContext", "getPloidy"),
    modeled_semantic("VariantContext", "getPloidy"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.VariantContext",
        "getNSamples",
    ),
    modeled_semantic("VariantContext", "getNSamples"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.VariantContext",
        "getCalledChrCount",
    ),
    modeled_semantic("VariantContext", "getCalledChrCount"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.VariantContext",
        "getHetCount",
    ),
    modeled_semantic("VariantContext", "getHetCount"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.VariantContext",
        "getHomVarCount",
    ),
    modeled_semantic("VariantContext", "getHomVarCount"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.VariantContext",
        "getNAlleles",
    ),
    modeled_semantic("VariantContext", "getNAlleles"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.VariantContext",
        "getAttributeAsInt",
    ),
    modeled_semantic("VariantContext", "getAttributeAsInt"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.VariantContext",
        "getAttributeAsDouble",
    ),
    modeled_semantic("VariantContext", "getAttributeAsDouble"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.VariantContext",
        "getAttributeAsBoolean",
    ),
    modeled_semantic("VariantContext", "getAttributeAsBoolean"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.VariantContext",
        "getAttributeAsList",
    ),
    modeled_semantic("VariantContext", "getAttributeAsList"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.CommonInfo",
        "getAttributeAsList",
    ),
    modeled_semantic("CommonInfo", "getAttributeAsList"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.VariantContext",
        "getAttributeAsDoubleList",
    ),
    modeled_semantic("VariantContext", "getAttributeAsDoubleList"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.VariantContext",
        "getAttributeAsIntList",
    ),
    modeled_semantic("VariantContext", "getAttributeAsIntList"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.VariantContext",
        "getIndelLengths",
    ),
    modeled_semantic("VariantContext", "getIndelLengths"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.VariantContext",
        "getAlternateAllele",
    ),
    modeled_semantic("VariantContext", "getAlternateAllele"),
    modeled_semantic("htsjdk.variant.variantcontext.VariantContext", "getAllele"),
    modeled_semantic("VariantContext", "getAllele"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.VariantContext",
        "getAlleleIndex",
    ),
    modeled_semantic("VariantContext", "getAlleleIndex"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.VariantContext",
        "hasGenotype",
    ),
    modeled_semantic("VariantContext", "hasGenotype"),
    modeled_semantic("htsjdk.variant.variantcontext.VariantContext", "hasAllele"),
    modeled_semantic("VariantContext", "hasAllele"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.VariantContext",
        "hasAlternateAllele",
    ),
    modeled_semantic("VariantContext", "hasAlternateAllele"),
    modeled_semantic("htsjdk.variant.variantcontext.VariantContext", "isSymbolic"),
    modeled_semantic("VariantContext", "isSymbolic"),
    modeled_semantic("htsjdk.variant.variantcontext.VariantContext", "isVariant"),
    modeled_semantic("VariantContext", "isVariant"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.VariantContext",
        "hasExtendedAttribute",
    ),
    modeled_semantic("VariantContext", "hasExtendedAttribute"),
    modeled_semantic("htsjdk.variant.variantcontext.VariantContext", "getFilters"),
    modeled_semantic("VariantContext", "getFilters"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.VariantContext",
        "getSampleNames",
    ),
    modeled_semantic("VariantContext", "getSampleNames"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.VariantContext",
        "getGenotypesOrderedByName",
    ),
    modeled_semantic("VariantContext", "getGenotypesOrderedByName"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.VariantContext",
        "getAnyAttribute",
    ),
    modeled_semantic("VariantContext", "getAnyAttribute"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.VariantContext",
        "getAttributeAsStringList",
    ),
    modeled_semantic("VariantContext", "getAttributeAsStringList"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.CommonInfo",
        "getAttributeAsStringList",
    ),
    modeled_semantic("CommonInfo", "getAttributeAsStringList"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.VariantContext",
        "getCommonInfo",
    ),
    modeled_semantic("VariantContext", "getCommonInfo"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.VariantContext",
        "getMaxPloidy",
    ),
    modeled_semantic("VariantContext", "getMaxPloidy"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.VariantContext",
        "getNoCallCount",
    ),
    modeled_semantic("VariantContext", "getNoCallCount"),
    modeled_semantic("htsjdk.variant.variantcontext.VariantContext", "getSource"),
    modeled_semantic("VariantContext", "getSource"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.VariantContext",
        "getDisplayString",
    ),
    modeled_semantic("VariantContext", "getDisplayString"),
    modeled_semantic("htsjdk.variant.variantcontext.VariantContext", "isFiltered"),
    modeled_semantic("VariantContext", "isFiltered"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.VariantContext",
        "filtersWereApplied",
    ),
    modeled_semantic("VariantContext", "filtersWereApplied"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.VariantContext",
        "contigsMatch",
    ),
    modeled_semantic("VariantContext", "contigsMatch"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.VariantContext",
        "getPhredScaledQual",
    ),
    modeled_semantic("VariantContext", "getPhredScaledQual"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.VariantContext",
        "getLog10PError",
    ),
    modeled_semantic("VariantContext", "getLog10PError"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.VariantContext",
        "hasLog10PError",
    ),
    modeled_semantic("VariantContext", "hasLog10PError"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.VariantContext",
        "getStructuralVariantType",
    ),
    modeled_semantic("VariantContext", "getStructuralVariantType"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.VariantContext",
        "toStringWithoutGenotypes",
    ),
    modeled_semantic("VariantContext", "toStringWithoutGenotypes"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.VariantContext",
        "toStringDecodeGenotypes",
    ),
    modeled_semantic("VariantContext", "toStringDecodeGenotypes"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.VariantContext",
        "getGLIndicesOfAlternateAllele",
    ),
    modeled_semantic("VariantContext", "getGLIndicesOfAlternateAllele"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.VariantContext",
        "getAlleleIndices",
    ),
    modeled_semantic("VariantContext", "getAlleleIndices"),
    modeled_semantic("htsjdk.variant.variantcontext.VariantContext", "contains"),
    modeled_semantic("VariantContext", "contains"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.VariantContext",
        "withinDistanceOf",
    ),
    modeled_semantic("VariantContext", "withinDistanceOf"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.VariantContext",
        "getLengthOnReference",
    ),
    modeled_semantic("VariantContext", "getLengthOnReference"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.VariantContext",
        "isSymbolicOrSV",
    ),
    modeled_semantic("VariantContext", "isSymbolicOrSV"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.VariantContext",
        "subContextFromSamples",
    ),
    modeled_semantic("VariantContext", "subContextFromSamples"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.VariantContextBuilder",
        "rmAttribute",
    ),
    modeled_semantic("VariantContextBuilder", "rmAttribute"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.VariantContextBuilder",
        "rmAttributes",
    ),
    modeled_semantic("VariantContextBuilder", "rmAttributes"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.VariantContextBuilder",
        "unfiltered",
    ),
    modeled_semantic("VariantContextBuilder", "unfiltered"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.VariantContextBuilder",
        "passFilters",
    ),
    modeled_semantic("VariantContextBuilder", "passFilters"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.VariantContextBuilder",
        "noGenotypes",
    ),
    modeled_semantic("VariantContextBuilder", "noGenotypes"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.VariantContextBuilder",
        "computeEndFromAlleles",
    ),
    modeled_semantic("VariantContextBuilder", "computeEndFromAlleles"),
    modeled_semantic("htsjdk.variant.variantcontext.VariantContextBuilder", "loc"),
    modeled_semantic("VariantContextBuilder", "loc"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.VariantContextBuilder",
        "noID",
    ),
    modeled_semantic("VariantContextBuilder", "noID"),
    modeled_semantic("htsjdk.variant.variantcontext.VariantContextBuilder", "chr"),
    modeled_semantic("VariantContextBuilder", "chr"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.VariantContextBuilder",
        "start",
    ),
    modeled_semantic("VariantContextBuilder", "start"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.VariantContextBuilder",
        "stop",
    ),
    modeled_semantic("VariantContextBuilder", "stop"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.VariantContextBuilder",
        "getStop",
    ),
    modeled_semantic("VariantContextBuilder", "getStop"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.VariantContextBuilder",
        "alleles",
    ),
    modeled_semantic("VariantContextBuilder", "alleles"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.VariantContextBuilder",
        "genotypes",
    ),
    modeled_semantic("VariantContextBuilder", "genotypes"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.VariantContextBuilder",
        "attribute",
    ),
    modeled_semantic("VariantContextBuilder", "attribute"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.VariantContextBuilder",
        "source",
    ),
    modeled_semantic("VariantContextBuilder", "source"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.VariantContextBuilder",
        "attributes",
    ),
    modeled_semantic("VariantContextBuilder", "attributes"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.VariantContextBuilder",
        "putAttributes",
    ),
    modeled_semantic("VariantContextBuilder", "putAttributes"),
    modeled_semantic("htsjdk.variant.variantcontext.VariantContextBuilder", "id"),
    modeled_semantic("VariantContextBuilder", "id"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.VariantContextBuilder",
        "log10PError",
    ),
    modeled_semantic("VariantContextBuilder", "log10PError"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.VariantContextBuilder",
        "filter",
    ),
    modeled_semantic("VariantContextBuilder", "filter"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.VariantContextBuilder",
        "filters",
    ),
    modeled_semantic("VariantContextBuilder", "filters"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.VariantContextBuilder",
        "getFilters",
    ),
    modeled_semantic("VariantContextBuilder", "getFilters"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.VariantContextBuilder",
        "getAttributes",
    ),
    modeled_semantic("VariantContextBuilder", "getAttributes"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.VariantContextBuilder",
        "getGenotypes",
    ),
    modeled_semantic("VariantContextBuilder", "getGenotypes"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.VariantContextBuilder",
        "make",
    ),
    modeled_semantic("VariantContextBuilder", "make"),
    modeled_semantic("htsjdk.variant.variantcontext.GenotypeBuilder", "name"),
    modeled_semantic("GenotypeBuilder", "name"),
    modeled_semantic("htsjdk.variant.variantcontext.GenotypeBuilder", "alleles"),
    modeled_semantic("GenotypeBuilder", "alleles"),
    modeled_semantic("htsjdk.variant.variantcontext.GenotypeBuilder", "attribute"),
    modeled_semantic("GenotypeBuilder", "attribute"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.GenotypeBuilder",
        "attributes",
    ),
    modeled_semantic("GenotypeBuilder", "attributes"),
    modeled_semantic("htsjdk.variant.variantcontext.GenotypeBuilder", "DP"),
    modeled_semantic("GenotypeBuilder", "DP"),
    modeled_semantic("htsjdk.variant.variantcontext.GenotypeBuilder", "GQ"),
    modeled_semantic("GenotypeBuilder", "GQ"),
    modeled_semantic("htsjdk.variant.variantcontext.GenotypeBuilder", "noGQ"),
    modeled_semantic("GenotypeBuilder", "noGQ"),
    modeled_semantic("htsjdk.variant.variantcontext.GenotypeBuilder", "noDP"),
    modeled_semantic("GenotypeBuilder", "noDP"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.GenotypeBuilder",
        "noAttributes",
    ),
    modeled_semantic("GenotypeBuilder", "noAttributes"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.GenotypeBuilder",
        "log10PError",
    ),
    modeled_semantic("GenotypeBuilder", "log10PError"),
    modeled_semantic("htsjdk.variant.variantcontext.GenotypeBuilder", "phased"),
    modeled_semantic("GenotypeBuilder", "phased"),
    modeled_semantic("htsjdk.variant.variantcontext.GenotypeBuilder", "PL"),
    modeled_semantic("GenotypeBuilder", "PL"),
    modeled_semantic("htsjdk.variant.variantcontext.GenotypeBuilder", "filters"),
    modeled_semantic("GenotypeBuilder", "filters"),
    modeled_semantic("htsjdk.variant.variantcontext.GenotypeBuilder", "AD"),
    modeled_semantic("GenotypeBuilder", "AD"),
    modeled_semantic("htsjdk.variant.variantcontext.GenotypeBuilder", "noAD"),
    modeled_semantic("GenotypeBuilder", "noAD"),
    modeled_semantic("htsjdk.variant.variantcontext.GenotypeBuilder", "noPL"),
    modeled_semantic("GenotypeBuilder", "noPL"),
    modeled_semantic("htsjdk.variant.variantcontext.GenotypeBuilder", "make"),
    modeled_semantic("GenotypeBuilder", "make"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.GenotypeBuilder",
        "makeWithShallowCopy",
    ),
    modeled_semantic("GenotypeBuilder", "makeWithShallowCopy"),
    modeled_semantic("htsjdk.variant.variantcontext.Genotype", "getSampleName"),
    modeled_semantic("Genotype", "getSampleName"),
    modeled_semantic("htsjdk.variant.variantcontext.Genotype", "getAlleles"),
    modeled_semantic("Genotype", "getAlleles"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.Genotype",
        "getExtendedAttributes",
    ),
    modeled_semantic("Genotype", "getExtendedAttributes"),
    modeled_semantic("htsjdk.variant.variantcontext.Genotype", "getPloidy"),
    modeled_semantic("Genotype", "getPloidy"),
    modeled_semantic("htsjdk.variant.variantcontext.Genotype", "getDP"),
    modeled_semantic("Genotype", "getDP"),
    modeled_semantic("htsjdk.variant.variantcontext.Genotype", "getAllele"),
    modeled_semantic("Genotype", "getAllele"),
    modeled_semantic("htsjdk.variant.variantcontext.Genotype", "getType"),
    modeled_semantic("Genotype", "getType"),
    modeled_semantic("htsjdk.variant.variantcontext.Genotype", "getAnyAttribute"),
    modeled_semantic("Genotype", "getAnyAttribute"),
    modeled_semantic("htsjdk.variant.variantcontext.Genotype", "hasAnyAttribute"),
    modeled_semantic("Genotype", "hasAnyAttribute"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.Genotype",
        "getExtendedAttribute",
    ),
    modeled_semantic("Genotype", "getExtendedAttribute"),
    modeled_semantic("htsjdk.variant.variantcontext.Genotype", "getLikelihoods"),
    modeled_semantic("Genotype", "getLikelihoods"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.GenotypeLikelihoods",
        "fromLog10Likelihoods",
    ),
    modeled_semantic("GenotypeLikelihoods", "fromLog10Likelihoods"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.GenotypeLikelihoods",
        "fromPLs",
    ),
    modeled_semantic("GenotypeLikelihoods", "fromPLs"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.GenotypeLikelihoods",
        "calculatePLindex",
    ),
    modeled_semantic("GenotypeLikelihoods", "calculatePLindex"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.GenotypeLikelihoods",
        "getAllelePair",
    ),
    modeled_semantic("GenotypeLikelihoods", "getAllelePair"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.GenotypeLikelihoods",
        "numLikelihoods",
    ),
    modeled_semantic("GenotypeLikelihoods", "numLikelihoods"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.GenotypeLikelihoods",
        "getAsPLs",
    ),
    modeled_semantic("GenotypeLikelihoods", "getAsPLs"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.GenotypeLikelihoods",
        "getAsVector",
    ),
    modeled_semantic("GenotypeLikelihoods", "getAsVector"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.GenotypeLikelihoods",
        "getGQLog10FromLikelihoods",
    ),
    modeled_semantic("GenotypeLikelihoods", "getGQLog10FromLikelihoods"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.GenotypeLikelihoods",
        "forEach",
    ),
    modeled_semantic("GenotypeLikelihoods", "forEach"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.Genotype",
        "getGenotypeString",
    ),
    modeled_semantic("Genotype", "getGenotypeString"),
    modeled_semantic("htsjdk.variant.variantcontext.Genotype", "hasAD"),
    modeled_semantic("Genotype", "hasAD"),
    modeled_semantic("htsjdk.variant.variantcontext.Genotype", "hasDP"),
    modeled_semantic("Genotype", "hasDP"),
    modeled_semantic("htsjdk.variant.variantcontext.Genotype", "hasLikelihoods"),
    modeled_semantic("Genotype", "hasLikelihoods"),
    modeled_semantic("htsjdk.variant.variantcontext.Genotype", "hasPL"),
    modeled_semantic("Genotype", "hasPL"),
    modeled_semantic("htsjdk.variant.variantcontext.Genotype", "hasGQ"),
    modeled_semantic("Genotype", "hasGQ"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.Genotype",
        "hasExtendedAttribute",
    ),
    modeled_semantic("Genotype", "hasExtendedAttribute"),
    modeled_semantic("htsjdk.variant.variantcontext.Genotype", "isCalled"),
    modeled_semantic("Genotype", "isCalled"),
    modeled_semantic("htsjdk.variant.variantcontext.Genotype", "isNoCall"),
    modeled_semantic("Genotype", "isNoCall"),
    modeled_semantic("htsjdk.variant.variantcontext.Genotype", "isHomRef"),
    modeled_semantic("Genotype", "isHomRef"),
    modeled_semantic("htsjdk.variant.variantcontext.Genotype", "isHomVar"),
    modeled_semantic("Genotype", "isHomVar"),
    modeled_semantic("htsjdk.variant.variantcontext.Genotype", "isHet"),
    modeled_semantic("Genotype", "isHet"),
    modeled_semantic("htsjdk.variant.variantcontext.Genotype", "isHetNonRef"),
    modeled_semantic("Genotype", "isHetNonRef"),
    modeled_semantic("htsjdk.variant.variantcontext.Genotype", "sameGenotype"),
    modeled_semantic("Genotype", "sameGenotype"),
    modeled_semantic("htsjdk.variant.variantcontext.Genotype", "isMixed"),
    modeled_semantic("Genotype", "isMixed"),
    modeled_semantic("htsjdk.variant.variantcontext.Genotype", "isPhased"),
    modeled_semantic("Genotype", "isPhased"),
    modeled_semantic("htsjdk.variant.variantcontext.Genotype", "isFiltered"),
    modeled_semantic("Genotype", "isFiltered"),
    modeled_semantic("htsjdk.variant.variantcontext.Genotype", "isNonInformative"),
    modeled_semantic("Genotype", "isNonInformative"),
    modeled_semantic("htsjdk.variant.variantcontext.Genotype", "getFilters"),
    modeled_semantic("Genotype", "getFilters"),
    modeled_semantic("htsjdk.variant.variantcontext.Genotype", "countAllele"),
    modeled_semantic("Genotype", "countAllele"),
    modeled_semantic("htsjdk.variant.variantcontext.GenotypesContext", "size"),
    modeled_semantic("GenotypesContext", "size"),
    modeled_semantic("htsjdk.variant.variantcontext.GenotypesContext", "iterator"),
    modeled_semantic("GenotypesContext", "iterator"),
    modeled_semantic("htsjdk.variant.variantcontext.GenotypesContext", "stream"),
    modeled_semantic("GenotypesContext", "stream"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.GenotypesContext",
        "iterateInSampleNameOrder",
    ),
    modeled_semantic("GenotypesContext", "iterateInSampleNameOrder"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.GenotypesContext",
        "spliterator",
    ),
    modeled_semantic("GenotypesContext", "spliterator"),
    modeled_semantic("htsjdk.variant.variantcontext.GenotypesContext", "get"),
    modeled_semantic("GenotypesContext", "get"),
    modeled_semantic("htsjdk.variant.variantcontext.GenotypesContext", "isEmpty"),
    modeled_semantic("GenotypesContext", "isEmpty"),
    modeled_semantic("htsjdk.variant.variantcontext.GenotypesContext", "add"),
    modeled_semantic("GenotypesContext", "add"),
    modeled_semantic("htsjdk.variant.variantcontext.GenotypesContext", "addAll"),
    modeled_semantic("GenotypesContext", "addAll"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.GenotypesContext",
        "containsSample",
    ),
    modeled_semantic("GenotypesContext", "containsSample"),
    modeled_semantic("htsjdk.variant.variantcontext.GenotypesContext", "replace"),
    modeled_semantic("GenotypesContext", "replace"),
    modeled_semantic("htsjdk.variant.variantcontext.GenotypesContext", "create"),
    modeled_semantic("GenotypesContext", "create"),
    modeled_semantic("htsjdk.variant.variantcontext.GenotypesContext", "copy"),
    modeled_semantic("GenotypesContext", "copy"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.GenotypesContext",
        "getSampleNames",
    ),
    modeled_semantic("GenotypesContext", "getSampleNames"),
    modeled_semantic("htsjdk.variant.variantcontext.GenotypeType", "values"),
    modeled_semantic("GenotypeType", "values"),
];

const HDF5_MODELED_APIS: &[ExternalPackModeledApi] = &[
    modeled_mapped("HDF5File", "isOpen"),
    modeled_mapped("HDF5File", "exists"),
    modeled_mapped("HDF5File", "contains"),
    modeled_mapped("HDF5File", "getFile"),
    modeled_mapped("HDF5File", "getDataset"),
    modeled_mapped("HDF5File", "read"),
    modeled_mapped("HDF5File", "readDoubleArray"),
    modeled_mapped("HDF5File", "readDoubleMatrix"),
    modeled_mapped("HDF5File", "readDouble"),
    modeled_mapped("HDF5File", "readStringArray"),
    modeled_mapped("HDF5File", "readIntArray"),
    modeled_mapped("HDF5File", "makeDouble"),
    modeled_mapped("HDF5File", "makeDoubleArray"),
    modeled_mapped("HDF5File", "makeDoubleMatrix"),
    modeled_mapped("HDF5File", "makeStringArray"),
    modeled_mapped("HDF5File", "isPresent"),
    modeled_mapped("HDF5File", "write"),
    modeled_mapped("HDF5File", "writeDoubleArray"),
    modeled_mapped("HDF5File", "writeIntArray"),
    modeled_mapped("HDF5File", "flush"),
    modeled_mapped("HDF5File", "close"),
    modeled_mapped("HDF5Library", "load"),
];

const SQLITE_MODELED_APIS: &[ExternalPackModeledApi] = &[
    modeled_mapped("org.sqlite.SQLiteConfig", "setReadOnly"),
    modeled_mapped("SQLiteConfig", "setReadOnly"),
    modeled_mapped("org.sqlite.SQLiteConfig", "setBusyTimeout"),
    modeled_mapped("SQLiteConfig", "setBusyTimeout"),
    modeled_mapped("org.sqlite.SQLiteConfig", "setSharedCache"),
    modeled_mapped("SQLiteConfig", "setSharedCache"),
    modeled_mapped("org.sqlite.SQLiteConfig", "setJournalMode"),
    modeled_mapped("SQLiteConfig", "setJournalMode"),
    modeled_mapped("org.sqlite.SQLiteConfig", "createConnection"),
    modeled_mapped("SQLiteConfig", "createConnection"),
    modeled_mapped("org.sqlite.SQLiteConfig", "toProperties"),
    modeled_mapped("SQLiteConfig", "toProperties"),
    modeled_mapped("org.sqlite.SQLiteConnection", "prepareStatement"),
    modeled_mapped("SQLiteConnection", "prepareStatement"),
    modeled_mapped("org.sqlite.SQLiteConnection", "createStatement"),
    modeled_mapped("SQLiteConnection", "createStatement"),
    modeled_mapped("org.sqlite.SQLiteConnection", "setAutoCommit"),
    modeled_mapped("SQLiteConnection", "setAutoCommit"),
    modeled_mapped("org.sqlite.SQLiteConnection", "commit"),
    modeled_mapped("SQLiteConnection", "commit"),
    modeled_mapped("org.sqlite.SQLiteConnection", "rollback"),
    modeled_mapped("SQLiteConnection", "rollback"),
    modeled_mapped("org.sqlite.SQLiteConnection", "close"),
    modeled_mapped("SQLiteConnection", "close"),
    modeled_mapped("org.sqlite.SQLiteStatement", "bindString"),
    modeled_mapped("SQLiteStatement", "bindString"),
    modeled_mapped("org.sqlite.SQLiteStatement", "bindInt"),
    modeled_mapped("SQLiteStatement", "bindInt"),
    modeled_mapped("org.sqlite.SQLiteStatement", "bindDouble"),
    modeled_mapped("SQLiteStatement", "bindDouble"),
    modeled_mapped("org.sqlite.SQLiteStatement", "clearParameters"),
    modeled_mapped("SQLiteStatement", "clearParameters"),
    modeled_mapped("org.sqlite.SQLiteStatement", "execute"),
    modeled_mapped("SQLiteStatement", "execute"),
    modeled_mapped("org.sqlite.SQLiteStatement", "executeUpdate"),
    modeled_mapped("SQLiteStatement", "executeUpdate"),
    modeled_mapped("org.sqlite.SQLiteStatement", "executeQuery"),
    modeled_mapped("SQLiteStatement", "executeQuery"),
    modeled_mapped("org.sqlite.SQLiteStatement", "close"),
    modeled_mapped("SQLiteStatement", "close"),
    modeled_mapped("org.sqlite.SQLiteResultSet", "next"),
    modeled_mapped("SQLiteResultSet", "next"),
    modeled_mapped("org.sqlite.SQLiteResultSet", "getString"),
    modeled_mapped("SQLiteResultSet", "getString"),
    modeled_mapped("org.sqlite.SQLiteResultSet", "getInt"),
    modeled_mapped("SQLiteResultSet", "getInt"),
    modeled_mapped("org.sqlite.SQLiteResultSet", "getLong"),
    modeled_mapped("SQLiteResultSet", "getLong"),
    modeled_mapped("org.sqlite.SQLiteResultSet", "getBoolean"),
    modeled_mapped("SQLiteResultSet", "getBoolean"),
    modeled_mapped("org.sqlite.SQLiteResultSet", "getDouble"),
    modeled_mapped("SQLiteResultSet", "getDouble"),
    modeled_mapped("org.sqlite.SQLiteResultSet", "close"),
    modeled_mapped("SQLiteResultSet", "close"),
];

const JAVA_SQL_MODELED_APIS: &[ExternalPackModeledApi] = &[
    modeled_semantic("java.sql.DriverManager", "getConnection"),
    modeled_semantic("driver_manager", "getConnection"),
    modeled_semantic("java.sql.Connection", "createStatement"),
    modeled_semantic("java.sql.Connection", "prepareStatement"),
    modeled_semantic("java.sql.Connection", "getMetaData"),
    modeled_semantic("java.sql.Connection", "commit"),
    modeled_semantic("java.sql.Connection", "rollback"),
    modeled_semantic("java.sql.Connection", "close"),
    modeled_semantic("java.sql.Statement", "executeQuery"),
    modeled_semantic("java.sql.Statement", "close"),
    modeled_semantic("java.sql.PreparedStatement", "executeQuery"),
    modeled_semantic("java.sql.PreparedStatement", "executeUpdate"),
    modeled_semantic("java.sql.PreparedStatement", "setString"),
    modeled_semantic("java.sql.PreparedStatement", "setInt"),
    modeled_semantic("java.sql.PreparedStatement", "setDouble"),
    modeled_semantic("java.sql.PreparedStatement", "setLong"),
    modeled_semantic("java.sql.PreparedStatement", "setBoolean"),
    modeled_semantic("java.sql.PreparedStatement", "close"),
    modeled_semantic("java.sql.ResultSet", "next"),
    modeled_semantic("java.sql.ResultSet", "getString"),
    modeled_semantic("java.sql.ResultSet", "getInt"),
    modeled_semantic("java.sql.ResultSet", "getLong"),
    modeled_semantic("java.sql.ResultSet", "getDouble"),
    modeled_semantic("java.sql.ResultSet", "getFloat"),
    modeled_semantic("java.sql.ResultSet", "getShort"),
    modeled_semantic("java.sql.ResultSet", "getByte"),
    modeled_semantic("java.sql.ResultSet", "getBoolean"),
    modeled_semantic("java.sql.ResultSet", "getObject"),
    modeled_semantic("java.sql.ResultSet", "getBytes"),
    modeled_semantic("java.sql.ResultSet", "getDate"),
    modeled_semantic("java.sql.ResultSet", "getTimestamp"),
    modeled_semantic("java.sql.ResultSet", "wasNull"),
    modeled_semantic("java.sql.ResultSet", "getMetaData"),
    modeled_semantic("java.sql.ResultSet", "close"),
    modeled_semantic("java.sql.ResultSetMetaData", "getColumnCount"),
    modeled_semantic("java.sql.ResultSetMetaData", "getColumnName"),
    modeled_semantic("java.sql.ResultSetMetaData", "getColumnType"),
    modeled_semantic("java.sql.ResultSetMetaData", "getColumnTypeName"),
    modeled_semantic("java.sql.ResultSetMetaData", "getColumnClassName"),
    modeled_semantic("java.sql.ResultSetMetaData", "isNullable"),
    modeled_semantic("java.sql.ResultSetMetaData", "isSigned"),
    modeled_semantic("java.sql.ResultSetMetaData", "isAutoIncrement"),
    modeled_semantic("java.sql.ResultSetMetaData", "getPrecision"),
    modeled_semantic("java.sql.ResultSetMetaData", "getScale"),
    modeled_semantic("java.sql.ResultSetMetaData", "getColumnDisplaySize"),
    modeled_semantic("java.sql.DatabaseMetaData", "getDatabaseProductName"),
    modeled_semantic("java.sql.DatabaseMetaData", "getTables"),
    modeled_semantic("java.sql.DatabaseMetaData", "getColumns"),
];

const VCF_MODELED_APIS: &[ExternalPackModeledApi] = &[
    modeled_semantic("htsjdk.variant.vcf.VCFFileReader", "getFileHeader"),
    modeled_semantic("VCFFileReader", "getFileHeader"),
    modeled_semantic("htsjdk.variant.vcf.VCFFileReader", "iterator"),
    modeled_semantic("VCFFileReader", "iterator"),
    modeled_semantic("htsjdk.variant.vcf.VCFHeader", "getGenotypeSamples"),
    modeled_semantic("VCFHeader", "getGenotypeSamples"),
    modeled_semantic("htsjdk.variant.vcf.VCFHeader", "getSampleNamesInOrder"),
    modeled_semantic("VCFHeader", "getSampleNamesInOrder"),
    modeled_semantic("htsjdk.variant.vcf.VCFHeader", "getMetaDataInSortedOrder"),
    modeled_semantic("VCFHeader", "getMetaDataInSortedOrder"),
    modeled_semantic("htsjdk.variant.vcf.VCFHeader", "getNGenotypeSamples"),
    modeled_semantic("VCFHeader", "getNGenotypeSamples"),
    modeled_semantic("htsjdk.variant.vcf.VCFHeader", "getMetaDataInInputOrder"),
    modeled_semantic("VCFHeader", "getMetaDataInInputOrder"),
    modeled_semantic("htsjdk.variant.vcf.VCFHeader", "getInfoHeaderLines"),
    modeled_semantic("VCFHeader", "getInfoHeaderLines"),
    modeled_semantic("htsjdk.variant.vcf.VCFHeader", "getInfoHeaderLine"),
    modeled_semantic("VCFHeader", "getInfoHeaderLine"),
    modeled_semantic("htsjdk.variant.vcf.VCFHeader", "hasInfoLine"),
    modeled_semantic("VCFHeader", "hasInfoLine"),
    modeled_semantic("htsjdk.variant.vcf.VCFHeader", "hasFormatLine"),
    modeled_semantic("VCFHeader", "hasFormatLine"),
    modeled_semantic("htsjdk.variant.vcf.VCFHeader", "hasGenotypingData"),
    modeled_semantic("VCFHeader", "hasGenotypingData"),
    modeled_semantic("htsjdk.variant.vcf.VCFHeader", "getFormatHeaderLine"),
    modeled_semantic("VCFHeader", "getFormatHeaderLine"),
    modeled_semantic("htsjdk.variant.vcf.VCFHeader", "getVCFRecordComparator"),
    modeled_semantic("VCFHeader", "getVCFRecordComparator"),
    modeled_semantic("htsjdk.variant.vcf.VCFHeader", "getMetaDataLine"),
    modeled_semantic("VCFHeader", "getMetaDataLine"),
    modeled_semantic("htsjdk.variant.vcf.VCFHeader", "getOtherHeaderLine"),
    modeled_semantic("VCFHeader", "getOtherHeaderLine"),
    modeled_semantic("htsjdk.variant.vcf.VCFHeader", "getFormatHeaderLines"),
    modeled_semantic("VCFHeader", "getFormatHeaderLines"),
    modeled_semantic("htsjdk.variant.vcf.VCFHeader", "getFilterLines"),
    modeled_semantic("VCFHeader", "getFilterLines"),
    modeled_semantic("htsjdk.variant.vcf.VCFHeader", "getContigLines"),
    modeled_semantic("VCFHeader", "getContigLines"),
    modeled_semantic("htsjdk.variant.vcf.VCFHeader", "getSequenceDictionary"),
    modeled_semantic("VCFHeader", "getSequenceDictionary"),
    modeled_semantic("htsjdk.variant.vcf.VCFHeader", "setSequenceDictionary"),
    modeled_semantic("VCFHeader", "setSequenceDictionary"),
    modeled_semantic("htsjdk.variant.vcf.VCFHeader", "getVCFHeaderVersion"),
    modeled_semantic("VCFHeader", "getVCFHeaderVersion"),
    modeled_semantic("htsjdk.variant.vcf.VCFHeaderVersion", "getFormatString"),
    modeled_semantic("VCFHeaderVersion", "getFormatString"),
    modeled_semantic("htsjdk.variant.vcf.VCFHeaderVersion", "getVersionString"),
    modeled_semantic("VCFHeaderVersion", "getVersionString"),
    modeled_semantic("htsjdk.variant.vcf.VCFHeader", "getColumnCount"),
    modeled_semantic("VCFHeader", "getColumnCount"),
    modeled_semantic("htsjdk.variant.vcf.VCFHeader", "addMetaDataLine"),
    modeled_semantic("VCFHeader", "addMetaDataLine"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.writer.VariantContextWriterBuilder",
        "clearOptions",
    ),
    modeled_semantic("VariantContextWriterBuilder", "clearOptions"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.writer.VariantContextWriterBuilder",
        "setCreateMD5",
    ),
    modeled_semantic("VariantContextWriterBuilder", "setCreateMD5"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.writer.VariantContextWriterBuilder",
        "setOption",
    ),
    modeled_semantic("VariantContextWriterBuilder", "setOption"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.writer.VariantContextWriterBuilder",
        "setOutputFileType",
    ),
    modeled_semantic("VariantContextWriterBuilder", "setOutputFileType"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.writer.VariantContextWriterBuilder",
        "setReferenceDictionary",
    ),
    modeled_semantic("VariantContextWriterBuilder", "setReferenceDictionary"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.writer.VariantContextWriterBuilder",
        "setOutputPath",
    ),
    modeled_semantic("VariantContextWriterBuilder", "setOutputPath"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.writer.VariantContextWriterBuilder",
        "determineOutputTypeFromFile",
    ),
    modeled_semantic("VariantContextWriterBuilder", "determineOutputTypeFromFile"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.writer.VariantContextWriterBuilder",
        "build",
    ),
    modeled_semantic("VariantContextWriterBuilder", "build"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.writer.VariantContextWriter",
        "setHeader",
    ),
    modeled_semantic("VariantContextWriter", "setHeader"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.writer.VariantContextWriter",
        "writeHeader",
    ),
    modeled_semantic("VariantContextWriter", "writeHeader"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.writer.VariantContextWriter",
        "add",
    ),
    modeled_semantic("VariantContextWriter", "add"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.writer.VariantContextWriter",
        "checkError",
    ),
    modeled_semantic("VariantContextWriter", "checkError"),
    modeled_semantic(
        "htsjdk.variant.variantcontext.writer.VariantContextWriter",
        "close",
    ),
    modeled_semantic("VariantContextWriter", "close"),
    modeled_semantic("htsjdk.variant.vcf.VCFHeaderLine", "getKey"),
    modeled_semantic("VCFHeaderLine", "getKey"),
    modeled_semantic("htsjdk.variant.vcf.VCFInfoHeaderLine", "getKey"),
    modeled_semantic("VCFInfoHeaderLine", "getKey"),
    modeled_semantic("htsjdk.variant.vcf.VCFFormatHeaderLine", "getKey"),
    modeled_semantic("VCFFormatHeaderLine", "getKey"),
    modeled_semantic("htsjdk.variant.vcf.VCFFilterHeaderLine", "getKey"),
    modeled_semantic("VCFFilterHeaderLine", "getKey"),
    modeled_semantic("htsjdk.variant.vcf.VCFHeaderLine", "getValue"),
    modeled_semantic("VCFHeaderLine", "getValue"),
    modeled_semantic("htsjdk.variant.vcf.VCFInfoHeaderLine", "getValue"),
    modeled_semantic("VCFInfoHeaderLine", "getValue"),
    modeled_semantic("htsjdk.variant.vcf.VCFFormatHeaderLine", "getValue"),
    modeled_semantic("VCFFormatHeaderLine", "getValue"),
    modeled_semantic("htsjdk.variant.vcf.VCFFilterHeaderLine", "getValue"),
    modeled_semantic("VCFFilterHeaderLine", "getValue"),
    modeled_semantic("htsjdk.variant.vcf.VCFHeaderLine", "getID"),
    modeled_semantic("VCFHeaderLine", "getID"),
    modeled_semantic("htsjdk.variant.vcf.VCFInfoHeaderLine", "getID"),
    modeled_semantic("VCFInfoHeaderLine", "getID"),
    modeled_semantic("htsjdk.variant.vcf.VCFFormatHeaderLine", "getID"),
    modeled_semantic("VCFFormatHeaderLine", "getID"),
    modeled_semantic("htsjdk.variant.vcf.VCFFilterHeaderLine", "getID"),
    modeled_semantic("VCFFilterHeaderLine", "getID"),
    modeled_semantic("htsjdk.variant.vcf.VCFHeaderLine", "getDescription"),
    modeled_semantic("VCFHeaderLine", "getDescription"),
    modeled_semantic("htsjdk.variant.vcf.VCFInfoHeaderLine", "getDescription"),
    modeled_semantic("VCFInfoHeaderLine", "getDescription"),
    modeled_semantic("htsjdk.variant.vcf.VCFFormatHeaderLine", "getDescription"),
    modeled_semantic("VCFFormatHeaderLine", "getDescription"),
    modeled_semantic("htsjdk.variant.vcf.VCFFilterHeaderLine", "getDescription"),
    modeled_semantic("VCFFilterHeaderLine", "getDescription"),
    modeled_semantic("htsjdk.variant.vcf.VCFHeaderLine", "toString"),
    modeled_semantic("VCFHeaderLine", "toString"),
    modeled_semantic("htsjdk.variant.vcf.VCFInfoHeaderLine", "toString"),
    modeled_semantic("VCFInfoHeaderLine", "toString"),
    modeled_semantic("htsjdk.variant.vcf.VCFFormatHeaderLine", "toString"),
    modeled_semantic("VCFFormatHeaderLine", "toString"),
    modeled_semantic("htsjdk.variant.vcf.VCFFilterHeaderLine", "toString"),
    modeled_semantic("VCFFilterHeaderLine", "toString"),
    modeled_semantic("htsjdk.variant.vcf.VCFInfoHeaderLine", "getCount"),
    modeled_semantic("VCFInfoHeaderLine", "getCount"),
    modeled_semantic("htsjdk.variant.vcf.VCFFormatHeaderLine", "getCount"),
    modeled_semantic("VCFFormatHeaderLine", "getCount"),
    modeled_semantic("htsjdk.variant.vcf.VCFInfoHeaderLine", "getCountType"),
    modeled_semantic("VCFInfoHeaderLine", "getCountType"),
    modeled_semantic("htsjdk.variant.vcf.VCFFormatHeaderLine", "getCountType"),
    modeled_semantic("VCFFormatHeaderLine", "getCountType"),
    modeled_semantic("htsjdk.variant.vcf.VCFHeaderLineType", "valueOf"),
    modeled_semantic("VCFHeaderLineType", "valueOf"),
    modeled_semantic("htsjdk.variant.vcf.VCFInfoHeaderLine", "isFixedCount"),
    modeled_semantic("VCFInfoHeaderLine", "isFixedCount"),
    modeled_semantic("htsjdk.variant.vcf.VCFFormatHeaderLine", "isFixedCount"),
    modeled_semantic("VCFFormatHeaderLine", "isFixedCount"),
    modeled_semantic("htsjdk.variant.vcf.VCFInfoHeaderLine", "getType"),
    modeled_semantic("VCFInfoHeaderLine", "getType"),
    modeled_semantic("htsjdk.variant.vcf.VCFFormatHeaderLine", "getType"),
    modeled_semantic("VCFFormatHeaderLine", "getType"),
];

const SAM_HEADER_MODELED_APIS: &[ExternalPackModeledApi] = &[
    modeled_semantic("htsjdk.samtools.SAMSequenceDictionary", "getSequence"),
    modeled_semantic("SAMSequenceDictionary", "getSequence"),
    modeled_semantic("htsjdk.samtools.SAMSequenceDictionary", "getSequenceIndex"),
    modeled_semantic("SAMSequenceDictionary", "getSequenceIndex"),
    modeled_semantic(
        "htsjdk.samtools.SAMSequenceDictionary",
        "getReferenceLength",
    ),
    modeled_semantic("SAMSequenceDictionary", "getReferenceLength"),
    modeled_semantic("htsjdk.samtools.SAMSequenceDictionary", "getSequences"),
    modeled_semantic("SAMSequenceDictionary", "getSequences"),
    modeled_semantic(
        "htsjdk.samtools.SAMSequenceDictionary",
        "assertSameDictionary",
    ),
    modeled_semantic("SAMSequenceDictionary", "assertSameDictionary"),
    modeled_semantic("htsjdk.samtools.SAMSequenceDictionary", "isSameDictionary"),
    modeled_semantic("SAMSequenceDictionary", "isSameDictionary"),
    modeled_semantic("htsjdk.samtools.SAMSequenceDictionary", "addSequence"),
    modeled_semantic("SAMSequenceDictionary", "addSequence"),
    modeled_semantic("htsjdk.samtools.SAMSequenceDictionary", "size"),
    modeled_semantic("SAMSequenceDictionary", "size"),
    modeled_semantic("htsjdk.samtools.SAMSequenceDictionary", "isEmpty"),
    modeled_semantic("SAMSequenceDictionary", "isEmpty"),
    modeled_semantic("htsjdk.samtools.SAMFileHeader", "getComments"),
    modeled_semantic("SAMFileHeader", "getComments"),
    modeled_semantic("htsjdk.samtools.SAMSequenceRecord", "getSequenceName"),
    modeled_semantic("SAMSequenceRecord", "getSequenceName"),
    modeled_semantic("htsjdk.samtools.SAMSequenceRecord", "getSequenceLength"),
    modeled_semantic("SAMSequenceRecord", "getSequenceLength"),
    modeled_semantic("htsjdk.samtools.SAMSequenceRecord", "setSequenceLength"),
    modeled_semantic("SAMSequenceRecord", "setSequenceLength"),
    modeled_semantic("htsjdk.samtools.SAMSequenceRecord", "getSequenceIndex"),
    modeled_semantic("SAMSequenceRecord", "getSequenceIndex"),
    modeled_semantic("htsjdk.samtools.SAMSequenceRecord", "getMd5"),
    modeled_semantic("SAMSequenceRecord", "getMd5"),
    modeled_semantic("htsjdk.samtools.SAMSequenceRecord", "getAttribute"),
    modeled_semantic("SAMSequenceRecord", "getAttribute"),
    modeled_semantic("htsjdk.samtools.SAMSequenceRecord", "getAttributes"),
    modeled_semantic("SAMSequenceRecord", "getAttributes"),
    modeled_semantic("htsjdk.samtools.SAMSequenceRecord", "getAssembly"),
    modeled_semantic("SAMSequenceRecord", "getAssembly"),
    modeled_semantic("htsjdk.samtools.SAMSequenceRecord", "truncateSequenceName"),
    modeled_semantic("SAMSequenceRecord", "truncateSequenceName"),
    modeled_semantic("htsjdk.samtools.SAMFileHeader", "getSequenceDictionary"),
    modeled_semantic("SAMFileHeader", "getSequenceDictionary"),
    modeled_semantic("htsjdk.samtools.SAMFileHeader", "getSequence"),
    modeled_semantic("SAMFileHeader", "getSequence"),
    modeled_semantic("htsjdk.samtools.SAMFileHeader", "getSequenceIndex"),
    modeled_semantic("SAMFileHeader", "getSequenceIndex"),
    modeled_semantic("htsjdk.samtools.SAMFileHeader", "getSortOrder"),
    modeled_semantic("SAMFileHeader", "getSortOrder"),
    modeled_semantic("htsjdk.samtools.SAMFileHeader", "getGroupOrder"),
    modeled_semantic("SAMFileHeader", "getGroupOrder"),
    modeled_semantic("htsjdk.samtools.SAMFileHeader", "getReadGroups"),
    modeled_semantic("SAMFileHeader", "getReadGroups"),
    modeled_semantic("htsjdk.samtools.SAMFileHeader", "getReadGroup"),
    modeled_semantic("SAMFileHeader", "getReadGroup"),
    modeled_semantic("htsjdk.samtools.SAMFileHeader", "clone"),
    modeled_semantic("SAMFileHeader", "clone"),
    modeled_semantic("htsjdk.samtools.SAMFileHeader", "addSequence"),
    modeled_semantic("SAMFileHeader", "addSequence"),
    modeled_semantic("htsjdk.samtools.SAMTextHeaderCodec", "encode"),
    modeled_semantic("SAMTextHeaderCodec", "encode"),
    modeled_semantic("htsjdk.samtools.SAMFileHeader", "getProgramRecords"),
    modeled_semantic("SAMFileHeader", "getProgramRecords"),
    modeled_semantic("htsjdk.samtools.SAMFileHeader", "getProgramRecord"),
    modeled_semantic("SAMFileHeader", "getProgramRecord"),
    modeled_semantic("htsjdk.samtools.SAMFileHeader", "getSAMString"),
    modeled_semantic("SAMFileHeader", "getSAMString"),
    modeled_semantic("htsjdk.samtools.SAMFileHeader", "setSequenceDictionary"),
    modeled_semantic("SAMFileHeader", "setSequenceDictionary"),
    modeled_semantic("htsjdk.samtools.SAMFileHeader", "setComments"),
    modeled_semantic("SAMFileHeader", "setComments"),
    modeled_semantic("htsjdk.samtools.SAMFileHeader", "setReadGroups"),
    modeled_semantic("SAMFileHeader", "setReadGroups"),
    modeled_semantic("htsjdk.samtools.SAMFileHeader", "setProgramRecords"),
    modeled_semantic("SAMFileHeader", "setProgramRecords"),
    modeled_semantic("htsjdk.samtools.SAMFileHeader", "setAttribute"),
    modeled_semantic("SAMFileHeader", "setAttribute"),
    modeled_semantic("htsjdk.samtools.SAMFileHeader", "setSortOrder"),
    modeled_semantic("SAMFileHeader", "setSortOrder"),
    modeled_semantic("htsjdk.samtools.SAMFileHeader", "setGroupOrder"),
    modeled_semantic("SAMFileHeader", "setGroupOrder"),
    modeled_semantic("htsjdk.samtools.SAMFileHeader.SortOrder", "name"),
    modeled_semantic("SAMFileHeader.SortOrder", "name"),
    modeled_semantic("SortOrder", "name"),
    modeled_semantic("htsjdk.samtools.SAMFileHeader.GroupOrder", "name"),
    modeled_semantic("SAMFileHeader.GroupOrder", "name"),
    modeled_semantic("GroupOrder", "name"),
    modeled_semantic("htsjdk.samtools.SAMFileHeader", "addReadGroup"),
    modeled_semantic("SAMFileHeader", "addReadGroup"),
    modeled_semantic("htsjdk.samtools.SAMFileHeader", "addProgramRecord"),
    modeled_semantic("SAMFileHeader", "addProgramRecord"),
    modeled_semantic("htsjdk.samtools.SAMProgramRecord", "getId"),
    modeled_semantic("SAMProgramRecord", "getId"),
    modeled_semantic("htsjdk.samtools.SAMProgramRecord", "getProgramName"),
    modeled_semantic("SAMProgramRecord", "getProgramName"),
    modeled_semantic("htsjdk.samtools.SAMProgramRecord", "getProgramGroupId"),
    modeled_semantic("SAMProgramRecord", "getProgramGroupId"),
    modeled_semantic("htsjdk.samtools.SAMProgramRecord", "setCommandLine"),
    modeled_semantic("SAMProgramRecord", "setCommandLine"),
    modeled_semantic(
        "htsjdk.samtools.SAMProgramRecord",
        "setPreviousProgramGroupId",
    ),
    modeled_semantic("SAMProgramRecord", "setPreviousProgramGroupId"),
    modeled_semantic("htsjdk.samtools.SAMProgramRecord", "setProgramName"),
    modeled_semantic("SAMProgramRecord", "setProgramName"),
    modeled_semantic("htsjdk.samtools.SAMProgramRecord", "setProgramVersion"),
    modeled_semantic("SAMProgramRecord", "setProgramVersion"),
    modeled_semantic("htsjdk.samtools.SAMReadGroupRecord", "getId"),
    modeled_semantic("SAMReadGroupRecord", "getId"),
    modeled_semantic("htsjdk.samtools.SAMReadGroupRecord", "getSample"),
    modeled_semantic("SAMReadGroupRecord", "getSample"),
    modeled_semantic("htsjdk.samtools.SAMReadGroupRecord", "getLibrary"),
    modeled_semantic("SAMReadGroupRecord", "getLibrary"),
    modeled_semantic("htsjdk.samtools.SAMReadGroupRecord", "getPlatform"),
    modeled_semantic("SAMReadGroupRecord", "getPlatform"),
    modeled_semantic("htsjdk.samtools.SAMReadGroupRecord", "getPlatformUnit"),
    modeled_semantic("SAMReadGroupRecord", "getPlatformUnit"),
    modeled_semantic("htsjdk.samtools.SAMReadGroupRecord", "getFlowOrder"),
    modeled_semantic("SAMReadGroupRecord", "getFlowOrder"),
    modeled_semantic("htsjdk.samtools.SAMReadGroupRecord", "getSequencingCenter"),
    modeled_semantic("SAMReadGroupRecord", "getSequencingCenter"),
    modeled_semantic("htsjdk.samtools.SAMReadGroupRecord", "getAttribute"),
    modeled_semantic("SAMReadGroupRecord", "getAttribute"),
    modeled_semantic("htsjdk.samtools.SAMReadGroupRecord", "getReadGroupId"),
    modeled_semantic("SAMReadGroupRecord", "getReadGroupId"),
    modeled_semantic("htsjdk.samtools.SAMReadGroupRecord", "setSample"),
    modeled_semantic("SAMReadGroupRecord", "setSample"),
    modeled_semantic("htsjdk.samtools.SAMReadGroupRecord", "setLibrary"),
    modeled_semantic("SAMReadGroupRecord", "setLibrary"),
    modeled_semantic("htsjdk.samtools.SAMReadGroupRecord", "setPlatform"),
    modeled_semantic("SAMReadGroupRecord", "setPlatform"),
    modeled_semantic("htsjdk.samtools.SAMReadGroupRecord", "setPlatformUnit"),
    modeled_semantic("SAMReadGroupRecord", "setPlatformUnit"),
    modeled_semantic("htsjdk.samtools.SAMReadGroupRecord", "setFlowOrder"),
    modeled_semantic("SAMReadGroupRecord", "setFlowOrder"),
    modeled_semantic("htsjdk.samtools.SAMReadGroupRecord", "setSequencingCenter"),
    modeled_semantic("SAMReadGroupRecord", "setSequencingCenter"),
    modeled_semantic("htsjdk.samtools.SAMReadGroupRecord", "setAttribute"),
    modeled_semantic("SAMReadGroupRecord", "setAttribute"),
];

const SAM_RECORD_MODELED_APIS: &[ExternalPackModeledApi] = &[
    modeled_semantic("htsjdk.samtools.SAMRecord", "getContig"),
    modeled_semantic("SAMRecord", "getContig"),
    modeled_semantic("htsjdk.samtools.SAMRecord", "getStart"),
    modeled_semantic("SAMRecord", "getStart"),
    modeled_semantic("htsjdk.samtools.SAMRecord", "getEnd"),
    modeled_semantic("SAMRecord", "getEnd"),
    modeled_semantic("htsjdk.samtools.SAMRecord", "getReadName"),
    modeled_semantic("SAMRecord", "getReadName"),
    modeled_semantic("htsjdk.samtools.SAMRecord", "getHeader"),
    modeled_semantic("SAMRecord", "getHeader"),
    modeled_semantic("htsjdk.samtools.SAMRecordCoordinateComparator", "compare"),
    modeled_semantic("SAMRecordCoordinateComparator", "compare"),
    modeled_semantic("htsjdk.samtools.SAMRecordQueryNameComparator", "compare"),
    modeled_semantic("SAMRecordQueryNameComparator", "compare"),
    modeled_semantic("htsjdk.samtools.Cigar", "add"),
    modeled_semantic("Cigar", "add"),
    modeled_semantic("htsjdk.samtools.Cigar", "getCigarElement"),
    modeled_semantic("Cigar", "getCigarElement"),
    modeled_semantic("htsjdk.samtools.Cigar", "getCigarElements"),
    modeled_semantic("Cigar", "getCigarElements"),
    modeled_semantic("htsjdk.samtools.Cigar", "getReadLength"),
    modeled_semantic("Cigar", "getReadLength"),
    modeled_semantic("htsjdk.samtools.Cigar", "getReferenceLength"),
    modeled_semantic("Cigar", "getReferenceLength"),
    modeled_semantic("htsjdk.samtools.Cigar", "getPaddedReferenceLength"),
    modeled_semantic("Cigar", "getPaddedReferenceLength"),
    modeled_semantic("htsjdk.samtools.Cigar", "isEmpty"),
    modeled_semantic("Cigar", "isEmpty"),
    modeled_semantic("htsjdk.samtools.Cigar", "getFirstCigarElement"),
    modeled_semantic("Cigar", "getFirstCigarElement"),
    modeled_semantic("htsjdk.samtools.Cigar", "getLastCigarElement"),
    modeled_semantic("Cigar", "getLastCigarElement"),
    modeled_semantic("htsjdk.samtools.Cigar", "isValid"),
    modeled_semantic("Cigar", "isValid"),
    modeled_semantic("htsjdk.samtools.Cigar", "numCigarElements"),
    modeled_semantic("Cigar", "numCigarElements"),
    modeled_semantic("htsjdk.samtools.CigarElement", "getOperator"),
    modeled_semantic("CigarElement", "getOperator"),
    modeled_semantic("htsjdk.samtools.CigarOperator", "consumesReadBases"),
    modeled_semantic("CigarOperator", "consumesReadBases"),
    modeled_semantic("htsjdk.samtools.CigarOperator", "consumesReferenceBases"),
    modeled_semantic("CigarOperator", "consumesReferenceBases"),
    modeled_semantic("htsjdk.samtools.CigarOperator", "isClipping"),
    modeled_semantic("CigarOperator", "isClipping"),
    modeled_semantic("htsjdk.samtools.CigarOperator", "isAlignment"),
    modeled_semantic("CigarOperator", "isAlignment"),
    modeled_semantic("htsjdk.samtools.CigarOperator", "isIndel"),
    modeled_semantic("CigarOperator", "isIndel"),
    modeled_semantic("htsjdk.samtools.CigarOperator", "characterToEnum"),
    modeled_semantic("CigarOperator", "characterToEnum"),
    modeled_semantic("htsjdk.samtools.SAMRecord", "getCigar"),
    modeled_semantic("SAMRecord", "getCigar"),
    modeled_semantic("htsjdk.samtools.SAMRecord", "getReadLength"),
    modeled_semantic("SAMRecord", "getReadLength"),
    modeled_semantic("htsjdk.samtools.SAMRecord", "getAlignmentStart"),
    modeled_semantic("SAMRecord", "getAlignmentStart"),
    modeled_semantic("htsjdk.samtools.SAMRecord", "getAlignmentEnd"),
    modeled_semantic("SAMRecord", "getAlignmentEnd"),
    modeled_semantic("htsjdk.samtools.SAMRecord", "getMappingQuality"),
    modeled_semantic("SAMRecord", "getMappingQuality"),
    modeled_semantic("htsjdk.samtools.SAMRecord", "getFlags"),
    modeled_semantic("SAMRecord", "getFlags"),
    modeled_semantic("htsjdk.samtools.SAMRecord", "getMateAlignmentStart"),
    modeled_semantic("SAMRecord", "getMateAlignmentStart"),
    modeled_semantic("htsjdk.samtools.SAMRecord", "getInferredInsertSize"),
    modeled_semantic("SAMRecord", "getInferredInsertSize"),
    modeled_semantic("htsjdk.samtools.SAMRecord", "getCigarLength"),
    modeled_semantic("SAMRecord", "getCigarLength"),
    modeled_semantic("htsjdk.samtools.SAMRecord", "getMateAlignmentEnd"),
    modeled_semantic("SAMRecord", "getMateAlignmentEnd"),
    modeled_semantic("htsjdk.samtools.SAMRecord", "getMateReferenceIndex"),
    modeled_semantic("SAMRecord", "getMateReferenceIndex"),
    modeled_semantic("htsjdk.samtools.SAMRecord", "getUnclippedStart"),
    modeled_semantic("SAMRecord", "getUnclippedStart"),
    modeled_semantic("htsjdk.samtools.SAMRecord", "getUnclippedEnd"),
    modeled_semantic("SAMRecord", "getUnclippedEnd"),
    modeled_semantic("htsjdk.samtools.SAMRecord", "getReadUnmappedFlag"),
    modeled_semantic("SAMRecord", "getReadUnmappedFlag"),
    modeled_semantic("htsjdk.samtools.SAMRecord", "getReadNegativeStrandFlag"),
    modeled_semantic("SAMRecord", "getReadNegativeStrandFlag"),
    modeled_semantic("htsjdk.samtools.SAMRecord", "getReadPairedFlag"),
    modeled_semantic("SAMRecord", "getReadPairedFlag"),
    modeled_semantic("htsjdk.samtools.SAMRecord", "getMateUnmappedFlag"),
    modeled_semantic("SAMRecord", "getMateUnmappedFlag"),
    modeled_semantic("htsjdk.samtools.SAMRecord", "getSecondOfPairFlag"),
    modeled_semantic("SAMRecord", "getSecondOfPairFlag"),
    modeled_semantic("htsjdk.samtools.SAMRecord", "getFirstOfPairFlag"),
    modeled_semantic("SAMRecord", "getFirstOfPairFlag"),
    modeled_semantic("htsjdk.samtools.SAMRecord", "getProperPairFlag"),
    modeled_semantic("SAMRecord", "getProperPairFlag"),
    modeled_semantic("htsjdk.samtools.SAMRecord", "getDuplicateReadFlag"),
    modeled_semantic("SAMRecord", "getDuplicateReadFlag"),
    modeled_semantic("htsjdk.samtools.SAMRecord", "isSecondaryAlignment"),
    modeled_semantic("SAMRecord", "isSecondaryAlignment"),
    modeled_semantic("htsjdk.samtools.SAMRecord", "isSecondaryOrSupplementary"),
    modeled_semantic("SAMRecord", "isSecondaryOrSupplementary"),
    modeled_semantic("htsjdk.samtools.SAMRecord", "getSupplementaryAlignmentFlag"),
    modeled_semantic("SAMRecord", "getSupplementaryAlignmentFlag"),
    modeled_semantic("htsjdk.samtools.SAMRecord", "getMateNegativeStrandFlag"),
    modeled_semantic("SAMRecord", "getMateNegativeStrandFlag"),
    modeled_semantic(
        "htsjdk.samtools.SAMRecord",
        "getReadFailsVendorQualityCheckFlag",
    ),
    modeled_semantic("SAMRecord", "getReadFailsVendorQualityCheckFlag"),
    modeled_semantic("htsjdk.samtools.SAMRecord", "getReferenceName"),
    modeled_semantic("SAMRecord", "getReferenceName"),
    modeled_semantic("htsjdk.samtools.SAMRecord", "getMateReferenceName"),
    modeled_semantic("SAMRecord", "getMateReferenceName"),
    modeled_semantic("htsjdk.samtools.SAMRecord", "getCigarString"),
    modeled_semantic("SAMRecord", "getCigarString"),
    modeled_semantic("htsjdk.samtools.SAMRecord", "getSAMString"),
    modeled_semantic("SAMRecord", "getSAMString"),
    modeled_semantic("htsjdk.samtools.SAMRecord", "getAttribute"),
    modeled_semantic("SAMRecord", "getAttribute"),
    modeled_semantic("htsjdk.samtools.SAMRecord", "getIntegerAttribute"),
    modeled_semantic("SAMRecord", "getIntegerAttribute"),
    modeled_semantic("htsjdk.samtools.SAMRecord", "getByteArrayAttribute"),
    modeled_semantic("SAMRecord", "getByteArrayAttribute"),
    modeled_semantic("htsjdk.samtools.SAMRecord", "getSignedByteArrayAttribute"),
    modeled_semantic("SAMRecord", "getSignedByteArrayAttribute"),
    modeled_semantic("htsjdk.samtools.SAMRecord", "getStringAttribute"),
    modeled_semantic("SAMRecord", "getStringAttribute"),
    modeled_semantic("htsjdk.samtools.SAMRecord", "getTransientAttribute"),
    modeled_semantic("SAMRecord", "getTransientAttribute"),
    modeled_semantic("htsjdk.samtools.SAMRecord", "getReadGroup"),
    modeled_semantic("SAMRecord", "getReadGroup"),
    modeled_semantic("htsjdk.samtools.SAMRecord", "getAttributes"),
    modeled_semantic("SAMRecord", "getAttributes"),
    modeled_semantic("htsjdk.samtools.SAMRecord", "getAlignmentBlocks"),
    modeled_semantic("SAMRecord", "getAlignmentBlocks"),
    modeled_semantic("htsjdk.samtools.SAMRecord", "clone"),
    modeled_semantic("SAMRecord", "clone"),
    modeled_semantic("htsjdk.samtools.SAMRecord", "deepCopy"),
    modeled_semantic("SAMRecord", "deepCopy"),
    modeled_semantic("htsjdk.samtools.SAMRecord", "hasAttribute"),
    modeled_semantic("SAMRecord", "hasAttribute"),
    modeled_semantic("htsjdk.samtools.SAMRecord", "setAttribute"),
    modeled_semantic("SAMRecord", "setAttribute"),
    modeled_semantic("htsjdk.samtools.SAMRecord", "setBaseQualities"),
    modeled_semantic("SAMRecord", "setBaseQualities"),
    modeled_semantic("htsjdk.samtools.SAMRecord", "setReadBases"),
    modeled_semantic("SAMRecord", "setReadBases"),
    modeled_semantic("htsjdk.samtools.SAMRecord", "setCigar"),
    modeled_semantic("SAMRecord", "setCigar"),
    modeled_semantic("htsjdk.samtools.SAMRecord", "setReadName"),
    modeled_semantic("SAMRecord", "setReadName"),
    modeled_semantic("htsjdk.samtools.SAMRecord", "setReferenceName"),
    modeled_semantic("SAMRecord", "setReferenceName"),
    modeled_semantic("htsjdk.samtools.SAMRecord", "setMappingQuality"),
    modeled_semantic("SAMRecord", "setMappingQuality"),
    modeled_semantic("htsjdk.samtools.SAMRecord", "setMateUnmappedFlag"),
    modeled_semantic("SAMRecord", "setMateUnmappedFlag"),
    modeled_semantic("htsjdk.samtools.SAMRecord", "setProperPairFlag"),
    modeled_semantic("SAMRecord", "setProperPairFlag"),
    modeled_semantic("htsjdk.samtools.SAMRecord", "setSecondaryAlignment"),
    modeled_semantic("SAMRecord", "setSecondaryAlignment"),
    modeled_semantic("htsjdk.samtools.SAMRecord", "setFirstOfPairFlag"),
    modeled_semantic("SAMRecord", "setFirstOfPairFlag"),
    modeled_semantic("htsjdk.samtools.SAMRecord", "setSecondOfPairFlag"),
    modeled_semantic("SAMRecord", "setSecondOfPairFlag"),
    modeled_semantic("htsjdk.samtools.SAMRecord", "setCigarString"),
    modeled_semantic("SAMRecord", "setCigarString"),
    modeled_semantic("htsjdk.samtools.SAMRecord", "setMateAlignmentStart"),
    modeled_semantic("SAMRecord", "setMateAlignmentStart"),
    modeled_semantic("htsjdk.samtools.SAMRecord", "setMateNegativeStrandFlag"),
    modeled_semantic("SAMRecord", "setMateNegativeStrandFlag"),
    modeled_semantic("htsjdk.samtools.SAMRecord", "setReferenceIndex"),
    modeled_semantic("SAMRecord", "setReferenceIndex"),
    modeled_semantic("htsjdk.samtools.SAMRecord", "setMateReferenceIndex"),
    modeled_semantic("SAMRecord", "setMateReferenceIndex"),
    modeled_semantic("htsjdk.samtools.SAMRecord", "setMateReferenceName"),
    modeled_semantic("SAMRecord", "setMateReferenceName"),
    modeled_semantic("htsjdk.samtools.SAMRecord", "setReadUnmappedFlag"),
    modeled_semantic("SAMRecord", "setReadUnmappedFlag"),
    modeled_semantic("htsjdk.samtools.SAMRecord", "setAlignmentStart"),
    modeled_semantic("SAMRecord", "setAlignmentStart"),
    modeled_semantic("htsjdk.samtools.SAMRecord", "setFlags"),
    modeled_semantic("SAMRecord", "setFlags"),
    modeled_semantic("htsjdk.samtools.SAMRecord", "setHeaderStrict"),
    modeled_semantic("SAMRecord", "setHeaderStrict"),
    modeled_semantic("htsjdk.samtools.SAMRecord", "clearAttributes"),
    modeled_semantic("SAMRecord", "clearAttributes"),
    modeled_semantic("htsjdk.samtools.SAMRecord", "removeTransientAttribute"),
    modeled_semantic("SAMRecord", "removeTransientAttribute"),
    modeled_semantic("htsjdk.samtools.SAMRecord", "setReadNegativeStrandFlag"),
    modeled_semantic("SAMRecord", "setReadNegativeStrandFlag"),
    modeled_semantic("htsjdk.samtools.SAMRecord", "setReadPairedFlag"),
    modeled_semantic("SAMRecord", "setReadPairedFlag"),
    modeled_semantic("htsjdk.samtools.SAMRecord", "setSupplementaryAlignmentFlag"),
    modeled_semantic("SAMRecord", "setSupplementaryAlignmentFlag"),
    modeled_semantic(
        "htsjdk.samtools.SAMRecord",
        "setReadFailsVendorQualityCheckFlag",
    ),
    modeled_semantic("SAMRecord", "setReadFailsVendorQualityCheckFlag"),
    modeled_semantic("htsjdk.samtools.SAMRecord", "setDuplicateReadFlag"),
    modeled_semantic("SAMRecord", "setDuplicateReadFlag"),
    modeled_semantic("htsjdk.samtools.SAMRecord", "setTransientAttribute"),
    modeled_semantic("SAMRecord", "setTransientAttribute"),
    modeled_semantic("htsjdk.samtools.SAMRecord", "setInferredInsertSize"),
    modeled_semantic("SAMRecord", "setInferredInsertSize"),
    modeled_semantic("htsjdk.samtools.SAMRecord", "reverseComplement"),
    modeled_semantic("SAMRecord", "reverseComplement"),
];

const SAM_READER_MODELED_APIS: &[ExternalPackModeledApi] = &[
    modeled_semantic("htsjdk.samtools.SamReaderFactory", "makeDefault"),
    modeled_semantic("SamReaderFactory", "makeDefault"),
    modeled_semantic("htsjdk.samtools.SamReaderFactory", "open"),
    modeled_semantic("SamReaderFactory", "open"),
    modeled_semantic("htsjdk.samtools.SamReaderFactory", "referenceSequence"),
    modeled_semantic("SamReaderFactory", "referenceSequence"),
    modeled_semantic("htsjdk.samtools.SamFileHeaderMerger", "getMergedHeader"),
    modeled_semantic("SamFileHeaderMerger", "getMergedHeader"),
];

const SAM_WRITER_MODELED_APIS: &[ExternalPackModeledApi] = &[
    modeled_semantic("htsjdk.samtools.SAMFileWriterFactory", "makeWriter"),
    modeled_semantic("SAMFileWriterFactory", "makeWriter"),
    modeled_semantic("htsjdk.samtools.SAMFileWriterFactory", "setCreateIndex"),
    modeled_semantic("SAMFileWriterFactory", "setCreateIndex"),
    modeled_semantic("htsjdk.samtools.SAMFileWriterFactory", "setCreateMd5File"),
    modeled_semantic("SAMFileWriterFactory", "setCreateMd5File"),
];

const SAM_REFERENCE_MODELED_APIS: &[ExternalPackModeledApi] = &[
    modeled_semantic("htsjdk.samtools.reference.ReferenceSequence", "getName"),
    modeled_semantic("ReferenceSequence", "getName"),
    modeled_semantic(
        "htsjdk.samtools.reference.ReferenceSequence",
        "getContigIndex",
    ),
    modeled_semantic("ReferenceSequence", "getContigIndex"),
    modeled_semantic("htsjdk.samtools.reference.ReferenceSequence", "getBases"),
    modeled_semantic("ReferenceSequence", "getBases"),
    modeled_semantic(
        "htsjdk.samtools.reference.FastaSequenceFile",
        "nextSequence",
    ),
    modeled_semantic("FastaSequenceFile", "nextSequence"),
    modeled_semantic("htsjdk.samtools.reference.FastaSequenceFile", "close"),
    modeled_semantic("FastaSequenceFile", "close"),
    modeled_semantic(
        "htsjdk.samtools.reference.ReferenceSequenceFileWalker",
        "getSequenceDictionary",
    ),
    modeled_semantic("ReferenceSequenceFileWalker", "getSequenceDictionary"),
    modeled_semantic(
        "htsjdk.samtools.reference.ReferenceSequenceFileWalker",
        "get",
    ),
    modeled_semantic("ReferenceSequenceFileWalker", "get"),
];

const TRIBBLE_FEATURE_MODELED_APIS: &[ExternalPackModeledApi] = &[
    modeled_semantic("htsjdk.tribble.Feature", "getContig"),
    modeled_semantic("Feature", "getContig"),
    modeled_semantic("htsjdk.tribble.SimpleFeature", "getContig"),
    modeled_semantic("SimpleFeature", "getContig"),
    modeled_semantic("htsjdk.tribble.NamedFeature", "getContig"),
    modeled_semantic("NamedFeature", "getContig"),
    modeled_semantic("htsjdk.tribble.bed.BEDFeature", "getContig"),
    modeled_semantic("BEDFeature", "getContig"),
    modeled_semantic("htsjdk.tribble.Feature", "contigsMatch"),
    modeled_semantic("Feature", "contigsMatch"),
    modeled_semantic("htsjdk.tribble.Feature", "getStart"),
    modeled_semantic("Feature", "getStart"),
    modeled_semantic("htsjdk.tribble.SimpleFeature", "getStart"),
    modeled_semantic("SimpleFeature", "getStart"),
    modeled_semantic("htsjdk.tribble.NamedFeature", "getStart"),
    modeled_semantic("NamedFeature", "getStart"),
    modeled_semantic("htsjdk.tribble.bed.BEDFeature", "getStart"),
    modeled_semantic("BEDFeature", "getStart"),
    modeled_semantic("htsjdk.tribble.Feature", "getEnd"),
    modeled_semantic("Feature", "getEnd"),
    modeled_semantic("htsjdk.tribble.SimpleFeature", "getEnd"),
    modeled_semantic("SimpleFeature", "getEnd"),
    modeled_semantic("htsjdk.tribble.NamedFeature", "getEnd"),
    modeled_semantic("NamedFeature", "getEnd"),
    modeled_semantic("htsjdk.tribble.bed.BEDFeature", "getEnd"),
    modeled_semantic("BEDFeature", "getEnd"),
    modeled_semantic("htsjdk.tribble.SimpleFeature", "getName"),
    modeled_semantic("SimpleFeature", "getName"),
    modeled_semantic("htsjdk.tribble.NamedFeature", "getName"),
    modeled_semantic("NamedFeature", "getName"),
    modeled_semantic("htsjdk.tribble.bed.BEDFeature", "getName"),
    modeled_semantic("BEDFeature", "getName"),
    modeled_semantic("htsjdk.tribble.bed.BEDFeature", "getScore"),
    modeled_semantic("BEDFeature", "getScore"),
    modeled_semantic("htsjdk.tribble.AbstractFeatureReader", "getFeatureReader"),
    modeled_semantic("AbstractFeatureReader", "getFeatureReader"),
    modeled_semantic("htsjdk.tribble.FeatureReader", "getHeader"),
    modeled_semantic("FeatureReader", "getHeader"),
    modeled_semantic("htsjdk.tribble.FeatureReader", "query"),
    modeled_semantic("FeatureReader", "query"),
    modeled_semantic("htsjdk.tribble.FeatureReader", "iterator"),
    modeled_semantic("FeatureReader", "iterator"),
    modeled_semantic("htsjdk.tribble.FeatureReader", "getSequenceNames"),
    modeled_semantic("FeatureReader", "getSequenceNames"),
    modeled_semantic("htsjdk.tribble.FeatureReader", "isQueryable"),
    modeled_semantic("FeatureReader", "isQueryable"),
    modeled_semantic("htsjdk.tribble.FeatureReader", "close"),
    modeled_semantic("FeatureReader", "close"),
];

const SAMTOOLS_UTIL_MODELED_APIS: &[ExternalPackModeledApi] = &[
    modeled_semantic("htsjdk.samtools.util.Locatable", "getContig"),
    modeled_semantic("Locatable", "getContig"),
    modeled_semantic("htsjdk.samtools.util.Interval", "getContig"),
    modeled_semantic("Interval", "getContig"),
    modeled_semantic("htsjdk.samtools.util.IntervalList", "getContig"),
    modeled_semantic("IntervalList", "getContig"),
    modeled_semantic("htsjdk.samtools.util.Locatable", "getStart"),
    modeled_semantic("Locatable", "getStart"),
    modeled_semantic("htsjdk.samtools.util.Interval", "getStart"),
    modeled_semantic("Interval", "getStart"),
    modeled_semantic("htsjdk.samtools.util.IntervalList", "getStart"),
    modeled_semantic("IntervalList", "getStart"),
    modeled_semantic("htsjdk.samtools.util.Locatable", "getEnd"),
    modeled_semantic("Locatable", "getEnd"),
    modeled_semantic("htsjdk.samtools.util.Locatable", "contains"),
    modeled_semantic("Locatable", "contains"),
    modeled_semantic("htsjdk.samtools.util.Locatable", "overlaps"),
    modeled_semantic("Locatable", "overlaps"),
    modeled_semantic("htsjdk.samtools.util.Locatable", "contigsMatch"),
    modeled_semantic("Locatable", "contigsMatch"),
    modeled_semantic("htsjdk.samtools.util.Locatable", "getLengthOnReference"),
    modeled_semantic("Locatable", "getLengthOnReference"),
    modeled_semantic("htsjdk.samtools.util.Interval", "getEnd"),
    modeled_semantic("Interval", "getEnd"),
    modeled_semantic("htsjdk.samtools.util.IntervalList", "getEnd"),
    modeled_semantic("IntervalList", "getEnd"),
    modeled_semantic("htsjdk.samtools.util.Interval", "getName"),
    modeled_semantic("Interval", "getName"),
    modeled_semantic("htsjdk.samtools.util.Interval", "getInterval"),
    modeled_semantic("Interval", "getInterval"),
    modeled_semantic("htsjdk.samtools.util.Interval", "toString"),
    modeled_semantic("Interval", "toString"),
    modeled_semantic("htsjdk.samtools.util.Interval", "isNegativeStrand"),
    modeled_semantic("Interval", "isNegativeStrand"),
    modeled_semantic("htsjdk.samtools.util.IntervalList", "isNegativeStrand"),
    modeled_semantic("IntervalList", "isNegativeStrand"),
    modeled_semantic("htsjdk.samtools.util.Interval", "intersects"),
    modeled_semantic("Interval", "intersects"),
    modeled_semantic("htsjdk.samtools.util.IntervalList", "intersects"),
    modeled_semantic("IntervalList", "intersects"),
    modeled_semantic("htsjdk.samtools.util.Interval", "contains"),
    modeled_semantic("Interval", "contains"),
    modeled_semantic("htsjdk.samtools.util.IntervalList", "contains"),
    modeled_semantic("IntervalList", "contains"),
    modeled_semantic("htsjdk.samtools.util.Interval", "overlaps"),
    modeled_semantic("Interval", "overlaps"),
    modeled_semantic("htsjdk.samtools.util.IntervalList", "overlaps"),
    modeled_semantic("IntervalList", "overlaps"),
    modeled_semantic("htsjdk.samtools.util.Interval", "size"),
    modeled_semantic("Interval", "size"),
    modeled_semantic("htsjdk.samtools.util.IntervalList", "size"),
    modeled_semantic("IntervalList", "size"),
    modeled_semantic("htsjdk.samtools.util.IntervalList", "add"),
    modeled_semantic("IntervalList", "add"),
    modeled_semantic("htsjdk.samtools.util.IntervalList", "write"),
    modeled_semantic("IntervalList", "write"),
    modeled_semantic("htsjdk.samtools.util.IntervalList", "fromFile"),
    modeled_semantic("IntervalList", "fromFile"),
    modeled_semantic("htsjdk.samtools.util.IntervalTree", "size"),
    modeled_semantic("IntervalTree", "size"),
    modeled_semantic("htsjdk.samtools.util.IntervalTree", "put"),
    modeled_semantic("IntervalTree", "put"),
    modeled_semantic("htsjdk.samtools.util.IntervalTree", "minOverlapper"),
    modeled_semantic("IntervalTree", "minOverlapper"),
    modeled_semantic("htsjdk.samtools.util.Histogram", "size"),
    modeled_semantic("Histogram", "size"),
    modeled_semantic("htsjdk.samtools.util.Histogram", "keySet"),
    modeled_semantic("Histogram", "keySet"),
    modeled_semantic("htsjdk.samtools.util.Histogram", "values"),
    modeled_semantic("Histogram", "values"),
    modeled_semantic("htsjdk.samtools.util.Histogram", "getCount"),
    modeled_semantic("Histogram", "getCount"),
    modeled_semantic("htsjdk.samtools.util.Histogram", "getSumOfValues"),
    modeled_semantic("Histogram", "getSumOfValues"),
    modeled_semantic("htsjdk.samtools.util.Histogram", "getMin"),
    modeled_semantic("Histogram", "getMin"),
    modeled_semantic("htsjdk.samtools.util.Histogram", "getMax"),
    modeled_semantic("Histogram", "getMax"),
    modeled_semantic("htsjdk.samtools.util.Histogram", "getMean"),
    modeled_semantic("Histogram", "getMean"),
    modeled_semantic("htsjdk.samtools.util.Histogram", "getStandardDeviation"),
    modeled_semantic("Histogram", "getStandardDeviation"),
    modeled_semantic("htsjdk.samtools.util.Histogram", "getMedian"),
    modeled_semantic("Histogram", "getMedian"),
    modeled_semantic(
        "htsjdk.samtools.util.Histogram",
        "getMedianAbsoluteDeviation",
    ),
    modeled_semantic("Histogram", "getMedianAbsoluteDeviation"),
    modeled_semantic("htsjdk.samtools.util.Histogram", "increment"),
    modeled_semantic("Histogram", "increment"),
    modeled_semantic("htsjdk.samtools.util.Histogram", "prefillBins"),
    modeled_semantic("Histogram", "prefillBins"),
    modeled_semantic("htsjdk.samtools.util.Histogram", "trimByWidth"),
    modeled_semantic("Histogram", "trimByWidth"),
    modeled_semantic("htsjdk.samtools.util.IntervalList", "isEmpty"),
    modeled_semantic("IntervalList", "isEmpty"),
    modeled_semantic("htsjdk.samtools.util.IntervalTree", "isEmpty"),
    modeled_semantic("IntervalTree", "isEmpty"),
    modeled_semantic("htsjdk.samtools.util.Histogram", "isEmpty"),
    modeled_semantic("Histogram", "isEmpty"),
    modeled_semantic("htsjdk.samtools.util.IntervalList", "iterator"),
    modeled_semantic("IntervalList", "iterator"),
    modeled_semantic("htsjdk.samtools.util.IntervalTree", "iterator"),
    modeled_semantic("IntervalTree", "iterator"),
    modeled_semantic("htsjdk.samtools.util.Histogram", "iterator"),
    modeled_semantic("Histogram", "iterator"),
    modeled_semantic("htsjdk.samtools.util.Histogram", "getValueLabel"),
    modeled_semantic("Histogram", "getValueLabel"),
    modeled_semantic("htsjdk.samtools.util.Histogram", "get"),
    modeled_semantic("Histogram", "get"),
    modeled_semantic("htsjdk.samtools.SAMUtils", "phredToFastq"),
    modeled_semantic("SAMUtils", "phredToFastq"),
    modeled_semantic("htsjdk.samtools.SAMUtils", "cigarMapsNoBasesToRef"),
    modeled_semantic("SAMUtils", "cigarMapsNoBasesToRef"),
    modeled_semantic("htsjdk.samtools.SAMUtils", "combineMapqs"),
    modeled_semantic("SAMUtils", "combineMapqs"),
    modeled_semantic("htsjdk.samtools.SAMUtils", "compareMapqs"),
    modeled_semantic("SAMUtils", "compareMapqs"),
    modeled_semantic("htsjdk.samtools.SAMUtils", "getMateAlignmentEnd"),
    modeled_semantic("SAMUtils", "getMateAlignmentEnd"),
    modeled_semantic("htsjdk.samtools.SAMUtils", "hasMateCigar"),
    modeled_semantic("SAMUtils", "hasMateCigar"),
    modeled_semantic(
        "htsjdk.samtools.SAMUtils",
        "recordMapsEntirelyBeyondEndOfReference",
    ),
    modeled_semantic("SAMUtils", "recordMapsEntirelyBeyondEndOfReference"),
    modeled_semantic("htsjdk.samtools.SAMUtils", "getMateCigar"),
    modeled_semantic("SAMUtils", "getMateCigar"),
    modeled_semantic("htsjdk.samtools.SamPairUtil", "setMateInfo"),
    modeled_semantic("SamPairUtil", "setMateInfo"),
    modeled_semantic(
        "htsjdk.samtools.SamPairUtil",
        "setMateInformationOnSupplementalAlignment",
    ),
    modeled_semantic("SamPairUtil", "setMateInformationOnSupplementalAlignment"),
    modeled_semantic("htsjdk.samtools.SamPairUtil", "setProperPairFlags"),
    modeled_semantic("SamPairUtil", "setProperPairFlags"),
    modeled_semantic("htsjdk.samtools.SAMUtils", "chainSAMProgramRecord"),
    modeled_semantic("SAMUtils", "chainSAMProgramRecord"),
    modeled_semantic("htsjdk.samtools.SAMUtils", "makeReadUnmapped"),
    modeled_semantic("SAMUtils", "makeReadUnmapped"),
    modeled_semantic("htsjdk.samtools.SAMUtils", "fastqToPhred"),
    modeled_semantic("SAMUtils", "fastqToPhred"),
    modeled_semantic("htsjdk.samtools.util.SequenceUtil", "isNoCall"),
    modeled_semantic("SequenceUtil", "isNoCall"),
    modeled_semantic("htsjdk.samtools.util.SequenceUtil", "reverseComplement"),
    modeled_semantic("SequenceUtil", "reverseComplement"),
    modeled_semantic("htsjdk.samtools.util.SequenceUtil", "calculateSamNmTag"),
    modeled_semantic("SequenceUtil", "calculateSamNmTag"),
    modeled_semantic(
        "htsjdk.samtools.util.SequenceUtil",
        "sumQualitiesOfMismatches",
    ),
    modeled_semantic("SequenceUtil", "sumQualitiesOfMismatches"),
    modeled_semantic("htsjdk.samtools.util.PeekableIterator", "hasNext"),
    modeled_semantic("PeekableIterator", "hasNext"),
    modeled_semantic("htsjdk.samtools.util.DelegatingIterator", "hasNext"),
    modeled_semantic("DelegatingIterator", "hasNext"),
    modeled_semantic("htsjdk.samtools.util.MergingIterator", "hasNext"),
    modeled_semantic("MergingIterator", "hasNext"),
    modeled_semantic("htsjdk.samtools.util.IntervalMergerIterator", "hasNext"),
    modeled_semantic("IntervalMergerIterator", "hasNext"),
    modeled_semantic("htsjdk.samtools.util.CloseableIterator", "hasNext"),
    modeled_semantic("CloseableIterator", "hasNext"),
    modeled_semantic("htsjdk.samtools.util.PeekableIterator", "next"),
    modeled_semantic("PeekableIterator", "next"),
    modeled_semantic("htsjdk.samtools.util.DelegatingIterator", "next"),
    modeled_semantic("DelegatingIterator", "next"),
    modeled_semantic("htsjdk.samtools.util.MergingIterator", "next"),
    modeled_semantic("MergingIterator", "next"),
    modeled_semantic("htsjdk.samtools.util.IntervalMergerIterator", "next"),
    modeled_semantic("IntervalMergerIterator", "next"),
    modeled_semantic("htsjdk.samtools.util.CloseableIterator", "next"),
    modeled_semantic("CloseableIterator", "next"),
    modeled_semantic("htsjdk.samtools.util.PeekableIterator", "close"),
    modeled_semantic("PeekableIterator", "close"),
    modeled_semantic("htsjdk.samtools.util.DelegatingIterator", "close"),
    modeled_semantic("DelegatingIterator", "close"),
    modeled_semantic("htsjdk.samtools.util.CloseableIterator", "close"),
    modeled_semantic("CloseableIterator", "close"),
    modeled_semantic("htsjdk.samtools.util.CloseableIterator", "remove"),
    modeled_semantic("CloseableIterator", "remove"),
    modeled_semantic("htsjdk.samtools.util.BlockCompressedOutputStream", "flush"),
    modeled_semantic("BlockCompressedOutputStream", "flush"),
    modeled_semantic("htsjdk.samtools.util.BlockCompressedOutputStream", "close"),
    modeled_semantic("BlockCompressedOutputStream", "close"),
    modeled_semantic("htsjdk.samtools.util.PeekableIterator", "peek"),
    modeled_semantic("PeekableIterator", "peek"),
    modeled_semantic("htsjdk.samtools.util.BinaryCodec", "writeBytes"),
    modeled_semantic("BinaryCodec", "writeBytes"),
    modeled_semantic("htsjdk.samtools.util.BinaryCodec", "writeInt"),
    modeled_semantic("BinaryCodec", "writeInt"),
    modeled_semantic("htsjdk.samtools.util.BinaryCodec", "writeString"),
    modeled_semantic("BinaryCodec", "writeString"),
];

const fn modeled_semantic(
    receiver_type: &'static str,
    method_name: &'static str,
) -> ExternalPackModeledApi {
    ExternalPackModeledApi {
        receiver_type,
        method_name,
        coverage: ExternalApiCoverage::Semantic,
    }
}

const fn modeled_mapped(
    receiver_type: &'static str,
    method_name: &'static str,
) -> ExternalPackModeledApi {
    ExternalPackModeledApi {
        receiver_type,
        method_name,
        coverage: ExternalApiCoverage::Mapped,
    }
}

const fn modeled_semantic_constructor(
    type_name: &'static str,
    arg_count: usize,
) -> ExternalPackModeledConstructor {
    ExternalPackModeledConstructor {
        type_name,
        arg_count,
        rust_type_name: None,
        rust_name: None,
        coverage: ExternalApiCoverage::Semantic,
    }
}

const fn modeled_mapped_constructor(
    type_name: &'static str,
    arg_count: usize,
) -> ExternalPackModeledConstructor {
    ExternalPackModeledConstructor {
        type_name,
        arg_count,
        rust_type_name: None,
        rust_name: Some("new"),
        coverage: ExternalApiCoverage::Mapped,
    }
}

const fn modeled_semantic_field(
    type_name: &'static str,
    field_name: &'static str,
) -> ExternalPackModeledField {
    ExternalPackModeledField {
        type_name,
        field_name,
        coverage: ExternalApiCoverage::Semantic,
    }
}

const fn modeled_mapped_field(
    type_name: &'static str,
    field_name: &'static str,
) -> ExternalPackModeledField {
    ExternalPackModeledField {
        type_name,
        field_name,
        coverage: ExternalApiCoverage::Mapped,
    }
}

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
const JAVA_SQL_UNSUPPORTED_APIS: &[ExternalPackUnsupportedApi] = &[
    ExternalPackUnsupportedApi {
        receiver_type: "java.sql.ResultSet",
        method_name: "getBlob",
        reason: "JDBC Blob is a streaming/resource handle; requires an explicit facade mapping",
    },
    ExternalPackUnsupportedApi {
        receiver_type: "java.sql.ResultSet",
        method_name: "getClob",
        reason: "JDBC Clob is a streaming/resource handle; requires an explicit facade mapping",
    },
    ExternalPackUnsupportedApi {
        receiver_type: "java.sql.ResultSet",
        method_name: "getArray",
        reason: "JDBC Array needs element-shape and ownership policy before lowering",
    },
    ExternalPackUnsupportedApi {
        receiver_type: "java.sql.ResultSet",
        method_name: "getSQLXML",
        reason: "JDBC SQLXML is a resource-backed XML handle; requires an explicit facade mapping",
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
            object_strategies: VARIANTCONTEXT_OBJECT_STRATEGIES,
            closed_hierarchy: NO_CLOSED_HIERARCHY,
            object_safety: NO_OBJECT_SAFETY,
            type_capabilities: VARIANTCONTEXT_TYPE_CAPABILITIES,
            field_nullability: NO_FIELD_NULLABILITY,
            argument_effects: VARIANTCONTEXT_ARGUMENT_EFFECTS,
            exceptions: VARIANTCONTEXT_EXCEPTIONS,
            api_effects: VARIANTCONTEXT_API_EFFECTS,
            modeled_apis: VARIANTCONTEXT_MODELED_APIS,
            modeled_constructors: VARIANTCONTEXT_MODELED_CONSTRUCTORS,
            modeled_fields: VARIANTCONTEXT_MODELED_FIELDS,
            unsupported_apis: VARIANTCONTEXT_UNSUPPORTED_APIS,
        },
        ExternalPackManifest {
            pack_name: VCF_PACK,
            owned_type_patterns: VCF_OWNED_TYPES,
            mode: ExternalLibraryMode::ExternalFactModel,
            default_coverage: ExternalApiCoverage::Semantic,
            rust_target: Some("crate::external::htsjdk::vcf"),
            object_strategies: VCF_OBJECT_STRATEGIES,
            closed_hierarchy: NO_CLOSED_HIERARCHY,
            object_safety: NO_OBJECT_SAFETY,
            type_capabilities: VCF_TYPE_CAPABILITIES,
            field_nullability: NO_FIELD_NULLABILITY,
            argument_effects: NO_ARGUMENT_EFFECTS,
            exceptions: NO_EXCEPTIONS,
            api_effects: VCF_API_EFFECTS,
            modeled_apis: VCF_MODELED_APIS,
            modeled_constructors: VCF_MODELED_CONSTRUCTORS,
            modeled_fields: VCF_MODELED_FIELDS,
            unsupported_apis: VCF_UNSUPPORTED_APIS,
        },
        ExternalPackManifest {
            pack_name: BCF_PACK,
            owned_type_patterns: BCF_OWNED_TYPES,
            mode: ExternalLibraryMode::ExternalFactModel,
            default_coverage: ExternalApiCoverage::Semantic,
            rust_target: Some("crate::external::htsjdk::bcf"),
            object_strategies: BCF_OBJECT_STRATEGIES,
            closed_hierarchy: NO_CLOSED_HIERARCHY,
            object_safety: NO_OBJECT_SAFETY,
            type_capabilities: BCF_TYPE_CAPABILITIES,
            field_nullability: NO_FIELD_NULLABILITY,
            argument_effects: NO_ARGUMENT_EFFECTS,
            exceptions: NO_EXCEPTIONS,
            api_effects: &[],
            modeled_apis: &[],
            modeled_constructors: BCF_MODELED_CONSTRUCTORS,
            modeled_fields: NO_MODELED_FIELDS,
            unsupported_apis: &[],
        },
        ExternalPackManifest {
            pack_name: SAM_HEADER_PACK,
            owned_type_patterns: SAM_HEADER_OWNED_TYPES,
            mode: ExternalLibraryMode::ExternalFactModel,
            default_coverage: ExternalApiCoverage::Semantic,
            rust_target: Some("crate::external::htsjdk::sam_header"),
            object_strategies: SAM_HEADER_OBJECT_STRATEGIES,
            closed_hierarchy: NO_CLOSED_HIERARCHY,
            object_safety: NO_OBJECT_SAFETY,
            type_capabilities: SAM_HEADER_TYPE_CAPABILITIES,
            field_nullability: SAM_HEADER_FIELD_NULLABILITY,
            argument_effects: NO_ARGUMENT_EFFECTS,
            exceptions: SAM_HEADER_EXCEPTIONS,
            api_effects: SAM_HEADER_API_EFFECTS,
            modeled_apis: SAM_HEADER_MODELED_APIS,
            modeled_constructors: SAM_HEADER_MODELED_CONSTRUCTORS,
            modeled_fields: SAM_HEADER_MODELED_FIELDS,
            unsupported_apis: &[],
        },
        ExternalPackManifest {
            pack_name: SAM_RECORD_PACK,
            owned_type_patterns: SAM_RECORD_OWNED_TYPES,
            mode: ExternalLibraryMode::ExternalFactModel,
            default_coverage: ExternalApiCoverage::Semantic,
            rust_target: Some("crate::external::htsjdk::sam_record"),
            object_strategies: SAM_RECORD_OBJECT_STRATEGIES,
            closed_hierarchy: NO_CLOSED_HIERARCHY,
            object_safety: NO_OBJECT_SAFETY,
            type_capabilities: SAM_RECORD_TYPE_CAPABILITIES,
            field_nullability: NO_FIELD_NULLABILITY,
            argument_effects: NO_ARGUMENT_EFFECTS,
            exceptions: NO_EXCEPTIONS,
            api_effects: SAM_RECORD_API_EFFECTS,
            modeled_apis: SAM_RECORD_MODELED_APIS,
            modeled_constructors: SAM_RECORD_MODELED_CONSTRUCTORS,
            modeled_fields: SAM_RECORD_MODELED_FIELDS,
            unsupported_apis: &[],
        },
        ExternalPackManifest {
            pack_name: SAM_REFERENCE_PACK,
            owned_type_patterns: SAM_REFERENCE_OWNED_TYPES,
            mode: ExternalLibraryMode::ExternalFactModel,
            default_coverage: ExternalApiCoverage::Semantic,
            rust_target: Some("crate::external::htsjdk::reference"),
            object_strategies: SAM_REFERENCE_OBJECT_STRATEGIES,
            closed_hierarchy: NO_CLOSED_HIERARCHY,
            object_safety: NO_OBJECT_SAFETY,
            type_capabilities: SAM_REFERENCE_TYPE_CAPABILITIES,
            field_nullability: NO_FIELD_NULLABILITY,
            argument_effects: NO_ARGUMENT_EFFECTS,
            exceptions: NO_EXCEPTIONS,
            api_effects: SAM_REFERENCE_API_EFFECTS,
            modeled_apis: SAM_REFERENCE_MODELED_APIS,
            modeled_constructors: SAM_REFERENCE_MODELED_CONSTRUCTORS,
            modeled_fields: NO_MODELED_FIELDS,
            unsupported_apis: &[],
        },
        ExternalPackManifest {
            pack_name: SAM_READER_PACK,
            owned_type_patterns: SAM_READER_OWNED_TYPES,
            mode: ExternalLibraryMode::ExternalFactModel,
            default_coverage: ExternalApiCoverage::Semantic,
            rust_target: Some("crate::external::htsjdk::sam_reader"),
            object_strategies: SAM_READER_OBJECT_STRATEGIES,
            closed_hierarchy: NO_CLOSED_HIERARCHY,
            object_safety: NO_OBJECT_SAFETY,
            type_capabilities: SAM_READER_TYPE_CAPABILITIES,
            field_nullability: NO_FIELD_NULLABILITY,
            argument_effects: NO_ARGUMENT_EFFECTS,
            exceptions: NO_EXCEPTIONS,
            api_effects: SAM_READER_API_EFFECTS,
            modeled_apis: SAM_READER_MODELED_APIS,
            modeled_constructors: SAM_READER_MODELED_CONSTRUCTORS,
            modeled_fields: SAM_READER_MODELED_FIELDS,
            unsupported_apis: &[],
        },
        ExternalPackManifest {
            pack_name: SAM_WRITER_PACK,
            owned_type_patterns: SAM_WRITER_OWNED_TYPES,
            mode: ExternalLibraryMode::ExternalFactModel,
            default_coverage: ExternalApiCoverage::Semantic,
            rust_target: Some("crate::external::htsjdk::sam_writer"),
            object_strategies: SAM_WRITER_OBJECT_STRATEGIES,
            closed_hierarchy: NO_CLOSED_HIERARCHY,
            object_safety: NO_OBJECT_SAFETY,
            type_capabilities: SAM_WRITER_TYPE_CAPABILITIES,
            field_nullability: NO_FIELD_NULLABILITY,
            argument_effects: NO_ARGUMENT_EFFECTS,
            exceptions: NO_EXCEPTIONS,
            api_effects: SAM_WRITER_API_EFFECTS,
            modeled_apis: SAM_WRITER_MODELED_APIS,
            modeled_constructors: SAM_WRITER_MODELED_CONSTRUCTORS,
            modeled_fields: NO_MODELED_FIELDS,
            unsupported_apis: &[],
        },
        ExternalPackManifest {
            pack_name: SAM_BAM_PACK,
            owned_type_patterns: SAM_BAM_OWNED_TYPES,
            mode: ExternalLibraryMode::ExternalFactModel,
            default_coverage: ExternalApiCoverage::Semantic,
            rust_target: Some("crate::external::htsjdk::sam_bam"),
            object_strategies: SAM_BAM_OBJECT_STRATEGIES,
            closed_hierarchy: NO_CLOSED_HIERARCHY,
            object_safety: NO_OBJECT_SAFETY,
            type_capabilities: SAM_BAM_TYPE_CAPABILITIES,
            field_nullability: NO_FIELD_NULLABILITY,
            argument_effects: NO_ARGUMENT_EFFECTS,
            exceptions: NO_EXCEPTIONS,
            api_effects: &[],
            modeled_apis: &[],
            modeled_constructors: SAM_BAM_MODELED_CONSTRUCTORS,
            modeled_fields: SAM_BAM_MODELED_FIELDS,
            unsupported_apis: &[],
        },
        ExternalPackManifest {
            pack_name: SAM_CRAM_PACK,
            owned_type_patterns: SAM_CRAM_OWNED_TYPES,
            mode: ExternalLibraryMode::ExternalFactModel,
            default_coverage: ExternalApiCoverage::Semantic,
            rust_target: Some("crate::external::htsjdk::sam_cram"),
            object_strategies: SAM_CRAM_OBJECT_STRATEGIES,
            closed_hierarchy: NO_CLOSED_HIERARCHY,
            object_safety: NO_OBJECT_SAFETY,
            type_capabilities: SAM_CRAM_TYPE_CAPABILITIES,
            field_nullability: NO_FIELD_NULLABILITY,
            argument_effects: NO_ARGUMENT_EFFECTS,
            exceptions: NO_EXCEPTIONS,
            api_effects: &[],
            modeled_apis: &[],
            modeled_constructors: SAM_CRAM_MODELED_CONSTRUCTORS,
            modeled_fields: SAM_CRAM_MODELED_FIELDS,
            unsupported_apis: &[],
        },
        ExternalPackManifest {
            pack_name: SAM_METRICS_PACK,
            owned_type_patterns: SAM_METRICS_OWNED_TYPES,
            mode: ExternalLibraryMode::ExternalFactModel,
            default_coverage: ExternalApiCoverage::Semantic,
            rust_target: Some("crate::external::htsjdk::sam_metrics"),
            object_strategies: SAM_METRICS_OBJECT_STRATEGIES,
            closed_hierarchy: NO_CLOSED_HIERARCHY,
            object_safety: NO_OBJECT_SAFETY,
            type_capabilities: SAM_METRICS_TYPE_CAPABILITIES,
            field_nullability: NO_FIELD_NULLABILITY,
            argument_effects: NO_ARGUMENT_EFFECTS,
            exceptions: NO_EXCEPTIONS,
            api_effects: GUAVA_CONCURRENT_API_EFFECTS,
            modeled_apis: GUAVA_CONCURRENT_MODELED_APIS,
            modeled_constructors: SAM_METRICS_MODELED_CONSTRUCTORS,
            modeled_fields: NO_MODELED_FIELDS,
            unsupported_apis: &[],
        },
        ExternalPackManifest {
            pack_name: HTSJDK_NESTED_HELPER_PACK,
            owned_type_patterns: HTSJDK_NESTED_HELPER_OWNED_TYPES,
            mode: ExternalLibraryMode::ExternalFactModel,
            default_coverage: ExternalApiCoverage::Semantic,
            rust_target: Some("crate::external::htsjdk::nested_helper"),
            object_strategies: HTSJDK_NESTED_HELPER_OBJECT_STRATEGIES,
            closed_hierarchy: NO_CLOSED_HIERARCHY,
            object_safety: NO_OBJECT_SAFETY,
            type_capabilities: HTSJDK_NESTED_HELPER_TYPE_CAPABILITIES,
            field_nullability: NO_FIELD_NULLABILITY,
            argument_effects: NO_ARGUMENT_EFFECTS,
            exceptions: NO_EXCEPTIONS,
            api_effects: &[],
            modeled_apis: &[],
            modeled_constructors: HTSJDK_NESTED_HELPER_MODELED_CONSTRUCTORS,
            modeled_fields: NO_MODELED_FIELDS,
            unsupported_apis: &[],
        },
        ExternalPackManifest {
            pack_name: TRIBBLE_FEATURE_PACK,
            owned_type_patterns: TRIBBLE_FEATURE_OWNED_TYPES,
            mode: ExternalLibraryMode::ExternalFactModel,
            default_coverage: ExternalApiCoverage::Semantic,
            rust_target: Some("crate::external::htsjdk::tribble_feature"),
            object_strategies: TRIBBLE_FEATURE_OBJECT_STRATEGIES,
            closed_hierarchy: NO_CLOSED_HIERARCHY,
            object_safety: NO_OBJECT_SAFETY,
            type_capabilities: TRIBBLE_FEATURE_TYPE_CAPABILITIES,
            field_nullability: NO_FIELD_NULLABILITY,
            argument_effects: TRIBBLE_FEATURE_ARGUMENT_EFFECTS,
            exceptions: NO_EXCEPTIONS,
            api_effects: TRIBBLE_FEATURE_API_EFFECTS,
            modeled_apis: TRIBBLE_FEATURE_MODELED_APIS,
            modeled_constructors: TRIBBLE_FEATURE_MODELED_CONSTRUCTORS,
            modeled_fields: TRIBBLE_FEATURE_MODELED_FIELDS,
            unsupported_apis: &[],
        },
        ExternalPackManifest {
            pack_name: TRIBBLE_INDEX_PACK,
            owned_type_patterns: TRIBBLE_INDEX_OWNED_TYPES,
            mode: ExternalLibraryMode::ExternalFactModel,
            default_coverage: ExternalApiCoverage::Semantic,
            rust_target: Some("crate::external::htsjdk::tribble_index"),
            object_strategies: TRIBBLE_INDEX_OBJECT_STRATEGIES,
            closed_hierarchy: NO_CLOSED_HIERARCHY,
            object_safety: NO_OBJECT_SAFETY,
            type_capabilities: TRIBBLE_INDEX_TYPE_CAPABILITIES,
            field_nullability: NO_FIELD_NULLABILITY,
            argument_effects: NO_ARGUMENT_EFFECTS,
            exceptions: NO_EXCEPTIONS,
            api_effects: &[],
            modeled_apis: &[],
            modeled_constructors: &[],
            modeled_fields: TRIBBLE_INDEX_MODELED_FIELDS,
            unsupported_apis: &[],
        },
        ExternalPackManifest {
            pack_name: SAMTOOLS_UTIL_PACK,
            owned_type_patterns: SAMTOOLS_UTIL_OWNED_TYPES,
            mode: ExternalLibraryMode::ExternalFactModel,
            default_coverage: ExternalApiCoverage::Semantic,
            rust_target: Some("crate::external::htsjdk::samtools_util"),
            object_strategies: SAMTOOLS_UTIL_OBJECT_STRATEGIES,
            closed_hierarchy: NO_CLOSED_HIERARCHY,
            object_safety: NO_OBJECT_SAFETY,
            type_capabilities: SAMTOOLS_UTIL_TYPE_CAPABILITIES,
            field_nullability: NO_FIELD_NULLABILITY,
            argument_effects: NO_ARGUMENT_EFFECTS,
            exceptions: SAMTOOLS_UTIL_EXCEPTIONS,
            api_effects: SAMTOOLS_UTIL_API_EFFECTS,
            modeled_apis: SAMTOOLS_UTIL_MODELED_APIS,
            modeled_constructors: SAMTOOLS_UTIL_MODELED_CONSTRUCTORS,
            modeled_fields: SAMTOOLS_UTIL_MODELED_FIELDS,
            unsupported_apis: &[],
        },
        ExternalPackManifest {
            pack_name: APACHE_COMMONS_MATH_PACK,
            owned_type_patterns: APACHE_COMMONS_MATH_OWNED_TYPES,
            mode: ExternalLibraryMode::ExternalFactModel,
            default_coverage: ExternalApiCoverage::Semantic,
            rust_target: Some("crate::external::apache_commons_math"),
            object_strategies: APACHE_COMMONS_MATH_OBJECT_STRATEGIES,
            closed_hierarchy: NO_CLOSED_HIERARCHY,
            object_safety: NO_OBJECT_SAFETY,
            type_capabilities: APACHE_COMMONS_MATH_TYPE_CAPABILITIES,
            field_nullability: NO_FIELD_NULLABILITY,
            argument_effects: NO_ARGUMENT_EFFECTS,
            exceptions: NO_EXCEPTIONS,
            api_effects: &[],
            modeled_apis: &[],
            modeled_constructors: APACHE_COMMONS_MATH_MODELED_CONSTRUCTORS,
            modeled_fields: NO_MODELED_FIELDS,
            unsupported_apis: &[],
        },
        ExternalPackManifest {
            pack_name: APACHE_COMMONS_MATH_OPTIM_PACK,
            owned_type_patterns: APACHE_COMMONS_MATH_OPTIM_OWNED_TYPES,
            mode: ExternalLibraryMode::ExternalFactModel,
            default_coverage: ExternalApiCoverage::Semantic,
            rust_target: Some("crate::external::apache_commons_math::optim"),
            object_strategies: APACHE_COMMONS_MATH_OPTIM_OBJECT_STRATEGIES,
            closed_hierarchy: NO_CLOSED_HIERARCHY,
            object_safety: NO_OBJECT_SAFETY,
            type_capabilities: APACHE_COMMONS_MATH_OPTIM_TYPE_CAPABILITIES,
            field_nullability: NO_FIELD_NULLABILITY,
            argument_effects: NO_ARGUMENT_EFFECTS,
            exceptions: NO_EXCEPTIONS,
            api_effects: &[],
            modeled_apis: &[],
            modeled_constructors: APACHE_COMMONS_MATH_OPTIM_MODELED_CONSTRUCTORS,
            modeled_fields: APACHE_COMMONS_MATH_OPTIM_MODELED_FIELDS,
            unsupported_apis: &[],
        },
        ExternalPackManifest {
            pack_name: APACHE_COMMONS_MATH_STATS_PACK,
            owned_type_patterns: APACHE_COMMONS_MATH_STATS_OWNED_TYPES,
            mode: ExternalLibraryMode::ExternalFactModel,
            default_coverage: ExternalApiCoverage::Semantic,
            rust_target: Some("crate::external::apache_commons_math::stats"),
            object_strategies: APACHE_COMMONS_MATH_STATS_OBJECT_STRATEGIES,
            closed_hierarchy: NO_CLOSED_HIERARCHY,
            object_safety: NO_OBJECT_SAFETY,
            type_capabilities: APACHE_COMMONS_MATH_STATS_TYPE_CAPABILITIES,
            field_nullability: NO_FIELD_NULLABILITY,
            argument_effects: NO_ARGUMENT_EFFECTS,
            exceptions: NO_EXCEPTIONS,
            api_effects: &[],
            modeled_apis: &[],
            modeled_constructors: APACHE_COMMONS_MATH_STATS_MODELED_CONSTRUCTORS,
            modeled_fields: APACHE_COMMONS_MATH_STATS_MODELED_FIELDS,
            unsupported_apis: &[],
        },
        ExternalPackManifest {
            pack_name: APACHE_COMMONS_COLLECTIONS_PACK,
            owned_type_patterns: APACHE_COMMONS_COLLECTIONS_OWNED_TYPES,
            mode: ExternalLibraryMode::ExternalFactModel,
            default_coverage: ExternalApiCoverage::Semantic,
            rust_target: Some("crate::external::apache_commons::collections"),
            object_strategies: APACHE_COMMONS_COLLECTIONS_OBJECT_STRATEGIES,
            closed_hierarchy: NO_CLOSED_HIERARCHY,
            object_safety: NO_OBJECT_SAFETY,
            type_capabilities: APACHE_COMMONS_COLLECTIONS_TYPE_CAPABILITIES,
            field_nullability: NO_FIELD_NULLABILITY,
            argument_effects: NO_ARGUMENT_EFFECTS,
            exceptions: NO_EXCEPTIONS,
            api_effects: APACHE_COMMONS_COLLECTIONS_API_EFFECTS,
            modeled_apis: APACHE_COMMONS_COLLECTIONS_MODELED_APIS,
            modeled_constructors: APACHE_COMMONS_COLLECTIONS_MODELED_CONSTRUCTORS,
            modeled_fields: NO_MODELED_FIELDS,
            unsupported_apis: &[],
        },
        ExternalPackManifest {
            pack_name: APACHE_COMMONS_COMPRESS_PACK,
            owned_type_patterns: APACHE_COMMONS_COMPRESS_OWNED_TYPES,
            mode: ExternalLibraryMode::ExternalFactModel,
            default_coverage: ExternalApiCoverage::Semantic,
            rust_target: Some("crate::external::apache_commons::compress"),
            object_strategies: APACHE_COMMONS_COMPRESS_OBJECT_STRATEGIES,
            closed_hierarchy: NO_CLOSED_HIERARCHY,
            object_safety: NO_OBJECT_SAFETY,
            type_capabilities: APACHE_COMMONS_COMPRESS_TYPE_CAPABILITIES,
            field_nullability: NO_FIELD_NULLABILITY,
            argument_effects: NO_ARGUMENT_EFFECTS,
            exceptions: NO_EXCEPTIONS,
            api_effects: &[],
            modeled_apis: &[],
            modeled_constructors: APACHE_COMMONS_COMPRESS_MODELED_CONSTRUCTORS,
            modeled_fields: APACHE_COMMONS_COMPRESS_MODELED_FIELDS,
            unsupported_apis: &[],
        },
        ExternalPackManifest {
            pack_name: APACHE_COMMONS_LANG_PACK,
            owned_type_patterns: APACHE_COMMONS_LANG_OWNED_TYPES,
            mode: ExternalLibraryMode::ExternalFactModel,
            default_coverage: ExternalApiCoverage::Semantic,
            rust_target: Some("crate::external::apache_commons::lang"),
            object_strategies: APACHE_COMMONS_LANG_OBJECT_STRATEGIES,
            closed_hierarchy: NO_CLOSED_HIERARCHY,
            object_safety: NO_OBJECT_SAFETY,
            type_capabilities: APACHE_COMMONS_LANG_TYPE_CAPABILITIES,
            field_nullability: NO_FIELD_NULLABILITY,
            argument_effects: NO_ARGUMENT_EFFECTS,
            exceptions: APACHE_COMMONS_LANG_EXCEPTIONS,
            api_effects: APACHE_COMMONS_LANG_API_EFFECTS,
            modeled_apis: APACHE_COMMONS_LANG_MODELED_APIS,
            modeled_constructors: APACHE_COMMONS_LANG_MODELED_CONSTRUCTORS,
            modeled_fields: APACHE_COMMONS_LANG_MODELED_FIELDS,
            unsupported_apis: &[],
        },
        ExternalPackManifest {
            pack_name: APACHE_COMMONS_CODEC_PACK,
            owned_type_patterns: APACHE_COMMONS_CODEC_OWNED_TYPES,
            mode: ExternalLibraryMode::ExternalFactModel,
            default_coverage: ExternalApiCoverage::Semantic,
            rust_target: Some("crate::external::apache_commons::codec"),
            object_strategies: APACHE_COMMONS_CODEC_OBJECT_STRATEGIES,
            closed_hierarchy: NO_CLOSED_HIERARCHY,
            object_safety: NO_OBJECT_SAFETY,
            type_capabilities: APACHE_COMMONS_CODEC_TYPE_CAPABILITIES,
            field_nullability: NO_FIELD_NULLABILITY,
            argument_effects: NO_ARGUMENT_EFFECTS,
            exceptions: NO_EXCEPTIONS,
            api_effects: &[],
            modeled_apis: &[],
            modeled_constructors: &[],
            modeled_fields: APACHE_COMMONS_CODEC_MODELED_FIELDS,
            unsupported_apis: &[],
        },
        ExternalPackManifest {
            pack_name: JAMA_PACK,
            owned_type_patterns: JAMA_OWNED_TYPES,
            mode: ExternalLibraryMode::ExternalFactModel,
            default_coverage: ExternalApiCoverage::Semantic,
            rust_target: Some("crate::external::jama"),
            object_strategies: JAMA_OBJECT_STRATEGIES,
            closed_hierarchy: NO_CLOSED_HIERARCHY,
            object_safety: NO_OBJECT_SAFETY,
            type_capabilities: JAMA_TYPE_CAPABILITIES,
            field_nullability: NO_FIELD_NULLABILITY,
            argument_effects: NO_ARGUMENT_EFFECTS,
            exceptions: NO_EXCEPTIONS,
            api_effects: JAMA_API_EFFECTS,
            modeled_apis: JAMA_MODELED_APIS,
            modeled_constructors: JAMA_MODELED_CONSTRUCTORS,
            modeled_fields: NO_MODELED_FIELDS,
            unsupported_apis: &[],
        },
        ExternalPackManifest {
            pack_name: HIPPARCHUS_DISTRIBUTION_PACK,
            owned_type_patterns: HIPPARCHUS_DISTRIBUTION_OWNED_TYPES,
            mode: ExternalLibraryMode::ExternalFactModel,
            default_coverage: ExternalApiCoverage::Semantic,
            rust_target: Some("crate::external::hipparchus::random"),
            object_strategies: HIPPARCHUS_DISTRIBUTION_OBJECT_STRATEGIES,
            closed_hierarchy: NO_CLOSED_HIERARCHY,
            object_safety: NO_OBJECT_SAFETY,
            type_capabilities: HIPPARCHUS_DISTRIBUTION_TYPE_CAPABILITIES,
            field_nullability: NO_FIELD_NULLABILITY,
            argument_effects: NO_ARGUMENT_EFFECTS,
            exceptions: NO_EXCEPTIONS,
            api_effects: &[],
            modeled_apis: &[],
            modeled_constructors: HIPPARCHUS_DISTRIBUTION_MODELED_CONSTRUCTORS,
            modeled_fields: NO_MODELED_FIELDS,
            unsupported_apis: &[],
        },
        ExternalPackManifest {
            pack_name: APACHE_HTTP_PACK,
            owned_type_patterns: APACHE_HTTP_OWNED_TYPES,
            mode: ExternalLibraryMode::ExternalFactModel,
            default_coverage: ExternalApiCoverage::Semantic,
            rust_target: Some("crate::external::apache_http"),
            object_strategies: APACHE_HTTP_OBJECT_STRATEGIES,
            closed_hierarchy: NO_CLOSED_HIERARCHY,
            object_safety: NO_OBJECT_SAFETY,
            type_capabilities: APACHE_HTTP_TYPE_CAPABILITIES,
            field_nullability: NO_FIELD_NULLABILITY,
            argument_effects: NO_ARGUMENT_EFFECTS,
            exceptions: NO_EXCEPTIONS,
            api_effects: APACHE_HTTP_API_EFFECTS,
            modeled_apis: APACHE_HTTP_MODELED_APIS,
            modeled_constructors: APACHE_HTTP_MODELED_CONSTRUCTORS,
            modeled_fields: NO_MODELED_FIELDS,
            unsupported_apis: &[],
        },
        ExternalPackManifest {
            pack_name: GOOGLE_BIGQUERY_PACK,
            owned_type_patterns: GOOGLE_BIGQUERY_OWNED_TYPES,
            mode: ExternalLibraryMode::ExternalFactModel,
            default_coverage: ExternalApiCoverage::Semantic,
            rust_target: Some("crate::external::google::bigquery"),
            object_strategies: GOOGLE_BIGQUERY_OBJECT_STRATEGIES,
            closed_hierarchy: NO_CLOSED_HIERARCHY,
            object_safety: NO_OBJECT_SAFETY,
            type_capabilities: GOOGLE_BIGQUERY_TYPE_CAPABILITIES,
            field_nullability: NO_FIELD_NULLABILITY,
            argument_effects: NO_ARGUMENT_EFFECTS,
            exceptions: NO_EXCEPTIONS,
            api_effects: &[],
            modeled_apis: &[],
            modeled_constructors: &[],
            modeled_fields: GOOGLE_BIGQUERY_MODELED_FIELDS,
            unsupported_apis: &[],
        },
        ExternalPackManifest {
            pack_name: GOOGLE_BIGQUERY_STORAGE_PACK,
            owned_type_patterns: GOOGLE_BIGQUERY_STORAGE_OWNED_TYPES,
            mode: ExternalLibraryMode::ExternalFactModel,
            default_coverage: ExternalApiCoverage::Semantic,
            rust_target: Some("crate::external::google::bigquery_storage"),
            object_strategies: GOOGLE_BIGQUERY_STORAGE_OBJECT_STRATEGIES,
            closed_hierarchy: NO_CLOSED_HIERARCHY,
            object_safety: NO_OBJECT_SAFETY,
            type_capabilities: GOOGLE_BIGQUERY_STORAGE_TYPE_CAPABILITIES,
            field_nullability: NO_FIELD_NULLABILITY,
            argument_effects: NO_ARGUMENT_EFFECTS,
            exceptions: NO_EXCEPTIONS,
            api_effects: &[],
            modeled_apis: &[],
            modeled_constructors: &[],
            modeled_fields: GOOGLE_BIGQUERY_STORAGE_MODELED_FIELDS,
            unsupported_apis: &[],
        },
        ExternalPackManifest {
            pack_name: GOOGLE_STORAGE_PACK,
            owned_type_patterns: GOOGLE_STORAGE_OWNED_TYPES,
            mode: ExternalLibraryMode::ExternalFactModel,
            default_coverage: ExternalApiCoverage::Semantic,
            rust_target: Some("crate::external::google::storage"),
            object_strategies: GOOGLE_STORAGE_OBJECT_STRATEGIES,
            closed_hierarchy: NO_CLOSED_HIERARCHY,
            object_safety: NO_OBJECT_SAFETY,
            type_capabilities: GOOGLE_STORAGE_TYPE_CAPABILITIES,
            field_nullability: NO_FIELD_NULLABILITY,
            argument_effects: NO_ARGUMENT_EFFECTS,
            exceptions: NO_EXCEPTIONS,
            api_effects: &[],
            modeled_apis: &[],
            modeled_constructors: &[],
            modeled_fields: GOOGLE_STORAGE_MODELED_FIELDS,
            unsupported_apis: &[],
        },
        ExternalPackManifest {
            pack_name: GOOGLE_CLOUD_NIO_PACK,
            owned_type_patterns: GOOGLE_CLOUD_NIO_OWNED_TYPES,
            mode: ExternalLibraryMode::ExternalFactModel,
            default_coverage: ExternalApiCoverage::Semantic,
            rust_target: Some("crate::external::google::cloud_nio"),
            object_strategies: GOOGLE_CLOUD_NIO_OBJECT_STRATEGIES,
            closed_hierarchy: NO_CLOSED_HIERARCHY,
            object_safety: NO_OBJECT_SAFETY,
            type_capabilities: GOOGLE_CLOUD_NIO_TYPE_CAPABILITIES,
            field_nullability: NO_FIELD_NULLABILITY,
            argument_effects: NO_ARGUMENT_EFFECTS,
            exceptions: NO_EXCEPTIONS,
            api_effects: &[],
            modeled_apis: &[],
            modeled_constructors: &[],
            modeled_fields: GOOGLE_CLOUD_NIO_MODELED_FIELDS,
            unsupported_apis: &[],
        },
        ExternalPackManifest {
            pack_name: GENOMICSDB_PACK,
            owned_type_patterns: GENOMICSDB_OWNED_TYPES,
            mode: ExternalLibraryMode::ExternalFactModel,
            default_coverage: ExternalApiCoverage::Semantic,
            rust_target: Some("crate::external::genomicsdb"),
            object_strategies: GENOMICSDB_OBJECT_STRATEGIES,
            closed_hierarchy: NO_CLOSED_HIERARCHY,
            object_safety: NO_OBJECT_SAFETY,
            type_capabilities: GENOMICSDB_TYPE_CAPABILITIES,
            field_nullability: NO_FIELD_NULLABILITY,
            argument_effects: GENOMICSDB_ARGUMENT_EFFECTS,
            exceptions: NO_EXCEPTIONS,
            api_effects: GENOMICSDB_API_EFFECTS,
            modeled_apis: GENOMICSDB_MODELED_APIS,
            modeled_constructors: GENOMICSDB_MODELED_CONSTRUCTORS,
            modeled_fields: GENOMICSDB_MODELED_FIELDS,
            unsupported_apis: &[],
        },
        ExternalPackManifest {
            pack_name: PICARD_PROGRAMGROUPS_PACK,
            owned_type_patterns: PICARD_PROGRAMGROUPS_OWNED_TYPES,
            mode: ExternalLibraryMode::ExternalFactModel,
            default_coverage: ExternalApiCoverage::Semantic,
            rust_target: Some("crate::external::picard::programgroups"),
            object_strategies: PICARD_PROGRAMGROUPS_OBJECT_STRATEGIES,
            closed_hierarchy: NO_CLOSED_HIERARCHY,
            object_safety: NO_OBJECT_SAFETY,
            type_capabilities: PICARD_PROGRAMGROUPS_TYPE_CAPABILITIES,
            field_nullability: NO_FIELD_NULLABILITY,
            argument_effects: NO_ARGUMENT_EFFECTS,
            exceptions: NO_EXCEPTIONS,
            api_effects: PICARD_PROGRAMGROUPS_API_EFFECTS,
            modeled_apis: PICARD_PROGRAMGROUPS_MODELED_APIS,
            modeled_constructors: PICARD_PROGRAMGROUPS_MODELED_CONSTRUCTORS,
            modeled_fields: NO_MODELED_FIELDS,
            unsupported_apis: &[],
        },
        ExternalPackManifest {
            pack_name: PICARD_MARKDUPLICATES_PACK,
            owned_type_patterns: PICARD_MARKDUPLICATES_OWNED_TYPES,
            mode: ExternalLibraryMode::ExternalFactModel,
            default_coverage: ExternalApiCoverage::Semantic,
            rust_target: Some("crate::external::picard::markduplicates"),
            object_strategies: PICARD_MARKDUPLICATES_OBJECT_STRATEGIES,
            closed_hierarchy: NO_CLOSED_HIERARCHY,
            object_safety: NO_OBJECT_SAFETY,
            type_capabilities: PICARD_MARKDUPLICATES_TYPE_CAPABILITIES,
            field_nullability: NO_FIELD_NULLABILITY,
            argument_effects: NO_ARGUMENT_EFFECTS,
            exceptions: NO_EXCEPTIONS,
            api_effects: &[],
            modeled_apis: &[],
            modeled_constructors: PICARD_MARKDUPLICATES_MODELED_CONSTRUCTORS,
            modeled_fields: PICARD_MARKDUPLICATES_MODELED_FIELDS,
            unsupported_apis: &[],
        },
        ExternalPackManifest {
            pack_name: PICARD_METRICS_PACK,
            owned_type_patterns: PICARD_METRICS_OWNED_TYPES,
            mode: ExternalLibraryMode::ExternalFactModel,
            default_coverage: ExternalApiCoverage::Semantic,
            rust_target: Some("crate::external::picard::metrics"),
            object_strategies: PICARD_METRICS_OBJECT_STRATEGIES,
            closed_hierarchy: NO_CLOSED_HIERARCHY,
            object_safety: NO_OBJECT_SAFETY,
            type_capabilities: PICARD_METRICS_TYPE_CAPABILITIES,
            field_nullability: NO_FIELD_NULLABILITY,
            argument_effects: NO_ARGUMENT_EFFECTS,
            exceptions: NO_EXCEPTIONS,
            api_effects: &[],
            modeled_apis: &[],
            modeled_constructors: &[],
            modeled_fields: PICARD_METRICS_MODELED_FIELDS,
            unsupported_apis: &[],
        },
        ExternalPackManifest {
            pack_name: AVRO_PACK,
            owned_type_patterns: AVRO_OWNED_TYPES,
            mode: ExternalLibraryMode::ExternalFactModel,
            default_coverage: ExternalApiCoverage::Semantic,
            rust_target: Some("crate::external::avro"),
            object_strategies: AVRO_OBJECT_STRATEGIES,
            closed_hierarchy: NO_CLOSED_HIERARCHY,
            object_safety: NO_OBJECT_SAFETY,
            type_capabilities: AVRO_TYPE_CAPABILITIES,
            field_nullability: NO_FIELD_NULLABILITY,
            argument_effects: NO_ARGUMENT_EFFECTS,
            exceptions: NO_EXCEPTIONS,
            api_effects: AVRO_API_EFFECTS,
            modeled_apis: AVRO_MODELED_APIS,
            modeled_constructors: AVRO_MODELED_CONSTRUCTORS,
            modeled_fields: NO_MODELED_FIELDS,
            unsupported_apis: &[],
        },
        ExternalPackManifest {
            pack_name: BARCLAY_HELP_PACK,
            owned_type_patterns: BARCLAY_HELP_OWNED_TYPES,
            mode: ExternalLibraryMode::ExternalFactModel,
            default_coverage: ExternalApiCoverage::Semantic,
            rust_target: Some("crate::external::barclay::help"),
            object_strategies: BARCLAY_HELP_OBJECT_STRATEGIES,
            closed_hierarchy: NO_CLOSED_HIERARCHY,
            object_safety: NO_OBJECT_SAFETY,
            type_capabilities: BARCLAY_HELP_TYPE_CAPABILITIES,
            field_nullability: NO_FIELD_NULLABILITY,
            argument_effects: NO_ARGUMENT_EFFECTS,
            exceptions: BARCLAY_HELP_EXCEPTIONS,
            api_effects: &[],
            modeled_apis: &[],
            modeled_constructors: BARCLAY_HELP_MODELED_CONSTRUCTORS,
            modeled_fields: BARCLAY_HELP_MODELED_FIELDS,
            unsupported_apis: &[],
        },
        ExternalPackManifest {
            pack_name: BARCLAY_ARGPARSER_PACK,
            owned_type_patterns: BARCLAY_ARGPARSER_OWNED_TYPES,
            mode: ExternalLibraryMode::ExternalFactModel,
            default_coverage: ExternalApiCoverage::Semantic,
            rust_target: Some("crate::external::barclay::argparser"),
            object_strategies: BARCLAY_ARGPARSER_OBJECT_STRATEGIES,
            closed_hierarchy: NO_CLOSED_HIERARCHY,
            object_safety: NO_OBJECT_SAFETY,
            type_capabilities: BARCLAY_ARGPARSER_TYPE_CAPABILITIES,
            field_nullability: NO_FIELD_NULLABILITY,
            argument_effects: NO_ARGUMENT_EFFECTS,
            exceptions: NO_EXCEPTIONS,
            api_effects: &[],
            modeled_apis: &[],
            modeled_constructors: BARCLAY_ARGPARSER_MODELED_CONSTRUCTORS,
            modeled_fields: BARCLAY_ARGPARSER_MODELED_FIELDS,
            unsupported_apis: &[],
        },
        ExternalPackManifest {
            pack_name: BARCLAY_UTILS_PACK,
            owned_type_patterns: BARCLAY_UTILS_OWNED_TYPES,
            mode: ExternalLibraryMode::ExternalFactModel,
            default_coverage: ExternalApiCoverage::Semantic,
            rust_target: Some("crate::external::barclay::utils"),
            object_strategies: BARCLAY_UTILS_OBJECT_STRATEGIES,
            closed_hierarchy: NO_CLOSED_HIERARCHY,
            object_safety: NO_OBJECT_SAFETY,
            type_capabilities: BARCLAY_UTILS_TYPE_CAPABILITIES,
            field_nullability: NO_FIELD_NULLABILITY,
            argument_effects: NO_ARGUMENT_EFFECTS,
            exceptions: NO_EXCEPTIONS,
            api_effects: &[],
            modeled_apis: &[],
            modeled_constructors: BARCLAY_UTILS_MODELED_CONSTRUCTORS,
            modeled_fields: NO_MODELED_FIELDS,
            unsupported_apis: &[],
        },
        ExternalPackManifest {
            pack_name: SCALA_TUPLE_PACK,
            owned_type_patterns: SCALA_TUPLE_OWNED_TYPES,
            mode: ExternalLibraryMode::ExternalFactModel,
            default_coverage: ExternalApiCoverage::Semantic,
            rust_target: Some("crate::external::scala::tuple"),
            object_strategies: SCALA_TUPLE_OBJECT_STRATEGIES,
            closed_hierarchy: NO_CLOSED_HIERARCHY,
            object_safety: NO_OBJECT_SAFETY,
            type_capabilities: SCALA_TUPLE_TYPE_CAPABILITIES,
            field_nullability: NO_FIELD_NULLABILITY,
            argument_effects: NO_ARGUMENT_EFFECTS,
            exceptions: NO_EXCEPTIONS,
            api_effects: SCALA_TUPLE_API_EFFECTS,
            modeled_apis: SCALA_TUPLE_MODELED_APIS,
            modeled_constructors: SCALA_TUPLE_MODELED_CONSTRUCTORS,
            modeled_fields: NO_MODELED_FIELDS,
            unsupported_apis: &[],
        },
        ExternalPackManifest {
            pack_name: KRYO_PACK,
            owned_type_patterns: KRYO_OWNED_TYPES,
            mode: ExternalLibraryMode::ExternalFactModel,
            default_coverage: ExternalApiCoverage::Semantic,
            rust_target: Some("crate::external::kryo"),
            object_strategies: KRYO_OBJECT_STRATEGIES,
            closed_hierarchy: NO_CLOSED_HIERARCHY,
            object_safety: NO_OBJECT_SAFETY,
            type_capabilities: KRYO_TYPE_CAPABILITIES,
            field_nullability: NO_FIELD_NULLABILITY,
            argument_effects: NO_ARGUMENT_EFFECTS,
            exceptions: NO_EXCEPTIONS,
            api_effects: KRYO_API_EFFECTS,
            modeled_apis: KRYO_MODELED_APIS,
            modeled_constructors: KRYO_MODELED_CONSTRUCTORS,
            modeled_fields: NO_MODELED_FIELDS,
            unsupported_apis: &[],
        },
        ExternalPackManifest {
            pack_name: OPENCSV_PACK,
            owned_type_patterns: OPENCSV_OWNED_TYPES,
            mode: ExternalLibraryMode::ExternalFactModel,
            default_coverage: ExternalApiCoverage::Semantic,
            rust_target: Some("crate::external::opencsv"),
            object_strategies: OPENCSV_OBJECT_STRATEGIES,
            closed_hierarchy: NO_CLOSED_HIERARCHY,
            object_safety: NO_OBJECT_SAFETY,
            type_capabilities: OPENCSV_TYPE_CAPABILITIES,
            field_nullability: NO_FIELD_NULLABILITY,
            argument_effects: NO_ARGUMENT_EFFECTS,
            exceptions: NO_EXCEPTIONS,
            api_effects: OPENCSV_API_EFFECTS,
            modeled_apis: OPENCSV_MODELED_APIS,
            modeled_constructors: OPENCSV_MODELED_CONSTRUCTORS,
            modeled_fields: NO_MODELED_FIELDS,
            unsupported_apis: &[],
        },
        ExternalPackManifest {
            pack_name: ORG_JSON_PACK,
            owned_type_patterns: ORG_JSON_OWNED_TYPES,
            mode: ExternalLibraryMode::ExternalFactModel,
            default_coverage: ExternalApiCoverage::Semantic,
            rust_target: Some("crate::external::org_json"),
            object_strategies: ORG_JSON_OBJECT_STRATEGIES,
            closed_hierarchy: NO_CLOSED_HIERARCHY,
            object_safety: NO_OBJECT_SAFETY,
            type_capabilities: ORG_JSON_TYPE_CAPABILITIES,
            field_nullability: NO_FIELD_NULLABILITY,
            argument_effects: NO_ARGUMENT_EFFECTS,
            exceptions: NO_EXCEPTIONS,
            api_effects: ORG_JSON_API_EFFECTS,
            modeled_apis: ORG_JSON_MODELED_APIS,
            modeled_constructors: ORG_JSON_MODELED_CONSTRUCTORS,
            modeled_fields: NO_MODELED_FIELDS,
            unsupported_apis: &[],
        },
        ExternalPackManifest {
            pack_name: REFLECTIONS_PACK,
            owned_type_patterns: REFLECTIONS_OWNED_TYPES,
            mode: ExternalLibraryMode::ExternalFactModel,
            default_coverage: ExternalApiCoverage::Conservative,
            rust_target: Some("crate::external::reflections"),
            object_strategies: REFLECTIONS_OBJECT_STRATEGIES,
            closed_hierarchy: NO_CLOSED_HIERARCHY,
            object_safety: NO_OBJECT_SAFETY,
            type_capabilities: REFLECTIONS_TYPE_CAPABILITIES,
            field_nullability: NO_FIELD_NULLABILITY,
            argument_effects: NO_ARGUMENT_EFFECTS,
            exceptions: NO_EXCEPTIONS,
            api_effects: REFLECTIONS_API_EFFECTS,
            modeled_apis: REFLECTIONS_MODELED_APIS,
            modeled_constructors: REFLECTIONS_MODELED_CONSTRUCTORS,
            modeled_fields: NO_MODELED_FIELDS,
            unsupported_apis: &[],
        },
        ExternalPackManifest {
            pack_name: HADOOP_CONF_PACK,
            owned_type_patterns: HADOOP_CONF_OWNED_TYPES,
            mode: ExternalLibraryMode::ExternalFactModel,
            default_coverage: ExternalApiCoverage::Semantic,
            rust_target: Some("crate::external::hadoop::conf"),
            object_strategies: HADOOP_CONF_OBJECT_STRATEGIES,
            closed_hierarchy: NO_CLOSED_HIERARCHY,
            object_safety: NO_OBJECT_SAFETY,
            type_capabilities: HADOOP_CONF_TYPE_CAPABILITIES,
            field_nullability: NO_FIELD_NULLABILITY,
            argument_effects: NO_ARGUMENT_EFFECTS,
            exceptions: NO_EXCEPTIONS,
            api_effects: HADOOP_CONF_API_EFFECTS,
            modeled_apis: HADOOP_CONF_MODELED_APIS,
            modeled_constructors: HADOOP_CONF_MODELED_CONSTRUCTORS,
            modeled_fields: NO_MODELED_FIELDS,
            unsupported_apis: &[],
        },
        ExternalPackManifest {
            pack_name: APACHE_COMMONS_CONFIG_PACK,
            owned_type_patterns: APACHE_COMMONS_CONFIG_OWNED_TYPES,
            mode: ExternalLibraryMode::ExternalFactModel,
            default_coverage: ExternalApiCoverage::Semantic,
            rust_target: Some("crate::external::apache_commons::configuration"),
            object_strategies: APACHE_COMMONS_CONFIG_OBJECT_STRATEGIES,
            closed_hierarchy: NO_CLOSED_HIERARCHY,
            object_safety: NO_OBJECT_SAFETY,
            type_capabilities: APACHE_COMMONS_CONFIG_TYPE_CAPABILITIES,
            field_nullability: NO_FIELD_NULLABILITY,
            argument_effects: NO_ARGUMENT_EFFECTS,
            exceptions: NO_EXCEPTIONS,
            api_effects: APACHE_COMMONS_CONFIG_API_EFFECTS,
            modeled_apis: APACHE_COMMONS_CONFIG_MODELED_APIS,
            modeled_constructors: APACHE_COMMONS_CONFIG_MODELED_CONSTRUCTORS,
            modeled_fields: NO_MODELED_FIELDS,
            unsupported_apis: &[],
        },
        ExternalPackManifest {
            pack_name: SPARK_PACK,
            owned_type_patterns: SPARK_OWNED_TYPES,
            mode: ExternalLibraryMode::ExternalFactModel,
            default_coverage: ExternalApiCoverage::Semantic,
            rust_target: Some("crate::external::spark"),
            object_strategies: SPARK_OBJECT_STRATEGIES,
            closed_hierarchy: NO_CLOSED_HIERARCHY,
            object_safety: NO_OBJECT_SAFETY,
            type_capabilities: SPARK_TYPE_CAPABILITIES,
            field_nullability: NO_FIELD_NULLABILITY,
            argument_effects: NO_ARGUMENT_EFFECTS,
            exceptions: NO_EXCEPTIONS,
            api_effects: SPARK_API_EFFECTS,
            modeled_apis: SPARK_MODELED_APIS,
            modeled_constructors: SPARK_MODELED_CONSTRUCTORS,
            modeled_fields: NO_MODELED_FIELDS,
            unsupported_apis: &[],
        },
        ExternalPackManifest {
            pack_name: BWA_PACK,
            owned_type_patterns: BWA_OWNED_TYPES,
            mode: ExternalLibraryMode::ExternalFactModel,
            default_coverage: ExternalApiCoverage::Semantic,
            rust_target: Some("crate::external::bwa"),
            object_strategies: BWA_OBJECT_STRATEGIES,
            closed_hierarchy: NO_CLOSED_HIERARCHY,
            object_safety: NO_OBJECT_SAFETY,
            type_capabilities: BWA_TYPE_CAPABILITIES,
            field_nullability: NO_FIELD_NULLABILITY,
            argument_effects: NO_ARGUMENT_EFFECTS,
            exceptions: NO_EXCEPTIONS,
            api_effects: BWA_API_EFFECTS,
            modeled_apis: BWA_MODELED_APIS,
            modeled_constructors: BWA_MODELED_CONSTRUCTORS,
            modeled_fields: BWA_MODELED_FIELDS,
            unsupported_apis: &[],
        },
        ExternalPackManifest {
            pack_name: JGRAPHT_PACK,
            owned_type_patterns: JGRAPHT_OWNED_TYPES,
            mode: ExternalLibraryMode::ExternalFactModel,
            default_coverage: ExternalApiCoverage::Semantic,
            rust_target: Some("crate::external::jgrapht"),
            object_strategies: JGRAPHT_OBJECT_STRATEGIES,
            closed_hierarchy: NO_CLOSED_HIERARCHY,
            object_safety: NO_OBJECT_SAFETY,
            type_capabilities: JGRAPHT_TYPE_CAPABILITIES,
            field_nullability: NO_FIELD_NULLABILITY,
            argument_effects: NO_ARGUMENT_EFFECTS,
            exceptions: NO_EXCEPTIONS,
            api_effects: JGRAPHT_API_EFFECTS,
            modeled_apis: JGRAPHT_MODELED_APIS,
            modeled_constructors: JGRAPHT_MODELED_CONSTRUCTORS,
            modeled_fields: NO_MODELED_FIELDS,
            unsupported_apis: &[],
        },
        ExternalPackManifest {
            pack_name: FASTUTIL_PACK,
            owned_type_patterns: FASTUTIL_OWNED_TYPES,
            mode: ExternalLibraryMode::ExternalFactModel,
            default_coverage: ExternalApiCoverage::Semantic,
            rust_target: Some("crate::external::fastutil"),
            object_strategies: FASTUTIL_OBJECT_STRATEGIES,
            closed_hierarchy: NO_CLOSED_HIERARCHY,
            object_safety: NO_OBJECT_SAFETY,
            type_capabilities: FASTUTIL_TYPE_CAPABILITIES,
            field_nullability: NO_FIELD_NULLABILITY,
            argument_effects: NO_ARGUMENT_EFFECTS,
            exceptions: NO_EXCEPTIONS,
            api_effects: FASTUTIL_API_EFFECTS,
            modeled_apis: FASTUTIL_MODELED_APIS,
            modeled_constructors: FASTUTIL_MODELED_CONSTRUCTORS,
            modeled_fields: NO_MODELED_FIELDS,
            unsupported_apis: &[],
        },
        ExternalPackManifest {
            pack_name: KRYO_SERIALIZERS_PACK,
            owned_type_patterns: KRYO_SERIALIZERS_OWNED_TYPES,
            mode: ExternalLibraryMode::ExternalFactModel,
            default_coverage: ExternalApiCoverage::Semantic,
            rust_target: Some("crate::external::kryo_serializers"),
            object_strategies: KRYO_SERIALIZERS_OBJECT_STRATEGIES,
            closed_hierarchy: NO_CLOSED_HIERARCHY,
            object_safety: NO_OBJECT_SAFETY,
            type_capabilities: KRYO_SERIALIZERS_TYPE_CAPABILITIES,
            field_nullability: NO_FIELD_NULLABILITY,
            argument_effects: NO_ARGUMENT_EFFECTS,
            exceptions: NO_EXCEPTIONS,
            api_effects: &[],
            modeled_apis: &[],
            modeled_constructors: KRYO_SERIALIZERS_MODELED_CONSTRUCTORS,
            modeled_fields: NO_MODELED_FIELDS,
            unsupported_apis: &[],
        },
        ExternalPackManifest {
            pack_name: DISQ_WRITE_PACK,
            owned_type_patterns: DISQ_WRITE_OWNED_TYPES,
            mode: ExternalLibraryMode::ExternalFactModel,
            default_coverage: ExternalApiCoverage::Semantic,
            rust_target: Some("crate::external::disq"),
            object_strategies: DISQ_WRITE_OBJECT_STRATEGIES,
            closed_hierarchy: NO_CLOSED_HIERARCHY,
            object_safety: NO_OBJECT_SAFETY,
            type_capabilities: DISQ_WRITE_TYPE_CAPABILITIES,
            field_nullability: DISQ_WRITE_FIELD_NULLABILITY,
            argument_effects: NO_ARGUMENT_EFFECTS,
            exceptions: NO_EXCEPTIONS,
            api_effects: &[],
            modeled_apis: &[],
            modeled_constructors: DISQ_WRITE_MODELED_CONSTRUCTORS,
            modeled_fields: DISQ_WRITE_MODELED_FIELDS,
            unsupported_apis: &[],
        },
        ExternalPackManifest {
            pack_name: INTEL_GKL_PACK,
            owned_type_patterns: INTEL_GKL_OWNED_TYPES,
            mode: ExternalLibraryMode::ExternalFactModel,
            default_coverage: ExternalApiCoverage::Semantic,
            rust_target: Some("crate::external::intel_gkl"),
            object_strategies: INTEL_GKL_OBJECT_STRATEGIES,
            closed_hierarchy: NO_CLOSED_HIERARCHY,
            object_safety: NO_OBJECT_SAFETY,
            type_capabilities: INTEL_GKL_TYPE_CAPABILITIES,
            field_nullability: NO_FIELD_NULLABILITY,
            argument_effects: NO_ARGUMENT_EFFECTS,
            exceptions: NO_EXCEPTIONS,
            api_effects: &[],
            modeled_apis: &[],
            modeled_constructors: INTEL_GKL_MODELED_CONSTRUCTORS,
            modeled_fields: INTEL_GKL_MODELED_FIELDS,
            unsupported_apis: &[],
        },
        ExternalPackManifest {
            pack_name: GUAVA_IMMUTABLE_COLLECT_PACK,
            owned_type_patterns: GUAVA_IMMUTABLE_COLLECT_OWNED_TYPES,
            mode: ExternalLibraryMode::ExternalFactModel,
            default_coverage: ExternalApiCoverage::Semantic,
            rust_target: Some("crate::external::guava::collect"),
            object_strategies: GUAVA_IMMUTABLE_COLLECT_OBJECT_STRATEGIES,
            closed_hierarchy: NO_CLOSED_HIERARCHY,
            object_safety: NO_OBJECT_SAFETY,
            type_capabilities: GUAVA_IMMUTABLE_COLLECT_TYPE_CAPABILITIES,
            field_nullability: NO_FIELD_NULLABILITY,
            argument_effects: NO_ARGUMENT_EFFECTS,
            exceptions: NO_EXCEPTIONS,
            api_effects: GUAVA_IMMUTABLE_COLLECT_API_EFFECTS,
            modeled_apis: GUAVA_IMMUTABLE_COLLECT_MODELED_APIS,
            modeled_constructors: GUAVA_IMMUTABLE_COLLECT_MODELED_CONSTRUCTORS,
            modeled_fields: NO_MODELED_FIELDS,
            unsupported_apis: &[],
        },
        ExternalPackManifest {
            pack_name: GUAVA_CONCURRENT_PACK,
            owned_type_patterns: GUAVA_CONCURRENT_OWNED_TYPES,
            mode: ExternalLibraryMode::ExternalFactModel,
            default_coverage: ExternalApiCoverage::Semantic,
            rust_target: Some("crate::external::guava::concurrent"),
            object_strategies: GUAVA_CONCURRENT_OBJECT_STRATEGIES,
            closed_hierarchy: NO_CLOSED_HIERARCHY,
            object_safety: NO_OBJECT_SAFETY,
            type_capabilities: GUAVA_CONCURRENT_TYPE_CAPABILITIES,
            field_nullability: NO_FIELD_NULLABILITY,
            argument_effects: NO_ARGUMENT_EFFECTS,
            exceptions: NO_EXCEPTIONS,
            api_effects: GUAVA_CONCURRENT_API_EFFECTS,
            modeled_apis: GUAVA_CONCURRENT_MODELED_APIS,
            modeled_constructors: GUAVA_CONCURRENT_MODELED_CONSTRUCTORS,
            modeled_fields: NO_MODELED_FIELDS,
            unsupported_apis: &[],
        },
        ExternalPackManifest {
            pack_name: BROAD_HELLBENDER_UTILS_PACK,
            owned_type_patterns: BROAD_HELLBENDER_UTILS_OWNED_TYPES,
            mode: ExternalLibraryMode::ExternalFactModel,
            default_coverage: ExternalApiCoverage::Semantic,
            rust_target: Some("crate::external::broad::hellbender::utils"),
            object_strategies: &[],
            closed_hierarchy: NO_CLOSED_HIERARCHY,
            object_safety: NO_OBJECT_SAFETY,
            type_capabilities: NO_TYPE_CAPABILITIES,
            field_nullability: NO_FIELD_NULLABILITY,
            argument_effects: BROAD_HELLBENDER_UTILS_ARGUMENT_EFFECTS,
            exceptions: NO_EXCEPTIONS,
            api_effects: BROAD_HELLBENDER_UTILS_API_EFFECTS,
            modeled_apis: BROAD_HELLBENDER_UTILS_MODELED_APIS,
            modeled_constructors: &[],
            modeled_fields: NO_MODELED_FIELDS,
            unsupported_apis: &[],
        },
        ExternalPackManifest {
            pack_name: BROAD_HTTP_PACK,
            owned_type_patterns: BROAD_HTTP_OWNED_TYPES,
            mode: ExternalLibraryMode::ExternalFactModel,
            default_coverage: ExternalApiCoverage::Semantic,
            rust_target: Some("crate::external::broad_http"),
            object_strategies: BROAD_HTTP_OBJECT_STRATEGIES,
            closed_hierarchy: NO_CLOSED_HIERARCHY,
            object_safety: NO_OBJECT_SAFETY,
            type_capabilities: BROAD_HTTP_TYPE_CAPABILITIES,
            field_nullability: NO_FIELD_NULLABILITY,
            argument_effects: NO_ARGUMENT_EFFECTS,
            exceptions: NO_EXCEPTIONS,
            api_effects: BROAD_HTTP_API_EFFECTS,
            modeled_apis: BROAD_HTTP_MODELED_APIS,
            modeled_constructors: BROAD_HTTP_MODELED_CONSTRUCTORS,
            modeled_fields: BROAD_HTTP_MODELED_FIELDS,
            unsupported_apis: &[],
        },
        ExternalPackManifest {
            pack_name: PICARD_VCF_PACK,
            owned_type_patterns: PICARD_VCF_OWNED_TYPES,
            mode: ExternalLibraryMode::ExternalFactModel,
            default_coverage: ExternalApiCoverage::Semantic,
            rust_target: Some("crate::external::picard::vcf"),
            object_strategies: PICARD_VCF_OBJECT_STRATEGIES,
            closed_hierarchy: NO_CLOSED_HIERARCHY,
            object_safety: NO_OBJECT_SAFETY,
            type_capabilities: PICARD_VCF_TYPE_CAPABILITIES,
            field_nullability: NO_FIELD_NULLABILITY,
            argument_effects: NO_ARGUMENT_EFFECTS,
            exceptions: NO_EXCEPTIONS,
            api_effects: &[],
            modeled_apis: &[],
            modeled_constructors: PICARD_VCF_MODELED_CONSTRUCTORS,
            modeled_fields: PICARD_VCF_MODELED_FIELDS,
            unsupported_apis: &[],
        },
        ExternalPackManifest {
            pack_name: JACKSON_DATABIND_PACK,
            owned_type_patterns: JACKSON_DATABIND_OWNED_TYPES,
            mode: ExternalLibraryMode::ExternalFactModel,
            default_coverage: ExternalApiCoverage::Semantic,
            rust_target: Some("crate::external::jackson::databind"),
            object_strategies: JACKSON_DATABIND_OBJECT_STRATEGIES,
            closed_hierarchy: NO_CLOSED_HIERARCHY,
            object_safety: NO_OBJECT_SAFETY,
            type_capabilities: JACKSON_DATABIND_TYPE_CAPABILITIES,
            field_nullability: NO_FIELD_NULLABILITY,
            argument_effects: NO_ARGUMENT_EFFECTS,
            exceptions: NO_EXCEPTIONS,
            api_effects: &[],
            modeled_apis: &[],
            modeled_constructors: &[],
            modeled_fields: JACKSON_DATABIND_MODELED_FIELDS,
            unsupported_apis: &[],
        },
        ExternalPackManifest {
            pack_name: HDF5_PACK,
            owned_type_patterns: HDF5_OWNED_TYPES,
            mode: ExternalLibraryMode::RustCrateMapping,
            default_coverage: ExternalApiCoverage::Mapped,
            rust_target: Some("hdf5_pure_rust"),
            object_strategies: HDF5_OBJECT_STRATEGIES,
            closed_hierarchy: NO_CLOSED_HIERARCHY,
            object_safety: NO_OBJECT_SAFETY,
            type_capabilities: HDF5_TYPE_CAPABILITIES,
            field_nullability: HDF5_FIELD_NULLABILITY,
            argument_effects: NO_ARGUMENT_EFFECTS,
            exceptions: HDF5_EXCEPTIONS,
            api_effects: HDF5_API_EFFECTS,
            modeled_apis: HDF5_MODELED_APIS,
            modeled_constructors: HDF5_MODELED_CONSTRUCTORS,
            modeled_fields: HDF5_MODELED_FIELDS,
            unsupported_apis: &[],
        },
        ExternalPackManifest {
            pack_name: SQLITE_PACK,
            owned_type_patterns: SQLITE_OWNED_TYPES,
            mode: ExternalLibraryMode::RustCrateMapping,
            default_coverage: ExternalApiCoverage::Mapped,
            rust_target: Some("sqlite"),
            object_strategies: SQLITE_OBJECT_STRATEGIES,
            closed_hierarchy: NO_CLOSED_HIERARCHY,
            object_safety: NO_OBJECT_SAFETY,
            type_capabilities: NO_TYPE_CAPABILITIES,
            field_nullability: NO_FIELD_NULLABILITY,
            argument_effects: SQLITE_ARGUMENT_EFFECTS,
            exceptions: SQLITE_EXCEPTIONS,
            api_effects: SQLITE_API_EFFECTS,
            modeled_apis: SQLITE_MODELED_APIS,
            modeled_constructors: SQLITE_MODELED_CONSTRUCTORS,
            modeled_fields: NO_MODELED_FIELDS,
            unsupported_apis: &[],
        },
        ExternalPackManifest {
            pack_name: JAVA_SQL_PACK,
            owned_type_patterns: JAVA_SQL_OWNED_TYPES,
            mode: ExternalLibraryMode::ExternalFactModel,
            default_coverage: ExternalApiCoverage::Semantic,
            rust_target: Some("crate::external::java_sql"),
            object_strategies: JAVA_SQL_OBJECT_STRATEGIES,
            closed_hierarchy: NO_CLOSED_HIERARCHY,
            object_safety: NO_OBJECT_SAFETY,
            type_capabilities: NO_TYPE_CAPABILITIES,
            field_nullability: NO_FIELD_NULLABILITY,
            argument_effects: JAVA_SQL_ARGUMENT_EFFECTS,
            exceptions: JAVA_SQL_EXCEPTIONS,
            api_effects: JAVA_SQL_API_EFFECTS,
            modeled_apis: JAVA_SQL_MODELED_APIS,
            modeled_constructors: &[],
            modeled_fields: NO_MODELED_FIELDS,
            unsupported_apis: JAVA_SQL_UNSUPPORTED_APIS,
        },
    ]
}

pub fn builtin_external_pack_manifest(pack_name: &str) -> Option<ExternalPackManifest> {
    builtin_external_pack_manifests()
        .into_iter()
        .find(|manifest| manifest.pack_name == pack_name)
}

pub fn builtin_external_pack_manifest_for_provider_name(
    provider_name: &str,
) -> Option<ExternalPackManifest> {
    let pack_name = provider_name
        .strip_prefix("builtin-pack:")
        .unwrap_or(provider_name);
    builtin_external_pack_manifest(pack_name)
}

fn manifest_by_name(pack_name: &str) -> ExternalPackManifest {
    builtin_external_pack_manifest(pack_name)
        .unwrap_or_else(|| panic!("missing builtin external pack manifest for {pack_name}"))
}

pub fn builtin_external_library_packs() -> FactSet {
    let mut facts = FactSet::default();
    facts.extend_from(htsjdk_variantcontext_pack_facts());
    facts.extend_from(htsjdk_vcf_pack_facts());
    facts.extend_from(htsjdk_bcf_pack_facts());
    facts.extend_from(htsjdk_sam_header_pack_facts());
    facts.extend_from(htsjdk_sam_record_pack_facts());
    facts.extend_from(htsjdk_sam_reference_pack_facts());
    facts.extend_from(htsjdk_sam_reader_pack_facts());
    facts.extend_from(htsjdk_sam_writer_pack_facts());
    facts.extend_from(htsjdk_sam_bam_pack_facts());
    facts.extend_from(htsjdk_sam_cram_pack_facts());
    facts.extend_from(htsjdk_sam_metrics_pack_facts());
    facts.extend_from(htsjdk_nested_helper_pack_facts());
    facts.extend_from(htsjdk_tribble_feature_pack_facts());
    facts.extend_from(htsjdk_tribble_index_pack_facts());
    facts.extend_from(htsjdk_samtools_util_pack_facts());
    facts.extend_from(apache_commons_math_pack_facts());
    facts.extend_from(apache_commons_math_optim_pack_facts());
    facts.extend_from(apache_commons_math_stats_pack_facts());
    facts.extend_from(apache_commons_collections_pack_facts());
    facts.extend_from(apache_commons_compress_pack_facts());
    facts.extend_from(apache_commons_lang_pack_facts());
    facts.extend_from(apache_commons_codec_pack_facts());
    facts.extend_from(jama_pack_facts());
    facts.extend_from(hipparchus_distribution_pack_facts());
    facts.extend_from(apache_http_pack_facts());
    facts.extend_from(google_bigquery_pack_facts());
    facts.extend_from(google_bigquery_storage_pack_facts());
    facts.extend_from(google_storage_pack_facts());
    facts.extend_from(google_cloud_nio_pack_facts());
    facts.extend_from(genomicsdb_pack_facts());
    facts.extend_from(picard_programgroups_pack_facts());
    facts.extend_from(picard_markduplicates_pack_facts());
    facts.extend_from(picard_metrics_pack_facts());
    facts.extend_from(avro_pack_facts());
    facts.extend_from(barclay_help_pack_facts());
    facts.extend_from(barclay_argparser_pack_facts());
    facts.extend_from(barclay_utils_pack_facts());
    facts.extend_from(scala_tuple_pack_facts());
    facts.extend_from(kryo_pack_facts());
    facts.extend_from(opencsv_pack_facts());
    facts.extend_from(org_json_pack_facts());
    facts.extend_from(reflections_pack_facts());
    facts.extend_from(hadoop_conf_pack_facts());
    facts.extend_from(apache_commons_config_pack_facts());
    facts.extend_from(spark_pack_facts());
    facts.extend_from(bwa_pack_facts());
    facts.extend_from(jgrapht_pack_facts());
    facts.extend_from(fastutil_pack_facts());
    facts.extend_from(kryo_serializers_pack_facts());
    facts.extend_from(disq_write_pack_facts());
    facts.extend_from(intel_gkl_pack_facts());
    facts.extend_from(guava_immutable_collect_pack_facts());
    facts.extend_from(guava_concurrent_pack_facts());
    facts.extend_from(broad_hellbender_utils_pack_facts());
    facts.extend_from(broad_http_pack_facts());
    facts.extend_from(picard_vcf_pack_facts());
    facts.extend_from(jackson_databind_pack_facts());
    facts.extend_from(hdf5_pack_facts());
    facts.extend_from(sqlite_pack_facts());
    facts.extend_from(java_sql_pack_facts());
    facts
}

pub fn builtin_external_api_argument_effect_facts() -> Vec<ExternalApiArgumentEffectFact> {
    builtin_external_library_packs().external_api_argument_effect_facts
}

pub fn java_platform_api_argument_effect_facts() -> Vec<ExternalApiArgumentEffectFact> {
    ["Class", "java.lang.Class"]
        .into_iter()
        .map(|receiver_type| ExternalApiArgumentEffectFact {
            method_name: "forName".to_string(),
            receiver_type: Some(receiver_type.to_string()),
            parameter_index: 0,
            parameter_type: Some(JavaType::String),
            parameter_nullability: NullabilityState::DefinitelyNonNull,
            argument_mutability: ReceiverMutabilityFact::ReadOnly,
            alias_policy: ExternalArgumentAliasPolicy::Borrowed,
            provenance: FactProvenance::BuiltinExternalModel,
        })
        .collect()
}

pub fn java_platform_exception_facts() -> Vec<ExceptionFact> {
    ["ClassNotFoundException", "java.lang.ClassNotFoundException"]
        .into_iter()
        .map(|type_name| ExceptionFact {
            type_name: type_name.to_string(),
            kind: ExceptionKind::Checked,
            provenance: FactProvenance::BuiltinExternalModel,
        })
        .collect()
}

pub fn builtin_external_pack_facts(pack_name: &str) -> Option<FactSet> {
    builtin_external_pack_fact_sets()
        .iter()
        .find(|(name, _)| name == pack_name)
        .map(|(_, facts)| facts.clone())
}

fn builtin_external_pack_fact_sets() -> &'static [(String, FactSet)] {
    static FACT_SETS: OnceLock<Vec<(String, FactSet)>> = OnceLock::new();
    FACT_SETS.get_or_init(|| {
        vec![
            (
                VARIANTCONTEXT_PACK.to_string(),
                htsjdk_variantcontext_pack_facts(),
            ),
            (VCF_PACK.to_string(), htsjdk_vcf_pack_facts()),
            (BCF_PACK.to_string(), htsjdk_bcf_pack_facts()),
            (SAM_HEADER_PACK.to_string(), htsjdk_sam_header_pack_facts()),
            (SAM_RECORD_PACK.to_string(), htsjdk_sam_record_pack_facts()),
            (
                SAM_REFERENCE_PACK.to_string(),
                htsjdk_sam_reference_pack_facts(),
            ),
            (SAM_READER_PACK.to_string(), htsjdk_sam_reader_pack_facts()),
            (SAM_WRITER_PACK.to_string(), htsjdk_sam_writer_pack_facts()),
            (SAM_BAM_PACK.to_string(), htsjdk_sam_bam_pack_facts()),
            (SAM_CRAM_PACK.to_string(), htsjdk_sam_cram_pack_facts()),
            (
                SAM_METRICS_PACK.to_string(),
                htsjdk_sam_metrics_pack_facts(),
            ),
            (
                HTSJDK_NESTED_HELPER_PACK.to_string(),
                htsjdk_nested_helper_pack_facts(),
            ),
            (
                TRIBBLE_FEATURE_PACK.to_string(),
                htsjdk_tribble_feature_pack_facts(),
            ),
            (
                TRIBBLE_INDEX_PACK.to_string(),
                htsjdk_tribble_index_pack_facts(),
            ),
            (
                SAMTOOLS_UTIL_PACK.to_string(),
                htsjdk_samtools_util_pack_facts(),
            ),
            (
                APACHE_COMMONS_MATH_PACK.to_string(),
                apache_commons_math_pack_facts(),
            ),
            (
                APACHE_COMMONS_MATH_OPTIM_PACK.to_string(),
                apache_commons_math_optim_pack_facts(),
            ),
            (
                APACHE_COMMONS_MATH_STATS_PACK.to_string(),
                apache_commons_math_stats_pack_facts(),
            ),
            (
                APACHE_COMMONS_COLLECTIONS_PACK.to_string(),
                apache_commons_collections_pack_facts(),
            ),
            (
                APACHE_COMMONS_COMPRESS_PACK.to_string(),
                apache_commons_compress_pack_facts(),
            ),
            (
                APACHE_COMMONS_LANG_PACK.to_string(),
                apache_commons_lang_pack_facts(),
            ),
            (
                APACHE_COMMONS_CODEC_PACK.to_string(),
                apache_commons_codec_pack_facts(),
            ),
            (JAMA_PACK.to_string(), jama_pack_facts()),
            (
                HIPPARCHUS_DISTRIBUTION_PACK.to_string(),
                hipparchus_distribution_pack_facts(),
            ),
            (APACHE_HTTP_PACK.to_string(), apache_http_pack_facts()),
            (
                GOOGLE_BIGQUERY_PACK.to_string(),
                google_bigquery_pack_facts(),
            ),
            (
                GOOGLE_BIGQUERY_STORAGE_PACK.to_string(),
                google_bigquery_storage_pack_facts(),
            ),
            (GOOGLE_STORAGE_PACK.to_string(), google_storage_pack_facts()),
            (
                GOOGLE_CLOUD_NIO_PACK.to_string(),
                google_cloud_nio_pack_facts(),
            ),
            (GENOMICSDB_PACK.to_string(), genomicsdb_pack_facts()),
            (
                PICARD_PROGRAMGROUPS_PACK.to_string(),
                picard_programgroups_pack_facts(),
            ),
            (
                PICARD_MARKDUPLICATES_PACK.to_string(),
                picard_markduplicates_pack_facts(),
            ),
            (PICARD_METRICS_PACK.to_string(), picard_metrics_pack_facts()),
            (AVRO_PACK.to_string(), avro_pack_facts()),
            (BARCLAY_HELP_PACK.to_string(), barclay_help_pack_facts()),
            (
                BARCLAY_ARGPARSER_PACK.to_string(),
                barclay_argparser_pack_facts(),
            ),
            (BARCLAY_UTILS_PACK.to_string(), barclay_utils_pack_facts()),
            (SCALA_TUPLE_PACK.to_string(), scala_tuple_pack_facts()),
            (KRYO_PACK.to_string(), kryo_pack_facts()),
            (OPENCSV_PACK.to_string(), opencsv_pack_facts()),
            (ORG_JSON_PACK.to_string(), org_json_pack_facts()),
            (REFLECTIONS_PACK.to_string(), reflections_pack_facts()),
            (HADOOP_CONF_PACK.to_string(), hadoop_conf_pack_facts()),
            (
                APACHE_COMMONS_CONFIG_PACK.to_string(),
                apache_commons_config_pack_facts(),
            ),
            (SPARK_PACK.to_string(), spark_pack_facts()),
            (BWA_PACK.to_string(), bwa_pack_facts()),
            (JGRAPHT_PACK.to_string(), jgrapht_pack_facts()),
            (FASTUTIL_PACK.to_string(), fastutil_pack_facts()),
            (
                KRYO_SERIALIZERS_PACK.to_string(),
                kryo_serializers_pack_facts(),
            ),
            (DISQ_WRITE_PACK.to_string(), disq_write_pack_facts()),
            (INTEL_GKL_PACK.to_string(), intel_gkl_pack_facts()),
            (
                GUAVA_IMMUTABLE_COLLECT_PACK.to_string(),
                guava_immutable_collect_pack_facts(),
            ),
            (
                GUAVA_CONCURRENT_PACK.to_string(),
                guava_concurrent_pack_facts(),
            ),
            (
                BROAD_HELLBENDER_UTILS_PACK.to_string(),
                broad_hellbender_utils_pack_facts(),
            ),
            (BROAD_HTTP_PACK.to_string(), broad_http_pack_facts()),
            (PICARD_VCF_PACK.to_string(), picard_vcf_pack_facts()),
            (
                JACKSON_DATABIND_PACK.to_string(),
                jackson_databind_pack_facts(),
            ),
            (HDF5_PACK.to_string(), hdf5_pack_facts()),
            (SQLITE_PACK.to_string(), sqlite_pack_facts()),
            (JAVA_SQL_PACK.to_string(), java_sql_pack_facts()),
        ]
    })
}

#[derive(Default)]
struct BuiltinExternalPackLookupIndex {
    api_effects: HashMap<(String, String, Option<usize>), ExternalApiEffectFact>,
    unsupported_apis: HashMap<(String, String), UnsupportedExternalApiFact>,
    constructors: HashMap<(String, usize), ExternalConstructorFact>,
    unsupported_constructors: HashMap<(String, usize), UnsupportedExternalConstructorFact>,
    fields: HashMap<(String, String), ExternalFieldFact>,
    unsupported_fields: HashMap<(String, String), UnsupportedExternalFieldFact>,
    crate_api_mappings: HashMap<(String, String, Option<usize>), ExternalCrateApiMappingFact>,
    crate_constructor_mappings: HashMap<(String, usize), ExternalCrateConstructorMappingFact>,
    representation_facts: HashMap<String, FactSet>,
}

fn builtin_external_pack_lookup_index() -> &'static BuiltinExternalPackLookupIndex {
    static INDEX: OnceLock<BuiltinExternalPackLookupIndex> = OnceLock::new();
    INDEX.get_or_init(|| {
        let mut index = BuiltinExternalPackLookupIndex::default();
        for (_, facts) in builtin_external_pack_fact_sets() {
            for fact in facts.external_api_effect_facts.iter().rev() {
                if let Some(receiver_type) = &fact.receiver_type {
                    for receiver_alias in external_receiver_lookup_aliases(receiver_type) {
                        for method_alias in external_method_name_lookup_aliases(&fact.method_name) {
                            index
                                .api_effects
                                .entry((receiver_alias.clone(), method_alias, fact.arg_count))
                                .or_insert_with(|| fact.clone());
                        }
                    }
                }
            }
            for fact in facts.unsupported_external_api_facts.iter().rev() {
                for receiver_alias in external_receiver_lookup_aliases(&fact.receiver_type) {
                    index
                        .unsupported_apis
                        .entry((receiver_alias, fact.method_name.clone()))
                        .or_insert_with(|| fact.clone());
                }
            }
            for fact in facts.external_constructor_facts.iter().rev() {
                for type_alias in external_receiver_lookup_aliases(&fact.type_name) {
                    index
                        .constructors
                        .entry((type_alias, fact.arg_types.len()))
                        .or_insert_with(|| fact.clone());
                }
            }
            for fact in facts.unsupported_external_constructor_facts.iter().rev() {
                for type_alias in external_receiver_lookup_aliases(&fact.type_name) {
                    index
                        .unsupported_constructors
                        .entry((type_alias, fact.arg_count))
                        .or_insert_with(|| fact.clone());
                }
            }
            for fact in facts.external_field_facts.iter().rev() {
                for type_alias in external_field_type_lookup_aliases(&fact.field.type_name) {
                    for field_alias in external_field_name_lookup_aliases(&fact.field.field_name) {
                        index
                            .fields
                            .entry((type_alias.clone(), field_alias))
                            .or_insert_with(|| fact.clone());
                    }
                }
            }
            for fact in facts.unsupported_external_field_facts.iter().rev() {
                for type_alias in external_field_type_lookup_aliases(&fact.field.type_name) {
                    for field_alias in external_field_name_lookup_aliases(&fact.field.field_name) {
                        index
                            .unsupported_fields
                            .entry((type_alias.clone(), field_alias))
                            .or_insert_with(|| fact.clone());
                    }
                }
            }
            for fact in facts.external_crate_api_mapping_facts.iter().rev() {
                for receiver_alias in external_receiver_lookup_aliases(&fact.receiver_type) {
                    index
                        .crate_api_mappings
                        .entry((receiver_alias, fact.method_name.clone(), fact.arg_count))
                        .or_insert_with(|| fact.clone());
                }
            }
            for fact in facts.external_crate_constructor_mapping_facts.iter().rev() {
                for type_alias in external_receiver_lookup_aliases(&fact.type_name) {
                    index
                        .crate_constructor_mappings
                        .entry((type_alias, fact.arg_count))
                        .or_insert_with(|| fact.clone());
                }
            }
            for fact in facts.hierarchy_overrides.iter().rev() {
                for type_alias in external_receiver_lookup_aliases(&fact.type_name) {
                    index
                        .representation_facts
                        .entry(type_alias)
                        .or_default()
                        .hierarchy_overrides
                        .push(fact.clone());
                }
            }
            for fact in facts.closed_hierarchy_facts.iter().rev() {
                for type_alias in external_receiver_lookup_aliases(&fact.type_name) {
                    index
                        .representation_facts
                        .entry(type_alias)
                        .or_default()
                        .closed_hierarchy_facts
                        .push(fact.clone());
                }
            }
            for fact in facts.object_safety_facts.iter().rev() {
                for type_alias in external_receiver_lookup_aliases(&fact.type_name) {
                    index
                        .representation_facts
                        .entry(type_alias)
                        .or_default()
                        .object_safety_facts
                        .push(fact.clone());
                }
            }
            for fact in facts.type_capability_overrides.iter().rev() {
                for type_alias in external_receiver_lookup_aliases(&fact.type_name) {
                    index
                        .representation_facts
                        .entry(type_alias)
                        .or_default()
                        .type_capability_overrides
                        .push(fact.clone());
                }
            }
            for fact in facts.owned_recovery_capability_facts.iter().rev() {
                for type_alias in external_receiver_lookup_aliases(&fact.type_name) {
                    index
                        .representation_facts
                        .entry(type_alias)
                        .or_default()
                        .owned_recovery_capability_facts
                        .push(fact.clone());
                }
            }
            for fact in facts.exception_facts.iter().rev() {
                for type_alias in external_receiver_lookup_aliases(&fact.type_name) {
                    index
                        .representation_facts
                        .entry(type_alias)
                        .or_default()
                        .exception_facts
                        .push(fact.clone());
                }
            }
        }
        index
    })
}

fn external_receiver_lookup_aliases(receiver_type: &str) -> Vec<String> {
    let canonical = crate::stdlib::canonical_external_receiver_type_key(receiver_type);
    let snake = crate::parser::to_snake_case(receiver_type);
    let mut aliases = vec![receiver_type.to_string(), canonical.clone(), snake];
    if let Some(simple) = package_qualified_simple_alias(receiver_type) {
        aliases.extend([
            simple.to_string(),
            crate::stdlib::canonical_external_receiver_type_key(simple),
            crate::parser::to_snake_case(simple),
        ]);
    }
    if let Some(simple) = package_qualified_simple_alias(&canonical) {
        aliases.extend([
            simple.to_string(),
            crate::stdlib::canonical_external_receiver_type_key(simple),
            crate::parser::to_snake_case(simple),
        ]);
    }
    aliases.sort();
    aliases.dedup();
    aliases
}

fn package_qualified_simple_alias(type_name: &str) -> Option<&str> {
    let (prefix, simple) = type_name.rsplit_once('.')?;
    if prefix
        .split('.')
        .any(|segment| segment.chars().next().is_some_and(char::is_lowercase))
    {
        Some(simple)
    } else {
        None
    }
}

fn external_field_type_lookup_aliases(type_name: &str) -> Vec<String> {
    let canonical = crate::stdlib::canonical_external_receiver_type_key(type_name);
    let snake = crate::parser::to_snake_case(type_name);
    let mut aliases = vec![type_name.to_string(), canonical, snake];
    aliases.sort();
    aliases.dedup();
    aliases
}

fn external_field_name_lookup_aliases(field_name: &str) -> Vec<String> {
    let canonical = crate::parser::to_snake_case(field_name);
    let mut aliases = vec![field_name.to_string(), canonical];
    aliases.sort();
    aliases.dedup();
    aliases
}

fn external_method_name_lookup_aliases(method_name: &str) -> Vec<String> {
    let canonical = crate::parser::to_snake_case(method_name);
    let mut aliases = vec![method_name.to_string(), canonical];
    aliases.sort();
    aliases.dedup();
    aliases
}

pub fn external_api_effect_for_receiver(
    receiver_type: Option<&str>,
    method_name: &str,
) -> Option<ExternalApiEffectFact> {
    external_api_effect_for_receiver_with_arity(receiver_type, method_name, None)
}

pub fn external_api_effect_for_receiver_with_arity(
    receiver_type: Option<&str>,
    method_name: &str,
    arg_count: Option<usize>,
) -> Option<ExternalApiEffectFact> {
    let receiver_type = receiver_type?;
    if let Some(effect) =
        java_platform_api_effect_for_receiver_with_arity(receiver_type, method_name, arg_count)
    {
        return Some(effect);
    }
    let index = builtin_external_pack_lookup_index();
    for receiver_alias in external_receiver_lookup_aliases(receiver_type) {
        for method_alias in external_method_name_lookup_aliases(method_name) {
            if let Some(effect) = arg_count.and_then(|arg_count| {
                index.api_effects.get(&(
                    receiver_alias.clone(),
                    method_alias.clone(),
                    Some(arg_count),
                ))
            }) {
                return Some(effect.clone());
            }
            if let Some(effect) =
                index
                    .api_effects
                    .get(&(receiver_alias.clone(), method_alias.clone(), None))
            {
                return Some(effect.clone());
            }
        }
    }
    None
}

pub fn java_platform_api_effect_for_receiver_with_arity(
    receiver_type: &str,
    method_name: &str,
    arg_count: Option<usize>,
) -> Option<ExternalApiEffectFact> {
    if !is_java_platform_class_receiver(receiver_type) {
        return None;
    }
    let matches_arg_count =
        |expected: usize| arg_count.is_none_or(|arg_count| arg_count == expected);
    let (expected_args, role, return_type, return_nullability, thrown) = match method_name {
        "getSimpleName" | "getName" if matches_arg_count(0) => (
            0,
            ExternalApiRole::Getter,
            Some(JavaType::String),
            NullabilityState::DefinitelyNonNull,
            Vec::new(),
        ),
        "getCanonicalName" if matches_arg_count(0) => (
            0,
            ExternalApiRole::Getter,
            Some(JavaType::String),
            NullabilityState::MaybeNull,
            Vec::new(),
        ),
        "isAssignableFrom" if matches_arg_count(1) => (
            1,
            ExternalApiRole::BooleanGetter,
            Some(JavaType::Primitive(PrimitiveType::Boolean)),
            NullabilityState::DefinitelyNonNull,
            Vec::new(),
        ),
        "forName" if matches_arg_count(1) => (
            1,
            ExternalApiRole::Getter,
            Some(JavaType::Named {
                name: "Class".to_string(),
                args: Vec::new(),
            }),
            NullabilityState::DefinitelyNonNull,
            vec![JavaType::Named {
                name: "ClassNotFoundException".to_string(),
                args: Vec::new(),
            }],
        ),
        _ => return None,
    };
    Some(ExternalApiEffectFact {
        method_name: method_name.to_string(),
        receiver_type: Some(receiver_type.to_string()),
        arg_count: Some(expected_args),
        role,
        receiver_mutability: ReceiverMutabilityFact::ReadOnly,
        return_type,
        return_nullability,
        thrown,
        provenance: FactProvenance::BuiltinExternalModel,
    })
}

fn is_java_platform_class_receiver(receiver_type: &str) -> bool {
    matches!(
        receiver_type.trim(),
        "Class" | "java.lang.Class" | "class" | "java_lang_class"
    )
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
    builtin_external_pack_lookup_index()
        .unsupported_apis
        .get(&(receiver_type.to_string(), method_name.to_string()))
        .cloned()
}

pub fn external_constructor_for(
    type_name: &str,
    arg_types: &[JavaType],
) -> Option<ExternalConstructorFact> {
    builtin_external_pack_lookup_index()
        .constructors
        .get(&(type_name.to_string(), arg_types.len()))
        .cloned()
}

pub fn external_field_for(type_name: &str, field_name: &str) -> Option<ExternalFieldFact> {
    let canonical_type_name = crate::stdlib::canonical_external_receiver_type_key(type_name);
    let snake_type_name = crate::parser::to_snake_case(type_name);
    let canonical_field_name = crate::parser::to_snake_case(field_name);
    let index = builtin_external_pack_lookup_index();
    [
        (type_name, field_name),
        (type_name, canonical_field_name.as_str()),
        (canonical_type_name.as_str(), field_name),
        (canonical_type_name.as_str(), canonical_field_name.as_str()),
        (snake_type_name.as_str(), field_name),
        (snake_type_name.as_str(), canonical_field_name.as_str()),
    ]
    .into_iter()
    .find_map(|(type_key, field_key)| {
        index
            .fields
            .get(&(type_key.to_string(), field_key.to_string()))
            .cloned()
    })
}

pub fn unsupported_external_constructor_for(
    type_name: &str,
    arg_count: usize,
) -> Option<UnsupportedExternalConstructorFact> {
    builtin_external_pack_lookup_index()
        .unsupported_constructors
        .get(&(type_name.to_string(), arg_count))
        .cloned()
}

pub fn unsupported_external_field_for(
    type_name: &str,
    field_name: &str,
) -> Option<UnsupportedExternalFieldFact> {
    builtin_external_pack_lookup_index()
        .unsupported_fields
        .get(&(type_name.to_string(), field_name.to_string()))
        .cloned()
}

pub fn external_crate_api_mapping_for_receiver(
    receiver_type: &str,
    method_name: &str,
    arg_count: Option<usize>,
) -> Option<ExternalCrateApiMappingFact> {
    let index = builtin_external_pack_lookup_index();
    arg_count
        .and_then(|arg_count| {
            index.crate_api_mappings.get(&(
                receiver_type.to_string(),
                method_name.to_string(),
                Some(arg_count),
            ))
        })
        .or_else(|| {
            index.crate_api_mappings.get(&(
                receiver_type.to_string(),
                method_name.to_string(),
                None,
            ))
        })
        .cloned()
}

pub fn external_crate_constructor_mapping_for(
    type_name: &str,
    arg_count: usize,
) -> Option<ExternalCrateConstructorMappingFact> {
    builtin_external_pack_lookup_index()
        .crate_constructor_mappings
        .get(&(type_name.to_string(), arg_count))
        .cloned()
}

pub fn representation_facts_for_type(type_name: &str) -> FactSet {
    let mut facts = builtin_external_pack_lookup_index()
        .representation_facts
        .get(type_name)
        .cloned()
        .unwrap_or_default();
    if let Some(pack) = external_library_pack_for_type(type_name) {
        facts.external_library_packs.push(pack);
    }
    facts
}

pub fn external_library_pack_for_type(type_name: &str) -> Option<ExternalLibraryPackFact> {
    builtin_external_pack_type_matchers()
        .iter()
        .find(|matcher| matcher.matches(type_name))
        .map(|matcher| matcher.pack.clone())
}

#[derive(Debug, Clone)]
struct BuiltinExternalPackTypeMatcher {
    pattern: String,
    kind: BuiltinExternalPackTypeMatcherKind,
    pack: ExternalLibraryPackFact,
}

impl BuiltinExternalPackTypeMatcher {
    fn matches(&self, type_name: &str) -> bool {
        match self.kind {
            BuiltinExternalPackTypeMatcherKind::Exact => type_name == self.pattern,
            BuiltinExternalPackTypeMatcherKind::Prefix => {
                type_name == self.pattern
                    || type_name
                        .strip_prefix(&self.pattern)
                        .is_some_and(|suffix| suffix.starts_with('.'))
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BuiltinExternalPackTypeMatcherKind {
    Exact,
    Prefix,
}

fn builtin_external_pack_type_matchers() -> &'static [BuiltinExternalPackTypeMatcher] {
    static MATCHERS: OnceLock<Vec<BuiltinExternalPackTypeMatcher>> = OnceLock::new();
    MATCHERS.get_or_init(build_builtin_external_pack_type_matchers)
}

fn build_builtin_external_pack_type_matchers() -> Vec<BuiltinExternalPackTypeMatcher> {
    let mut matchers = Vec::new();
    for manifest in builtin_external_pack_manifests() {
        let pack = manifest_pack_fact(&manifest);
        for pattern in manifest.owned_type_patterns {
            push_builtin_pack_matcher(
                &mut matchers,
                pattern,
                BuiltinExternalPackTypeMatcherKind::Prefix,
                &pack,
            );
        }
        for strategy in manifest.object_strategies {
            push_builtin_pack_matcher(
                &mut matchers,
                strategy.type_name,
                BuiltinExternalPackTypeMatcherKind::Exact,
                &pack,
            );
        }
        for capability in manifest.type_capabilities {
            push_builtin_pack_matcher(
                &mut matchers,
                capability.type_name,
                BuiltinExternalPackTypeMatcherKind::Exact,
                &pack,
            );
        }
        for field in manifest.field_nullability {
            push_builtin_pack_matcher(
                &mut matchers,
                field.type_name,
                BuiltinExternalPackTypeMatcherKind::Exact,
                &pack,
            );
        }
        for argument in manifest.argument_effects {
            push_builtin_pack_matcher(
                &mut matchers,
                argument.receiver_type,
                BuiltinExternalPackTypeMatcherKind::Exact,
                &pack,
            );
        }
        for exception in manifest.exceptions {
            push_builtin_pack_matcher(
                &mut matchers,
                exception.type_name,
                BuiltinExternalPackTypeMatcherKind::Exact,
                &pack,
            );
        }
        for api in manifest.api_effects {
            push_builtin_pack_matcher(
                &mut matchers,
                api.receiver_type,
                BuiltinExternalPackTypeMatcherKind::Exact,
                &pack,
            );
        }
        for api in manifest.modeled_apis {
            push_builtin_pack_matcher(
                &mut matchers,
                api.receiver_type,
                BuiltinExternalPackTypeMatcherKind::Exact,
                &pack,
            );
        }
        for field in manifest.modeled_fields {
            push_builtin_pack_matcher(
                &mut matchers,
                field.type_name,
                BuiltinExternalPackTypeMatcherKind::Exact,
                &pack,
            );
        }
        for api in manifest.unsupported_apis {
            push_builtin_pack_matcher(
                &mut matchers,
                api.receiver_type,
                BuiltinExternalPackTypeMatcherKind::Exact,
                &pack,
            );
        }
    }
    matchers
}

fn builtin_external_pack_boundary_audit_matchers() -> &'static [BuiltinExternalPackTypeMatcher] {
    static MATCHERS: OnceLock<Vec<BuiltinExternalPackTypeMatcher>> = OnceLock::new();
    MATCHERS.get_or_init(build_builtin_external_pack_boundary_audit_matchers)
}

fn build_builtin_external_pack_boundary_audit_matchers() -> Vec<BuiltinExternalPackTypeMatcher> {
    let mut matchers = build_builtin_external_pack_type_matchers();
    for manifest in builtin_external_pack_manifests() {
        let pack = manifest_pack_fact(&manifest);
        for constructor in manifest.modeled_constructors {
            push_builtin_pack_matcher(
                &mut matchers,
                constructor.type_name,
                BuiltinExternalPackTypeMatcherKind::Exact,
                &pack,
            );
        }
    }
    matchers
}

fn push_builtin_pack_matcher(
    matchers: &mut Vec<BuiltinExternalPackTypeMatcher>,
    pattern: &str,
    kind: BuiltinExternalPackTypeMatcherKind,
    pack: &ExternalLibraryPackFact,
) {
    if pattern.is_empty() {
        return;
    }
    if matchers
        .iter()
        .any(|matcher| matcher.pattern == pattern && matcher.kind == kind)
    {
        return;
    }
    matchers.push(BuiltinExternalPackTypeMatcher {
        pattern: pattern.to_string(),
        kind,
        pack: pack.clone(),
    });
}

fn manifest_pack_fact(manifest: &ExternalPackManifest) -> ExternalLibraryPackFact {
    ExternalLibraryPackFact {
        pack_name: manifest.pack_name.to_string(),
        package_prefix: manifest
            .owned_type_patterns
            .first()
            .copied()
            .unwrap_or(manifest.pack_name)
            .to_string(),
        mode: manifest.mode,
        coverage: manifest.default_coverage,
        rust_target: manifest.rust_target.map(str::to_string),
        provenance: FactProvenance::ExternalLibraryPack,
    }
}

pub fn hdf5_pack_facts() -> FactSet {
    let mut facts = FactSet::default();
    manifest_by_name(HDF5_PACK).apply_boundary_facts(&mut facts);

    facts
}

pub fn sqlite_pack_facts() -> FactSet {
    let mut facts = FactSet::default();
    manifest_by_name(SQLITE_PACK).apply_boundary_facts(&mut facts);

    facts
}

pub fn java_sql_pack_facts() -> FactSet {
    let mut facts = FactSet::default();
    manifest_by_name(JAVA_SQL_PACK).apply_boundary_facts(&mut facts);

    facts
}

pub fn htsjdk_samtools_util_pack_facts() -> FactSet {
    let mut facts = FactSet::default();
    manifest_by_name(SAMTOOLS_UTIL_PACK).apply_boundary_facts(&mut facts);

    facts
}

pub fn apache_commons_math_pack_facts() -> FactSet {
    let mut facts = FactSet::default();
    manifest_by_name(APACHE_COMMONS_MATH_PACK).apply_boundary_facts(&mut facts);

    facts
}

pub fn apache_commons_math_optim_pack_facts() -> FactSet {
    let mut facts = FactSet::default();
    manifest_by_name(APACHE_COMMONS_MATH_OPTIM_PACK).apply_boundary_facts(&mut facts);

    facts
}

pub fn apache_commons_math_stats_pack_facts() -> FactSet {
    let mut facts = FactSet::default();
    manifest_by_name(APACHE_COMMONS_MATH_STATS_PACK).apply_boundary_facts(&mut facts);

    facts
}

pub fn apache_commons_collections_pack_facts() -> FactSet {
    let mut facts = FactSet::default();
    manifest_by_name(APACHE_COMMONS_COLLECTIONS_PACK).apply_boundary_facts(&mut facts);

    facts
}

pub fn apache_commons_compress_pack_facts() -> FactSet {
    let mut facts = FactSet::default();
    manifest_by_name(APACHE_COMMONS_COMPRESS_PACK).apply_boundary_facts(&mut facts);

    facts
}

pub fn apache_commons_lang_pack_facts() -> FactSet {
    let mut facts = FactSet::default();
    manifest_by_name(APACHE_COMMONS_LANG_PACK).apply_boundary_facts(&mut facts);

    facts
}

pub fn apache_commons_codec_pack_facts() -> FactSet {
    let mut facts = FactSet::default();
    manifest_by_name(APACHE_COMMONS_CODEC_PACK).apply_boundary_facts(&mut facts);

    facts
}

pub fn jama_pack_facts() -> FactSet {
    let mut facts = FactSet::default();
    manifest_by_name(JAMA_PACK).apply_boundary_facts(&mut facts);

    facts
}

pub fn hipparchus_distribution_pack_facts() -> FactSet {
    let mut facts = FactSet::default();
    manifest_by_name(HIPPARCHUS_DISTRIBUTION_PACK).apply_boundary_facts(&mut facts);

    facts
}

pub fn apache_http_pack_facts() -> FactSet {
    let mut facts = FactSet::default();
    manifest_by_name(APACHE_HTTP_PACK).apply_boundary_facts(&mut facts);

    facts
}

pub fn google_bigquery_pack_facts() -> FactSet {
    let mut facts = FactSet::default();
    manifest_by_name(GOOGLE_BIGQUERY_PACK).apply_boundary_facts(&mut facts);

    facts
}

pub fn google_bigquery_storage_pack_facts() -> FactSet {
    let mut facts = FactSet::default();
    manifest_by_name(GOOGLE_BIGQUERY_STORAGE_PACK).apply_boundary_facts(&mut facts);

    facts
}

pub fn google_storage_pack_facts() -> FactSet {
    let mut facts = FactSet::default();
    manifest_by_name(GOOGLE_STORAGE_PACK).apply_boundary_facts(&mut facts);

    facts
}

pub fn google_cloud_nio_pack_facts() -> FactSet {
    let mut facts = FactSet::default();
    manifest_by_name(GOOGLE_CLOUD_NIO_PACK).apply_boundary_facts(&mut facts);

    facts
}

pub fn genomicsdb_pack_facts() -> FactSet {
    let mut facts = FactSet::default();
    manifest_by_name(GENOMICSDB_PACK).apply_boundary_facts(&mut facts);

    facts
}

pub fn picard_programgroups_pack_facts() -> FactSet {
    let mut facts = FactSet::default();
    manifest_by_name(PICARD_PROGRAMGROUPS_PACK).apply_boundary_facts(&mut facts);

    facts
}

pub fn picard_markduplicates_pack_facts() -> FactSet {
    let mut facts = FactSet::default();
    manifest_by_name(PICARD_MARKDUPLICATES_PACK).apply_boundary_facts(&mut facts);

    facts
}

pub fn picard_metrics_pack_facts() -> FactSet {
    let mut facts = FactSet::default();
    manifest_by_name(PICARD_METRICS_PACK).apply_boundary_facts(&mut facts);

    facts
}

pub fn avro_pack_facts() -> FactSet {
    let mut facts = FactSet::default();
    manifest_by_name(AVRO_PACK).apply_boundary_facts(&mut facts);

    facts
}

pub fn barclay_help_pack_facts() -> FactSet {
    let mut facts = FactSet::default();
    manifest_by_name(BARCLAY_HELP_PACK).apply_boundary_facts(&mut facts);

    facts
}

pub fn barclay_argparser_pack_facts() -> FactSet {
    let mut facts = FactSet::default();
    manifest_by_name(BARCLAY_ARGPARSER_PACK).apply_boundary_facts(&mut facts);

    facts
}

pub fn barclay_utils_pack_facts() -> FactSet {
    let mut facts = FactSet::default();
    manifest_by_name(BARCLAY_UTILS_PACK).apply_boundary_facts(&mut facts);

    facts
}

pub fn scala_tuple_pack_facts() -> FactSet {
    let mut facts = FactSet::default();
    manifest_by_name(SCALA_TUPLE_PACK).apply_boundary_facts(&mut facts);

    facts
}

pub fn kryo_pack_facts() -> FactSet {
    let mut facts = FactSet::default();
    manifest_by_name(KRYO_PACK).apply_boundary_facts(&mut facts);

    facts
}

pub fn opencsv_pack_facts() -> FactSet {
    let mut facts = FactSet::default();
    manifest_by_name(OPENCSV_PACK).apply_boundary_facts(&mut facts);

    facts
}

pub fn org_json_pack_facts() -> FactSet {
    let mut facts = FactSet::default();
    manifest_by_name(ORG_JSON_PACK).apply_boundary_facts(&mut facts);

    facts
}

pub fn reflections_pack_facts() -> FactSet {
    let mut facts = FactSet::default();
    manifest_by_name(REFLECTIONS_PACK).apply_boundary_facts(&mut facts);

    facts
}

pub fn hadoop_conf_pack_facts() -> FactSet {
    let mut facts = FactSet::default();
    manifest_by_name(HADOOP_CONF_PACK).apply_boundary_facts(&mut facts);

    facts
}

pub fn apache_commons_config_pack_facts() -> FactSet {
    let mut facts = FactSet::default();
    manifest_by_name(APACHE_COMMONS_CONFIG_PACK).apply_boundary_facts(&mut facts);

    facts
}

pub fn spark_pack_facts() -> FactSet {
    let mut facts = FactSet::default();
    manifest_by_name(SPARK_PACK).apply_boundary_facts(&mut facts);

    facts
}

pub fn bwa_pack_facts() -> FactSet {
    let mut facts = FactSet::default();
    manifest_by_name(BWA_PACK).apply_boundary_facts(&mut facts);

    facts
}

pub fn jgrapht_pack_facts() -> FactSet {
    let mut facts = FactSet::default();
    manifest_by_name(JGRAPHT_PACK).apply_boundary_facts(&mut facts);

    facts
}

pub fn fastutil_pack_facts() -> FactSet {
    let mut facts = FactSet::default();
    manifest_by_name(FASTUTIL_PACK).apply_boundary_facts(&mut facts);

    facts
}

pub fn kryo_serializers_pack_facts() -> FactSet {
    let mut facts = FactSet::default();
    manifest_by_name(KRYO_SERIALIZERS_PACK).apply_boundary_facts(&mut facts);

    facts
}

pub fn disq_write_pack_facts() -> FactSet {
    let mut facts = FactSet::default();
    manifest_by_name(DISQ_WRITE_PACK).apply_boundary_facts(&mut facts);

    facts
}

pub fn intel_gkl_pack_facts() -> FactSet {
    let mut facts = FactSet::default();
    manifest_by_name(INTEL_GKL_PACK).apply_boundary_facts(&mut facts);

    facts
}

pub fn guava_immutable_collect_pack_facts() -> FactSet {
    let mut facts = FactSet::default();
    manifest_by_name(GUAVA_IMMUTABLE_COLLECT_PACK).apply_boundary_facts(&mut facts);

    facts
}

pub fn guava_concurrent_pack_facts() -> FactSet {
    let mut facts = FactSet::default();
    manifest_by_name(GUAVA_CONCURRENT_PACK).apply_boundary_facts(&mut facts);

    facts
}

pub fn broad_http_pack_facts() -> FactSet {
    let mut facts = FactSet::default();
    manifest_by_name(BROAD_HTTP_PACK).apply_boundary_facts(&mut facts);

    facts
}

pub fn broad_hellbender_utils_pack_facts() -> FactSet {
    let mut facts = FactSet::default();
    manifest_by_name(BROAD_HELLBENDER_UTILS_PACK).apply_boundary_facts(&mut facts);

    facts
}

pub fn picard_vcf_pack_facts() -> FactSet {
    let mut facts = FactSet::default();
    manifest_by_name(PICARD_VCF_PACK).apply_boundary_facts(&mut facts);

    facts
}

pub fn jackson_databind_pack_facts() -> FactSet {
    let mut facts = FactSet::default();
    manifest_by_name(JACKSON_DATABIND_PACK).apply_boundary_facts(&mut facts);

    facts
}

pub fn htsjdk_tribble_feature_pack_facts() -> FactSet {
    let mut facts = FactSet::default();
    manifest_by_name(TRIBBLE_FEATURE_PACK).apply_boundary_facts(&mut facts);

    facts
}

pub fn htsjdk_tribble_index_pack_facts() -> FactSet {
    let mut facts = FactSet::default();
    manifest_by_name(TRIBBLE_INDEX_PACK).apply_boundary_facts(&mut facts);

    facts
}

pub fn htsjdk_sam_header_pack_facts() -> FactSet {
    let mut facts = FactSet::default();
    manifest_by_name(SAM_HEADER_PACK).apply_boundary_facts(&mut facts);

    facts
}

pub fn htsjdk_sam_record_pack_facts() -> FactSet {
    let mut facts = FactSet::default();
    manifest_by_name(SAM_RECORD_PACK).apply_boundary_facts(&mut facts);

    facts
}

pub fn htsjdk_sam_reference_pack_facts() -> FactSet {
    let mut facts = FactSet::default();
    manifest_by_name(SAM_REFERENCE_PACK).apply_boundary_facts(&mut facts);

    facts
}

pub fn htsjdk_sam_reader_pack_facts() -> FactSet {
    let mut facts = FactSet::default();
    manifest_by_name(SAM_READER_PACK).apply_boundary_facts(&mut facts);

    facts
}

pub fn htsjdk_sam_writer_pack_facts() -> FactSet {
    let mut facts = FactSet::default();
    manifest_by_name(SAM_WRITER_PACK).apply_boundary_facts(&mut facts);

    facts
}

pub fn htsjdk_sam_bam_pack_facts() -> FactSet {
    let mut facts = FactSet::default();
    manifest_by_name(SAM_BAM_PACK).apply_boundary_facts(&mut facts);

    facts
}

pub fn htsjdk_sam_cram_pack_facts() -> FactSet {
    let mut facts = FactSet::default();
    manifest_by_name(SAM_CRAM_PACK).apply_boundary_facts(&mut facts);

    facts
}

pub fn htsjdk_sam_metrics_pack_facts() -> FactSet {
    let mut facts = FactSet::default();
    manifest_by_name(SAM_METRICS_PACK).apply_boundary_facts(&mut facts);

    facts
}

pub fn htsjdk_nested_helper_pack_facts() -> FactSet {
    let mut facts = FactSet::default();
    manifest_by_name(HTSJDK_NESTED_HELPER_PACK).apply_boundary_facts(&mut facts);

    facts
}

pub fn htsjdk_vcf_pack_facts() -> FactSet {
    let mut facts = FactSet::default();
    manifest_by_name(VCF_PACK).apply_boundary_facts(&mut facts);

    facts
}

pub fn htsjdk_bcf_pack_facts() -> FactSet {
    let mut facts = FactSet::default();
    manifest_by_name(BCF_PACK).apply_boundary_facts(&mut facts);

    facts
}

pub fn htsjdk_variantcontext_pack_facts() -> FactSet {
    let mut facts = FactSet::default();
    manifest_by_name(VARIANTCONTEXT_PACK).apply_boundary_facts(&mut facts);
    facts
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
