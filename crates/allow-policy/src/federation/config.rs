use allow_core::{CargoAllowError, CargoAllowResult, LaneEnforcementMode};
use serde::Deserialize;
use std::path::Path;
use std::str::FromStr;

pub use super::drain::DrainWindow;
use super::drain::{DrainWindowToml, parse_drain_windows};

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
        let trimmed = value.trim();
        let normalized = trimmed.to_ascii_lowercase();
        match normalized.as_str() {
            "canonical" => Ok(Self::Canonical),
            "mirror" => Ok(Self::Mirror),
            "imported" => Ok(Self::Imported),
            _ => Err(CargoAllowError::new(format!(
                "unsupported ledger role `{trimmed}`; valid values: canonical, mirror, imported"
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
    pub drain_windows: Vec<DrainWindow>,
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
    UnknownDrainMirrorLedger,
    DrainWindowMissingField,
    DrainWindowInvalidDate,
    DrainWindowNotMirror,
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
            Self::UnknownDrainMirrorLedger => "unknown_drain_mirror_ledger",
            Self::DrainWindowMissingField => "drain_window_missing_field",
            Self::DrainWindowInvalidDate => "drain_window_invalid_date",
            Self::DrainWindowNotMirror => "drain_window_not_mirror",
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
#[serde(deny_unknown_fields)]
struct FederationConfigToml {
    schema_version: Option<String>,
    #[serde(default)]
    ledgers: Vec<LedgerEntryToml>,
    #[serde(default)]
    drain_windows: Vec<DrainWindowToml>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
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
        // `priority` is required: a missing value previously defaulted to the
        // array index, so reordering the `[[ledgers]]` array silently flipped
        // which ledger won precedence. Surface every missing priority as a
        // single aggregated error naming the ledger(s) so the fix is actionable.
        // (#2044)
        let missing_priority: Vec<&str> = self
            .ledgers
            .iter()
            .filter(|entry| entry.priority.is_none())
            .map(|entry| entry.id.as_str())
            .collect();
        if !missing_priority.is_empty() {
            let listed = missing_priority
                .iter()
                .map(|id| format!("  - {id}"))
                .collect::<Vec<_>>()
                .join("\n");
            return Err(CargoAllowError::new(format!(
                "federation ledger(s) missing required explicit `priority`; reordering the \
                 [[ledgers]] array would silently change precedence:\n{listed}\n\
                 Set `priority = <u32>` on every [[ledgers]] entry (lower priority wins)."
            )));
        }
        let ledgers = self
            .ledgers
            .into_iter()
            .enumerate()
            .map(|(index, entry)| entry.into_ledger_entry(index))
            .collect::<CargoAllowResult<Vec<_>>>()?;
        Ok(FederationConfig {
            schema_version: self.schema_version.unwrap_or_else(|| "1.0".to_string()),
            ledgers,
            drain_windows: parse_drain_windows(&self.drain_windows),
        })
    }
}

impl LedgerEntryToml {
    fn into_ledger_entry(self, index: usize) -> CargoAllowResult<LedgerEntry> {
        let role = LedgerRole::parse(&self.role)?;
        let mode = match self.mode.as_deref() {
            Some(value) => LaneEnforcementMode::from_str(value)
                .map_err(|err| CargoAllowError::new(format!("ledgers[{}].mode: {err}", self.id)))?,
            None => default_mode_for_role(role),
        };
        let path = normalize_repo_relative_path(&self.id, &self.path)?;
        Ok(LedgerEntry {
            id: self.id,
            path,
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
    parse_federation_config_at(None, input)
}

pub fn parse_federation_config_at(
    path: Option<&Path>,
    input: &str,
) -> CargoAllowResult<FederationConfig> {
    let parsed = toml::from_str::<FederationConfigToml>(input).map_err(|err| {
        CargoAllowError::with_kind(
            allow_core::CargoAllowErrorKind::InvalidConfig,
            format!("failed to parse federation config TOML: {err}"),
        )
        .with_toml_span(path, input, err.span())
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

/// Normalize a federation ledger `path` to a repo-relative, forward-slash form
/// and reject anything that could escape the source-tree root when later joined
/// via `root.join(path)` (#2011):
/// - absolute paths (`/etc/passwd`, `C:\...`) — `Path::join` with an absolute
///   arg silently *replaces* the base, pointing federation outside the repo.
/// - home-relative paths (`~/...`) — treated as a literal relative segment,
///   never expanded, so they silently never resolve correctly.
/// - parent-directory traversal (`..`) — can escape the root after join.
fn normalize_repo_relative_path(id: &str, path: &str) -> CargoAllowResult<String> {
    let normalized = path.replace('\\', "/");
    let trimmed = normalized.trim();
    if trimmed.is_empty() {
        return Err(CargoAllowError::new(format!(
            "federation ledger `{id}` has an empty path; expected a path relative to the repository root"
        )));
    }
    if trimmed.starts_with('/') || trimmed.contains(':') {
        return Err(CargoAllowError::new(format!(
            "federation ledger `{id}` path `{path}` must be relative to the repository root; \
             absolute paths are rejected (root.join would escape the repo)",
        )));
    }
    if trimmed.starts_with('~') {
        return Err(CargoAllowError::new(format!(
            "federation ledger `{id}` path `{path}` must be relative to the repository root; \
             `~` home paths are rejected (they are never expanded)",
        )));
    }
    if trimmed.split('/').any(|segment| segment == "..") {
        return Err(CargoAllowError::new(format!(
            "federation ledger `{id}` path `{path}` must not contain parent directory (`..`) \
             segments; they can escape the repository root",
        )));
    }
    Ok(normalized)
}
