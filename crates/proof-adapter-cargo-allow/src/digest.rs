//! Local digest helper without allow-core dependency.

use sha2::{Digest, Sha256};

pub fn sha256_v1_bytes(input: &[u8]) -> String {
    let digest = Sha256::digest(input);
    format!("sha256:v1:{}", hex::encode(digest))
}

mod hex {
    const HEX: &[u8; 16] = b"0123456789abcdef";

    pub fn encode(bytes: impl AsRef<[u8]>) -> String {
        let bytes = bytes.as_ref();
        let mut out = String::with_capacity(bytes.len() * 2);
        for byte in bytes {
            out.push(HEX[(byte >> 4) as usize] as char);
            out.push(HEX[(byte & 0x0f) as usize] as char);
        }
        out
    }
}
