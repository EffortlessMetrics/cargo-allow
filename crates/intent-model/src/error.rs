//! Intent-owned error and source utility contract.
//!
//! The model crate parses authored repository documents without depending on
//! cargo-allow product types. These helpers preserve the existing parse
//! location, bounded-read, path, and content-identity behavior while the
//! duplicated allow-policy snapshot is retired under #2935/#2568.

use std::fmt;
use std::io::Read;
use std::ops::Range;
use std::path::Path;

/// Result type for authored intent-model operations.
pub type IntentModelResult<T> = Result<T, IntentModelError>;

/// Stable failure classes owned by the authored intent-model boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IntentModelErrorKind {
    InvalidConfig,
    InvalidPolicy,
    Io,
    Unknown,
}

/// One-based source location attached to a parse diagnostic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntentModelErrorLocation {
    pub path: Option<String>,
    pub line: u32,
    pub column: u32,
}

/// Authored intent parsing or validation failure.
#[derive(Debug, Clone)]
pub struct IntentModelError {
    kind: IntentModelErrorKind,
    message: String,
    location: Option<IntentModelErrorLocation>,
}

impl IntentModelError {
    pub fn new(message: impl Into<String>) -> Self {
        Self::with_kind(IntentModelErrorKind::Unknown, message)
    }

    pub fn with_kind(kind: IntentModelErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            location: None,
        }
    }

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
        self.location = Some(IntentModelErrorLocation {
            path: path.map(|value| value.display().to_string()),
            line: u32::try_from(line).unwrap_or(u32::MAX),
            column: u32::try_from(column).unwrap_or(u32::MAX),
        });
        self
    }

    pub fn kind(&self) -> IntentModelErrorKind {
        self.kind
    }

    pub fn location(&self) -> Option<&IntentModelErrorLocation> {
        self.location.as_ref()
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for IntentModelError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for IntentModelError {}

/// FNV-1a content identity retained for authored-source parity.
pub fn stable_hash_hex(input: &str) -> String {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in input.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("fnv1a64:{hash:016x}")
}

/// Normalize repository paths to forward slashes.
pub fn normalize_path(path: impl AsRef<Path>) -> String {
    path.as_ref().to_string_lossy().replace('\\', "/")
}

/// Maximum source-document size accepted by the model reader.
pub const SOURCE_FILE_READ_MAX_BYTES: usize = 10 * 1024 * 1024;

/// Read UTF-8 source text without allowing unbounded allocation.
pub fn read_text_file_capped(path: &Path) -> IntentModelResult<String> {
    let file = std::fs::File::open(path).map_err(|error| {
        IntentModelError::with_kind(
            IntentModelErrorKind::Io,
            format!("failed to read {}: {error}", path.display()),
        )
    })?;
    let mut bytes = Vec::with_capacity(8192);
    file.take(SOURCE_FILE_READ_MAX_BYTES as u64)
        .read_to_end(&mut bytes)
        .map_err(|error| {
            IntentModelError::with_kind(
                IntentModelErrorKind::Io,
                format!("failed to read {}: {error}", path.display()),
            )
        })?;
    String::from_utf8(bytes).map_err(|error| {
        IntentModelError::with_kind(
            IntentModelErrorKind::Io,
            format!("file {} is not valid UTF-8: {error}", path.display()),
        )
    })
}
