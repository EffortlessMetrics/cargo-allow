pub fn normalize_snippet(input: &str) -> String {
    input.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub fn stable_hash_hex(input: &str) -> String {
    // FNV-1a 64-bit. Not cryptographic; stable across platforms and enough for drift hints.
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in input.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("fnv1a64:{hash:016x}")
}

pub fn maybe_line_distance_score(hint: Option<u32>, actual: Option<u32>) -> u32 {
    match (hint, actual) {
        (Some(h), Some(a)) => {
            let diff = h.abs_diff(a);
            if diff == 0 {
                15
            } else if diff <= 3 {
                12
            } else if diff <= 10 {
                8
            } else if diff <= 25 {
                3
            } else {
                0
            }
        }
        _ => 0,
    }
}
