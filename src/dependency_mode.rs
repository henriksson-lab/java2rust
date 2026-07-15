use std::path::{Path, PathBuf};

use crate::facts::FactSet;

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

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DependencyModeConfig {
    pub source_roots: Vec<DependencySourceRoot>,
    pub fact_providers: Vec<DependencyFactProvider>,
}

impl DependencyModeConfig {
    pub fn for_primary_source_root(path: impl Into<PathBuf>) -> Self {
        Self {
            source_roots: vec![DependencySourceRoot::new(
                path,
                DependencyMode::PrimarySource,
            )],
            fact_providers: Vec::new(),
        }
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
