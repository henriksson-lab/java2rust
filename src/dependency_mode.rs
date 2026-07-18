use std::fmt;
use std::path::{Path, PathBuf};

use crate::facts::{ExternalLibraryMode, FactSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DependencyMode {
    PrimarySource,
    CoTranslateSource,
    SourceSummary,
    ExternalFactModel,
    RustCrateMapping,
    FfiBoundary,
    ConservativeBoundary,
}

impl DependencyMode {
    pub fn participates_in_resolution(self) -> bool {
        matches!(self, Self::PrimarySource | Self::CoTranslateSource)
    }

    pub fn participates_in_source_index(self) -> bool {
        matches!(
            self,
            Self::PrimarySource | Self::CoTranslateSource | Self::SourceSummary
        )
    }

    pub fn emits_rust(self) -> bool {
        matches!(self, Self::PrimarySource | Self::CoTranslateSource)
    }

    pub fn is_fact_provider(self) -> bool {
        matches!(
            self,
            Self::SourceSummary
                | Self::ExternalFactModel
                | Self::RustCrateMapping
                | Self::FfiBoundary
                | Self::ConservativeBoundary
        )
    }

    pub fn from_external_library_mode(mode: ExternalLibraryMode) -> Self {
        match mode {
            ExternalLibraryMode::ExternalFactModel => Self::ExternalFactModel,
            ExternalLibraryMode::RustCrateMapping => Self::RustCrateMapping,
            ExternalLibraryMode::FfiBoundary => Self::FfiBoundary,
            ExternalLibraryMode::ConservativeBoundary => Self::ConservativeBoundary,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DependencySourceRoot {
    pub path: PathBuf,
    pub mode: DependencyMode,
}

impl DependencySourceRoot {
    pub fn new(path: impl Into<PathBuf>, mode: DependencyMode) -> Self {
        Self {
            path: path.into(),
            mode,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DependencyFactProvider {
    pub name: String,
    pub mode: DependencyMode,
    pub facts: FactSet,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DependencyFactProviderError {
    MissingExternalPackBoundary {
        provider_name: String,
    },
    MixedExternalPackModes {
        provider_name: String,
        modes: Vec<DependencyMode>,
    },
}

impl fmt::Display for DependencyFactProviderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DependencyFactProviderError::MissingExternalPackBoundary { provider_name } => {
                write!(
                    f,
                    "{provider_name} has no external-pack boundary rows; provider mode cannot be inferred"
                )
            }
            DependencyFactProviderError::MixedExternalPackModes {
                provider_name,
                modes,
            } => {
                write!(f, "{provider_name} declares multiple external-pack modes: ")?;
                for (index, mode) in modes.iter().enumerate() {
                    if index > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{mode}")?;
                }
                write!(f, "; split mixed-mode manifests into separate providers")
            }
        }
    }
}

impl std::error::Error for DependencyFactProviderError {}

impl fmt::Display for DependencyMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            DependencyMode::PrimarySource => "primary-source",
            DependencyMode::CoTranslateSource => "co-translate-source",
            DependencyMode::SourceSummary => "source-summary",
            DependencyMode::ExternalFactModel => "external-fact-model",
            DependencyMode::RustCrateMapping => "rust-crate-mapping",
            DependencyMode::FfiBoundary => "ffi-boundary",
            DependencyMode::ConservativeBoundary => "conservative-boundary",
        })
    }
}

impl DependencyFactProvider {
    pub fn new(name: impl Into<String>, mode: DependencyMode, facts: FactSet) -> Self {
        assert!(
            mode.is_fact_provider(),
            "fact provider mode must be source-summary, external, crate, FFI, or conservative"
        );
        Self {
            name: name.into(),
            mode,
            facts,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DependencyModeConfig {
    pub source_roots: Vec<DependencySourceRoot>,
    pub fact_providers: Vec<DependencyFactProvider>,
    pub enable_builtin_external_packs: bool,
}

impl Default for DependencyModeConfig {
    fn default() -> Self {
        Self {
            source_roots: Vec::new(),
            fact_providers: Vec::new(),
            enable_builtin_external_packs: true,
        }
    }
}

impl DependencyModeConfig {
    pub fn for_primary_source_root(path: impl Into<PathBuf>) -> Self {
        Self {
            source_roots: vec![DependencySourceRoot::new(
                path,
                DependencyMode::PrimarySource,
            )],
            fact_providers: Vec::new(),
            enable_builtin_external_packs: true,
        }
    }

    pub fn without_builtin_external_packs(mut self) -> Self {
        self.enable_builtin_external_packs = false;
        self
    }

    pub fn set_builtin_external_packs_enabled(&mut self, enabled: bool) {
        self.enable_builtin_external_packs = enabled;
    }

    pub fn add_source_root(&mut self, path: impl Into<PathBuf>, mode: DependencyMode) {
        self.source_roots
            .push(DependencySourceRoot::new(path, mode));
    }

    pub fn add_fact_provider(
        &mut self,
        name: impl Into<String>,
        mode: DependencyMode,
        facts: FactSet,
    ) {
        self.fact_providers
            .push(DependencyFactProvider::new(name, mode, facts));
    }

    pub fn add_external_pack_provider(
        &mut self,
        name: impl Into<String>,
        facts: FactSet,
    ) -> Result<(), DependencyFactProviderError> {
        let name = name.into();
        let mode = infer_external_pack_provider_mode(&name, &facts)?;
        self.add_fact_provider(name, mode, facts);
        Ok(())
    }

    pub fn source_roots_for_resolution(&self) -> impl Iterator<Item = &DependencySourceRoot> {
        self.source_roots
            .iter()
            .filter(|root| root.mode.participates_in_resolution())
    }

    pub fn source_roots_for_source_index(&self) -> impl Iterator<Item = &DependencySourceRoot> {
        self.source_roots
            .iter()
            .filter(|root| root.mode.participates_in_source_index())
    }

    pub fn primary_source_roots(&self) -> impl Iterator<Item = &DependencySourceRoot> {
        self.source_roots
            .iter()
            .filter(|root| root.mode == DependencyMode::PrimarySource)
    }

    pub fn fact_provider_facts(&self) -> impl Iterator<Item = &FactSet> {
        self.fact_providers.iter().map(|provider| &provider.facts)
    }

    pub fn mode_for_source_path(&self, path: &Path) -> Option<DependencyMode> {
        self.source_roots
            .iter()
            .filter(|root| path.starts_with(&root.path))
            .max_by_key(|root| root.path.components().count())
            .map(|root| root.mode)
    }
}

fn infer_external_pack_provider_mode(
    provider_name: &str,
    facts: &FactSet,
) -> Result<DependencyMode, DependencyFactProviderError> {
    let mut modes = Vec::new();
    for pack in &facts.external_library_packs {
        let mode = DependencyMode::from_external_library_mode(pack.mode);
        if !modes.contains(&mode) {
            modes.push(mode);
        }
    }
    match modes.len() {
        0 => Err(DependencyFactProviderError::MissingExternalPackBoundary {
            provider_name: provider_name.to_string(),
        }),
        1 => Ok(modes[0]),
        _ => Err(DependencyFactProviderError::MixedExternalPackModes {
            provider_name: provider_name.to_string(),
            modes,
        }),
    }
}
