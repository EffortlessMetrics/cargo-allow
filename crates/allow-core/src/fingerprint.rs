use crate::policy::AllowEntry;

pub fn normalize_snippet(input: &str) -> String {
    strip_rust_comments(input)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn strip_rust_comments(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '"' => consume_quoted_string(ch, &mut chars, &mut out),
            'r' if consume_raw_string(&mut chars, &mut out) => {}
            '/' => match chars.peek().copied() {
                Some('/') => {
                    let _ = chars.next();
                    consume_line_comment(&mut chars);
                    out.push(' ');
                }
                Some('*') => {
                    let _ = chars.next();
                    consume_block_comment(&mut chars);
                    out.push(' ');
                }
                _ => out.push(ch),
            },
            _ => out.push(ch),
        }
    }
    out
}

fn consume_quoted_string(
    quote: char,
    chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
    out: &mut String,
) {
    out.push(quote);
    let mut escaped = false;
    for ch in chars.by_ref() {
        out.push(ch);
        if escaped {
            escaped = false;
        } else if ch == '\\' {
            escaped = true;
        } else if ch == quote {
            break;
        }
    }
}

fn consume_raw_string(
    chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
    out: &mut String,
) -> bool {
    let mut lookahead = chars.clone();
    let mut hashes = 0usize;
    while lookahead.peek().copied() == Some('#') {
        let _ = lookahead.next();
        hashes += 1;
    }
    if lookahead.next() != Some('"') {
        return false;
    }

    out.push('r');
    for _ in 0..hashes {
        let Some(hash) = chars.next() else {
            return true;
        };
        out.push(hash);
    }
    let Some(quote) = chars.next() else {
        return true;
    };
    out.push(quote);
    consume_raw_string_tail(chars, out, hashes);
    true
}

fn consume_raw_string_tail(
    chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
    out: &mut String,
    hashes: usize,
) {
    while let Some(ch) = chars.next() {
        out.push(ch);
        if ch != '"' {
            continue;
        }
        let mut matched = 0usize;
        while matched < hashes && chars.peek().copied() == Some('#') {
            let Some(hash) = chars.next() else {
                break;
            };
            out.push(hash);
            matched += 1;
        }
        if matched == hashes {
            break;
        }
    }
}

fn consume_line_comment(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) {
    for ch in chars.by_ref() {
        if ch == '\n' {
            break;
        }
    }
}

fn consume_block_comment(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) {
    let mut depth = 1usize;
    while let Some(ch) = chars.next() {
        match (ch, chars.peek().copied()) {
            ('/', Some('*')) => {
                let _ = chars.next();
                depth += 1;
            }
            ('*', Some('/')) => {
                let _ = chars.next();
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    break;
                }
            }
            _ => {}
        }
    }
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

/// Deterministic content fingerprint of an allow entry's full state, for
/// mutation-receipt provenance (CARGO-ALLOW-SPEC-0008 "Mutation Receipt
/// Envelope"). Not identity: two entries with the same content but different
/// `id` hash differently, and any field change changes the fingerprint. Not
/// cryptographic; advisory provenance only, not a merge or matching key.
pub fn allow_entry_content_fingerprint(entry: &AllowEntry) -> String {
    stable_hash_hex(&format!("{entry:?}"))
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
