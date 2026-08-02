use std::fmt;
use std::ops::Range;
use std::path::Path;

/// One-based source location attached to a parse or validation diagnostic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CargoAllowErrorLocation {
    /// Source path when the caller had one; `None` means the input was an
    /// in-memory document without a known path.
    pub path: Option<String>,
    pub line: u32,
    pub column: u32,
}

/// Severity for a machine-readable diagnostic carried by a command error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CargoAllowDiagnosticSeverity {
    Error,
    Warning,
    Info,
}

/// Structured validation or execution detail.
///
/// The fields are intentionally owned and optional so diagnostics can be
/// produced by policy, federation, import, and command layers without making
/// those layers depend on a parser-specific representation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CargoAllowDiagnostic {
    pub code: String,
    pub category: String,
    pub severity: CargoAllowDiagnosticSeverity,
    pub path: Option<String>,
    pub span: Option<CargoAllowErrorLocation>,
    pub entry_id: Option<String>,
    pub field: Option<String>,
    pub message: String,
    pub help: Option<String>,
    pub causes: Vec<String>,
}

impl CargoAllowDiagnostic {
    pub fn error(
        code: impl Into<String>,
        category: impl Into<String>,
        entry_id: Option<&str>,
        field: Option<&str>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            code: code.into(),
            category: category.into(),
            severity: CargoAllowDiagnosticSeverity::Error,
            path: None,
            span: None,
            entry_id: entry_id.map(str::to_owned),
            field: field.map(str::to_owned),
            message: message.into(),
            help: None,
            causes: Vec::new(),
        }
    }
}

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
    /// All error kinds currently defined by this version.
    ///
    /// The enum is non-exhaustive; callers must still handle future kinds.
    pub const ALL: &[Self] = &[
        Self::Usage,
        Self::InvalidConfig,
        Self::InvalidPolicy,
        Self::Inventory,
        Self::Scan,
        Self::PolicyViolation,
        Self::Artifact,
        Self::Internal,
        Self::Unknown,
    ];

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

    /// Return the stable machine-readable error code for this kind.
    ///
    /// Codes are part of the public contract and must not be reused for a
    /// different failure class. See `docs/error-codes.md` for the registry.
    pub const fn code(self) -> &'static str {
        match self {
            Self::Usage => "E0001_USAGE",
            Self::InvalidConfig => "E0002_INVALID_CONFIG",
            Self::InvalidPolicy => "E0003_INVALID_POLICY",
            Self::Inventory => "E0004_INVENTORY",
            Self::Scan => "E0005_SCAN",
            Self::PolicyViolation => "E0006_POLICY_VIOLATION",
            Self::Artifact => "E0007_ARTIFACT",
            Self::Internal => "E0008_INTERNAL",
            Self::Unknown => "E0009_UNKNOWN",
        }
    }
}

impl fmt::Display for CargoAllowErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Linked cause node for [`std::error::Error::source`] walks.
#[derive(Debug, Clone)]
struct CauseError {
    message: String,
    next: Option<Box<CauseError>>,
}

