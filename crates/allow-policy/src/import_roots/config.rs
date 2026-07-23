use allow_core::{CargoAllowError, CargoAllowResult};
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ImportNodeRole {
    Owned,
    Imported,
    Legacy,
    Generated,
}

impl ImportNodeRole {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Owned => "owned",
            Self::Imported => "imported",
            Self::Legacy => "legacy",
            Self::Generated => "generated",
        }
    }

    pub fn parse(value: &str) -> CargoAllowResult<Self> {
        match value.trim() {
            "owned" => Ok(Self::Owned),
            "imported" => Ok(Self::Imported),
            "legacy" => Ok(Self::Legacy),
            "generated" => Ok(Self::Generated),
            other => Err(CargoAllowError::new(format!(
                "unsupported import root role `{other}`"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportConfidence {
    High,
    Medium,
    Low,
}

impl ImportConfidence {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::High => "high",
            Self::Medium => "medium",
            Self::Low => "low",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportProvenance {
    Configured,
    Discovered,
    LegacyFallback,
    GeneratedMarker,
}

impl ImportProvenance {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Configured => "configured",
            Self::Discovered => "discovered",
            Self::LegacyFallback => "legacy_fallback",
            Self::GeneratedMarker => "generated_marker",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportEdgeKind {
    Contains,
    References,
    Promotes,
}

impl ImportEdgeKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Contains => "contains",
            Self::References => "references",
            Self::Promotes => "promotes",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ImportRootEntry {
    pub id: String,
    pub path: String,
    pub ecosystem: String,
    pub role: ImportNodeRole,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct ImportRootsConfig {
    pub owned: Option<String>,
    #[serde(default)]
    pub entries: Vec<ImportRootEntry>,
}

#[derive(Debug, Default, Deserialize)]
struct ImportRootsConfigToml {
    owned: Option<String>,
    #[serde(default)]
    entries: Vec<ImportRootEntryToml>,
}

#[derive(Debug, Deserialize)]
struct ImportRootEntryToml {
    id: String,
    path: String,
    ecosystem: String,
    role: String,
}

pub fn parse_import_roots_config(input: &str) -> CargoAllowResult<ImportRootsConfig> {
    parse_import_roots_config_at(None, input)
}

pub fn parse_import_roots_config_at(
    path: Option<&Path>,
    input: &str,
) -> CargoAllowResult<ImportRootsConfig> {
    let parsed = toml::from_str::<ImportRootsConfigToml>(input).map_err(|err| {
        CargoAllowError::with_kind(
            allow_core::CargoAllowErrorKind::InvalidConfig,
            format!("failed to parse import roots config TOML: {err}"),
        )
        .with_toml_span(path, input, err.span())
    })?;
    let mut entries = Vec::with_capacity(parsed.entries.len());
    for entry in parsed.entries {
        entries.push(ImportRootEntry {
            id: entry.id,
            path: entry.path,
            ecosystem: entry.ecosystem,
            role: ImportNodeRole::parse(&entry.role)?,
        });
    }
    Ok(ImportRootsConfig {
        owned: parsed.owned,
        entries,
    })
}

pub const DEFAULT_OWNED_IMPORT_ROOT: &str = ".allow/imports";

pub fn default_import_roots_config() -> ImportRootsConfig {
    ImportRootsConfig {
        owned: Some(DEFAULT_OWNED_IMPORT_ROOT.to_string()),
        entries: vec![
            ImportRootEntry {
                id: "owned-imports".to_string(),
                path: DEFAULT_OWNED_IMPORT_ROOT.to_string(),
                ecosystem: "cargo-allow".to_string(),
                role: ImportNodeRole::Owned,
            },
            ImportRootEntry {
                id: "kiro".to_string(),
                path: ".kiro".to_string(),
                ecosystem: "kiro".to_string(),
                role: ImportNodeRole::Imported,
            },
            ImportRootEntry {
                id: "specify".to_string(),
                path: ".specify".to_string(),
                ecosystem: "spec-kit".to_string(),
                role: ImportNodeRole::Imported,
            },
            ImportRootEntry {
                id: "generic-spec".to_string(),
                path: ".spec".to_string(),
                ecosystem: "generic-spec".to_string(),
                role: ImportNodeRole::Imported,
            },
            ImportRootEntry {
                id: "generic-rails".to_string(),
                path: ".rails".to_string(),
                ecosystem: "generic-spec".to_string(),
                role: ImportNodeRole::Imported,
            },
            ImportRootEntry {
                id: "legacy-goals".to_string(),
                path: ".codex/goals".to_string(),
                ecosystem: "codex".to_string(),
                role: ImportNodeRole::Legacy,
            },
            ImportRootEntry {
                id: "xtask".to_string(),
                path: "xtask".to_string(),
                ecosystem: "xtask".to_string(),
                role: ImportNodeRole::Imported,
            },
        ],
    }
}

impl From<crate::spec_system::ImportNodeRole> for ImportNodeRole {
    fn from(role: crate::spec_system::ImportNodeRole) -> Self {
        match role {
            crate::spec_system::ImportNodeRole::Owned => Self::Owned,
            crate::spec_system::ImportNodeRole::Imported => Self::Imported,
            crate::spec_system::ImportNodeRole::Legacy => Self::Legacy,
            crate::spec_system::ImportNodeRole::Generated => Self::Generated,
        }
    }
}

impl From<&crate::spec_system::ImportRootEntry> for ImportRootEntry {
    fn from(entry: &crate::spec_system::ImportRootEntry) -> Self {
        Self {
            id: entry.id.clone(),
            path: entry.path.clone(),
            ecosystem: entry.ecosystem.clone(),
            role: entry.role.into(),
        }
    }
}

impl From<&crate::spec_system::ImportRootsConfig> for ImportRootsConfig {
    fn from(config: &crate::spec_system::ImportRootsConfig) -> Self {
        Self {
            owned: config.owned.clone(),
            entries: config.entries.iter().map(ImportRootEntry::from).collect(),
        }
    }
}
