use crate::{
    CargoAllowDiagnostic, CargoAllowError, CargoAllowErrorKind, CargoAllowResult, FindingKind,
    normalize_path, source_tree_path::normalize_source_tree_scope,
};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::str::FromStr;

use crate::lane_posture::{LaneConfig, LaneEnforcementMode, lane_enforcement_mode_for_kind};

/// The supported workspace default check modes, mirroring the CLI `--mode`
/// flag and the `[workspace] default_mode` policy field. A typo'd or
/// unsupported value (e.g. `"no_new"`) is rejected at validation time rather
/// than silently treated as a string that never matches a real mode.
///
/// Mirrors [`LaneEnforcementMode`]: a typed, `FromStr`-parseable enum so the
/// valid set is codified once in core instead of duplicated by every consumer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum WorkspaceMode {
    #[default]
    NoNew,
    Audit,
    Strict,
    Release,
}

impl WorkspaceMode {
    pub const ALL: &[Self] = &[Self::Audit, Self::NoNew, Self::Strict, Self::Release];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Audit => "audit",
            Self::NoNew => "no-new",
            Self::Strict => "strict",
            Self::Release => "release",
        }
    }
}

impl FromStr for WorkspaceMode {
    type Err = CargoAllowError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim() {
            "audit" => Ok(Self::Audit),
            "no-new" => Ok(Self::NoNew),
            "strict" => Ok(Self::Strict),
            "release" => Ok(Self::Release),
            other => Err(CargoAllowError::with_kind(
                CargoAllowErrorKind::InvalidPolicy,
                format!("unsupported workspace default_mode `{other}`"),
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LastSeen {
    pub line: u32,
    pub column: u32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Selector {
    pub ast_kind: Option<String>,
    pub container: Option<String>,
    pub callee: Option<String>,
    pub macro_name: Option<String>,
    pub lint: Option<String>,
    pub symbol: Option<String>,
    pub receiver_fingerprint: Option<String>,
    pub target_fingerprint: Option<String>,
    pub normalized_snippet_hash: Option<String>,
    pub line_hint: Option<u32>,
    pub glob: Option<String>,
}

impl Selector {
    pub fn has_structural_identity(&self) -> bool {
        [
            self.ast_kind.as_deref(),
            self.container.as_deref(),
            self.callee.as_deref(),
            self.macro_name.as_deref(),
            self.lint.as_deref(),
            self.symbol.as_deref(),
            self.receiver_fingerprint.as_deref(),
            self.target_fingerprint.as_deref(),
            self.normalized_snippet_hash.as_deref(),
        ]
        .into_iter()
        .any(|value| value.is_some_and(|text| !text.trim().is_empty()))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lifecycle {
    pub created: Option<String>,
    pub review_after: Option<String>,
    pub expires: Option<String>,
}

impl Lifecycle {
    pub fn empty() -> Self {
        Self {
            created: None,
            review_after: None,
            expires: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AllowEntry {
    pub id: String,
    pub kind: FindingKind,
    pub family: Option<String>,
    pub path: Option<PathBuf>,
    pub glob: Option<String>,
    pub owner: String,
    pub classification: String,
    pub reason: String,
    pub evidence: Vec<String>,
    pub links: Vec<String>,
    pub occurrence_limit: Option<u32>,
    pub lifecycle: Lifecycle,
    pub selector: Selector,
    pub last_seen: Option<LastSeen>,
}

impl AllowEntry {
    pub fn path_or_glob(&self) -> String {
        if let Some(path) = &self.path {
            normalize_path(path)
        } else if let Some(glob) = &self.glob {
            normalize_source_tree_scope(glob)
        } else if let Some(glob) = &self.selector.glob {
            normalize_source_tree_scope(glob)
        } else {
            String::new()
        }
    }

    /// Return the path or glob as an Option, avoiding the empty-string sentinel
    /// that `path_or_glob()` returns when no path/glob is set (#1923).
    pub fn path_or_glob_opt(&self) -> Option<String> {
        if let Some(path) = &self.path {
            return Some(normalize_path(path));
        }
        if let Some(glob) = &self.glob {
            return Some(normalize_source_tree_scope(glob));
        }
        self.selector.glob.as_deref().map(normalize_source_tree_scope)
    }
}

/// Per-ledger requirements toggles. Defaults are intentionally strict on
/// ownership/accountability (`owner`/`reason`/`classification`/lifecycle
/// required) and on unsafe findings (`unsafe_evidence_required: true`) while
/// ordinary evidence is advisory by default (`evidence_required: false`).
///
/// This asymmetry is deliberate: an unsafe finding always needs explicit
/// evidence even in the default profile, whereas general evidence links are
/// encouraged but not hard-required out of the box. Promote general evidence
/// to hard-required by setting `evidence_required = true` in the policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Requirements {
    pub owner_required: bool,
    pub reason_required: bool,
    pub classification_required: bool,
    pub evidence_required: bool,
    pub expires_or_review_after_required: bool,
    pub allow_bare_allow_attributes: bool,
    pub lint_policy_id_required: bool,
    pub stale_entries_fail: bool,
    pub unsafe_evidence_required: bool,
    pub unsafe_safety_comment_required: bool,
    /// Require at least one verified local-file evidence reference for unsafe
    /// entries (#3237). Traceability-only evidence (test:, issue:, pr:, etc.)
    /// is accepted as supplementary but does not satisfy this mandate alone.
    /// Default: false (opt-in to tighten the governance boundary).
    pub unsafe_verified_evidence_required: bool,
}

impl Default for Requirements {
    fn default() -> Self {
        Self {
            owner_required: true,
            reason_required: true,
            classification_required: true,
            evidence_required: false,
            expires_or_review_after_required: true,
            allow_bare_allow_attributes: false,
            lint_policy_id_required: false,
            stale_entries_fail: false,
            unsafe_evidence_required: true,
            unsafe_safety_comment_required: false,
            unsafe_verified_evidence_required: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileFamilyRule {
    /// Stable identifier for the repository-owned classification rule.
    pub id: String,
    /// Canonical family code emitted by the file-surface classifier.
    pub family: String,
    /// Source-tree-relative bounded glob matched by the rule.
    pub glob: String,
    /// Human rationale for retaining the repository-owned rule.
    pub reason: String,
}

/// Built-in non-Rust family codes that repository rules must not redefine.
///
/// Custom family classification is intentionally separate from the built-in
/// family vocabulary. Keeping the protected names in the shared contract
/// prevents the schema and scanner from silently disagreeing about reserved
/// names.
pub const BUILTIN_FILE_FAMILY_CODES: &[&str] = &[
    "generated_code",
    "ci_declarative",
    "editor_extension",
    "package_metadata",
    "test_fixture",
    "release_script",
    "documentation",
    "shell_script",
    "python_tool",
    "javascript_tool",
    "configuration",
    "unknown_non_rust",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceConfig {
    pub root: String,
    pub inventory: String,
    pub ignored: Vec<String>,
    pub generated: Vec<String>,
    pub default_mode: String,
    pub file_families: Vec<FileFamilyRule>,
}

impl Default for WorkspaceConfig {
    fn default() -> Self {
        Self {
            root: ".".to_string(),
            inventory: "git-tracked".to_string(),
            ignored: vec![".git/**".to_string(), "target/**".to_string()],
            generated: vec!["target/**".to_string(), "vendor/**".to_string()],
            default_mode: "no-new".to_string(),
            file_families: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AllowConfig {
    pub schema_version: String,
    pub policy: String,
    pub owner: Option<String>,
    pub status: Option<String>,
    pub workspace: WorkspaceConfig,
    pub requirements: Requirements,
    pub lanes: BTreeMap<String, LaneConfig>,
    pub allow: Vec<AllowEntry>,
}

impl AllowConfig {
    pub fn empty() -> Self {
        Self {
            schema_version: "0.1".to_string(),
            policy: "cargo-allow".to_string(),
            owner: None,
            status: Some("active".to_string()),
            workspace: WorkspaceConfig::default(),
            requirements: Requirements::default(),
            lanes: BTreeMap::new(),
            allow: Vec::new(),
        }
    }

    pub fn lane_enforcement_mode_for_kind(&self, kind: FindingKind) -> LaneEnforcementMode {
        lane_enforcement_mode_for_kind(&self.lanes, kind)
    }

    /// Validate the core invariants of this config: a non-empty, supported
    /// `schema_version`; a non-empty, recognized `policy` name; an optional
    /// but recognized `status`; and a `workspace.default_mode` that parses as a
    /// known [`WorkspaceMode`].
    ///
    /// This is the core-level validation entrypoint so that programmatic
    /// consumers of `allow-core` (building an `AllowConfig` directly, not via
    /// the TOML loader) get the same fail-closed feedback as a loaded policy.
    /// It codifies only the invariants the core data model owns; the
    /// `allow-policy` layer extends this with scope/glob/entry checks.
    ///
    /// Aggregates every problem into one error (rather than short-circuiting
    /// on the first), so an adopter sees the full list in a single run.
    pub fn validate(&self) -> CargoAllowResult<()> {
        join_errors(self.validation_errors())
    }

    /// Collect every core-level validation error for this config. Public to the
    /// crate family so `allow-policy` can fold core invariants into its own
    /// aggregated validation without re-implementing them.
    pub fn validation_errors(&self) -> Vec<CargoAllowError> {
        let mut errors = Vec::new();
        if let Err(e) = validate_schema_version(&self.schema_version) {
            errors.push(with_core_validation_diagnostic(e, "schema_version"));
        }
        if let Err(e) = validate_policy_name(&self.policy) {
            errors.push(with_core_validation_diagnostic(e, "policy"));
        }
        if let Err(e) = validate_optional_status(self.status.as_deref()) {
            errors.push(with_core_validation_diagnostic(e, "status"));
        }
        if let Err(e) = WorkspaceMode::from_str(&self.workspace.default_mode) {
            errors.push(with_core_validation_diagnostic(e, "workspace.default_mode"));
        }
        errors
    }
}

fn join_errors(errors: Vec<CargoAllowError>) -> CargoAllowResult<()> {
    match errors.as_slice() {
        [] => Ok(()),
        [single] => Err(single.clone()),
        _ => {
            let summary = errors
                .iter()
                .map(|e| format!("  - {e}"))
                .collect::<Vec<_>>()
                .join("\n");
            let diagnostics = errors
                .iter()
                .flat_map(|error| error.diagnostics().iter().cloned())
                .collect::<Vec<_>>();
            Err(CargoAllowError::with_kind(
                CargoAllowErrorKind::InvalidPolicy,
                format!(
                    "{count} policy validation errors:\n{summary}",
                    count = errors.len()
                ),
            )
            .with_diagnostics(diagnostics))
        }
    }
}

fn with_core_validation_diagnostic(error: CargoAllowError, field: &str) -> CargoAllowError {
    let code = error.code();
    let message = error.message().to_owned();
    error.with_diagnostic(CargoAllowDiagnostic::error(
        code,
        "policy_validation",
        None,
        Some(field),
        message,
    ))
}

/// Supported policy schema versions. `"1"` is accepted as a legacy alias.
pub const SUPPORTED_SCHEMA_VERSION: &str = "0.1";
pub const SUPPORTED_SCHEMA_VERSION_ALIAS: &str = "1";
/// The only recognized policy name.
pub const POLICY_NAME: &str = "cargo-allow";

fn require_non_empty(label: &str, value: &str) -> CargoAllowResult<()> {
    if value.trim().is_empty() {
        Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::InvalidPolicy,
            format!("policy {label} must not be empty"),
        ))
    } else {
        Ok(())
    }
}

fn validate_schema_version(value: &str) -> CargoAllowResult<()> {
    require_non_empty("schema_version", value)?;
    if value != SUPPORTED_SCHEMA_VERSION && value != SUPPORTED_SCHEMA_VERSION_ALIAS {
        return Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::InvalidPolicy,
            format!("unsupported policy schema_version `{value}`"),
        ));
    }
    Ok(())
}

fn validate_policy_name(value: &str) -> CargoAllowResult<()> {
    require_non_empty("policy name", value)?;
    if value != POLICY_NAME {
        return Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::InvalidPolicy,
            format!("unsupported policy `{value}`"),
        ));
    }
    Ok(())
}

fn validate_optional_status(status: Option<&str>) -> CargoAllowResult<()> {
    let Some(status) = status else {
        return Ok(());
    };
    if status.trim().is_empty() {
        return Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::InvalidPolicy,
            "policy status must not be empty".to_string(),
        ));
    }
    if !matches!(status, "active" | "advisory") {
        return Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::InvalidPolicy,
            format!("unsupported policy status `{status}`"),
        ));
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MatchStatus {
    Matched,
    New,
    Stale,
    Expired,
    ReviewDue,
    LocationDrift,
    Ambiguous,
    InvalidSelector,
    MissingRequiredField,
    EvidenceMissing,
    BaselineDebt,
}

impl MatchStatus {
    pub const ALL: &[Self] = &[
        Self::Matched,
        Self::New,
        Self::Stale,
        Self::Expired,
        Self::ReviewDue,
        Self::LocationDrift,
        Self::Ambiguous,
        Self::InvalidSelector,
        Self::MissingRequiredField,
        Self::EvidenceMissing,
        Self::BaselineDebt,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Matched => "matched",
            Self::New => "new",
            Self::Stale => "stale",
            Self::Expired => "expired",
            Self::ReviewDue => "review_due",
            Self::LocationDrift => "location_drift",
            Self::Ambiguous => "ambiguous",
            Self::InvalidSelector => "invalid_selector",
            Self::MissingRequiredField => "missing_required_field",
            Self::EvidenceMissing => "evidence_missing",
            Self::BaselineDebt => "baseline_debt",
        }
    }

    pub fn is_failure_in_strict(self) -> bool {
        !matches!(self, Self::Matched | Self::LocationDrift)
    }

    pub fn is_failure_in_no_new(self) -> bool {
        matches!(
            self,
            Self::New
                | Self::Expired
                | Self::Ambiguous
                | Self::InvalidSelector
                | Self::MissingRequiredField
                | Self::EvidenceMissing
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchOutcome {
    pub status: MatchStatus,
    pub allow_id: Option<String>,
    /// All policy entries considered for this finding, in deterministic policy
    /// order. For an ambiguous result this is the structured candidate list;
    /// consumers must not parse candidate IDs from `message`.
    pub candidate_ids: Vec<String>,
    pub finding_index: Option<usize>,
    pub message: String,
    pub score: u32,
}
