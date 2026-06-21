use std::fmt;

/// Structured kind for [`CargoAllowError`], enabling programmatic consumers
/// (CI tooling, sibling tools) to branch on error class instead of
/// string-matching the rendered message.
///
/// This enum is `#[non_exhaustive]` so new kinds can be added without a
/// breaking change for downstream library consumers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum CargoAllowErrorKind {
    /// CLI usage error (bad flags, conflicting arguments).
    Usage,
    /// Invalid or missing configuration file/values.
    InvalidConfig,
    /// Invalid policy ledger (validation failure, parse error, unknown field).
    InvalidPolicy,
    /// Inventory discovery failure (git error, unreadable directory).
    Inventory,
    /// Scan failure (read error, parse error in a source file).
    Scan,
    /// Policy violation (check/diff gate failed).
    PolicyViolation,
    /// Artifact or write failure (receipt rendering, policy write).
    Artifact,
    /// Internal invariant failure (should not happen).
    Internal,
    /// Unclassified — preserved for backward compatibility with `new()`.
    Unknown,
}

impl CargoAllowErrorKind {
    /// Render the kind as a stable, lowercase identifier suitable for
    /// machine consumption (e.g. receipt `error.kind` fields).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Usage => "usage",
            Self::InvalidConfig => "invalid_config",
            Self::InvalidPolicy => "invalid_policy",
            Self::Inventory => "inventory",
            Self::Scan => "scan",
            Self::PolicyViolation => "policy_violation",
            Self::Artifact => "artifact",
            Self::Internal => "internal",
            Self::Unknown => "unknown",
        }
    }
}

impl fmt::Display for CargoAllowErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The unified error type for the cargo-allow workspace.
///
/// Carries a structured [`CargoAllowErrorKind`], a human-readable message, and
/// an optional cause chain (rendered as a `caused by:` suffix in `Display`).
/// The `kind()` accessor lets programmatic consumers branch on error class
/// without string-matching.
#[derive(Debug, Clone)]
pub struct CargoAllowError {
    kind: CargoAllowErrorKind,
    message: String,
    /// Rendered cause chain (each element is the `Display` of an underlying
    /// error). Stored as strings so the struct stays `Clone` + `PartialEq`.
    causes: Vec<String>,
}

impl CargoAllowError {
    /// Create an error with [`CargoAllowErrorKind::Unknown`] (backward compat).
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            kind: CargoAllowErrorKind::Unknown,
            message: message.into(),
            causes: Vec::new(),
        }
    }

    /// Create an error with a structured kind.
    pub fn with_kind(kind: CargoAllowErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            causes: Vec::new(),
        }
    }

    /// Attach a cause (underlying error) to this error, returning a new value.
    /// The cause is rendered as a `caused by:` line in `Display`.
    pub fn with_cause(mut self, cause: &(impl std::error::Error + ?Sized)) -> Self {
        self.causes.push(cause.to_string());
        self
    }

    /// The structured error kind.
    pub fn kind(&self) -> CargoAllowErrorKind {
        self.kind
    }

    /// The human-readable message (without the cause chain).
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for CargoAllowError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)?;
        for cause in &self.causes {
            write!(f, "\n  caused by: {cause}")?;
        }
        Ok(())
    }
}

impl std::error::Error for CargoAllowError {}

/// `PartialEq` compares kind + message only (not the cause chain), so tests
/// that `assert_eq!` on constructed errors are not sensitive to cause text.
impl PartialEq for CargoAllowError {
    fn eq(&self, other: &Self) -> bool {
        self.kind == other.kind && self.message == other.message
    }
}

impl Eq for CargoAllowError {}

/// Auto-convert `io::Error` so `?` works at IO call sites without manual
/// `map_err`. The kind is [`CargoAllowErrorKind::Unknown`]; callers that want
/// a specific kind (e.g. `Inventory`) should use `with_kind` explicitly.
impl From<std::io::Error> for CargoAllowError {
    fn from(e: std::io::Error) -> Self {
        let mut err = CargoAllowError::with_kind(CargoAllowErrorKind::Unknown, e.to_string());
        err.causes.push(e.to_string());
        err.kind = match e.kind() {
            std::io::ErrorKind::NotFound => CargoAllowErrorKind::InvalidConfig,
            std::io::ErrorKind::PermissionDenied => CargoAllowErrorKind::Inventory,
            _ => CargoAllowErrorKind::Unknown,
        };
        err.message = e.to_string();
        err
    }
}

pub type CargoAllowResult<T> = Result<T, CargoAllowError>;

#[cfg(test)]
#[path = "error_tests.rs"]
mod tests;
