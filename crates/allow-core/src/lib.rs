use std::fmt;
use std::path::PathBuf;
use std::str::FromStr;

pub const STRUCTURAL_IDENTITY_SCHEMA_ID: &str = "cargo-allow.structural-identity.v1";

mod date;
mod error;
mod fingerprint;
mod json;
mod source_tree_path;
pub use date::SimpleDate;
pub use error::{CargoAllowError, CargoAllowResult};
pub use fingerprint::{maybe_line_distance_score, normalize_snippet, stable_hash_hex};
pub use json::json_escape;
pub use source_tree_path::{
    glob_matches, glob_matches_str, normalize_path, source_tree_path_matches_filter,
    source_tree_scope_has_wildcard,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    pub line: u32,
    pub column: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FindingKind {
    Panic,
    Unsafe,
    LintException,
    NonRustFile,
    GeneratedCode,
    PolicyException,
}

impl FindingKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Panic => "panic",
            Self::Unsafe => "unsafe",
            Self::LintException => "lint_exception",
            Self::NonRustFile => "non_rust_file",
            Self::GeneratedCode => "generated_code",
            Self::PolicyException => "policy_exception",
        }
    }
}

impl fmt::Display for FindingKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for FindingKind {
    type Err = CargoAllowError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim() {
            "panic" | "panic_family" | "panic-family" | "indexing" => Ok(Self::Panic),
            "unsafe" => Ok(Self::Unsafe),
            "lint_exception" | "lint-exception" | "clippy" | "allow_attribute"
            | "allow-attribute" | "expect_attribute" | "expect-attribute" => {
                Ok(Self::LintException)
            }
            "non_rust_file" | "non-rust-file" | "non_rust" | "non-rust" | "file" => {
                Ok(Self::NonRustFile)
            }
            "generated_code" | "generated-code" | "generated" => Ok(Self::GeneratedCode),
            "policy_exception" | "policy-exception" | "policy" => Ok(Self::PolicyException),
            other => Err(CargoAllowError::new(format!(
                "unsupported finding kind `{other}`"
            ))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructuralIdentity {
    pub language: String,
    pub crate_name: Option<String>,
    pub module: Option<String>,
    pub container: Option<String>,
    pub ast_kind: String,
    pub symbol: Option<String>,
    pub callee: Option<String>,
    pub macro_name: Option<String>,
    pub lint: Option<String>,
    pub receiver_fingerprint: Option<String>,
    pub target_fingerprint: Option<String>,
    pub normalized_snippet_hash: Option<String>,
    pub line_hint: Option<u32>,
    pub column_hint: Option<u32>,
}

impl StructuralIdentity {
    pub fn schema_id() -> &'static str {
        STRUCTURAL_IDENTITY_SCHEMA_ID
    }

    pub fn new(language: impl Into<String>, ast_kind: impl Into<String>) -> Self {
        Self {
            language: language.into(),
            crate_name: None,
            module: None,
            container: None,
            ast_kind: ast_kind.into(),
            symbol: None,
            callee: None,
            macro_name: None,
            lint: None,
            receiver_fingerprint: None,
            target_fingerprint: None,
            normalized_snippet_hash: None,
            line_hint: None,
            column_hint: None,
        }
    }

    pub fn stable_key(&self) -> String {
        stable_identity_key_from_parts(self.stable_key_parts())
    }

    pub fn stable_key_parts(&self) -> Vec<(&'static str, String)> {
        vec![
            ("language", self.language.clone()),
            ("crate_name", self.crate_name.clone().unwrap_or_default()),
            ("module", self.module.clone().unwrap_or_default()),
            ("container", self.container.clone().unwrap_or_default()),
            ("ast_kind", self.ast_kind.clone()),
            ("symbol", self.symbol.clone().unwrap_or_default()),
            ("callee", self.callee.clone().unwrap_or_default()),
            ("macro_name", self.macro_name.clone().unwrap_or_default()),
            ("lint", self.lint.clone().unwrap_or_default()),
            (
                "receiver_fingerprint",
                self.receiver_fingerprint.clone().unwrap_or_default(),
            ),
            (
                "target_fingerprint",
                self.target_fingerprint.clone().unwrap_or_default(),
            ),
            (
                "normalized_snippet_hash",
                self.normalized_snippet_hash.clone().unwrap_or_default(),
            ),
        ]
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub kind: FindingKind,
    pub family: Option<String>,
    pub path: PathBuf,
    pub span: Option<Span>,
    pub identity: StructuralIdentity,
    pub message: String,
}

impl Finding {
    pub fn source_package_name(&self) -> Option<&str> {
        self.identity
            .crate_name
            .as_deref()
            .map(str::trim)
            .filter(|name| !name.is_empty())
    }
}

pub fn finding_identity_key(finding: &Finding) -> String {
    let mut parts = vec![
        ("kind", finding.kind.as_str().to_string()),
        ("family", finding.family.clone().unwrap_or_default()),
        ("path", normalize_path(&finding.path)),
    ];
    parts.extend(finding.identity.stable_key_parts());
    stable_identity_key_from_parts(parts)
}

fn stable_identity_key_from_parts(parts: Vec<(&'static str, String)>) -> String {
    parts
        .into_iter()
        .map(|(name, value)| format!("{name}:{}:{value}", value.len()))
        .collect::<Vec<_>>()
        .join("|")
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
            glob.clone()
        } else if let Some(glob) = &self.selector.glob {
            glob.clone()
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
            allow: Vec::new(),
        }
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
        !matches!(self, Self::Matched | Self::ReviewDue)
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

#[cfg(test)]
mod tests;
