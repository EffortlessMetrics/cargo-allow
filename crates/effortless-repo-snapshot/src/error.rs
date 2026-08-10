//! Neutral error and utility types for effortless-repo-snapshot (#3146).
//!
//! Replaces allow-core dependency for error/utility types.
//! allow-inventory remains a hard dependency pending inventory type abstraction.

pub type SnapshotResult<T> = Result<T, SnapshotError>;

#[derive(Debug, Clone)]
pub struct SnapshotError {
    message: String,
    kind: SnapshotErrorKind,
    diagnostic: Option<Box<SnapshotDiagnostic>>,
}

impl SnapshotError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            kind: SnapshotErrorKind::Unknown,
            diagnostic: None,
        }
    }

    pub fn with_kind(kind: SnapshotErrorKind, message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            kind,
            diagnostic: None,
        }
    }

    pub fn with_diagnostic(mut self, diagnostic: SnapshotDiagnostic) -> Self {
        self.diagnostic = Some(Box::new(diagnostic));
        self
    }

    pub fn with_cause(self, _cause: impl std::fmt::Display) -> Self {
        self
    }

    pub fn as_str(&self) -> &str {
        &self.message
    }

    pub fn kind(&self) -> SnapshotErrorKind {
        self.kind
    }

    pub fn diagnostics(&self) -> Vec<&SnapshotDiagnostic> {
        match &self.diagnostic {
            Some(d) => vec![d.as_ref()],
            None => vec![],
        }
    }
}

impl std::fmt::Display for SnapshotError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for SnapshotError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapshotErrorKind {
    Internal,
    InvalidConfig,
    Inventory,
    Artifact,
    Unknown,
    Scan,
}

#[derive(Debug, Clone)]
pub struct SnapshotDiagnostic {
    pub code: String,
    pub category: String,
    pub path: Option<String>,
    pub entry_id: Option<String>,
    pub message: String,
}

impl SnapshotDiagnostic {
    pub fn error(
        code: impl Into<String>,
        category: impl Into<String>,
        path: Option<&str>,
        entry_id: Option<&str>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            code: code.into(),
            category: category.into(),
            path: path.map(|s| s.to_string()),
            entry_id: entry_id.map(|s| s.to_string()),
            message: message.into(),
        }
    }
}

pub fn sha256_v1_bytes(input: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(input);
    let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    format!("sha256:v1:{hex}")
}

#[cfg(feature = "allow-core-interop")]
impl From<SnapshotError> for allow_core::CargoAllowError {
    fn from(err: SnapshotError) -> Self {
        allow_core::CargoAllowError::new(err.as_str())
    }
}

#[cfg(feature = "allow-core-interop")]
impl From<allow_core::CargoAllowError> for SnapshotError {
    fn from(err: allow_core::CargoAllowError) -> Self {
        SnapshotError::new(err.to_string())
    }
}
