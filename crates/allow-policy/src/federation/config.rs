use allow_core::{CargoAllowError, CargoAllowResult, LaneEnforcementMode};
use serde::Deserialize;
use std::str::FromStr;

pub const NATIVE_POLICY_DIALECT: &str = "cargo-allow";
pub const DOC_ARTIFACTS_DIALECT: &str = "cargo-allow-doc-artifacts";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LedgerRole {
    Canonical,
    Mirror,
    Imported,
}

impl LedgerRole {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Canonical => "canonical",
            Self::Mirror => "mirror",
            Self::Imported => "imported",
        }
    }

    pub fn parse(value: &str) -> CargoAllowResult<Self> {
        match value.trim() {
            "canonical" => Ok(Self::Canonical),
            "mirror" => Ok(Self::Mirror),
            "imported" => Ok(Self::Imported),
            other => Err(CargoAllowError::new(format!(
                "unsupported ledger role `{other}`"
            ))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LedgerEntry {
    pub id: String,
    pub path: String,
    pub dialect: String,
    pub role: LedgerRole,
    pub lanes: Vec<String>,
    pub mode: LaneEnforcementMode,
    pub priority: u32,
    pub mirrors: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FederationConfig {
    pub schema_version: String,
    pub ledgers: Vec<LedgerEntry>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FederationDiagnosticKind {
    DuplicateId,
    DuplicatePath,
    DuplicateCanonicalLane,
    DialectConflict,
    DialectSkipped,
    MirrorMissingTarget,
    UnknownMirrorTarget,
}

impl FederationDiagnosticKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::DuplicateId => "duplicate_id",
            Self::DuplicatePath => "duplicate_path",
            Self::DuplicateCanonicalLane => "duplicate_canonical_lane",
            Self::DialectConflict => "dialect_conflict",
            Self::DialectSkipped => "dialect_skipped",
            Self::MirrorMissingTarget => "mirror_missing_target",
            Self::UnknownMirrorTarget => "unknown_mirror_target",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FederationDiagnostic {
    pub kind: FederationDiagnosticKind,
    pub message: String,
    pub ledger_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedFederationConfig {
    pub config: FederationConfig,
    pub diagnostics: Vec<FederationDiagnostic>,
    pub valid: bool,
}

#[derive(Debug, Default, Deserialize)]
struct FederationConfigToml {
    schema_version: Option<String>,
    #[serde(default)]
    ledgers: Vec<LedgerEntryToml>,
}

#[derive(Debug, Deserialize)]
struct LedgerEntryToml {
    id: String,
    path: String,
    dialect: String,
    role: String,
    #[serde(default)]
    lanes: Vec<String>,
    mode: Option<String>,
    priority: Option<u32>,
    mirrors: Option<String>,
}

impl FederationConfigToml {
    fn into_config(self) -> CargoAllowResult<FederationConfig> {
        let ledgers = self
            .ledgers
            .into_iter()
            .enumerate()
            .map(|(index, entry)| entry.into_ledger_entry(index))
            .collect::<CargoAllowResult<Vec<_>>>()?;
        Ok(FederationConfig {
            schema_version: self
                .schema_version
                .unwrap_or_else(|| "1.0".to_string()),
            ledgers,
        })
    }
}

impl LedgerEntryToml {
    fn into_ledger_entry(self, index: usize) -> CargoAllowResult<LedgerEntry> {
        let role = LedgerRole::parse(&self.role)?;
        let mode = match self.mode.as_deref() {
            Some(value) => LaneEnforcementMode::from_str(value).map_err(|err| {
                CargoAllowError::new(format!("ledgers[{}].mode: {err}", self.id))
            })?,
            None => default_mode_for_role(role),
        };
        Ok(LedgerEntry {
            id: self.id,
            path: normalize_repo_relative_path(&self.path),
            dialect: self.dialect,
            role,
            lanes: self.lanes,
            mode,
            priority: self.priority.unwrap_or_else(|| default_priority(index)),
            mirrors: self.mirrors,
        })
    }
}

pub fn parse_federation_config(input: &str) -> CargoAllowResult<FederationConfig> {
    let parsed = toml::from_str::<FederationConfigToml>(input).map_err(|err| {
        CargoAllowError::new(format!("failed to parse federation config TOML: {err}"))
    })?;
    parsed.into_config()
}

pub fn is_native_dialect(dialect: &str) -> bool {
    matches!(dialect, NATIVE_POLICY_DIALECT | DOC_ARTIFACTS_DIALECT)
}

fn default_mode_for_role(role: LedgerRole) -> LaneEnforcementMode {
    match role {
        LedgerRole::Canonical => LaneEnforcementMode::Blocking,
        LedgerRole::Mirror | LedgerRole::Imported => LaneEnforcementMode::Advisory,
    }
}

fn default_priority(index: usize) -> u32 {
    u32::try_from((index + 1) * 10).unwrap_or(u32::MAX)
}

fn normalize_repo_relative_path(path: &str) -> String {
    path.replace('\\', "/")
}
