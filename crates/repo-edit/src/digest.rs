//! Versioned content digests for apply receipts (#2602-C).

use sha2::{Digest, Sha256};

/// SHA-256 digest for exact byte bindings (`sha256:v1:...`).
pub fn sha256_v1_bytes(input: &[u8]) -> String {
    let digest = Sha256::digest(input);
    let hex = digest
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!("sha256:v1:{hex}")
}
