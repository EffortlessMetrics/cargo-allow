use crate::policy::AllowEntry;
use sha2::{Digest, Sha256};

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

const ALLOW_ENTRY_FINGERPRINT_SCHEMA: &str = "cargo-allow.allow-entry-fingerprint.v1";

/// Deterministic content fingerprint of an allow entry's full state, for
/// mutation-receipt provenance (CARGO-ALLOW-SPEC-0008 "Mutation Receipt
/// Envelope"). The `v1` canonical serialization is length-prefixed and has a
/// fixed field order, so it is independent of Rust's `Debug` formatting and
/// platform path separators — `path`, `glob`, and `selector.glob` are all
/// slash-normalized before hashing, so semantically identical entries
/// authored on Windows and Unix fingerprint identically. The SHA-256 digest is
/// provenance evidence, not an identity or matching key.
pub fn allow_entry_content_fingerprint(entry: &AllowEntry) -> String {
    let mut canonical = Vec::new();
    write_string(&mut canonical, ALLOW_ENTRY_FINGERPRINT_SCHEMA);
    write_string(&mut canonical, &entry.id);
    write_string(&mut canonical, entry.kind.as_str());
    write_optional_string(&mut canonical, entry.family.as_deref());
    write_optional_string(
        &mut canonical,
        entry.path.as_deref().map(crate::normalize_path).as_deref(),
    );
    write_optional_string(
        &mut canonical,
        entry.glob.as_deref().map(crate::normalize_path).as_deref(),
    );
    write_string(&mut canonical, &entry.owner);
    write_string(&mut canonical, &entry.classification);
    write_string(&mut canonical, &entry.reason);
    write_string_list(&mut canonical, &entry.evidence);
    write_string_list(&mut canonical, &entry.links);
    write_optional_u32(&mut canonical, entry.occurrence_limit);
    write_optional_string(&mut canonical, entry.lifecycle.created.as_deref());
    write_optional_string(&mut canonical, entry.lifecycle.review_after.as_deref());
    write_optional_string(&mut canonical, entry.lifecycle.expires.as_deref());

    write_optional_string(&mut canonical, entry.selector.ast_kind.as_deref());
    write_optional_string(&mut canonical, entry.selector.container.as_deref());
    write_optional_string(&mut canonical, entry.selector.callee.as_deref());
    write_optional_string(&mut canonical, entry.selector.macro_name.as_deref());
    write_optional_string(&mut canonical, entry.selector.lint.as_deref());
    write_optional_string(&mut canonical, entry.selector.symbol.as_deref());
    write_optional_string(
        &mut canonical,
        entry.selector.receiver_fingerprint.as_deref(),
    );
    write_optional_string(&mut canonical, entry.selector.target_fingerprint.as_deref());
    write_optional_string(
        &mut canonical,
        entry.selector.normalized_snippet_hash.as_deref(),
    );
    write_optional_u32(&mut canonical, entry.selector.line_hint);
    write_optional_string(
        &mut canonical,
        entry
            .selector
            .glob
            .as_deref()
            .map(crate::normalize_path)
            .as_deref(),
    );
    match &entry.last_seen {
        Some(last_seen) => {
            canonical.push(1);
            canonical.extend_from_slice(&last_seen.line.to_be_bytes());
            canonical.extend_from_slice(&last_seen.column.to_be_bytes());
        }
        None => canonical.push(0),
    }

    let digest = Sha256::digest(canonical);
    let hex = digest
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!("sha256:v1:{hex}")
}

fn write_string(output: &mut Vec<u8>, value: &str) {
    output.extend_from_slice(&(value.len() as u64).to_be_bytes());
    output.extend_from_slice(value.as_bytes());
}

fn write_optional_string(output: &mut Vec<u8>, value: Option<&str>) {
    match value {
        Some(value) => {
            output.push(1);
            write_string(output, value);
        }
        None => output.push(0),
    }
}

fn write_string_list(output: &mut Vec<u8>, values: &[String]) {
    output.extend_from_slice(&(values.len() as u64).to_be_bytes());
    for value in values {
        write_string(output, value);
    }
}

fn write_optional_u32(output: &mut Vec<u8>, value: Option<u32>) {
    match value {
        Some(value) => {
            output.push(1);
            output.extend_from_slice(&value.to_be_bytes());
        }
        None => output.push(0),
    }
}