impl fmt::Display for CauseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for CauseError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.next
            .as_ref()
            .map(|next| next.as_ref() as &(dyn std::error::Error + 'static))
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
    location: Option<CargoAllowErrorLocation>,
    diagnostics: Vec<CargoAllowDiagnostic>,
    /// Rendered cause chain (each element is the `Display` of an underlying
    /// error). Stored as strings so the struct stays `Clone` + `PartialEq`.
    causes: Vec<String>,
    /// Linked cause chain for `Error::source` / `successors` walks.
    source: Option<Box<CauseError>>,
}

impl CargoAllowError {
    /// Create an error with [`CargoAllowErrorKind::Unknown`] (backward compat).
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            kind: CargoAllowErrorKind::Unknown,
            message: message.into(),
            location: None,
            diagnostics: Vec::new(),
            causes: Vec::new(),
            source: None,
        }
    }

    /// Create an error with a structured kind.
    pub fn with_kind(kind: CargoAllowErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            location: None,
            diagnostics: Vec::new(),
            causes: Vec::new(),
            source: None,
        }
    }

    /// Attach a cause (underlying error) to this error, returning a new value.
    ///
    /// The cause is rendered as a `caused by:` line in `Display` and linked for
    /// [`std::error::Error::source`] walks.
    pub fn with_cause(mut self, cause: &(impl std::error::Error + ?Sized)) -> Self {
        let message = cause.to_string();
        self.causes.push(message.clone());
        let node = Box::new(CauseError {
            message,
            next: None,
        });
        match self.source.as_mut() {
            None => self.source = Some(node),
            Some(head) => append_cause(head, node),
        }
        self
    }

    /// Prefix the human-readable message without discarding structured error
    /// metadata such as the kind, source location, diagnostics, or causes.
    ///
    /// Context layers should use this instead of rebuilding an error from its
    /// rendered string. Rebuilding loses information that machine consumers
    /// and editor integrations rely on.
    pub fn with_message_prefix(mut self, prefix: impl AsRef<str>) -> Self {
        let prefix = prefix.as_ref();
        if !prefix.is_empty() {
            self.message.insert_str(0, prefix);
        }
        self
    }

    /// Rendered cause messages in attachment order (outermost first).
    pub fn causes(&self) -> &[String] {
        &self.causes
    }

    /// Attach one structured diagnostic detail, returning a new value.
    pub fn with_diagnostic(mut self, diagnostic: CargoAllowDiagnostic) -> Self {
        self.diagnostics.push(diagnostic);
        self
    }

    /// Attach multiple structured diagnostic details, returning a new value.
    pub fn with_diagnostics(
        mut self,
        diagnostics: impl IntoIterator<Item = CargoAllowDiagnostic>,
    ) -> Self {
        self.diagnostics.extend(diagnostics);
        self
    }

    /// Machine-readable details associated with this error.
    pub fn diagnostics(&self) -> &[CargoAllowDiagnostic] {
        &self.diagnostics
    }

    /// The structured error kind.
    pub fn kind(&self) -> CargoAllowErrorKind {
        self.kind
    }

    /// The stable machine-readable code for this error.
    pub fn code(&self) -> &'static str {
        self.kind.code()
    }

    /// Attach a one-based source location derived from a TOML byte span.
    ///
    /// TOML reports byte offsets. This conversion keeps the public error
    /// contract independent of the parser's error-display text and reports a
    /// character column suitable for editor and CI diagnostics.
    pub fn with_toml_span(
        mut self,
        path: Option<&Path>,
        source: &str,
        span: Option<Range<usize>>,
    ) -> Self {
        let Some(span) = span else {
            return self;
        };
        let prefix = source.get(..span.start).unwrap_or(source);
        let line = prefix.bytes().filter(|byte| *byte == b'\n').count() + 1;
        let column = prefix
            .rsplit_once('\n')
            .map(|(_, line)| line.chars().count() + 1)
            .unwrap_or_else(|| prefix.chars().count() + 1);
        self.location = Some(CargoAllowErrorLocation {
            path: path.map(|value| value.display().to_string()),
            line: u32::try_from(line).unwrap_or(u32::MAX),
            column: u32::try_from(column).unwrap_or(u32::MAX),
        });
        for diagnostic in &mut self.diagnostics {
            diagnostic.path = path.map(|value| value.display().to_string());
            diagnostic.span = self.location.clone();
        }
        self
    }

    /// Structured source location, when the error originated from located
    /// input such as a TOML parse.
    pub fn location(&self) -> Option<&CargoAllowErrorLocation> {
        self.location.as_ref()
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

impl std::error::Error for CargoAllowError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.source
            .as_ref()
            .map(|cause| cause.as_ref() as &(dyn std::error::Error + 'static))
    }
}

/// Auto-convert `io::Error` so `?` works at IO call sites without manual
/// `map_err`. The kind is [`CargoAllowErrorKind::Unknown`]; callers that want
/// a specific kind (e.g. `Inventory`) should use `with_kind` explicitly.
impl From<std::io::Error> for CargoAllowError {
    fn from(e: std::io::Error) -> Self {
        let message = e.to_string();
        let mut err = CargoAllowError::with_kind(CargoAllowErrorKind::Unknown, message.clone());
        err.kind = match e.kind() {
            std::io::ErrorKind::NotFound => CargoAllowErrorKind::InvalidConfig,
            std::io::ErrorKind::PermissionDenied => CargoAllowErrorKind::Inventory,
            _ => CargoAllowErrorKind::Unknown,
        };
        err.message = message.clone();
        // Keep the IO error visible to `Error::source` walkers without
        // duplicating the same text under Display's `caused by:` lines.
        err.source = Some(Box::new(CauseError {
            message,
            next: None,
        }));
        err
    }
}

fn append_cause(head: &mut CauseError, node: Box<CauseError>) {
    match head.next.as_mut() {
        Some(next) => append_cause(next, node),
        None => head.next = Some(node),
    }
}

pub type CargoAllowResult<T> = Result<T, CargoAllowError>;

#[cfg(test)]
#[path = "error_tests.rs"]
mod tests;
