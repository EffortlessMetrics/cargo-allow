use crate::{CargoAllowError, LedgerProvenance, normalize_path};
use std::fmt;
use std::path::PathBuf;
use std::str::FromStr;

pub const STRUCTURAL_IDENTITY_SCHEMA_ID: &str = "cargo-allow.structural-identity.v1";

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
    pub const ALL: &[Self] = &[
        Self::Panic,
        Self::Unsafe,
        Self::LintException,
        Self::NonRustFile,
        Self::GeneratedCode,
        Self::PolicyException,
    ];

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

    pub fn requires_source_selector_identity(self) -> bool {
        matches!(self, Self::Panic | Self::Unsafe | Self::LintException)
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
        let trimmed = s.trim();
        let normalized = trimmed.to_ascii_lowercase();
        match normalized.as_str() {
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
            _ => Err(CargoAllowError::new(format!(
                "unsupported finding kind `{trimmed}`; valid values: panic, unsafe, lint_exception, non_rust_file, generated_code, policy_exception"
            ))),
        }
    }
}

/// Maximum length (bytes) of any source-derived string field in a
/// [`StructuralIdentity`]. Caps the DoS / noisy-diff surface from a scanned
/// file with a megabyte-long identifier (#1919). Generous enough for realistic
/// Rust paths/identifiers (e.g. deeply-qualified module paths), small enough
/// that an artifact cannot be inflated by a single field.
pub const MAX_IDENTITY_FIELD_LEN: usize = 512;

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

    /// Cap every source-derived string field at [`MAX_IDENTITY_FIELD_LEN`] so a
    /// scanned file with a megabyte-long identifier cannot inflate report/receipt
    /// artifacts unboundedly (DoS / noisy-diff surface) (#1919). Applied in place
    /// at the finding-construction choke point before the identity reaches any
    /// artifact. Hashes (e.g. `normalized_snippet_hash`) are already fixed-width
    /// and excluded.
    pub fn truncate_in_place(&mut self) {
        let cap_opt = |s: &mut Option<String>| {
            if let Some(value) = s {
                truncate_identity_field(value);
            }
        };
        let cap_str = |s: &mut String| {
            truncate_identity_field(s);
        };
        cap_str(&mut self.language);
        cap_opt(&mut self.crate_name);
        cap_opt(&mut self.module);
        cap_opt(&mut self.container);
        cap_str(&mut self.ast_kind);
        cap_opt(&mut self.symbol);
        cap_opt(&mut self.callee);
        cap_opt(&mut self.macro_name);
        cap_opt(&mut self.lint);
        cap_opt(&mut self.receiver_fingerprint);
        cap_opt(&mut self.target_fingerprint);
    }

    /// Redact the source-text-bearing identity fields (`symbol`, `callee`,
    /// `container`, `module`, `macro_name`, `lint`) by clearing them, while
    /// preserving the structural anchors (`normalized_snippet_hash`,
    /// fingerprints, `ast_kind`, `line_hint`, `column_hint`) that matching
    /// relies on. Opt-in for CI artifacts where source-text-derived fields are
    /// an info-leak surface (#1920).
    pub fn redact_source_text_fields(&mut self) {
        self.symbol = None;
        self.callee = None;
        self.container = None;
        self.module = None;
        self.macro_name = None;
        self.lint = None;
    }

    pub fn stable_key(&self) -> String {
        stable_identity_key_from_parts(self.stable_key_parts())
    }

    pub fn stable_key_parts(&self) -> Vec<(&'static str, String)> {
        vec![
            ("language", self.language.clone()),
            (
                "crate_name",
                self.crate_name
                    .as_deref()
                    .map(str::trim)
                    .unwrap_or_default()
                    .to_string(),
            ),
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

fn truncate_identity_field(value: &mut String) {
    if value.len() <= MAX_IDENTITY_FIELD_LEN {
        return;
    }
    let mut end = MAX_IDENTITY_FIELD_LEN;
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    value.truncate(end);
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub kind: FindingKind,
    pub family: Option<String>,
    pub path: PathBuf,
    pub span: Option<Span>,
    pub identity: StructuralIdentity,
    pub message: String,
    pub ledger: Option<LedgerProvenance>,
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

#[cfg(test)]
#[path = "finding_tests.rs"]
mod tests;
