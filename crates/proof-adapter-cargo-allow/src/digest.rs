//! Local digest helper without allow-core dependency.

use sha2::{Digest, Sha256};

pub fn sha256_v1_bytes(input: &[u8]) -> String {
    let digest = Sha256::digest(input);
    let hex = digest
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!("sha256:v1:{hex}")
}
