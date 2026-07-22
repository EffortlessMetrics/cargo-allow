use allow_core::{CargoAllowError, CargoAllowResult};
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtractionShimKind {
    PublicReExport,
    PrivateReExport,
    TypeAlias,
    TraitFacade,
    FunctionFacade,
    ModuleFacade,
    FeatureGate,
    CargoDependencyEdge,
    DevDependencyEdge,
    BuildDependencyEdge,
    SchemaCompatibilityAdapter,
    FixturePathAdapter,
    ProcessProtocolAdapter,
    HistoricalReader,
    TestOnlyAdapter,
}

impl ExtractionShimKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::PublicReExport => "PublicReExport",
            Self::PrivateReExport => "PrivateReExport",
            Self::TypeAlias => "TypeAlias",
            Self::TraitFacade => "TraitFacade",
            Self::FunctionFacade => "FunctionFacade",
            Self::ModuleFacade => "ModuleFacade",
            Self::FeatureGate => "FeatureGate",
            Self::CargoDependencyEdge => "CargoDependencyEdge",
            Self::DevDependencyEdge => "DevDependencyEdge",
            Self::BuildDependencyEdge => "BuildDependencyEdge",
            Self::SchemaCompatibilityAdapter => "SchemaCompatibilityAdapter",
            Self::FixturePathAdapter => "FixturePathAdapter",
            Self::ProcessProtocolAdapter => "ProcessProtocolAdapter",
            Self::HistoricalReader => "HistoricalReader",
            Self::TestOnlyAdapter => "TestOnlyAdapter",
        }
    }

    pub fn parse(value: &str) -> CargoAllowResult<Self> {
        match value.trim() {
            "PublicReExport" => Ok(Self::PublicReExport),
            "PrivateReExport" => Ok(Self::PrivateReExport),
            "TypeAlias" => Ok(Self::TypeAlias),
            "TraitFacade" => Ok(Self::TraitFacade),
            "FunctionFacade" => Ok(Self::FunctionFacade),
            "ModuleFacade" => Ok(Self::ModuleFacade),
            "FeatureGate" => Ok(Self::FeatureGate),
            "CargoDependencyEdge" => Ok(Self::CargoDependencyEdge),
            "DevDependencyEdge" => Ok(Self::DevDependencyEdge),
            "BuildDependencyEdge" => Ok(Self::BuildDependencyEdge),
            "SchemaCompatibilityAdapter" => Ok(Self::SchemaCompatibilityAdapter),
            "FixturePathAdapter" => Ok(Self::FixturePathAdapter),
            "ProcessProtocolAdapter" => Ok(Self::ProcessProtocolAdapter),
            "HistoricalReader" => Ok(Self::HistoricalReader),
            "TestOnlyAdapter" => Ok(Self::TestOnlyAdapter),
            other => Err(CargoAllowError::new(format!(
                "unsupported extraction shim kind `{other}`"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShimPosture {
    Public,
    Private,
    TestOnly,
}

impl ShimPosture {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Public => "public",
            Self::Private => "private",
            Self::TestOnly => "test_only",
        }
    }

    pub fn parse(value: &str) -> CargoAllowResult<Self> {
        match value.trim() {
            "public" => Ok(Self::Public),
            "private" => Ok(Self::Private),
            "test_only" => Ok(Self::TestOnly),
            other => Err(CargoAllowError::new(format!(
                "unsupported shim posture `{other}`"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShimStatus {
    Planned,
    Active,
    Removed,
}

impl ShimStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Planned => "planned",
            Self::Active => "active",
            Self::Removed => "removed",
        }
    }

    pub fn parse(value: &str) -> CargoAllowResult<Self> {
        match value.trim() {
            "planned" => Ok(Self::Planned),
            "active" => Ok(Self::Active),
            "removed" => Ok(Self::Removed),
            other => Err(CargoAllowError::new(format!(
                "unsupported shim status `{other}`"
            ))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractionShim {
    pub id: String,
    pub old_identity: String,
    pub new_identity: String,
    pub kind: ExtractionShimKind,
    pub posture: ShimPosture,
    pub status: ShimStatus,
    pub move_ledger_entry: String,
    pub controlling_issue: u32,
    pub latest_allowed_stage: u32,
    pub removal_condition: String,
    pub parity_case: Option<String>,
    pub claim_boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractionShimRegistry {
    pub schema_version: String,
    pub registry_id: String,
    pub controlling_issue: u32,
    pub linked_move_ledger: String,
    pub shim: Vec<ExtractionShim>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExtractionShimRegistryToml {
    schema_version: Option<String>,
    registry_id: Option<String>,
    controlling_issue: Option<u32>,
    linked_move_ledger: Option<String>,
    #[serde(default)]
    shim: Vec<ExtractionShimToml>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExtractionShimToml {
    id: String,
    old_identity: String,
    new_identity: String,
    kind: String,
    posture: String,
    status: String,
    move_ledger_entry: String,
    controlling_issue: u32,
    latest_allowed_stage: u32,
    removal_condition: String,
    parity_case: Option<String>,
    claim_boundary: String,
}

impl ExtractionShimRegistryToml {
    fn into_registry(self) -> CargoAllowResult<ExtractionShimRegistry> {
        let registry_id = self
            .registry_id
            .ok_or_else(|| CargoAllowError::new("shim registry missing registry_id"))?;
        let controlling_issue = self
            .controlling_issue
            .ok_or_else(|| CargoAllowError::new("shim registry missing controlling_issue"))?;
        let linked_move_ledger = self
            .linked_move_ledger
            .ok_or_else(|| CargoAllowError::new("shim registry missing linked_move_ledger"))?;

        let mut shim = Vec::with_capacity(self.shim.len());
        for entry in self.shim {
            shim.push(ExtractionShim {
                id: entry.id,
                old_identity: entry.old_identity,
                new_identity: entry.new_identity,
                kind: ExtractionShimKind::parse(&entry.kind)?,
                posture: ShimPosture::parse(&entry.posture)?,
                status: ShimStatus::parse(&entry.status)?,
                move_ledger_entry: entry.move_ledger_entry,
                controlling_issue: entry.controlling_issue,
                latest_allowed_stage: entry.latest_allowed_stage,
                removal_condition: entry.removal_condition,
                parity_case: entry.parity_case,
                claim_boundary: entry.claim_boundary,
            });
        }

        Ok(ExtractionShimRegistry {
            schema_version: self.schema_version.unwrap_or_else(|| "1.0".to_string()),
            registry_id,
            controlling_issue,
            linked_move_ledger,
            shim,
        })
    }
}

pub fn parse_extraction_shim_registry(input: &str) -> CargoAllowResult<ExtractionShimRegistry> {
    parse_extraction_shim_registry_at(None, input)
}

pub fn parse_extraction_shim_registry_at(
    path: Option<&Path>,
    input: &str,
) -> CargoAllowResult<ExtractionShimRegistry> {
    let parsed = toml::from_str::<ExtractionShimRegistryToml>(input).map_err(|err| {
        CargoAllowError::with_kind(
            allow_core::CargoAllowErrorKind::InvalidConfig,
            format!("failed to parse extraction shim registry TOML: {err}"),
        )
        .with_toml_span(path, input, err.span())
    })?;
    parsed.into_registry()
}
