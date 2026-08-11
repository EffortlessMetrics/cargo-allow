//! Neutral error and utility types for effortless-repo-edit (#2969).
//!
//! These types replace the allow-core dependency, making this crate
//! product-neutral. The implementations are byte-for-byte compatible with
//! allow-core to preserve lock-key stability and JSON escaping behavior.

/// Result type for repo-edit operations.
pub type RepoEditResult<T> = Result<T, RepoEditError>;

/// Error type for repo-edit operations — intentionally simple (string-based)
/// since this crate is a neutral utility, not a product-level error authority.
#[derive(Debug, Clone)]
pub struct RepoEditError {
    message: String,
}

impl RepoEditError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    pub fn as_str(&self) -> &str {
        &self.message
    }
}

impl std::fmt::Display for RepoEditError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for RepoEditError {}

/// FNV-1a 64-bit hash for stable lock-key derivation.
///
/// Not cryptographic; stable across platforms. Byte-for-byte compatible with
/// `allow_core::stable_hash_hex` to preserve existing lock-key identity.
pub fn stable_hash_hex(input: &str) -> String {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in input.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("fnv1a64:{hash:016x}")
}

/// JSON string escaping.
///
/// Byte-for-byte compatible with `allow_core::json_escape`.
pub fn json_escape(input: &str) -> String {
    let mut out = String::new();
    for ch in input.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}
