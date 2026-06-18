use crate::{FindingKind, normalize_path, source_tree_path::normalize_source_tree_scope};
use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::lane_posture::{LaneConfig, LaneEnforcementMode, lane_enforcement_mode_for_kind};

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
}

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
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceConfig {
    pub root: String,
    pub inventory: String,
    pub ignored: Vec<String>,
    pub generated: Vec<String>,
    pub default_mode: String,
}

impl Default for WorkspaceConfig {
    fn default() -> Self {
        Self {
            root: ".".to_string(),
            inventory: "git-tracked".to_string(),
            ignored: vec![".git/**".to_string(), "target/**".to_string()],
            generated: vec!["target/**".to_string(), "vendor/**".to_string()],
            default_mode: "no-new".to_string(),
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MatchStatus {
    Matched,
    New,
    Stale,
    Expired,
    ReviewDue,
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
            Self::Ambiguous => "ambiguous",
            Self::InvalidSelector => "invalid_selector",
            Self::MissingRequiredField => "missing_required_field",
            Self::EvidenceMissing => "evidence_missing",
            Self::BaselineDebt => "baseline_debt",
        }
    }

    pub fn is_failure_in_strict(self) -> bool {
        !matches!(self, Self::Matched)
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
    pub finding_index: Option<usize>,
    pub message: String,
    pub score: u32,
}
