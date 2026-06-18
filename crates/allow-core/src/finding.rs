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
