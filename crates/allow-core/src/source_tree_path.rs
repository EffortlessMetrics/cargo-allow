use std::path::Path;

use crate::AllowEntry;
use unicode_normalization::UnicodeNormalization;

/// Normalize a path for source-tree identity and matching.
///
/// All backslashes are converted to forward slashes, Unicode is normalized to
/// NFC (composed form), `.`/`..` segments are folded, and the Windows verbatim
/// prefix (`\\?\`) is stripped. This is a lexical normalization only — it does
/// not touch the filesystem.
///
/// # Unicode NFC normalization (#1823)
///
/// macOS (HFS+/APFS) and git may represent the same path in different Unicode
/// normalization forms (NFC composed vs NFD decomposed). Without NFC
/// normalization, `files.sort(); files.dedup()` in the inventory treats NFC
/// and NFD forms of the same path as distinct, and finding→entry matching
/// (`normalize_path(finding_path) == normalize_path(entry_path)`) produces
/// **false positives/false negatives split across platforms** — a real
/// `unwrap()` finding goes unreceipted on macOS but matched on Linux.
/// Normalizing to NFC inside this function ensures all downstream matching,
/// fingerprinting, and identity keying sees one canonical Unicode form.
///
/// # Windows absolute paths (#1821)
///
/// `normalize_path` handles three Windows absolute shapes:
///
/// - **Verbatim prefix** (`\\?\C:\...` or `\\?\UNC\server\share\...`):
///   stripped so the path degrades to its non-verbatim form (`C:/...` or
///   `//server/share/...`). This is the case that silently produced wrong
///   identity keys because the `\\?\` prefix survived as path segments.
/// - **Drive letters** (`C:\...`): preserved as `C:/...`. The drive letter
///   is a meaningful absolute-path identity component, not a repo-relative
///   segment, and several callers (e.g. migrate evidence diagnostics) pass
///   absolute roots through this function. Stripping it would corrupt those
///   identities.
/// - **UNC roots** (`\\server\share\...`): preserved as `//server/share/...`.
///   Two leading slashes fold to a single Unix-style absolute root (`/`)
///   during the segment walk, so `//server/share/foo` → `/server/share/foo`.
///
/// The scanner resolves finding paths against the source-tree root before
/// calling this function, so repo-relative paths are the normal input.
pub fn normalize_path(path: impl AsRef<Path>) -> String {
    let text = path.as_ref().to_string_lossy().replace('\\', "/");
    // NFC-normalize the text so that composed/decomposed Unicode forms of the
    // same path produce the same identity key (#1823). This prevents
    // cross-platform (macOS NFD vs Linux NFC) matching divergence.
    let nfc: String = text.nfc().collect();
    // Strip the Windows verbatim prefix (\\?\) so it doesn't survive as path
    // segments. Drive letters and plain UNC roots are preserved (see docs).
    let (stripped, force_absolute) = strip_verbatim_prefix(&nfc);
    let absolute = force_absolute || stripped.starts_with('/');
    let mut parts = Vec::new();
    for part in stripped.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                if parts.last().is_some_and(|part| *part != "..") {
                    parts.pop();
                } else if !absolute {
                    parts.push(part);
                }
            }
            other => parts.push(other),
        }
    }
    let normalized = parts.join("/");
    if absolute {
        format!("/{normalized}")
    } else {
        normalized
    }
}

/// Strip the Windows verbatim prefix from a forward-slashed path string.
///
/// Returns the stripped path and a flag indicating whether the result should
/// be treated as absolute (set for verbatim UNC paths where the `\\?\UNC\`
/// prefix is stripped but the path is still absolute).
///
/// - `//?/C:/foo` → `("C:/foo", false)` — drive letter preserved.
/// - `//?/UNC/server/share/foo` → `("server/share/foo", true)` — verbatim UNC
///   stripped, force-absolute so the result is `/server/share/foo`.
fn strip_verbatim_prefix(text: &str) -> (&str, bool) {
    if let Some(rest) = text.strip_prefix("//?/UNC/") {
        return (rest, true);
    }
    if let Some(rest) = text.strip_prefix("//?/") {
        return (rest, false);
    }
    (text, false)
}

pub(crate) fn normalize_source_tree_scope(scope: &str) -> String {
    scope.replace('\\', "/")
}

pub fn glob_matches(pattern: &str, path: &Path) -> bool {
    let path = normalize_path(path);
    glob_matches_str(pattern, &path)
}

/// Maximum recursive match steps for one glob evaluation.
///
/// Protects against exponential backtracking from pathological patterns such
/// as many `*` / `**` tokens against long paths (#1924). When the budget is
/// exhausted the match fails closed (returns `false`) instead of hanging.
pub const GLOB_MATCH_MAX_STEPS: u32 = 10_000;

pub fn glob_matches_str(pattern: &str, path: &str) -> bool {
    let p = normalize_glob_pattern(pattern);
    let mut steps = 0;
    glob_match_tokens(&split_glob(&p), &split_glob(path), &mut steps)
}

