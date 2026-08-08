//! Neutral error and utility types for effortless-rust-source-index (#3147).
//!
//! Replaces allow-core dependency with local implementations that are
//! byte-for-byte compatible.

/// Result type for rust-source-index operations.
pub type IndexResult<T> = Result<T, IndexError>;

/// Error type — intentionally simple (string-based).
#[derive(Debug, Clone)]
pub struct IndexError {
    message: String,
}

impl IndexError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    pub fn as_str(&self) -> &str {
        &self.message
    }
}

impl std::fmt::Display for IndexError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for IndexError {}

/// FNV-1a 64-bit hash, byte-compatible with allow_core::stable_hash_hex.
pub fn stable_hash_hex(input: &str) -> String {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in input.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("fnv1a64:{hash:016x}")
}

/// Normalize path: replace backslashes with forward slashes.
/// Byte-compatible with allow_core::normalize_path for simple cases.
pub fn normalize_path(path: impl AsRef<std::path::Path>) -> String {
    path.as_ref().to_string_lossy().replace('\\', "/")
}

/// Maximum bytes to read from a single file.
pub const SOURCE_FILE_READ_MAX_BYTES: usize = 10 * 1024 * 1024;

/// Read a text file with a size cap, byte-compatible with allow_core::read_text_file_capped.
pub fn read_text_file_capped(path: &std::path::Path) -> IndexResult<String> {
    use std::io::Read;
    let file = std::fs::File::open(path)
        .map_err(|e| IndexError::new(format!("failed to read {}: {e}", path.display())))?;
    let mut buf = Vec::with_capacity(8192);
    file.take(SOURCE_FILE_READ_MAX_BYTES as u64)
        .read_to_end(&mut buf)
        .map_err(|e| IndexError::new(format!("failed to read {}: {e}", path.display())))?;
    String::from_utf8(buf)
        .map_err(|e| IndexError::new(format!("file {} is not valid UTF-8: {e}", path.display())))
}

/// Feature-gated conversion for product consumers.
#[cfg(feature = "allow-core-interop")]
impl From<IndexError> for allow_core::CargoAllowError {
    fn from(err: IndexError) -> Self {
        allow_core::CargoAllowError::new(err.as_str())
    }
}
