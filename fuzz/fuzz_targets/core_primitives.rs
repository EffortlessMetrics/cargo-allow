#![no_main]

use allow_core::{
    FindingKind, SimpleDate, glob_matches_str, json_escape, maybe_line_distance_score,
    normalize_path, normalize_snippet, source_tree_path_is_ignored,
    source_tree_path_matches_filter, source_tree_scope_has_wildcard, stable_hash_hex,
};
use libfuzzer_sys::fuzz_target;
use std::path::PathBuf;
use std::str::FromStr;

const MAX_TEXT: usize = 512;
const MAX_PATTERNS: usize = 8;

fuzz_target!(|data: &[u8]| {
    let parts = bounded_parts(data);
    let first = parts.first().map(String::as_str).unwrap_or_default();
    let second = parts.get(1).map(String::as_str).unwrap_or_default();

    let _ = FindingKind::from_str(first);
    let _ = SimpleDate::parse(first);
    if let Some(date) = SimpleDate::parse(first) {
        let days = second
            .bytes()
            .fold(0_i64, |acc, byte| acc + i64::from(byte))
            % 3650;
        let shifted = date.add_days(days - 1825);
        let _ = shifted.days_until(date);
        let _ = shifted.to_string();
    }

    let _ = normalize_path(PathBuf::from(first));
    let _ = glob_matches_str(first, second);
    let _ = source_tree_path_matches_filter(first, second);
    let _ = source_tree_scope_has_wildcard(first);
    let patterns = parts
        .iter()
        .skip(2)
        .take(MAX_PATTERNS)
        .cloned()
        .collect::<Vec<_>>();
    let _ = source_tree_path_is_ignored(PathBuf::from(first), &patterns);

    let normalized = normalize_snippet(first);
    let _ = stable_hash_hex(&normalized);
    let _ = json_escape(first);

    let hint = parts.get(2).and_then(|value| parse_u32_hint(value));
    let actual = parts.get(3).and_then(|value| parse_u32_hint(value));
    let _ = maybe_line_distance_score(hint, actual);
});

fn bounded_parts(data: &[u8]) -> Vec<String> {
    data.split(|byte| *byte == 0 || *byte == b'\n')
        .take(12)
        .map(|part| String::from_utf8_lossy(&part[..part.len().min(MAX_TEXT)]).into_owned())
        .collect()
}

fn parse_u32_hint(value: &str) -> Option<u32> {
    if value.is_empty() {
        None
    } else {
        Some(
            value.bytes().fold(0_u32, |acc, byte| {
                acc.wrapping_mul(257).wrapping_add(u32::from(byte))
            }) % 10_000,
        )
    }
}