fn normalize_glob_pattern(pattern: &str) -> String {
    pattern.replace('\\', "/").nfc().collect()
}

pub fn source_tree_path_matches_filter(item_path: &str, filter_path: &str) -> bool {
    let item_path = normalize_path(item_path);
    let filter_path = normalize_path(filter_path);
    let filter_path = filter_path.trim_end_matches('/');
    if filter_path.is_empty() || filter_path == "." {
        return true;
    }
    item_path == filter_path
        || item_path
            .strip_prefix(filter_path)
            .map(|suffix| suffix.starts_with('/'))
            .unwrap_or(false)
        // #2776: support glob matching in BOTH directions. The filter may
        // be a glob (e.g. `--path 'src/**/*.rs'` from CLI), or the item
        // path may be a glob (e.g. a broad-scope allow entry's scope).
        || (source_tree_scope_has_wildcard(filter_path)
            && glob_matches_str(filter_path, &item_path))
        || (source_tree_scope_has_wildcard(&item_path)
            && glob_matches_str(&item_path, filter_path))
}

pub fn source_tree_path_is_ignored(path: impl AsRef<Path>, patterns: &[String]) -> bool {
    let normalized = normalize_path(path);
    patterns.iter().any(|pattern| {
        let normalized_pattern = normalize_glob_pattern(pattern);
        let mut steps = 0;
        glob_match_tokens(
            &split_glob(&normalized_pattern),
            &split_glob(&normalized),
            &mut steps,
        )
    })
}

pub fn source_tree_scope_has_wildcard(scope: &str) -> bool {
    scope.chars().any(|ch| matches!(ch, '*' | '?'))
}

pub fn allow_entry_broad_scope(entry: &AllowEntry) -> Option<String> {
    entry
        .path
        .as_ref()
        .map(normalize_path)
        .filter(|scope| source_tree_scope_has_wildcard(scope))
        .or_else(|| {
            entry
                .glob
                .as_deref()
                .map(normalize_source_tree_scope)
                .filter(|scope| source_tree_scope_has_wildcard(scope))
        })
        .or_else(|| {
            entry
                .selector
                .glob
                .as_deref()
                .map(normalize_source_tree_scope)
                .filter(|scope| source_tree_scope_has_wildcard(scope))
        })
}

/// Strip Win32 verbatim path prefixes (`\\?\` and `\\?\UNC\`) from a path
/// string for clean display in error messages and JSON output (#3180-#3187).
///
/// On non-Windows or paths without the prefix, this is a no-op.
pub fn strip_win32_verbatim_prefix(path: &str) -> String {
    // Handle both backslash and forward slash variants
    let normalized = path.replace('\\', "/");
    if let Some(rest) = normalized.strip_prefix("//?/UNC/") {
        format!("/{rest}")
    } else if let Some(rest) = normalized.strip_prefix("//?/") {
        rest.to_string()
    } else {
        path.to_string()
    }
}

fn split_glob(s: &str) -> Vec<&str> {
    s.split('/').filter(|part| !part.is_empty()).collect()
}

fn take_glob_step(steps: &mut u32) -> bool {
    if *steps >= GLOB_MATCH_MAX_STEPS {
        return false;
    }
    *steps = steps.saturating_add(1);
    true
}

fn glob_match_tokens(pattern: &[&str], path: &[&str], steps: &mut u32) -> bool {
    if !take_glob_step(steps) {
        return false;
    }
    let Some((pattern_head, pattern_tail)) = pattern.split_first() else {
        return path.is_empty();
    };
    if *pattern_head == "**" {
        if glob_match_tokens(pattern_tail, path, steps) {
            return true;
        }
        return path
            .split_first()
            .is_some_and(|(_, path_tail)| glob_match_tokens(pattern, path_tail, steps));
    }
    path.split_first().is_some_and(|(path_head, path_tail)| {
        segment_matches(pattern_head, path_head, steps)
            && glob_match_tokens(pattern_tail, path_tail, steps)
    })
}

fn segment_matches(pattern: &str, text: &str, steps: &mut u32) -> bool {
    let pattern = pattern.chars().collect::<Vec<_>>();
    let text = text.chars().collect::<Vec<_>>();
    segment_match_chars(&pattern, &text, steps)
}

fn segment_match_chars(pattern: &[char], text: &[char], steps: &mut u32) -> bool {
    if !take_glob_step(steps) {
        return false;
    }
    let Some((&pattern_head, pattern_tail)) = pattern.split_first() else {
        return text.is_empty();
    };
    match pattern_head {
        '*' => {
            segment_match_chars(pattern_tail, text, steps)
                || text
                    .split_first()
                    .is_some_and(|(_, text_tail)| segment_match_chars(pattern, text_tail, steps))
        }
        '?' => text
            .split_first()
            .is_some_and(|(_, text_tail)| segment_match_chars(pattern_tail, text_tail, steps)),
        ch => text.split_first().is_some_and(|(&text_head, text_tail)| {
            ch == text_head && segment_match_chars(pattern_tail, text_tail, steps)
        }),
    }
}
